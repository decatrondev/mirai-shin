mod i18n;

use i18n::{fill, t};
use miraishin_ipc::{
    codes, methods, CallError, Client, Endpoint, LibraryList, RescanResult, StatusResult,
};
use miraishin_store::{Language, Paths, Settings};
use serde_json::{json, Value};
use std::process::ExitCode;

/// Salidas: 0 bien, 1 error, 2 daemon apagado (no es una falla).
const EXIT_NOT_RUNNING: u8 = 2;

fn main() -> ExitCode {
    let lang = language();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let as_json = args.iter().any(|a| a == "--json");
    let command = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(String::as_str);

    let method = match command {
        Some("status") => methods::STATUS,
        Some("list") => methods::LIBRARY_LIST,
        Some("rescan") => methods::LIBRARY_RESCAN,
        Some("ping") => methods::PING,
        Some("shutdown") => methods::SHUTDOWN,
        None | Some("help") => {
            println!(
                "{}",
                fill(t(lang, "usage"), &[&miraishin_brand::get().name])
            );
            return ExitCode::SUCCESS;
        }
        Some(other) => {
            eprintln!("{}", fill(t(lang, "unknown_command"), &[other]));
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
            Ok(_) => client.call(method, Value::Null).await,
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
                eprintln!("{}", fill(t(lang, "daemon_error"), &[&e.code]));
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
