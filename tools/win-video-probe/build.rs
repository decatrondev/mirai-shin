//! En Windows (MSVC) mete dentro del exe el mismo manifiesto que el daemon: sin declarar
//! Windows 8+ no se pueden crear ventanas hijas layered (escritorio elevado de 24H2+).

fn main() {
    let manifest = "../../apps/daemon/windows/daemon.manifest";
    println!("cargo:rerun-if-changed={manifest}");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if os == "windows" && env == "msvc" {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        let path = std::path::Path::new(&dir).join(manifest);
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
            path.display()
        );
    }
}
