//! Daemon: arranca, lee ajustes, mantiene la biblioteca, pone los fondos y responde por IPC.

pub mod copies;
pub mod display;

use chrono::Utc;
use display::{Display, SurfaceFactory};
use miraishin_core::fit::FitMode;
use miraishin_ipc::{
    codes, events, methods, serve, BindError, Endpoint, Event, Handler, HandlerFuture, HelloParams,
    HelloResult, LibraryItem, LibraryList, Listener, RescanResult, RpcError, StatusResult,
    WallpaperClearParams, WallpaperSetParams, PROTOCOL_VERSION,
};
use miraishin_license::LicenseProvider;
use miraishin_store::library::StoreError;
use miraishin_store::{paths::redact, Library, Paths, Settings, ALL_OUTPUTS};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::{broadcast, Notify};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Métodos que entiende esta versión (se anuncian en `hello`).
pub const METHODS: &[&str] = &[
    methods::HELLO,
    methods::PING,
    methods::STATUS,
    methods::LIBRARY_LIST,
    methods::LIBRARY_RESCAN,
    methods::SHUTDOWN,
    methods::OUTPUTS_LIST,
    methods::WALLPAPER_SET,
    methods::WALLPAPER_CLEAR,
];

pub struct RunConfig {
    pub paths: Paths,
    pub endpoint: Endpoint,
    /// En pruebas el registro global ya puede estar instalado: no se toca.
    pub init_logging: bool,
    pub console_logs: bool,
    /// Cómo se pone el fondo. `None` = la de este escritorio (`display::auto_surface`).
    pub surface: Option<SurfaceFactory>,
    /// Para pruebas: GPU, `ffmpeg` y tiempos de las copias de video. `None` = los de este
    /// equipo (`copies::no_real_gpu`, `copies::find_ffmpeg`).
    pub video_env: Option<VideoEnv>,
}

/// Lo que en un equipo real se detecta solo (ver `RunConfig::video_env`).
#[derive(Debug, Clone)]
pub struct VideoEnv {
    pub no_real_gpu: bool,
    pub ffmpeg: Option<std::path::PathBuf>,
    pub check_delays: Vec<std::time::Duration>,
}

#[derive(Debug)]
pub enum RunError {
    AlreadyRunning,
    Other(anyhow::Error),
}

impl From<anyhow::Error> for RunError {
    fn from(e: anyhow::Error) -> Self {
        Self::Other(e)
    }
}

pub async fn run(config: RunConfig) -> Result<(), RunError> {
    let (settings, warnings) = Settings::load(&config.paths.settings_file());
    let _log_guard = if config.init_logging {
        Some(miraishin_logging::init(
            &config.paths.logs_dir(),
            "daemon",
            settings.log_level.as_str(),
            config.console_logs,
        )?)
    } else {
        None
    };
    tracing::info!(
        version = VERSION,
        protocol = PROTOCOL_VERSION,
        "daemon starting"
    );
    for w in &warnings {
        tracing::warn!(warning = ?w, "settings: using the default for an invalid value");
    }

    let listener = match Listener::bind(&config.endpoint).await {
        Ok(l) => l,
        Err(BindError::AlreadyRunning) => {
            tracing::error!("another daemon is already running for this user");
            return Err(RunError::AlreadyRunning);
        }
        Err(BindError::Io(e)) => return Err(anyhow::Error::from(e).context("ipc bind").into()),
    };

    let wallpapers_dir = match &settings.wallpapers_dir {
        Some(dir) => dir.clone(),
        None => {
            // Solo se crea la carpeta por defecto; una elegida por la persona no se inventa.
            let dir = config.paths.default_wallpapers_dir();
            std::fs::create_dir_all(&dir).map_err(anyhow::Error::from)?;
            dir
        }
    };

    let (library, report) = Library::open(
        &config.paths.library_db(),
        &config.paths.backups_dir(),
        Utc::now(),
    )
    .map_err(anyhow::Error::from)?;
    if let Some(moved) = &report.damaged_moved_to {
        tracing::warn!(moved_to = %redact(moved), "library database was damaged; set aside and rebuilt");
    }
    if let Some(backup) = &report.backup {
        tracing::info!(backup = %redact(backup), "library backed up before migrating");
    }
    if report.version_before != report.version_after {
        tracing::info!(
            from = report.version_before,
            to = report.version_after,
            "library migrated"
        );
    }
    if report.newer_db {
        tracing::warn!(
            version = report.version_before,
            "library was created by a newer version"
        );
    }

    let (events_tx, _) = broadcast::channel(64);
    let shutdown = Arc::new(Notify::new());
    let library = Arc::new(Mutex::new(library));
    let screen = Display::start(
        config
            .surface
            .unwrap_or_else(|| display::auto_surface(config.paths.data_dir.clone())),
        library.clone(),
        wallpapers_dir.clone(),
        events_tx.clone(),
        {
            let env = config.video_env.unwrap_or_else(|| VideoEnv {
                no_real_gpu: copies::no_real_gpu(),
                ffmpeg: copies::find_ffmpeg(),
                check_delays: display::VideoOptions::DEFAULT_CHECKS.to_vec(),
            });
            display::VideoOptions {
                copies_dir: config.paths.video_copies_dir(),
                saver: settings.video_saver,
                no_real_gpu: env.no_real_gpu,
                ffmpeg: env.ffmpeg,
                check_delays: env.check_delays,
            }
        },
    );
    let (surface_name, surface_ok) = screen.surface();
    tracing::info!(
        surface = surface_name,
        available = surface_ok,
        "wallpaper surface"
    );
    let daemon = Arc::new(Daemon {
        library,
        display: screen.clone(),
        wallpapers_dir,
        started: Instant::now(),
        license: Box::new(miraishin_license::NotRequired),
        events: events_tx.clone(),
        shutdown: shutdown.clone(),
    });
    match daemon.rescan().await {
        Ok(r) => tracing::info!(valid = r.valid, invalid = r.invalid, "library scanned"),
        Err(e) => tracing::warn!(code = %e.code, "initial scan skipped"),
    }

    let stop = shutdown.clone();
    serve(listener, daemon, events_tx, async move {
        tokio::select! {
            _ = stop.notified() => tracing::info!("shutdown requested over ipc"),
            _ = termination() => tracing::info!("shutdown by signal"),
        }
    })
    .await;
    // Deja salir la respuesta a `shutdown` antes de cerrar.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    screen.stop();
    tracing::info!("daemon stopped");
    Ok(())
}

async fn termination() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = term.recv() => {},
                    _ = tokio::signal::ctrl_c() => {},
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

struct Daemon {
    library: Arc<Mutex<Library>>,
    display: Display,
    wallpapers_dir: PathBuf,
    started: Instant,
    license: Box<dyn LicenseProvider>,
    events: broadcast::Sender<Event>,
    shutdown: Arc<Notify>,
}

impl Daemon {
    async fn rescan(&self) -> Result<RescanResult, RpcError> {
        let lib = self.library.clone();
        let dir = self.wallpapers_dir.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            let mut lib = lib.lock().unwrap_or_else(|p| p.into_inner());
            lib.rescan(&dir, Utc::now())
        })
        .await
        .map_err(|_| RpcError::new(codes::INTERNAL))?;
        match outcome {
            Ok(r) => {
                let result = RescanResult {
                    valid: r.valid as u64,
                    invalid: r.invalid as u64,
                    removed: r.removed as u64,
                };
                let _ = self
                    .events
                    .send(Event::new(events::LIBRARY_CHANGED, json!(result)));
                // Un fondo puesto pudo cambiar o desaparecer.
                self.display.apply_nowait();
                Ok(result)
            }
            Err(StoreError::WallpapersDirMissing) => {
                Err(RpcError::new(codes::WALLPAPERS_DIR_MISSING))
            }
            Err(e) => Err(internal(e)),
        }
    }

    fn status(&self) -> Result<StatusResult, RpcError> {
        let (valid, invalid) = self.lock().counts().map_err(internal)?;
        Ok(StatusResult {
            version: VERSION.to_string(),
            protocol: PROTOCOL_VERSION,
            uptime_secs: self.started.elapsed().as_secs(),
            wallpapers_dir: redact(&self.wallpapers_dir),
            wallpapers_dir_exists: self.wallpapers_dir.is_dir(),
            valid: valid as u64,
            invalid: invalid as u64,
            license: self.license.kind().to_string(),
            surface: self.display.surface().0.to_string(),
            surface_available: self.display.surface().1,
            desktop: desktop_name(),
            session: env_text("XDG_SESSION_TYPE"),
        })
    }

    async fn set_wallpaper(&self, p: WallpaperSetParams) -> Result<Value, RpcError> {
        let mode = match p.mode.as_deref() {
            None => FitMode::default(),
            Some(m) => FitMode::parse(m).ok_or_else(|| RpcError::new(codes::INVALID_MODE))?,
        };
        let row = self
            .lock()
            .get(&p.folder)
            .map_err(internal)?
            .ok_or_else(|| RpcError::new(codes::WALLPAPER_NOT_FOUND))?;
        if !row.valid {
            return Err(RpcError::new(codes::WALLPAPER_INVALID));
        }
        if !matches!(row.kind.as_deref(), Some("image" | "video")) {
            return Err(RpcError::new(codes::WALLPAPER_KIND_UNSUPPORTED));
        }
        let output = self.checked_output(p.output).await?;
        self.lock()
            .assign(&output, &p.folder, mode.as_str(), Utc::now())
            .map_err(internal)?;
        self.display.apply().await;
        to_value(self.display.outputs().await)
    }

    async fn clear_wallpaper(&self, p: WallpaperClearParams) -> Result<Value, RpcError> {
        let output = self.checked_output(p.output).await?;
        self.lock()
            .unassign(&output, Utc::now())
            .map_err(internal)?;
        self.display.apply().await;
        to_value(self.display.outputs().await)
    }

    /// Monitor pedido, o «todos». Si hay superficie, el monitor tiene que estar conectado;
    /// si no la hay (todavía), se guarda igual para cuando la haya.
    async fn checked_output(&self, output: Option<String>) -> Result<String, RpcError> {
        let Some(name) = output.filter(|o| o != ALL_OUTPUTS) else {
            return Ok(ALL_OUTPUTS.to_string());
        };
        if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
            return Err(RpcError::new(codes::INVALID_PARAMS));
        }
        if self.display.surface().1 {
            let known = self.display.outputs().await;
            if !known.items.iter().any(|o| o.name == name) {
                return Err(RpcError::new(codes::UNKNOWN_OUTPUT));
            }
        }
        Ok(name)
    }

    fn list(&self) -> Result<LibraryList, RpcError> {
        let rows = self.lock().list().map_err(internal)?;
        Ok(LibraryList {
            items: rows
                .into_iter()
                .map(|r| LibraryItem {
                    folder: r.folder,
                    id: r.id,
                    kind: r.kind,
                    title: r.title,
                    valid: r.valid,
                    newer_schema: r.newer_schema,
                    issues: r.issues,
                })
                .collect(),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Library> {
        // Un pánico en otra tarea no debe dejar la biblioteca inservible.
        self.library.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// Error interno: al log con detalle, al cliente solo el código.
fn internal(e: impl std::fmt::Display) -> RpcError {
    tracing::error!(error = %e, "internal error");
    RpcError::new(codes::INTERNAL)
}

/// Escritorio para `status`: en Linux sale de `XDG_CURRENT_DESKTOP`; en Windows no hay
/// variable, y la forma del escritorio va en el nombre de la superficie.
fn desktop_name() -> String {
    if cfg!(windows) {
        return "Windows".into();
    }
    env_text("XDG_CURRENT_DESKTOP")
}

/// Variable de entorno para mostrar: corta y sin caracteres de control.
fn env_text(key: &str) -> String {
    std::env::var(key)
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .take(64)
        .collect()
}

fn parse<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, RpcError> {
    serde_json::from_value(params).map_err(|_| RpcError::new(codes::INVALID_PARAMS))
}

fn to_value(v: impl serde::Serialize) -> Result<Value, RpcError> {
    serde_json::to_value(v).map_err(internal)
}

impl Handler for Daemon {
    fn handle(&self, method: String, params: Value) -> HandlerFuture<'_> {
        Box::pin(async move {
            match method.as_str() {
                methods::HELLO => {
                    let p: HelloParams = if params.is_null() {
                        HelloParams::default()
                    } else {
                        serde_json::from_value(params)
                            .map_err(|_| RpcError::new(codes::INVALID_PARAMS))?
                    };
                    tracing::debug!(client = %p.client, protocol = p.protocol, "hello");
                    to_value(HelloResult {
                        protocol: PROTOCOL_VERSION,
                        daemon_version: VERSION.to_string(),
                        methods: METHODS.iter().map(|m| m.to_string()).collect(),
                    })
                }
                methods::PING => Ok(json!("pong")),
                methods::STATUS => to_value(self.status()?),
                methods::LIBRARY_LIST => to_value(self.list()?),
                methods::LIBRARY_RESCAN => to_value(self.rescan().await?),
                methods::OUTPUTS_LIST => to_value(self.display.outputs().await),
                methods::WALLPAPER_SET => self.set_wallpaper(parse(params)?).await,
                methods::WALLPAPER_CLEAR => {
                    let p = if params.is_null() {
                        WallpaperClearParams::default()
                    } else {
                        parse(params)?
                    };
                    self.clear_wallpaper(p).await
                }
                methods::SHUTDOWN => {
                    self.shutdown.notify_one();
                    Ok(json!({ "stopping": true }))
                }
                _ => Err(RpcError::new(codes::UNKNOWN_METHOD)),
            }
        })
    }
}
