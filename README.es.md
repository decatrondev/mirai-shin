# Mirai Shin

> 未来・芯 — *El núcleo del mañana.* El nombre del producto vive en [`brand.json`](brand.json) y en ningún otro lugar.

Fondos de pantalla animados para Linux y Windows, pensados para gastar lo mínimo. Rust + Tauri. Código abierto con licencia AGPL-3.0.

**Estado:** en desarrollo. El plan está cerrado y se está construyendo la base. Todavía no hay nada para descargar.

Sitio: <https://miraishin.decatron.net>

## Qué hará

- **Se pausa solo** cuando juegas en pantalla completa, con batería o con la pantalla bloqueada.
- **Linux de verdad:** Wayland nativo en KDE, Hyprland y Sway. GNOME y X11, después.
- **Un fondo por monitor**, o el mismo en todos.

Plataformas: Linux primero, luego Windows. Mac a futuro.

## Estructura

```
brand.json          identidad del producto (nombre, kanji, lema, enlaces)
apps/landing/       sitio web (React, Vite, TypeScript)       en curso
apps/daemon/        proceso de fondo que dibuja el fondo     fase 1
apps/ctl/           consola para hablar con el daemon        fase 1
crates/core/        reglas puras: formato de fondo, contrato de superficie, encuadre
crates/surface-wayland/  fondo en Sway/Hyprland (wlr-layer-shell)
crates/surface-plasma/   fondo en KDE Plasma (complemento de fondo propio)
crates/surface-windows/  fondo en Windows (ventana detrás de los íconos del escritorio)
crates/render-image/     imagen fija: decodificar y encuadrar a cada monitor
crates/render-video/     video con libmpv (se carga al arrancar; compilación LGPL)
crates/store/       biblioteca local (SQLite), ajustes (TOML), rutas
crates/ipc/         protocolo app <-> daemon (socket Unix / named pipe)
crates/logging/     logs a archivo con rotación diaria
crates/license/     interfaz de licencias (la verificación real llega después)
crates/brand/       lee brand.json
services/web/       API del panel /admin del sitio
packages/ui/        tokens y componentes, compartidos con la app
packages/i18n/      textos en inglés y español
docs/  docs/es/     documentación (inglés y español)
```

La app Tauri se agrega en la fase 4. La parte de Rust se compila y prueba con `cargo build --workspace` y `cargo test --workspace`.

## Licencia

[GNU AGPL-3.0](LICENSE) con una [excepción para el Steamworks SDK](LICENSE-EXCEPTION.md). Los aportes se aceptan bajo los mismos términos: ver [`CONTRIBUTING.es.md`](CONTRIBUTING.es.md).

---

In English: [`README.md`](README.md).
