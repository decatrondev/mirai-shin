mod i18n;

use i18n::{fill, t};
use miraishin_ipc::{
    codes, methods, CallError, Client, Endpoint, LibraryList, OutputsList, RescanResult,
    StatusResult,
};
use miraishin_store::{Language, Paths, Settings};
use serde_json::{json, Value};
use std::process::ExitCode;

/// Salidas: 0 bien, 1 error, 2 daemon apagado (no es una falla).
const EXIT_NOT_RUNNING: u8 = 2;

/// Lo que se le pide al daemon, ya leído de la línea de comandos.
#[derive(Debug, PartialEq)]
struct Command {
    method: &'static str,
    params: Value,
    as_json: bool,
}

#[derive(Debug, PartialEq)]
enum Parsed {
    Run(Command),
    Help,
    /// (clave del texto, lo que estaba mal)
    Bad(&'static str, String),
}

/// Lee los argumentos. Pura: se prueba sin daemon.
fn parse(args: &[String]) -> Parsed {
    let mut as_json = false;
    let mut monitor: Option<String> = None;
    let mut mode: Option<String> = None;
    let mut positional: Vec<&str> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--json" => as_json = true,
            "--monitor" | "--mode" => {
                let Some(v) = it.next() else {
                    return Parsed::Bad("missing_value", a.clone());
                };
                if a == "--monitor" {
                    monitor = Some(v.clone());
                } else {
                    mode = Some(v.clone());
                }
            }
            "--help" | "-h" => return Parsed::Help,
            f if f.starts_with("--") => return Parsed::Bad("unknown_option", f.to_string()),
            p => positional.push(p),
        }
    }
    let run = |method, params| {
        Parsed::Run(Command {
            method,
            params,
            as_json,
        })
    };
    match positional.as_slice() {
        [] | ["help"] => Parsed::Help,
        ["status"] => run(methods::STATUS, Value::Null),
        ["list"] => run(methods::LIBRARY_LIST, Value::Null),
        ["rescan"] => run(methods::LIBRARY_RESCAN, Value::Null),
        ["ping"] => run(methods::PING, Value::Null),
        ["shutdown"] => run(methods::SHUTDOWN, Value::Null),
        ["outputs"] => run(methods::OUTPUTS_LIST, Value::Null),
        ["set", folder] => run(
            methods::WALLPAPER_SET,
            json!({ "folder": folder, "output": monitor, "mode": mode }),
        ),
        ["set"] => Parsed::Bad("set_needs_folder", String::new()),
        ["clear"] => run(methods::WALLPAPER_CLEAR, json!({ "output": monitor })),
        ["pause"] => run(methods::PLAYBACK_PAUSE, json!({ "output": monitor })),
        ["resume"] => run(methods::PLAYBACK_RESUME, json!({ "output": monitor })),
        [cmd, ..] if KNOWN.contains(cmd) => {
            Parsed::Bad("extra_argument", positional[1..].join(" "))
        }
        [other, ..] => Parsed::Bad("unknown_command", other.to_string()),
    }
}

const KNOWN: &[&str] = &[
    "status", "list", "rescan", "ping", "shutdown", "outputs", "set", "clear", "pause", "resume",
    "help",
];

fn main() -> ExitCode {
    let lang = language();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Command {
        method,
        params,
        as_json,
    } = match parse(&args) {
        Parsed::Run(c) => c,
        Parsed::Help => {
            println!(
                "{}",
                fill(t(lang, "usage"), &[&miraishin_brand::get().name])
            );
            return ExitCode::SUCCESS;
        }
        Parsed::Bad(key, arg) => {
            eprintln!("{}", fill(t(lang, key), &[&arg]));
            return ExitCode::FAILURE;
        }
    };

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    runtime.block_on(async move {
        let Ok(endpoint) = Endpoint::default_for_user() else {
            eprintln!("{}", t(lang, "not_running"));
            return ExitCode::from(EXIT_NOT_RUNNING);
        };
        let mut client = match Client::connect(&endpoint).await {
            Ok(c) => c,
            Err(_) => {
                eprintln!("{}", t(lang, "not_running"));
                return ExitCode::from(EXIT_NOT_RUNNING);
            }
        };
        let hello = json!({ "protocol": miraishin_ipc::PROTOCOL_VERSION, "client": "ctl" });
        let result = match client.call(methods::HELLO, hello).await {
            Ok(_) => client.call(method, params).await,
            Err(e) => Err(e),
        };
        match result {
            Ok(value) => {
                if as_json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&value).unwrap_or_default()
                    );
                } else {
                    print_human(lang, method, value);
                }
                ExitCode::SUCCESS
            }
            Err(CallError::Rpc(e)) if e.code == codes::WALLPAPERS_DIR_MISSING => {
                let dir = status_dir(&mut client).await.unwrap_or_default();
                eprintln!("{}", fill(t(lang, "dir_missing"), &[&dir]));
                ExitCode::FAILURE
            }
            Err(CallError::Rpc(e)) => {
                match explained(&e.code) {
                    Some(key) => eprintln!("{}", t(lang, key)),
                    None => eprintln!("{}", fill(t(lang, "daemon_error"), &[&e.code])),
                }
                ExitCode::FAILURE
            }
            Err(_) => {
                eprintln!("{}", t(lang, "connection_lost"));
                ExitCode::FAILURE
            }
        }
    })
}

/// Idioma de los ajustes; si es «sistema», el de `LC_ALL` / `LANG`.
fn language() -> Language {
    let configured = Paths::system()
        .map(|p| Settings::load(&p.settings_file()).0.language)
        .unwrap_or_default();
    let locale = std::env::var("LC_ALL")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("LANG").ok());
    configured.resolve(locale.as_deref())
}

async fn status_dir(client: &mut Client) -> Option<String> {
    let v = client.call(methods::STATUS, Value::Null).await.ok()?;
    serde_json::from_value::<StatusResult>(v)
        .ok()
        .map(|s| s.wallpapers_dir)
}

fn print_human(lang: Language, method: &str, value: Value) {
    match method {
        methods::STATUS => {
            let s: StatusResult = serde_json::from_value(value).unwrap_or_default();
            println!(
                "{}",
                fill(
                    t(lang, "status_version"),
                    &[&s.version, &s.protocol.to_string()]
                )
            );
            println!(
                "{}",
                fill(t(lang, "status_uptime"), &[&uptime(s.uptime_secs)])
            );
            println!("{}", fill(t(lang, "status_folder"), &[&s.wallpapers_dir]));
            if !s.wallpapers_dir_exists {
                println!("{}", t(lang, "status_folder_missing"));
            }
            println!(
                "{}",
                fill(
                    t(lang, "status_counts"),
                    &[&s.valid.to_string(), &s.invalid.to_string()]
                )
            );
            println!("{}", fill(t(lang, "status_license"), &[&s.license]));
            let unknown = t(lang, "status_unknown");
            let desktop = if s.desktop.is_empty() {
                unknown
            } else {
                &s.desktop
            };
            if s.session.is_empty() && !s.desktop.is_empty() {
                // Windows: no hay tipo de sesión que mostrar.
                println!("{}", fill(t(lang, "status_desktop_only"), &[desktop]));
            } else {
                let session = if s.session.is_empty() {
                    unknown
                } else {
                    &s.session
                };
                println!("{}", fill(t(lang, "status_desktop"), &[desktop, session]));
            }
            print_surface(lang, &s.surface, s.surface_available);
            // Vacío con un daemon anterior a la fase 3.2: no se muestra.
            let power = match s.power.as_str() {
                "" => None,
                "ac" => Some("status_power_ac"),
                "battery" => Some("status_power_battery"),
                _ => Some("status_power_unavailable"),
            };
            if let Some(key) = power {
                println!("{}", t(lang, key));
            }
            // Vacío con un daemon anterior a la fase 3.4: no se muestra.
            let lock = match s.lock.as_str() {
                "" => None,
                "locked" => Some("status_lock_locked"),
                "unlocked" => Some("status_lock_unlocked"),
                _ => Some("status_lock_unavailable"),
            };
            if let Some(key) = lock {
                println!("{}", t(lang, key));
            }
        }
        methods::OUTPUTS_LIST => print_outputs(lang, value),
        methods::WALLPAPER_SET | methods::WALLPAPER_CLEAR => {
            let list: OutputsList = serde_json::from_value(value.clone()).unwrap_or_default();
            if list.surface_available {
                println!("{}", t(lang, "applied"));
            } else {
                println!("{}", t(lang, "saved_not_shown"));
            }
            print_outputs(lang, value);
        }
        methods::PLAYBACK_PAUSE | methods::PLAYBACK_RESUME => {
            let list: OutputsList = serde_json::from_value(value.clone()).unwrap_or_default();
            let key = if method == methods::PLAYBACK_PAUSE {
                "paused_done"
            } else {
                "resumed_done"
            };
            println!("{}", t(lang, key));
            if list.items.iter().any(|o| o.pause_unsupported) {
                println!("{}", t(lang, "pause_unsupported"));
            }
            print_outputs(lang, value);
        }
        methods::LIBRARY_LIST => {
            let list: LibraryList = serde_json::from_value(value).unwrap_or_default();
            if list.items.is_empty() {
                println!("{}", t(lang, "list_empty"));
            }
            for item in list.items {
                let title = item.title.unwrap_or_else(|| item.folder.clone());
                let kind = item.kind.unwrap_or_else(|| "-".into());
                if item.valid {
                    let newer = if item.newer_schema {
                        format!("  ({})", t(lang, "list_newer"))
                    } else {
                        String::new()
                    };
                    println!("  {title}  [{kind}]  {}{newer}", item.folder);
                } else {
                    println!(
                        "✗ {title}  [{kind}]  {} — {}: {}",
                        item.folder,
                        t(lang, "list_invalid"),
                        item.issues.join(", ")
                    );
                }
            }
        }
        methods::LIBRARY_RESCAN => {
            let r: RescanResult = serde_json::from_value(value).unwrap_or_default();
            println!(
                "{}",
                fill(
                    t(lang, "rescan_done"),
                    &[
                        &r.valid.to_string(),
                        &r.invalid.to_string(),
                        &r.removed.to_string()
                    ]
                )
            );
        }
        methods::PING => println!("{}", t(lang, "pong")),
        methods::SHUTDOWN => println!("{}", t(lang, "stopping")),
        _ => println!("{value}"),
    }
}

/// Errores del daemon que tienen su explicación propia.
fn explained(code: &str) -> Option<&'static str> {
    Some(match code {
        codes::WALLPAPER_NOT_FOUND => "err_wallpaper_not_found",
        codes::WALLPAPER_INVALID => "err_wallpaper_invalid",
        codes::WALLPAPER_KIND_UNSUPPORTED => "err_kind_unsupported",
        codes::UNKNOWN_OUTPUT => "err_unknown_output",
        codes::INVALID_MODE => "err_invalid_mode",
        _ => return None,
    })
}

/// Línea de `status` sobre cómo se pone el fondo, y si no se puede, por qué.
fn print_surface(lang: Language, surface: &str, available: bool) {
    if available {
        println!("{}", fill(t(lang, "surface_ok"), &[surface]));
        return;
    }
    let key = match surface {
        "kde_plugin_needs_restart" => "surface_kde_restart",
        "kde_plasma_unavailable" => "surface_kde_unavailable",
        "kde_plugin_install_failed" => "surface_kde_install_failed",
        "no_wayland" => "surface_no_wayland",
        "no_layer_shell" => "surface_no_layer_shell",
        "platform_pending" => "surface_platform_pending",
        "windows_no_desktop" => "surface_windows_no_desktop",
        _ => "surface_unavailable",
    };
    println!("{}", fill(t(lang, key), &[surface]));
}

fn print_outputs(lang: Language, value: Value) {
    let list: OutputsList = serde_json::from_value(value).unwrap_or_default();
    if !list.surface_available {
        print_surface(lang, &list.surface, false);
    } else if list.items.is_empty() {
        println!("{}", t(lang, "outputs_none"));
    }
    if let Some(all) = &list.all {
        let mode = list.all_mode.clone().unwrap_or_default();
        println!("{}", fill(t(lang, "outputs_all"), &[all, &mode]));
    }
    for o in list.items {
        let what = match (&o.wallpaper, &o.problem) {
            (Some(w), _) if o.copy_progress.is_some() => fill(
                t(lang, "outputs_copying"),
                &[w, &o.copy_progress.unwrap_or_default().to_string()],
            ),
            (Some(w), Some(p)) => match t(lang, &format!("problem_{p}")) {
                "" => fill(t(lang, "outputs_problem"), &[w, p]),
                own => fill(own, &[w]),
            },
            (Some(w), None) => {
                let mode = o.mode.as_deref().unwrap_or_default();
                match &o.copy {
                    Some(size) => fill(t(lang, "outputs_showing_copy"), &[w, mode, size]),
                    None => fill(t(lang, "outputs_showing"), &[w, mode]),
                }
            }
            (None, _) => t(lang, "outputs_empty").to_string(),
        };
        let what = match &o.paused {
            Some(reason) => {
                let why = match t(lang, &format!("pause_{reason}")) {
                    "" => reason.as_str(),
                    own => own,
                };
                format!("{what} — {}", fill(t(lang, "outputs_paused"), &[why]))
            }
            None => what,
        };
        let desc = if o.description.is_empty() {
            String::new()
        } else {
            format!(" — {}", o.description)
        };
        println!(
            "  {}{desc} — {}×{} ({} %) — {what}",
            o.name, o.width, o.height, o.scale_percent
        );
    }
}

fn uptime(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, secs % 3600 / 60, secs % 60);
    if h > 0 {
        format!("{h} h {m} min")
    } else if m > 0 {
        format!("{m} min {s} s")
    } else {
        format!("{s} s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Parsed {
        parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn comandos_simples() {
        assert_eq!(p(&[]), Parsed::Help);
        let Parsed::Run(c) = p(&["status", "--json"]) else {
            panic!()
        };
        assert_eq!((c.method, c.as_json), (methods::STATUS, true));
        assert_eq!(
            p(&["volar"]),
            Parsed::Bad("unknown_command", "volar".into())
        );
        assert_eq!(
            p(&["--nada"]),
            Parsed::Bad("unknown_option", "--nada".into())
        );
        assert_eq!(p(&["ping", "x"]), Parsed::Bad("extra_argument", "x".into()));
    }

    #[test]
    fn set_y_clear() {
        let Parsed::Run(c) = p(&["set", "lluvia"]) else {
            panic!()
        };
        assert_eq!(c.method, methods::WALLPAPER_SET);
        assert_eq!(
            c.params,
            json!({"folder": "lluvia", "output": null, "mode": null})
        );
        // Las opciones van en cualquier orden.
        let Parsed::Run(c) = p(&["--mode", "fit", "set", "mar", "--monitor", "DP-1"]) else {
            panic!()
        };
        assert_eq!(
            c.params,
            json!({"folder": "mar", "output": "DP-1", "mode": "fit"})
        );
        assert_eq!(p(&["set"]), Parsed::Bad("set_needs_folder", String::new()));
        assert_eq!(
            p(&["set", "x", "--monitor"]),
            Parsed::Bad("missing_value", "--monitor".into())
        );
        let Parsed::Run(c) = p(&["clear", "--monitor", "HDMI-A-1"]) else {
            panic!()
        };
        assert_eq!(
            (c.method, c.params),
            (methods::WALLPAPER_CLEAR, json!({"output": "HDMI-A-1"}))
        );
    }

    #[test]
    fn pausa_y_reanudar() {
        let Parsed::Run(c) = p(&["pause"]) else {
            panic!()
        };
        assert_eq!(
            (c.method, c.params),
            (methods::PLAYBACK_PAUSE, json!({"output": null}))
        );
        let Parsed::Run(c) = p(&["resume", "--monitor", "DP-1"]) else {
            panic!()
        };
        assert_eq!(
            (c.method, c.params),
            (methods::PLAYBACK_RESUME, json!({"output": "DP-1"}))
        );
        assert_eq!(
            p(&["pause", "x"]),
            Parsed::Bad("extra_argument", "x".into())
        );
    }
}
