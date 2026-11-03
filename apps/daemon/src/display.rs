//! Fondos en pantalla (fase 1.5): lo elegido en la biblioteca → un cuadro por monitor.
//! Los videos (fase 2) no pasan por aquí cuadro a cuadro: se le entrega el archivo a la
//! superficie, que lo reproduce.
//!
//! Corre en su propio hilo, porque decodificar y escalar una imagen tarda (hasta ~1 s con
//! una foto grande) y no debe frenar las respuestas del IPC. Habla con el resto por canales.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use miraishin_core::copies::{fixable_by_copy, CopyLevel, CopyTarget, VideoSaver};
use miraishin_core::fit::FitMode;
use miraishin_core::pause::{self, OutputSignals, PauseReason, PauseSettings, SystemSignals};
use miraishin_core::surface::Size;
use miraishin_core::surface::{EventSink, Frame, OutputInfo, Surface, SurfaceError, SurfaceEvent};
use miraishin_ipc::{events, Event, OutputItem, OutputsList};
use miraishin_render_image as render;
use miraishin_store::{Assignment, Library, ALL_OUTPUTS};
use serde_json::json;
use tokio::sync::{broadcast, oneshot};

use crate::copies;

/// Crea la superficie. Devuelve el nombre de la forma usada (`layer_shell`, …) o el código
/// de por qué no hay.
pub type SurfaceFactory =
    Box<dyn FnOnce(EventSink) -> Result<(Box<dyn Surface>, &'static str), String> + Send>;

/// Color de respaldo cuando lo elegido no se puede mostrar (grafito del sistema de diseño).
pub const FALLBACK: [u8; 4] = [0x1c, 0x1d, 0x21, 0xff];

/// Lo que se le puede pedir al hilo.
enum Msg {
    /// Volver a leer lo elegido y ponerlo (después de `set`, `clear` o `rescan`).
    Apply(Option<oneshot::Sender<()>>),
    Surface(SurfaceEvent),
    Outputs(oneshot::Sender<OutputsList>),
    /// Avance de una copia compatible (fase 2.9).
    CopyProgress(PathBuf, u8),
    /// Terminó una copia: `Err` con el código si falló.
    CopyDone(PathBuf, Result<(), &'static str>),
    /// Pausa a mano (`true`) o la quita, de un monitor o de todos (fase 3).
    Pause(Option<String>, bool, oneshot::Sender<()>),
    /// El equipo pasó a batería (`true`) o al cargador (fase 3.2).
    OnBattery(bool),
    Stop,
}

/// Cómo se manejan los videos que este equipo no reproduce bien (fase 2.9).
#[derive(Debug, Clone)]
pub struct VideoOptions {
    /// `<datos>/video-copies`.
    pub copies_dir: PathBuf,
    pub saver: VideoSaver,
    /// Tarjeta gráfica virtual o ninguna (`copies::no_real_gpu`).
    pub no_real_gpu: bool,
    /// El programa `ffmpeg` (`None` = no se pueden crear copias).
    pub ffmpeg: Option<PathBuf>,
    /// Cuándo se le pregunta a la superficie si un video se ve, después de ponerlo. Lo de
    /// 22 s alcanza para `video_too_slow` (primer cuadro + dos ventanas de 5 s en KDE).
    pub check_delays: Vec<Duration>,
}

impl VideoOptions {
    pub const DEFAULT_CHECKS: [Duration; 3] = [
        Duration::from_secs(3),
        Duration::from_secs(10),
        Duration::from_secs(22),
    ];
}

/// Manija para el resto del daemon.
#[derive(Clone)]
pub struct Display {
    tx: mpsc::Sender<Msg>,
    surface: String,
    available: bool,
}

impl Display {
    pub fn start(
        factory: SurfaceFactory,
        library: Arc<Mutex<Library>>,
        wallpapers_dir: PathBuf,
        events: broadcast::Sender<Event>,
        video: VideoOptions,
        pause: PauseSettings,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let sink_tx = tx.clone();
        let sink: EventSink = Box::new(move |e| {
            let _ = sink_tx.send(Msg::Surface(e));
        });
        let (surface, name, available) = match factory(sink) {
            Ok((s, name)) => (Some(s), name.to_string(), true),
            Err(code) => {
                tracing::warn!(code = %code, "no wallpaper surface on this desktop");
                (None, code, false)
            }
        };
        if surface.is_some() {
            // Copias de fondos que ya no existen y copias a medias de otra sesión.
            copies::cleanup(&video.copies_dir, |folder| {
                let lib = library.lock().unwrap_or_else(|p| p.into_inner());
                lib.get(folder).ok().flatten().is_some()
            });
        }
        tracing::info!(
            saver = video.saver.as_str(),
            no_real_gpu = video.no_real_gpu,
            ffmpeg = video.ffmpeg.is_some(),
            "video copies"
        );
        let worker = Worker {
            tx: tx.clone(),
            video,
            levels: HashMap::new(),
            fits_saver: HashMap::new(),
            jobs: HashMap::new(),
            failed: HashMap::new(),
            copying: HashMap::new(),
            copy_label: HashMap::new(),
            checks: Vec::new(),
            surface,
            surface_name: name.clone(),
            available,
            library,
            wallpapers_dir,
            events,
            shown: HashMap::new(),
            videos: HashSet::new(),
            first: true,
            pause_settings: pause,
            system: SystemSignals::default(),
            manual: HashSet::new(),
            signals: HashMap::new(),
            paused: HashMap::new(),
            pause_unsupported: HashSet::new(),
        };
        let started = std::thread::Builder::new()
            .name("display".into())
            .spawn(move || worker.run(rx));
        if started.is_err() {
            tracing::error!("could not start the display thread");
        }
        let display = Self {
            tx,
            surface: name,
            available: available && started.is_ok(),
        };
        display.apply_nowait();
        display
    }

    pub fn surface(&self) -> (&str, bool) {
        (&self.surface, self.available)
    }

    /// Pone lo elegido y espera a que termine (para responder con el estado real).
    pub async fn apply(&self) {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(Msg::Apply(Some(tx))).is_ok() {
            let _ = rx.await;
        }
    }

    pub fn apply_nowait(&self) {
        let _ = self.tx.send(Msg::Apply(None));
    }

    pub async fn outputs(&self) -> OutputsList {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(Msg::Outputs(tx)).is_err() {
            return OutputsList::default();
        }
        rx.await.unwrap_or_default()
    }

    /// Pausa a mano o la quita (`ctl pause` / `ctl resume`), y espera a que se aplique.
    pub async fn pause(&self, output: Option<String>, paused: bool) {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(Msg::Pause(output, paused, tx)).is_ok() {
            let _ = rx.await;
        }
    }

    /// Con batería o con cargador (lo avisa `power`). No espera.
    pub fn set_on_battery(&self, on_battery: bool) {
        let _ = self.tx.send(Msg::OnBattery(on_battery));
    }

    pub fn stop(&self) {
        let _ = self.tx.send(Msg::Stop);
    }
}

/// Qué muestra un monitor ahora. Si nada de esto cambia, no se vuelve a dibujar.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Shown {
    folder: String,
    mode: FitMode,
    /// `updated_at` del fondo: si el archivo cambió (rescan), se dibuja de nuevo.
    version: String,
    pixels: (u32, u32),
    problem: Option<String>,
}

struct Worker {
    /// Para que los hilos de las copias avisen.
    tx: mpsc::Sender<Msg>,
    video: VideoOptions,
    /// Nivel al que subió cada video (carpeta, versión) por no verse bien.
    levels: HashMap<(String, String), CopyLevel>,
    /// Videos (carpeta, versión) que ya son como la copia de ahorro: se leen una vez.
    fits_saver: HashMap<(String, String), bool>,
    /// Copias en curso y cuánto van.
    jobs: HashMap<PathBuf, u8>,
    /// Copias que fallaron, con su código (se reintentan con el próximo `set`).
    failed: HashMap<PathBuf, &'static str>,
    /// Monitores esperando una copia (sigue el fondo anterior) y su tamaño.
    copying: HashMap<String, (PathBuf, String)>,
    /// Monitores que muestran una copia, y su tamaño (`1280x720`).
    copy_label: HashMap<String, String>,
    /// Cuándo preguntar si el video de un monitor se ve.
    checks: Vec<(Instant, String)>,
    surface: Option<Box<dyn Surface>>,
    surface_name: String,
    available: bool,
    library: Arc<Mutex<Library>>,
    wallpapers_dir: PathBuf,
    events: broadcast::Sender<Event>,
    shown: HashMap<String, Shown>,
    /// Monitores con un video puesto: en `outputs` se le pregunta a la superficie si de
    /// verdad se ve (en KDE lo sabe Plasma, después de `show_video`).
    videos: HashSet<String>,
    /// En la primera pasada se quita el fondo de los monitores sin nada elegido: en KDE el
    /// de una sesión anterior sigue puesto aunque el daemon se haya reiniciado.
    first: bool,
    // ---- Pausa (fase 3)
    pause_settings: PauseSettings,
    system: SystemSignals,
    /// Monitores pausados a mano uno por uno (lo de todos va en `system.manual_all`). Solo
    /// en memoria: al reiniciar el daemon, los videos vuelven a reproducirse.
    manual: HashSet<String>,
    /// Señales de cada monitor (pantalla completa, pantalla apagada…).
    signals: HashMap<String, OutputSignals>,
    /// Lo que ya se le pidió a la superficie en cada monitor con video: `Some(motivo)` en
    /// pausa, `None` reproduciendo. Sin entrada = no se sabe (video recién puesto): se pide.
    paused: HashMap<String, Option<PauseReason>>,
    /// Monitores donde se quiso pausar y la superficie no sabe.
    pause_unsupported: HashSet<String>,
}

impl Worker {
    fn run(mut self, rx: mpsc::Receiver<Msg>) {
        if let Some(s) = self.surface.as_mut() {
            s.set_window_rules(&self.pause_settings);
        }
        loop {
            let next = self.checks.iter().map(|c| c.0).min();
            let msg = match next {
                Some(due) => match rx.recv_timeout(due.saturating_duration_since(Instant::now())) {
                    Ok(m) => m,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        self.run_checks();
                        continue;
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                },
                None => match rx.recv() {
                    Ok(m) => m,
                    Err(_) => break,
                },
            };
            match msg {
                Msg::Apply(done) => {
                    if done.is_some() {
                        // Un `set`/`rescan` explícito vuelve a intentar las copias fallidas.
                        self.failed.clear();
                    }
                    self.apply();
                    if let Some(d) = done {
                        let _ = d.send(());
                    }
                }
                Msg::Surface(SurfaceEvent::Covered {
                    output,
                    fullscreen,
                    maximized,
                }) => {
                    let s = self.signals.entry(output.clone()).or_default();
                    if (s.fullscreen, s.maximized) != (fullscreen, maximized) {
                        s.fullscreen = fullscreen;
                        s.maximized = maximized;
                        tracing::debug!(output = %output, fullscreen, maximized, "windows over the wallpaper");
                        if self.update_pause() {
                            self.send_outputs();
                        }
                    }
                }
                Msg::Surface(e) => {
                    if let SurfaceEvent::OutputRemoved { name } = &e {
                        self.shown.remove(name);
                        self.videos.remove(name);
                        self.signals.remove(name);
                        tracing::info!(output = %name, "monitor removed");
                    } else {
                        tracing::info!(event = ?e, "monitor changed");
                    }
                    self.apply();
                }
                Msg::Outputs(reply) => {
                    let _ = reply.send(self.list());
                }
                Msg::CopyProgress(path, pct) => {
                    if let Some(p) = self.jobs.get_mut(&path) {
                        *p = pct;
                    }
                }
                Msg::CopyDone(path, result) => {
                    self.jobs.remove(&path);
                    match result {
                        Ok(()) => {
                            tracing::info!(copy = %miraishin_store::paths::redact(&path), "video copy ready")
                        }
                        Err(code) => {
                            tracing::warn!(code, "video copy could not be made");
                            self.failed.insert(path.clone(), code);
                        }
                    }
                    // Los monitores que la esperaban se vuelven a poner.
                    let waiting: Vec<String> = self
                        .copying
                        .iter()
                        .filter(|(_, (p, _))| *p == path)
                        .map(|(o, _)| o.clone())
                        .collect();
                    for o in waiting {
                        self.copying.remove(&o);
                        self.shown.remove(&o);
                    }
                    self.apply();
                }
                Msg::Pause(output, paused, reply) => {
                    self.set_manual(output, paused);
                    if self.update_pause() {
                        self.send_outputs();
                    }
                    let _ = reply.send(());
                }
                Msg::OnBattery(on) => {
                    if self.system.on_battery != on {
                        self.system.on_battery = on;
                        if self.update_pause() {
                            self.send_outputs();
                        }
                    }
                }
                Msg::Stop => break,
            }
        }
    }

    fn assignments(library: &Mutex<Library>) -> Vec<Assignment> {
        let lib = library.lock().unwrap_or_else(|p| p.into_inner());
        lib.assignments().unwrap_or_else(|e| {
            tracing::error!(error = %e, "could not read assignments");
            Vec::new()
        })
    }

    fn apply(&mut self) {
        let assignments = Self::assignments(&self.library);
        let Some(surface) = self.surface.as_mut() else {
            return;
        };
        let outputs = surface.outputs();
        let largest = outputs
            .iter()
            .map(|o| o.pixel_size())
            .max_by_key(|s| u64::from(s.width) * u64::from(s.height))
            .unwrap_or(Size::new(1920, 1080));
        let mut changed = false;
        // La misma imagen se decodifica una sola vez para todos los monitores.
        let mut decoded: HashMap<PathBuf, Result<render::RgbaImage, render::RenderError>> =
            HashMap::new();

        for output in &outputs {
            let Some((folder, mode)) = resolve(&assignments, &output.name) else {
                self.videos.remove(&output.name);
                self.copying.remove(&output.name);
                self.copy_label.remove(&output.name);
                if self.shown.remove(&output.name).is_some() || self.first {
                    let _ = surface.clear(&output.name);
                    changed = true;
                }
                continue;
            };
            let px = output.pixel_size();
            let (version, source) = lookup(&self.library, &self.wallpapers_dir, &folder);
            let key = Shown {
                folder: folder.clone(),
                mode,
                version,
                pixels: (px.width, px.height),
                problem: None,
            };
            if self.shown.get(&output.name).is_some_and(|s| {
                Shown {
                    problem: None,
                    ..s.clone()
                } == key
            }) {
                continue;
            }
            self.copying.remove(&output.name);
            self.copy_label.remove(&output.name);
            // Video: lo reproduce la superficie, el original o una copia compatible (2.9).
            // Si no sabe, el archivo falta o la copia falla, respaldo.
            let source = match source {
                Ok(Source::Video(path)) if !path.is_file() => Err("video_unreadable".to_string()),
                Ok(Source::Video(path)) => {
                    let fits = fits_saver(
                        &self.video,
                        &mut self.fits_saver,
                        &folder,
                        &key.version,
                        &path,
                    );
                    let level = level_for(
                        &self.video,
                        &self.levels,
                        &folder,
                        &key.version,
                        largest,
                        fits,
                    );
                    let play = match level.target(largest) {
                        None => Ok(Some((path.clone(), None))),
                        Some(t) => {
                            let cp = copies::copy_path(
                                &self.video.copies_dir,
                                &folder,
                                &key.version,
                                &t,
                            );
                            if cp.is_file() {
                                Ok(Some((cp, Some(t.label()))))
                            } else if let Some(code) = self.failed.get(&cp) {
                                Err(code.to_string())
                            } else {
                                if !self.jobs.contains_key(&cp) {
                                    start_copy(&self.video, &self.tx, &path, &cp, t, &folder);
                                    self.jobs.insert(cp.clone(), 0);
                                }
                                self.copying.insert(output.name.clone(), (cp, t.label()));
                                Ok(None)
                            }
                        }
                    };
                    match play {
                        // Mientras se crea la copia sigue el fondo anterior.
                        Ok(None) => {
                            tracing::info!(output = %output.name, folder = %folder,
                                level = ?level, "making a compatible copy of the video");
                            self.videos.remove(&output.name);
                            self.shown.insert(output.name.clone(), key);
                            changed = true;
                            continue;
                        }
                        Ok(Some((file, label))) => {
                            match surface.show_video(&output.name, &file, mode) {
                                Ok(()) => {
                                    tracing::info!(output = %output.name, folder = %folder,
                                    mode = mode.as_str(), copy = ?label, "video wallpaper shown");
                                    // Recién puesto: la pausa se vuelve a pedir abajo.
                                    self.paused.remove(&output.name);
                                    self.shown.insert(output.name.clone(), key);
                                    self.videos.insert(output.name.clone());
                                    if let Some(l) = label {
                                        self.copy_label.insert(output.name.clone(), l);
                                    }
                                    // Más tarde se pregunta si de verdad se ve (KDE).
                                    self.checks.retain(|(_, o)| *o != output.name);
                                    let now = Instant::now();
                                    for d in &self.video.check_delays {
                                        self.checks.push((now + *d, output.name.clone()));
                                    }
                                    changed = true;
                                    continue;
                                }
                                Err(e) => Err(e.as_str().to_string()),
                            }
                        }
                        Err(code) => Err(code),
                    }
                }
                Ok(Source::Image(path)) => Ok(path),
                Err(code) => Err(code),
            };
            let frame = source.and_then(|path| {
                let image = decoded
                    .entry(path.clone())
                    .or_insert_with(|| render::load_file(&path, render::DEFAULT_MAX_PIXELS));
                let image = image.as_ref().map_err(|e| e.as_str().to_string())?;
                let opts = render::Options {
                    mode,
                    ..render::Options::default()
                };
                render::compose(image, px, &opts).map_err(|e| e.as_str().to_string())
            });
            let (frame, problem) = match frame {
                Ok(f) => (f, None),
                Err(code) => {
                    tracing::warn!(output = %output.name, folder = %folder, code = %code,
                        "wallpaper could not be shown; using the fallback color");
                    match Frame::solid(px, FALLBACK) {
                        Ok(f) => (f, Some(code)),
                        Err(_) => continue,
                    }
                }
            };
            match surface.show(&output.name, frame) {
                Ok(()) => {
                    tracing::info!(output = %output.name, folder = %folder, mode = mode.as_str(),
                        problem = ?problem, "wallpaper shown");
                    self.shown
                        .insert(output.name.clone(), Shown { problem, ..key });
                    self.videos.remove(&output.name);
                    changed = true;
                }
                Err(e) => {
                    tracing::warn!(output = %output.name, code = e.as_str(), "surface refused the frame");
                }
            }
        }
        // Lo decodificado se suelta aquí: solo quedan los cuadros en la superficie.
        drop(decoded);
        self.first = false;
        let present: Vec<&str> = outputs.iter().map(|o| o.name.as_str()).collect();
        self.shown
            .retain(|name, _| present.contains(&name.as_str()));
        self.videos.retain(|name| present.contains(&name.as_str()));
        self.copying
            .retain(|name, _| present.contains(&name.as_str()));
        self.copy_label
            .retain(|name, _| present.contains(&name.as_str()));
        self.checks
            .retain(|(_, name)| present.contains(&name.as_str()));
        self.signals
            .retain(|name, _| present.contains(&name.as_str()));
        if self.update_pause() {
            changed = true;
        }
        if changed {
            self.send_outputs();
        }
    }

    fn send_outputs(&self) {
        let _ = self
            .events
            .send(Event::new(events::OUTPUTS_CHANGED, json!(self.list())));
    }

    /// Pausa a mano: sin monitor, todos; con monitor, solo ese. Quitar la pausa de un monitor
    /// cuando estaban todos en pausa deja en pausa a los demás.
    fn set_manual(&mut self, output: Option<String>, paused: bool) {
        match (output, paused) {
            (None, true) => self.system.manual_all = true,
            (None, false) => {
                self.system.manual_all = false;
                self.manual.clear();
            }
            (Some(o), true) => {
                self.manual.insert(o);
            }
            (Some(o), false) => {
                if self.system.manual_all {
                    self.system.manual_all = false;
                    let all: Vec<String> = self
                        .surface
                        .as_ref()
                        .map(|s| s.outputs())
                        .unwrap_or_default()
                        .into_iter()
                        .map(|i| i.name)
                        .collect();
                    self.manual.extend(all);
                }
                self.manual.remove(&o);
            }
        }
        tracing::info!(all = self.system.manual_all, outputs = ?self.manual, "manual pause");
    }

    /// Pide a la superficie la pausa que corresponde en cada monitor con video. Devuelve si
    /// cambió algo que `outputs` muestra.
    fn update_pause(&mut self) -> bool {
        let Some(surface) = self.surface.as_mut() else {
            return false;
        };
        let mut changed = false;
        self.paused.retain(|o, _| self.videos.contains(o));
        self.pause_unsupported.retain(|o| self.videos.contains(o));
        let mut videos: Vec<&String> = self.videos.iter().collect();
        videos.sort();
        for output in videos {
            let mut signals = self.signals.get(output).copied().unwrap_or_default();
            signals.manual = self.manual.contains(output);
            let wanted = pause::decide(&self.pause_settings, &self.system, &signals);
            match self.paused.get(output) {
                Some(now) if now.is_some() == wanted.is_some() => {
                    // Ya no se quiere pausa: el aviso de «no sabe pausar» sobra.
                    if wanted.is_none() && self.pause_unsupported.remove(output) {
                        changed = true;
                    }
                    if *now != wanted {
                        // Sigue en pausa, por otro motivo: solo cambia lo que se muestra.
                        self.paused.insert(output.clone(), wanted);
                        changed = true;
                    }
                    continue;
                }
                _ => {}
            }
            match surface.set_paused(output, wanted.is_some()) {
                Ok(()) => {
                    if self.paused.insert(output.clone(), wanted) != Some(wanted) {
                        changed = true;
                    }
                    if self.pause_unsupported.remove(output) {
                        changed = true;
                    }
                    tracing::info!(output = %output, reason = wanted.map(|r| r.as_str()),
                        "video pause");
                }
                Err(SurfaceError::PauseUnsupported) => {
                    // Se sigue reproduciendo; se recuerda para no volver a pedirlo.
                    self.paused.insert(output.clone(), None);
                    if wanted.is_some() && self.pause_unsupported.insert(output.clone()) {
                        tracing::info!(output = %output, "this surface cannot pause videos yet");
                        changed = true;
                    } else if wanted.is_none() && self.pause_unsupported.remove(output) {
                        changed = true;
                    }
                }
                Err(e) => {
                    tracing::warn!(output = %output, code = e.as_str(), "could not change the video pause");
                }
            }
        }
        changed
    }

    /// Pregunta a la superficie por los videos cuyo momento llegó. Si uno no se ve por algo
    /// que una copia arregla, sube un nivel (original → pantalla → ahorro) y lo vuelve a poner.
    fn run_checks(&mut self) {
        let now = Instant::now();
        let due: Vec<String> = self
            .checks
            .iter()
            .filter(|(t, _)| *t <= now)
            .map(|(_, o)| o.clone())
            .collect();
        self.checks.retain(|(t, _)| *t > now);
        let mut again = false;
        for output in due {
            if !self.videos.contains(&output) {
                continue;
            }
            let Some(problem) = self.surface.as_ref().and_then(|s| s.video_problem(&output)) else {
                continue;
            };
            let Some(shown) = self.shown.get(&output).cloned() else {
                continue;
            };
            if !fixable_by_copy(&problem) {
                continue;
            }
            let largest = self
                .surface
                .as_ref()
                .map(|s| s.outputs())
                .unwrap_or_default()
                .iter()
                .map(|o| o.pixel_size())
                .max_by_key(|s| u64::from(s.width) * u64::from(s.height))
                .unwrap_or(Size::new(1920, 1080));
            let key = (shown.folder.clone(), shown.version.clone());
            let fits = self.fits_saver.get(&key).copied().unwrap_or(false);
            let current = level_for(
                &self.video,
                &self.levels,
                &shown.folder,
                &shown.version,
                largest,
                fits,
            );
            let (start, cap) = self.video.saver.levels(self.video.no_real_gpu);
            // Un original que ya era de ahorro y no se ve pasa directo a la copia de ahorro.
            let next = if current == CopyLevel::Original && start == CopyLevel::Saver {
                Some(CopyLevel::Saver)
            } else {
                current.next()
            };
            let Some(next) = next.filter(|n| *n <= cap) else {
                continue;
            };
            tracing::info!(output = %output, folder = %shown.folder, problem = %problem,
                from = ?current, to = ?next, "video does not play well here; using a compatible copy");
            self.levels
                .insert((shown.folder.clone(), shown.version.clone()), next);
            // Todos los monitores con ese video se vuelven a poner.
            let same: Vec<String> = self
                .shown
                .iter()
                .filter(|(_, s)| s.folder == shown.folder)
                .map(|(o, _)| o.clone())
                .collect();
            for o in same {
                self.shown.remove(&o);
                self.checks.retain(|(_, c)| *c != o);
            }
            again = true;
        }
        if again {
            self.apply();
        }
    }

    fn list(&self) -> OutputsList {
        let outputs: Vec<OutputInfo> = self
            .surface
            .as_ref()
            .map(|s| s.outputs())
            .unwrap_or_default();
        let assignments = Self::assignments(&self.library);
        let all = assignments.iter().find(|a| a.output == ALL_OUTPUTS);
        OutputsList {
            items: outputs
                .iter()
                .map(|o| {
                    let px = o.pixel_size();
                    let shown = self.shown.get(&o.name);
                    // Un video puesto puede no verse aunque la superficie lo aceptó.
                    let problem = shown.and_then(|s| s.problem.clone()).or_else(|| {
                        self.videos
                            .contains(&o.name)
                            .then(|| self.surface.as_ref()?.video_problem(&o.name))
                            .flatten()
                    });
                    OutputItem {
                        name: o.name.clone(),
                        description: o.description.clone(),
                        width: o.logical.width,
                        height: o.logical.height,
                        scale_percent: o.scale.percent(),
                        pixel_width: px.width,
                        pixel_height: px.height,
                        wallpaper: shown.map(|s| s.folder.clone()),
                        mode: shown.map(|s| s.mode.as_str().to_string()),
                        problem,
                        copy: self.copy_label.get(&o.name).cloned(),
                        copy_progress: self
                            .copying
                            .get(&o.name)
                            .and_then(|(p, _)| self.jobs.get(p).copied()),
                        paused: self
                            .paused
                            .get(&o.name)
                            .copied()
                            .flatten()
                            .or_else(|| {
                                // KDE: el complemento pausa solo con una ventana encima.
                                self.videos
                                    .contains(&o.name)
                                    .then(|| self.surface.as_ref()?.window_pause(&o.name))
                                    .flatten()
                            })
                            .map(|r| r.as_str().to_string()),
                        pause_unsupported: self.pause_unsupported.contains(&o.name),
                    }
                })
                .collect(),
            surface: self.surface_name.clone(),
            surface_available: self.available,
            all: all.and_then(|a| a.folder.clone()),
            all_mode: all.map(|a| a.mode.clone()),
        }
    }
}

/// ¿El video ya es como la copia de ahorro? Solo se lee cuando se empezaría por ella, y una
/// vez por versión del fondo.
fn fits_saver(
    video: &VideoOptions,
    cache: &mut HashMap<(String, String), bool>,
    folder: &str,
    version: &str,
    path: &Path,
) -> bool {
    let (start, _) = video.saver.levels(video.no_real_gpu);
    if start != CopyLevel::Saver {
        return false;
    }
    let key = (folder.to_string(), version.to_string());
    if let Some(f) = cache.get(&key) {
        return *f;
    }
    let info = copies::probe(video.ffmpeg.as_deref(), path);
    let fits = info
        .as_ref()
        .is_some_and(|i| miraishin_core::copies::already_fits(i, &CopyTarget::SAVER));
    tracing::info!(folder = %folder, info = ?info, fits, "video header read");
    cache.insert(key, fits);
    fits
}

/// Nivel con que se muestra un video: el de inicio según el ajuste y la GPU, lo que ya subió
/// en esta sesión, o una copia que ya existe de antes (sin pasar del máximo). Si se
/// empezaría por la copia de ahorro pero el video ya es así (`fits_saver`), el original.
fn level_for(
    video: &VideoOptions,
    levels: &HashMap<(String, String), CopyLevel>,
    folder: &str,
    version: &str,
    largest: Size,
    fits_saver: bool,
) -> CopyLevel {
    let (mut start, cap) = video.saver.levels(video.no_real_gpu);
    let raised = levels
        .get(&(folder.to_string(), version.to_string()))
        .copied();
    if start == CopyLevel::Saver && fits_saver && raised.is_none() {
        start = CopyLevel::Original;
    }
    let mut level = raised.unwrap_or(start).max(start);
    for l in [CopyLevel::Screen, CopyLevel::Saver] {
        if l > level
            && l <= cap
            && l.target(largest).is_some_and(|t| {
                copies::copy_path(&video.copies_dir, folder, version, &t).is_file()
            })
        {
            level = l;
        }
    }
    level.min(cap)
}

/// Crea una copia en un hilo aparte; avisa el avance y el final por el canal del hilo.
fn start_copy(
    video: &VideoOptions,
    tx: &mpsc::Sender<Msg>,
    input: &Path,
    output: &Path,
    target: CopyTarget,
    folder: &str,
) {
    copies::prepare_folder(&video.copies_dir, folder, &[]);
    let (fail_tx, fail_path) = (tx.clone(), output.to_path_buf());
    let (ffmpeg, input, output, tx) = (
        video.ffmpeg.clone(),
        input.to_path_buf(),
        output.to_path_buf(),
        tx.clone(),
    );
    let spawned = std::thread::Builder::new()
        .name("video-copy".into())
        .spawn(move || {
            let mut last = None;
            let result = copies::make_copy(ffmpeg.as_deref(), &input, &output, &target, |p| {
                if last != Some(p) {
                    last = Some(p);
                    let _ = tx.send(Msg::CopyProgress(output.clone(), p));
                }
            });
            let _ = tx.send(Msg::CopyDone(output, result));
        });
    if spawned.is_err() {
        let _ = fail_tx.send(Msg::CopyDone(fail_path, Err("copy_failed")));
    }
}

/// Qué va en un monitor: lo suyo si tiene, si no lo de «todos». `None` = sin fondo.
pub fn resolve(assignments: &[Assignment], output: &str) -> Option<(String, FitMode)> {
    let own = assignments.iter().find(|a| a.output == output);
    let chosen = own.or_else(|| assignments.iter().find(|a| a.output == ALL_OUTPUTS))?;
    let folder = chosen.folder.clone()?;
    Some((folder, FitMode::parse(&chosen.mode).unwrap_or_default()))
}

/// Archivo de un fondo según su tipo.
enum Source {
    Image(PathBuf),
    Video(PathBuf),
}

/// Versión y archivo de un fondo, o el código de por qué no se puede mostrar.
fn lookup(
    library: &Arc<Mutex<Library>>,
    wallpapers_dir: &std::path::Path,
    folder: &str,
) -> (String, Result<Source, String>) {
    let lib = library.lock().unwrap_or_else(|p| p.into_inner());
    let row = match lib.get(folder) {
        Ok(Some(r)) => r,
        Ok(None) => return (String::new(), Err("wallpaper_not_found".into())),
        Err(_) => return (String::new(), Err("internal".into())),
    };
    let version = row.updated_at.clone();
    if !row.valid {
        return (version, Err("wallpaper_invalid".into()));
    }
    let Some(file) = row.file else {
        return (version, Err("wallpaper_invalid".into()));
    };
    let path = wallpapers_dir.join(folder).join(file);
    match row.kind.as_deref() {
        Some("image") => (version, Ok(Source::Image(path))),
        Some("video") => (version, Ok(Source::Video(path))),
        _ => (version, Err("wallpaper_kind_unsupported".into())),
    }
}

/// La superficie de este escritorio, si hay. Los códigos los explica `ctl` (fase 1.6).
/// `data_dir`: carpeta de datos de la app (en KDE ahí van los cuadros que lee Plasma).
pub fn auto_surface(data_dir: PathBuf) -> SurfaceFactory {
    Box::new(move |sink| {
        #[cfg(target_os = "linux")]
        {
            let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
            // En KDE, layer-shell tapa los íconos (prueba del 2026-10-30): ahí el fondo va por
            // el complemento de Plasma.
            if desktop.split(':').any(|d| d.eq_ignore_ascii_case("KDE")) {
                return miraishin_surface_plasma::PlasmaSurface::connect(
                    sink,
                    data_dir.join("plasma"),
                )
                .map(|s| (Box::new(s) as Box<dyn Surface>, "plasma"));
            }
            miraishin_surface_wayland::WaylandSurface::connect(sink)
                .map(|s| (Box::new(s) as Box<dyn Surface>, "layer_shell"))
                .map_err(|e| e.as_str().to_string())
        }
        #[cfg(windows)]
        {
            drop(data_dir);
            miraishin_surface_windows::WindowsSurface::connect(sink)
                .map(|s| {
                    let name = s.surface_name();
                    (Box::new(s) as Box<dyn Surface>, name)
                })
                .map_err(|e| e.as_str().to_string())
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            drop((sink, data_dir));
            Err("platform_pending".into())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(output: &str, folder: Option<&str>, mode: &str) -> Assignment {
        Assignment {
            output: output.into(),
            folder: folder.map(Into::into),
            mode: mode.into(),
        }
    }

    #[test]
    fn lo_del_monitor_gana_a_todos() {
        let list = [
            a("*", Some("lluvia"), "fill"),
            a("DP-1", Some("mar"), "fit"),
        ];
        assert_eq!(resolve(&list, "DP-1"), Some(("mar".into(), FitMode::Fit)));
        assert_eq!(
            resolve(&list, "HDMI-A-1"),
            Some(("lluvia".into(), FitMode::Fill))
        );
    }

    #[test]
    fn sin_fondo() {
        assert_eq!(resolve(&[], "DP-1"), None);
        let list = [a("*", Some("lluvia"), "fill"), a("DP-1", None, "fill")];
        assert_eq!(resolve(&list, "DP-1"), None);
    }

    #[test]
    fn modo_desconocido_usa_rellenar() {
        let list = [a("*", Some("lluvia"), "zoom")];
        assert_eq!(resolve(&list, "X"), Some(("lluvia".into(), FitMode::Fill)));
    }
}
