//! Copias compatibles de un video (fase 2.9). Si este equipo no puede reproducir bien el
//! original (falta el decodificador, no llega a la pantalla o va a saltos), el daemon crea
//! una vez una copia en VP9 (WebM, formato libre que traen todas las distros) y reproduce
//! esa. El original no se toca.
//!
//! Hay tres niveles y se sube de a uno cuando el nivel actual no se ve bien:
//! original → copia del tamaño de la pantalla → copia de ahorro (720p, 30 cuadros).
//! Un video que ya es como la copia de ahorro (`already_fits`) empieza por el original.
//! Aquí solo están las reglas; crear el archivo lo hace el daemon con `ffmpeg`.

use std::ffi::OsString;
use std::path::Path;

use crate::surface::Size;

/// Tamaño máximo y cuadros por segundo de una copia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopyTarget {
    pub max_width: u32,
    pub max_height: u32,
    pub fps_cap: u32,
}

impl CopyTarget {
    /// Modo ahorro: lo que en una máquina virtual sin GPU real se vio fluido (2026-11-01).
    pub const SAVER: Self = Self {
        max_width: 1280,
        max_height: 720,
        fps_cap: 30,
    };

    /// Del tamaño de la pantalla más grande (en píxeles reales), hasta 60 cuadros.
    pub fn screen(largest: Size) -> Self {
        Self {
            max_width: even(largest.width.max(2)),
            max_height: even(largest.height.max(2)),
            fps_cap: 60,
        }
    }

    /// Para mostrar y para el nombre del archivo: `1280x720`.
    pub fn label(&self) -> String {
        format!("{}x{}", self.max_width, self.max_height)
    }
}

fn even(n: u32) -> u32 {
    n & !1
}

/// Nivel de un video en un equipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CopyLevel {
    Original,
    Screen,
    Saver,
}

impl CopyLevel {
    pub fn next(self) -> Option<Self> {
        match self {
            Self::Original => Some(Self::Screen),
            Self::Screen => Some(Self::Saver),
            Self::Saver => None,
        }
    }

    pub fn target(self, largest: Size) -> Option<CopyTarget> {
        match self {
            Self::Original => None,
            Self::Screen => Some(CopyTarget::screen(largest)),
            Self::Saver => Some(CopyTarget::SAVER),
        }
    }
}

/// Ajuste `video_saver`: `auto` (ahorro si el equipo no tiene GPU real), `on` (siempre),
/// `off` (nunca: se llega como mucho a la copia del tamaño de la pantalla).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VideoSaver {
    #[default]
    Auto,
    On,
    Off,
}

impl VideoSaver {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::On => "on",
            Self::Off => "off",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "on" => Some(Self::On),
            "off" => Some(Self::Off),
            _ => None,
        }
    }

    /// Nivel con que empieza un video y el más alto al que puede subir.
    pub fn levels(self, no_real_gpu: bool) -> (CopyLevel, CopyLevel) {
        match self {
            Self::On => (CopyLevel::Saver, CopyLevel::Saver),
            Self::Off => (CopyLevel::Original, CopyLevel::Screen),
            Self::Auto if no_real_gpu => (CopyLevel::Saver, CopyLevel::Saver),
            Self::Auto => (CopyLevel::Original, CopyLevel::Saver),
        }
    }
}

/// Problemas de reproducción que se arreglan con una copia (los informa la superficie).
pub fn fixable_by_copy(code: &str) -> bool {
    matches!(
        code,
        "video_no_decoder" | "video_no_frames" | "video_too_slow"
    )
}

/// Fabricantes PCI de tarjetas gráficas virtuales: VMware, VirtualBox, virtio, QXL/Red Hat,
/// QEMU (bochs) y Hyper-V. Ahí no hay decodificación por GPU y dibujar cuesta.
pub fn is_virtual_gpu_vendor(id: u16) -> bool {
    matches!(id, 0x15ad | 0x80ee | 0x1af4 | 0x1b36 | 0x1234 | 0x1414)
}

/// «Sin GPU real»: ninguna tarjeta, o todas virtuales. `vendors` son los fabricantes de
/// `/sys/class/drm/card*/device/vendor`.
pub fn no_real_gpu(vendors: &[u16]) -> bool {
    vendors.iter().all(|v| is_virtual_gpu_vendor(*v))
}

/// Lee un `0x15ad` de `/sys`.
pub fn parse_vendor(text: &str) -> Option<u16> {
    u16::from_str_radix(text.trim().trim_start_matches("0x"), 16).ok()
}

/// Hash estable (FNV-1a de 64 bits) para nombres de archivo: no cambia entre versiones de
/// Rust, así una actualización no obliga a rehacer las copias.
pub fn stable_hash(parts: &[&str]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for b in p.bytes().chain(std::iter::once(0)) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{h:016x}")
}

/// Carpeta (dentro de la de copias) de las copias de un fondo.
pub fn folder_dir_name(folder: &str) -> String {
    stable_hash(&[folder])
}

/// Archivo de una copia: cambia si cambia el video (`version`) o el destino.
pub fn copy_file_name(version: &str, target: &CopyTarget) -> String {
    format!(
        "{}-{}-{}.webm",
        target.label(),
        target.fps_cap,
        &stable_hash(&[version])[..12]
    )
}

/// Argumentos de `ffmpeg` para crear la copia. Sin teclado (`-nostdin`: si no, se come la
/// entrada), sin sonido ni subtítulos, solo achica (nunca agranda), y el avance por la
/// salida estándar. Ajuste rápido de VP9: en un Ryzen 5 3500U tarda ~0,7× lo que dura
/// el video (medido 2026-11-01).
pub fn ffmpeg_args(input: &Path, output: &Path, t: &CopyTarget) -> Vec<OsString> {
    let vf = format!(
        "scale='min(iw,{w})':'min(ih,{h})':force_original_aspect_ratio=decrease:force_divisible_by=2:flags=bicubic,fps='min(source_fps,{f})'",
        w = t.max_width,
        h = t.max_height,
        f = t.fps_cap,
    );
    let mut args: Vec<OsString> = [
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "info",
        "-nostats",
        "-y",
        "-i",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    args.push(input.as_os_str().to_owned());
    for a in [
        "-map",
        "0:v:0",
        "-an",
        "-sn",
        "-dn",
        "-vf",
        &vf,
        "-c:v",
        "libvpx-vp9",
        "-deadline",
        "realtime",
        "-cpu-used",
        "8",
        "-row-mt",
        "1",
        "-b:v",
        "0",
        "-crf",
        "32",
        "-f",
        "webm",
        "-progress",
        "pipe:1",
    ] {
        args.push(a.into());
    }
    args.push(output.as_os_str().to_owned());
    args
}

/// Lo que importa de un video para decidir si necesita copia: códec, tamaño y cuadros.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoInfo {
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
}

/// Argumentos para leer solo la cabecera del video (`ffmpeg -i`, sin salida: termina con
/// error, pero antes escribe los flujos en la salida de errores).
pub fn probe_args(input: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["-nostdin", "-hide_banner", "-i"]
        .iter()
        .map(OsString::from)
        .collect();
    args.push(input.as_os_str().to_owned());
    args
}

/// Lee el primer flujo de video de lo que `ffmpeg -i` escribe:
/// `Stream #0:0…: Video: h264 (High) (avc1 / 0x31637661), yuv420p(…), 1280x720 […], 30 fps, …`.
/// Las portadas (`attached pic`) no cuentan.
pub fn parse_video_info(text: &str) -> Option<VideoInfo> {
    let line = text
        .lines()
        .find(|l| l.contains("Stream #") && l.contains("Video:") && !l.contains("attached pic"))?;
    let rest = line.split_once("Video:")?.1;
    let codec = rest
        .split_whitespace()
        .next()?
        .trim_end_matches(',')
        .to_string();
    let parts: Vec<&str> = rest.split(',').map(str::trim).collect();
    let (width, height) = parts.iter().skip(1).find_map(|p| {
        let (w, h) = p.split_whitespace().next()?.split_once('x')?;
        let (w, h) = (w.parse::<u32>().ok()?, h.parse::<u32>().ok()?);
        (w > 0 && h > 0).then_some((w, h))
    })?;
    let rate = |suffix: &str| {
        parts.iter().find_map(|p| {
            p.strip_suffix(suffix)
                .and_then(|n| n.trim().parse::<f64>().ok())
        })
    };
    let fps = rate(" fps").or_else(|| rate(" tbr"))?;
    Some(VideoInfo {
        codec,
        width,
        height,
        fps,
    })
}

/// Códecs que se ven en todas partes sin copia (el de la copia es VP9). HEVC no: Plasma de
/// Nobara no lo trae, y ahí se vio gris.
const COMMON_CODECS: &[&str] = &["h264", "vp8", "vp9", "av1"];

/// ¿El video ya es como su copia de `target` (no más grande, no más cuadros, códec común)?
/// Entonces convertirlo no ganaría nada: se reproduce el original, y solo si la superficie
/// dice que no se ve se hace la copia.
pub fn already_fits(info: &VideoInfo, target: &CopyTarget) -> bool {
    COMMON_CODECS.contains(&info.codec.as_str())
        && info.width <= target.max_width
        && info.height <= target.max_height
        && info.fps <= f64::from(target.fps_cap) + 0.01
}

/// ¿`ffmpeg` no pudo abrir el video porque el archivo está roto (cortado, a medio bajar)?
/// Se mira en sus últimas líneas de error.
pub fn is_damaged_input(stderr_tail: &[String]) -> bool {
    stderr_tail.iter().any(|l| {
        l.contains("moov atom not found") || l.contains("Invalid data found when processing input")
    })
}

/// Duración del video en microsegundos, de la línea `  Duration: 00:01:02.50, …` que
/// `ffmpeg` escribe al abrirlo.
pub fn parse_duration_us(line: &str) -> Option<u64> {
    let rest = line.trim_start().strip_prefix("Duration:")?.trim_start();
    let hms = rest.split(',').next()?.trim();
    let mut it = hms.split(':');
    let (h, m, s) = (it.next()?, it.next()?, it.next()?);
    let secs: f64 = s.parse().ok()?;
    let total = h.parse::<u64>().ok()? * 3600 + m.parse::<u64>().ok()? * 60;
    Some(total * 1_000_000 + (secs * 1_000_000.0) as u64)
}

/// Avance de `-progress`: líneas `out_time_us=12345678`.
pub fn parse_progress_us(line: &str) -> Option<u64> {
    line.trim()
        .strip_prefix("out_time_us=")
        .and_then(|v| v.parse().ok())
}

/// Porcentaje hecho (0–99 mientras trabaja; 100 lo marca el daemon al terminar).
pub fn percent(done_us: u64, total_us: u64) -> u8 {
    if total_us == 0 {
        return 0;
    }
    ((done_us.saturating_mul(100) / total_us).min(99)) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destinos() {
        let t = CopyTarget::screen(Size::new(1367, 769));
        assert_eq!((t.max_width, t.max_height, t.fps_cap), (1366, 768, 60));
        assert_eq!(CopyTarget::SAVER.label(), "1280x720");
        assert_eq!(CopyLevel::Original.target(Size::new(10, 10)), None);
        assert_eq!(
            CopyLevel::Saver.target(Size::new(3840, 2160)),
            Some(CopyTarget::SAVER)
        );
        assert_eq!(CopyLevel::Original.next(), Some(CopyLevel::Screen));
        assert_eq!(CopyLevel::Screen.next(), Some(CopyLevel::Saver));
        assert_eq!(CopyLevel::Saver.next(), None);
    }

    #[test]
    fn ajuste_de_ahorro() {
        use CopyLevel::*;
        assert_eq!(VideoSaver::parse(" ON "), Some(VideoSaver::On));
        assert_eq!(VideoSaver::parse("tal vez"), None);
        assert_eq!(VideoSaver::Auto.levels(false), (Original, Saver));
        assert_eq!(VideoSaver::Auto.levels(true), (Saver, Saver));
        assert_eq!(VideoSaver::On.levels(false), (Saver, Saver));
        assert_eq!(VideoSaver::Off.levels(true), (Original, Screen));
    }

    #[test]
    fn gpu_virtual() {
        assert!(no_real_gpu(&[]));
        assert!(no_real_gpu(&[0x15ad]));
        assert!(!no_real_gpu(&[0x1002]));
        assert!(!no_real_gpu(&[0x15ad, 0x10de]));
        assert_eq!(parse_vendor("0x15ad\n"), Some(0x15ad));
        assert_eq!(parse_vendor("basura"), None);
        assert!(fixable_by_copy("video_too_slow"));
        assert!(!fixable_by_copy("video_unreadable"));
    }

    #[test]
    fn nombres_estables() {
        // Si esto cambia, todas las copias guardadas se rehacen: no cambiarlo a la ligera.
        assert_eq!(stable_hash(&["a"]), "089be207b544f1e4");
        assert_ne!(stable_hash(&["ab", "c"]), stable_hash(&["a", "bc"]));
        let n = copy_file_name("2026-11-01T00:00:00Z", &CopyTarget::SAVER);
        assert!(n.starts_with("1280x720-30-") && n.ends_with(".webm"), "{n}");
        assert_ne!(n, copy_file_name("otra", &CopyTarget::SAVER));
        assert_eq!(folder_dir_name("olas").len(), 16);
    }

    #[test]
    fn argumentos_de_ffmpeg() {
        let a = ffmpeg_args(
            Path::new("/v/olas.mp4"),
            Path::new("/c/x.webm.part"),
            &CopyTarget::SAVER,
        );
        let a: Vec<String> = a.iter().map(|s| s.to_string_lossy().into_owned()).collect();
        assert_eq!(a[0], "-nostdin");
        let i = a.iter().position(|x| x == "-i").unwrap();
        assert_eq!(a[i + 1], "/v/olas.mp4");
        assert_eq!(a.last().unwrap(), "/c/x.webm.part");
        let vf = &a[a.iter().position(|x| x == "-vf").unwrap() + 1];
        assert!(
            vf.contains("min(iw,1280)") && vf.contains("min(ih,720)"),
            "{vf}"
        );
        assert!(vf.contains("min(source_fps,30)"), "{vf}");
        assert!(a.contains(&"libvpx-vp9".to_string()) && a.contains(&"-an".to_string()));
    }

    #[test]
    fn avance() {
        assert_eq!(
            parse_duration_us("  Duration: 00:01:02.50, start: 0.000000, bitrate: 8316 kb/s"),
            Some(62_500_000)
        );
        assert_eq!(parse_duration_us("  Duration: N/A, bitrate: N/A"), None);
        assert_eq!(parse_progress_us("out_time_us=20000000"), Some(20_000_000));
        assert_eq!(parse_progress_us("out_time_us=N/A"), None);
        assert_eq!(percent(10, 20), 50);
        assert_eq!(percent(30, 20), 99);
        assert_eq!(percent(5, 0), 0);
    }

    #[test]
    fn videos_que_ya_caben() {
        let h264 = "  Stream #0:0[0x1](und): Video: h264 (High 4:4:4 Predictive) (avc1 / 0x31637661), yuv444p(progressive), 1280x720 [SAR 1:1 DAR 16:9], 140 kb/s, 30 fps, 30 tbr, 15360 tbn (default)";
        let vp9 = "  Stream #0:0: Video: vp9 (Profile 1), gbrp(pc, gbr/unknown/unknown, progressive), 640x360, SAR 1:1 DAR 16:9, 29.97 fps, 29.97 tbr, 1k tbn";
        let hevc = "  Stream #0:0: Video: hevc (Rext), gbrp(pc, gbr/unknown/unknown, progressive), 1280x720 [SAR 1:1 DAR 16:9], 30 fps, 30 tbr, 1k tbn";
        let vertical = "  Stream #0:0[0x1](und): Video: h264 (High) (avc1 / 0x31637661), yuv420p(progressive), 720x1280 [SAR 1:1 DAR 9:16], 128 kb/s, 60 fps, 60 tbr, 15360 tbn (default)";
        let solo_tbr = "  Stream #0:0: Video: vp8, yuv420p(progressive), 1280x720, 25 tbr, 1k tbn";
        let portada = "  Stream #0:1: Video: mjpeg (Baseline), yuvj420p(pc), 600x600, 90k tbr, 90k tbn (attached pic)";

        let i = parse_video_info(&format!("Input #0, mov\n{h264}\n")).unwrap();
        assert_eq!(
            (i.codec.as_str(), i.width, i.height, i.fps),
            ("h264", 1280, 720, 30.0)
        );
        assert!(already_fits(&i, &CopyTarget::SAVER));

        let i = parse_video_info(vp9).unwrap();
        assert_eq!((i.width, i.height), (640, 360));
        assert!(
            already_fits(&i, &CopyTarget::SAVER),
            "29,97 cuadros caben en 30"
        );

        let i = parse_video_info(hevc).unwrap();
        assert!(
            !already_fits(&i, &CopyTarget::SAVER),
            "HEVC siempre se copia"
        );

        let i = parse_video_info(vertical).unwrap();
        assert_eq!((i.width, i.height, i.fps), (720, 1280, 60.0));
        assert!(!already_fits(&i, &CopyTarget::SAVER));

        assert_eq!(parse_video_info(solo_tbr).unwrap().fps, 25.0);
        assert_eq!(parse_video_info(portada), None);
        assert_eq!(
            parse_video_info(&format!("{portada}\n{vp9}"))
                .unwrap()
                .codec,
            "vp9"
        );
        assert_eq!(
            parse_video_info("Invalid data found when processing input"),
            None
        );

        let roto = [
            "[mov,mp4,m4a,3gp,3g2,mj2 @ 0x55e4c0391840] moov atom not found".to_string(),
            "Error opening input files: Invalid data found when processing input".to_string(),
        ];
        assert!(is_damaged_input(&roto));
        assert!(!is_damaged_input(&["No space left on device".to_string()]));

        let a = probe_args(Path::new("/v/olas.mp4"));
        assert_eq!(a.last().unwrap(), "/v/olas.mp4");
        assert_eq!(a[0], "-nostdin");
    }
}
