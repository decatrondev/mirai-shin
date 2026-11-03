# Daemon y `miraishin-ctl`

> En desarrollo: el daemon muestra imágenes fijas en KDE Plasma (con su propio complemento de fondo de Plasma), Sway y Hyprland (Wayland con `wlr-layer-shell`), y en Windows (en prueba). Los videos se reproducen en KDE Plasma, en Sway y Hyprland, y en Windows (Sway, Hyprland y Windows en prueba). X11 y GNOME llegan en próximas versiones.

## Daemon

`miraishin-daemon` corre en segundo plano, aparte de la app, para que el fondo siga aunque se cierre la app. Al arrancar:

1. Lee los ajustes.
2. Abre la biblioteca (y la migra si hace falta, con un respaldo antes).
3. Recorre la carpeta de fondos.
4. Vuelve a poner el fondo elegido en cada monitor.
5. Queda atento a la app y a `miraishin-ctl`.

Corre un solo daemon por usuario. Un segundo sale con el código `3` y no toca al primero.

## `miraishin-ctl`

Herramienta de consola para hablar con el daemon. Se entrega con la app, para diagnóstico y soporte.

| Comando | Qué hace |
|---|---|
| `status` | Versión del daemon, tiempo encendido, carpeta de fondos, cuántos fondos son válidos o tienen problemas, licencia, si este escritorio puede mostrar el fondo (y si no, por qué), y si el equipo está con cargador o con batería |
| `list` | Todos los fondos de la biblioteca; los que tienen problemas muestran sus [códigos](wallpaper-format.md#problemas) |
| `rescan` | Vuelve a leer la carpeta de fondos (un fondo en pantalla que cambió se vuelve a dibujar) |
| `outputs` | Monitores (nombre, tamaño, escala) y lo que muestra cada uno |
| `set <carpeta> [--monitor <nombre>] [--mode <modo>]` | Pone un fondo. Sin `--monitor`, el mismo en todos (y se olvida lo que tenía cada monitor). Modos: `fill` (rellenar, por defecto, recorta), `fit` (ajustar, con bordes), `stretch` (estirar), `center` (centrar, tamaño real) |
| `clear [--monitor <nombre>]` | Quita el fondo. Con `--monitor`, ese monitor queda sin fondo aunque haya uno para todos |
| `pause [--monitor <nombre>]` | Pausa el video de fondo (queda el último cuadro en pantalla) hasta `resume` o hasta reiniciar el daemon. Las imágenes no cambian. `outputs` muestra `en pausa: a mano` |
| `resume [--monitor <nombre>]` | Vuelve a reproducir lo que pausó `pause`. Reanudar un monitor cuando estaban todos en pausa deja en pausa a los demás |
| `ping` | Comprueba que el daemon responde |
| `shutdown` | Apaga el daemon |

La carpeta es la que muestra `list`; el nombre del monitor, el que muestra `outputs`. Un monitor nuevo recibe el fondo elegido para todos. Si una imagen no se puede leer, ese monitor muestra un color grafito y `outputs` muestra el código del problema.

Un fondo de video se repite en bucle, sin sonido, y usa los mismos modos. Donde los videos todavía no están disponibles, el monitor muestra el color grafito y `outputs` lo explica (`video_unsupported`, no es un error); si falta el archivo del video, el código es `video_unreadable`.

En Sway y Hyprland los videos los reproduce libmpv (el Flatpak la trae; fuera de él, instala el paquete `mpv` de tu distribución) y se dibujan con la tarjeta gráfica por EGL. Los monitores que muestran el mismo video con el mismo modo comparten un reproductor, así que el video se decodifica una sola vez. Códigos: `mpv_unavailable` (falta libmpv), `video_no_gpu` (EGL no disponible), `video_no_decoder` (formato o códec no disponible, prueba WebM), `video_unplayable` (archivo dañado o no es un video). `MIRAISHIN_MPV_LOG=v` manda los mensajes detallados de mpv al registro, para diagnóstico.

Donde todavía no se puede mostrar el fondo (X11 y GNOME), `set` guarda igual lo elegido y `status` explica por qué; no es un error.

En KDE Plasma el daemon instala su complemento de fondo en `~/.local/share/plasma/wallpapers/` y cambia cada pantalla a ese tipo de fondo; `clear` le devuelve a la pantalla el tipo de fondo que tenía antes. La primera vez, Plasma puede necesitar reiniciarse (cerrar sesión y volver a entrar) para ver el complemento: `status` lo avisa. Los videos los reproduce Plasma dentro del complemento (Qt Multimedia), con los decodificadores que instala tu distribución. Al cambiar de video, el anterior sigue en pantalla hasta que el nuevo tiene su primer cuadro. El complemento le dice al daemon si el video de verdad se ve, y `outputs` lo explica cuando no: `video_no_decoder` (Plasma no tiene decodificador para ese formato, por ejemplo HEVC en algunas distribuciones; la pantalla muestra el color grafito), `video_no_frames` (el video se abrió pero en 10 segundos no llegó ni un cuadro a la pantalla; color grafito) y `video_too_slow` (llega menos del 80 % de los cuadros y menos de 24 por segundo, así que se ve a saltos; pasa mucho en máquinas virtuales). Ninguno es un error de la app: un video de menor resolución (por ejemplo 720p) o WebM suele funcionar.

En Windows el daemon pone una ventana por monitor detrás de los íconos del escritorio; los clics llegan a los íconos y al escritorio como siempre. Windows tiene dos formas del escritorio y el daemon detecta cuál hay: `status` muestra `windows_raised` (Windows 11 24H2 en adelante) o `windows_classic` (Windows 10 y Windows 11 hasta 23H2). Los videos los dibuja libmpv (`libmpv-2.dll`, junto al daemon) con Direct3D 11; sin ella, un video muestra el color de respaldo y `outputs` dice por qué. Si el Explorador se reinicia, el fondo vuelve solo, también los videos. Si el fondo no se ve, la variable `MIRAISHIN_WORKERW` cambia lo que el daemon le pide al Explorador que prepare: `auto` (por defecto), `none`, o dos números como `0,0` o `0xD,1`; es una ayuda de diagnóstico mientras se prueba la versión de Windows.

Un video en pausa deja de decodificarse: en Sway y Hyprland se pausa mpv (si dos monitores comparten el mismo video y solo uno está en pausa, mpv sigue para el otro y el que está en pausa deja de dibujar); en KDE Plasma el complemento pausa su reproductor. `outputs` dice por qué está en pausa cada monitor. Windows todavía no puede pausar: `pause` lo avisa y el video sigue (no es un error).

Con `--json` muestra la respuesta del daemon tal cual. Códigos de salida: `0` bien, `1` error, `2` el daemon no está corriendo (no es una falla). Los textos salen en el idioma de los ajustes o, si es `system`, en el de `LANG`.

## Archivos

| Qué | Linux | Windows |
|---|---|---|
| Ajustes | `~/.config/net.decatron.MiraiShin/settings.toml` | `%APPDATA%\net.decatron.MiraiShin\settings.toml` |
| Biblioteca | `~/.local/share/net.decatron.MiraiShin/library.db` | `%APPDATA%\net.decatron.MiraiShin\library.db` |
| Respaldos antes de migrar (los 5 últimos) | `…/backups/` | `…\backups\` |
| Logs (uno por día, los 7 últimos) | `…/logs/` | `…\logs\` |
| IPC | `$XDG_RUNTIME_DIR/app/net.decatron.MiraiShin/daemon.sock` (solo tu usuario) | named pipe `\\.\pipe\net.decatron.MiraiShin-<usuario>` |

La biblioteca también guarda lo elegido para cada monitor.

La biblioteca es solo un índice: la verdad son las carpetas de fondos. Si el archivo de la biblioteca se daña, se aparta como `library.db.danada-<fecha>` y se reconstruye desde las carpetas.

## Ajustes (`settings.toml`)

```toml
wallpapers_dir = "/home/ana/Fondos"  # opcional; por defecto, la carpeta de arriba
language = "system"                   # system, es, en
log_level = "info"                    # error, warn, info, debug, trace
video_saver = "auto"                  # auto, on, off (ver «Copias compatibles»)
pause_on_fullscreen = true            # pausa el video con una app en pantalla completa en ese monitor
pause_on_maximized = false            # … con una ventana maximizada en ese monitor
pause_on_battery = true               # … con el equipo funcionando con batería
pause_on_lock = true                  # … con la sesión bloqueada
pause_on_screen_off = true            # … con la pantalla de ese monitor apagada
```

Las claves `pause_on_*` ya se leen, pero cada una actúa recién cuando llega su señal. **La batería** ya funciona en Linux: el daemon le pregunta a UPower (el Flatpak tiene permiso para hablarle) y pausa los videos apenas se desenchufa el cargador; al volver a enchufarlo, siguen. `outputs` muestra «en pausa: con batería». Una pausa hecha con `pause` manda: enchufar no la quita. Si el sistema no tiene UPower, `status` dice que no se sabe la energía y los videos no se pausan por batería. **La pantalla completa** funciona en KDE Plasma, Sway y Hyprland, y pausa solo el monitor donde está la ventana (con `pause_on_maximized = true`, una ventana maximizada hace lo mismo). En KDE el complemento de fondo ve las ventanas de su pantalla, del escritorio virtual y de la actividad que se ven, y pausa solo; las minimizadas no cuentan. En Sway y Hyprland el daemon lee la lista de ventanas (`wlr-foreign-toplevel`): una ventana en pantalla completa pausa su monitor mientras es la activa, o mientras la activa está en otro monitor (un juego en un monitor sigue en pausa aunque hagas clic en el otro); una ventana en pantalla completa que quedó en un escritorio oculto no pausa nada. Si pasas a un escritorio vacío, el video vuelve a moverse. `outputs` muestra «en pausa: app en pantalla completa». Bloqueo y pantalla apagada llegan en los pasos siguientes de la fase 3; en Windows, la batería y la pantalla completa llegan con ellos.

Todas las claves son opcionales. Un valor no válido usa su valor por defecto y se anota en el log. Las claves que esta versión no conoce se conservan al guardar. `RUST_LOG` pisa `log_level` para depurar.

## Copias compatibles de los videos

Si este equipo no puede reproducir bien un video, el daemon crea una vez una copia en VP9 (WebM, formato libre que traen todas las distribuciones) y reproduce esa. El original nunca se toca. Sube de a un paso: el original, después una copia del tamaño del monitor más grande, y después una copia de ahorro (720p, 30 cuadros por segundo). En KDE Plasma sube cuando Plasma informa `video_no_decoder`, `video_no_frames` o `video_too_slow` (pregunta a los 3, 10 y 22 segundos de poner el video). Mientras se crea la copia sigue el fondo anterior y `outputs` muestra el avance; al terminar reemplaza al viejo sin hueco. En un equipo con un procesador rápido la copia tarda menos de lo que dura el video (un Ryzen 5 3500U convierte 20 segundos de video 4K en unos 14 segundos).

`video_saver`: `auto` (por defecto) empieza con la copia de ahorro cuando el equipo no tiene una tarjeta gráfica real (máquina virtual, o sin tarjeta) y si no, sube solo si hace falta; `on` usa siempre la copia de ahorro; `off` nunca la usa (como mucho, la copia del tamaño del monitor). Cuando se empezaría por la copia de ahorro (`auto` sin tarjeta real, u `on`) pero el video ya es así (hasta 1280×720 y 30 cuadros por segundo, en H.264, VP8, VP9 o AV1), se reproduce el original sin convertirlo, y la copia se crea solo si KDE Plasma informa que no se ve. Un video HEVC se copia siempre, porque algunas distribuciones no traen su decodificador. Las copias van en `video-copies/` dentro de la carpeta de datos de la app; se pueden borrar y se vuelven a crear cuando hacen falta. Si el video de un fondo cambia, su copia vieja se borra al crear la nueva; las copias de fondos quitados se borran la próxima vez que arranca el daemon. Códigos: `video_damaged` (el archivo del video está dañado o incompleto, por ejemplo una descarga cortada: `ffmpeg` no lo puede abrir; hay que volver a bajarlo o copiarlo), `copy_failed` (no se pudo convertir por otra razón, por ejemplo falta espacio en disco; al volver a elegir el fondo se intenta otra vez) y `copy_unavailable` (no hay `ffmpeg`; el Flatpak lo trae).
