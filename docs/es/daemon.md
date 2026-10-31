# Daemon y `miraishin-ctl`

> En desarrollo: el daemon todavía no dibuja fondos (eso empieza en la fase 1). Hoy mantiene la biblioteca, escribe logs y responde por IPC.

## Daemon

`miraishin-daemon` corre en segundo plano, aparte de la app, para que el fondo siga aunque se cierre la app. Al arrancar:

1. Lee los ajustes.
2. Abre la biblioteca (y la migra si hace falta, con un respaldo antes).
3. Recorre la carpeta de fondos.
4. Queda atento a la app y a `miraishin-ctl`.

Corre un solo daemon por usuario. Un segundo sale con el código `3` y no toca al primero.

## `miraishin-ctl`

Herramienta de consola para hablar con el daemon. Se entrega con la app, para diagnóstico y soporte.

| Comando | Qué hace |
|---|---|
| `status` | Versión del daemon, tiempo encendido, carpeta de fondos, cuántos fondos son válidos o tienen problemas, licencia |
| `list` | Todos los fondos de la biblioteca; los que tienen problemas muestran sus [códigos](wallpaper-format.md#problemas) |
| `rescan` | Vuelve a leer la carpeta de fondos |
| `ping` | Comprueba que el daemon responde |
| `shutdown` | Apaga el daemon |

Con `--json` muestra la respuesta del daemon tal cual. Códigos de salida: `0` bien, `1` error, `2` el daemon no está corriendo (no es una falla). Los textos salen en el idioma de los ajustes o, si es `system`, en el de `LANG`.

## Archivos

| Qué | Linux | Windows |
|---|---|---|
| Ajustes | `~/.config/net.decatron.MiraiShin/settings.toml` | `%APPDATA%\net.decatron.MiraiShin\settings.toml` |
| Biblioteca | `~/.local/share/net.decatron.MiraiShin/library.db` | `%APPDATA%\net.decatron.MiraiShin\library.db` |
| Respaldos antes de migrar (los 5 últimos) | `…/backups/` | `…\backups\` |
| Logs (uno por día, los 7 últimos) | `…/logs/` | `…\logs\` |
| IPC | `$XDG_RUNTIME_DIR/app/net.decatron.MiraiShin/daemon.sock` (solo tu usuario) | named pipe `\\.\pipe\net.decatron.MiraiShin-<usuario>` |

La biblioteca es solo un índice: la verdad son las carpetas de fondos. Si el archivo de la biblioteca se daña, se aparta como `library.db.danada-<fecha>` y se reconstruye desde las carpetas.

## Ajustes (`settings.toml`)

```toml
wallpapers_dir = "/home/ana/Fondos"  # opcional; por defecto, la carpeta de arriba
language = "system"                   # system, es, en
log_level = "info"                    # error, warn, info, debug, trace
```

Todas las claves son opcionales. Un valor no válido usa su valor por defecto y se anota en el log. Las claves que esta versión no conoce se conservan al guardar. `RUST_LOG` pisa `log_level` para depurar.
