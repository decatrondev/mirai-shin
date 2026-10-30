# Mirai Shin

> 未来・芯 — *El núcleo del mañana.* El nombre del producto vive en [`brand.json`](brand.json) y en ningún otro lugar.

Fondos de pantalla animados para Windows y Linux, pensados para gastar lo mínimo. Rust + Tauri. Código abierto con licencia AGPL-3.0.

**Estado:** en desarrollo. El plan está cerrado y se está construyendo la base. Todavía no hay nada para descargar.

Sitio: <https://miraishin.decatron.net>

## Qué hará

- **Se pausa solo** cuando juegas en pantalla completa, con batería o con la pantalla bloqueada.
- **Linux de verdad:** Wayland nativo en KDE, Hyprland y Sway. GNOME y X11, después.
- **Un fondo por monitor**, o el mismo en todos.

Plataformas: Windows y Linux. Mac a futuro.

## Estructura

```
brand.json          identidad del producto (nombre, kanji, lema, enlaces)
apps/landing/       sitio web (React, Vite, TypeScript)       en curso
packages/ui/        tokens y componentes, compartidos con la app
packages/i18n/      textos en inglés y español
docs/  docs/es/     documentación (inglés y español)
```

La app de escritorio (crates de Rust y la app Tauri) se agrega cuando empiece su primera fase.

## Licencia

[GNU AGPL-3.0](LICENSE) con una [excepción para el Steamworks SDK](LICENSE-EXCEPTION.md). Los aportes se aceptan bajo los mismos términos: ver [`CONTRIBUTING.es.md`](CONTRIBUTING.es.md).

---

In English: [`README.md`](README.md).
