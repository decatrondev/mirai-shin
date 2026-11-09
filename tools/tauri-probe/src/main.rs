// Prueba 4.0: ventana Tauri sin marco del sistema. Imprime su PID y cuánto tardó en arrancar.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::Instant;

fn main() {
    let t = Instant::now();
    tauri::Builder::default()
        .setup(move |_app| {
            eprintln!(
                "probe pid={} setup_ms={}",
                std::process::id(),
                t.elapsed().as_millis()
            );
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("tauri");
}
