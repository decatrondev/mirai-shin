# Mirai Shin

> 未来・芯 — *El núcleo del mañana.* El nombre del producto vive en [`brand.json`](brand.json) y en ningún otro lugar.

Fondos de pantalla animados para Linux y Windows, pensados para gastar lo mínimo. Rust + Tauri. Código abierto con licencia AGPL-3.0.

**Estado:** en desarrollo. El plan está cerrado y se está construyendo la base. Todavía no hay nada para descargar.

Sitio: <https://miraishin.net>

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

### Componentes de terceros

El paquete de Linux (Flatpak) incluye estas bibliotecas, compiladas sin cambios desde sus fuentes oficiales. Las versiones exactas y las opciones de compilación están en [`packaging/flatpak/net.decatron.MiraiShin.yml`](packaging/flatpak/net.decatron.MiraiShin.yml).

| Biblioteca | Versión | Licencia | Para qué se usa |
|------------|---------|----------|-----------------|
| [mpv](https://mpv.io) (libmpv) | 0.41.0 | LGPL-2.1-or-later (compilada con `-Dgpl=false`) | Reproduce los fondos de video en Sway y Hyprland |
| [FFmpeg](https://ffmpeg.org) | 7.1.2 | LGPL-2.1-or-later (decodificadores, más el codificador VP9 y la salida WebM; sin partes GPL ni no libres) | Decodifica el video, y crea una copia compatible en VP9 cuando el equipo no puede reproducir el original |
| [libplacebo](https://code.videolan.org/videolan/libplacebo) | 7.387.0 | LGPL-2.1-or-later | Dibuja los cuadros del video con la tarjeta gráfica |
| [libass](https://github.com/libass/libass) | 0.17.4 | ISC | La necesita mpv |
| [libdisplay-info](https://gitlab.freedesktop.org/emersion/libdisplay-info) | 0.4.0 | MIT | La necesita mpv para decodificar por hardware |

Las bibliotecas LGPL van como bibliotecas compartidas aparte en `/app/lib`, así que se pueden reemplazar por otra compilación compatible. Sus textos de licencia van dentro del paquete, en `/app/share/licenses/net.decatron.MiraiShin/`. La app habla con libmpv a través de sus encabezados públicos, que tienen licencia ISC ([`crates/render-video`](crates/render-video)). Todo lo demás viene del runtime Freedesktop 25.08, con sus propias licencias (entre ellas libvpx, BSD, que FFmpeg usa para codificar VP9). En KDE Plasma el video lo reproduce el propio Plasma (Qt Multimedia), que no va dentro del paquete.

El paquete de Windows lleva las mismas versiones de mpv, FFmpeg, libplacebo y libass, compiladas en el CI con [`packaging/windows/build-libmpv.sh`](packaging/windows/build-libmpv.sh) (MSYS2). Ahí mpv dibuja con Direct3D 11 y decodifica con la tarjeta gráfica (D3D11VA), y para eso necesita además [shaderc](https://github.com/google/shaderc) y [SPIRV-Cross](https://github.com/KhronosGroup/SPIRV-Cross) (Apache-2.0). Van junto al daemon como DLL aparte (`libmpv-2.dll`, `avcodec-61.dll`…; libass va dentro de `libmpv-2.dll`), así que también se pueden reemplazar, con las DLL que necesitan de MSYS2 (FreeType, HarfBuzz, FriBidi, libvpx, dav1d, zlib, el runtime de GCC y otras, todas con licencias permisivas o LGPL). La carpeta `licenses\` tiene cada texto de licencia y `VERSIONES.txt` la versión exacta de cada pieza; la compilación se detiene si una DLL sale de un paquete fuera de su lista permitida.

---

In English: [`README.md`](README.md).
