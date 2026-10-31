# Daemon and `miraishin-ctl`

> In development: the daemon shows still images on KDE Plasma (through its own Plasma wallpaper add-on), Sway and Hyprland (Wayland with `wlr-layer-shell`), and on Windows (being tested). Video, X11 and GNOME come in upcoming versions.

## Daemon

`miraishin-daemon` runs in the background, separate from the app, so the wallpaper keeps going when the app is closed. On start it:

1. Reads the settings.
2. Opens the library (and migrates it if needed, making a backup first).
3. Scans the wallpapers folder.
4. Puts back the wallpaper chosen for each monitor.
5. Listens for the app and `miraishin-ctl`.

Only one daemon runs per user. A second one exits with code `3` and leaves the first one untouched.

## `miraishin-ctl`

A console tool to talk to the daemon, shipped with the app for diagnostics and support.

| Command | What it does |
|---|---|
| `status` | Daemon version, uptime, wallpapers folder, how many wallpapers are valid or have problems, license, and whether this desktop can show the wallpaper (and why not) |
| `list` | Every wallpaper in the library; the ones with problems show their [codes](wallpaper-format.md#problems) |
| `rescan` | Reads the wallpapers folder again (a wallpaper on screen that changed is drawn again) |
| `outputs` | Monitors (name, size, scale) and what each one shows |
| `set <folder> [--monitor <name>] [--mode <mode>]` | Sets a wallpaper. Without `--monitor`, the same one on all monitors (and drops what each monitor had). Modes: `fill` (default, crops), `fit` (borders), `stretch`, `center` (real size) |
| `clear [--monitor <name>]` | Removes the wallpaper. With `--monitor`, that monitor stays without one even if there is one for all |
| `ping` | Checks that the daemon answers |
| `shutdown` | Stops the daemon |

The folder is the one shown by `list`; the monitor name, the one shown by `outputs`. A new monitor gets the wallpaper chosen for all. If an image cannot be read, that monitor shows a solid graphite color and `outputs` shows the problem code.

Where the wallpaper cannot be shown yet (X11, GNOME), `set` still saves the choice and `status` explains why; it is not an error.

On KDE Plasma the daemon installs its wallpaper add-on in `~/.local/share/plasma/wallpapers/` and switches each screen to it; `clear` gives the screen back the wallpaper type it had before. The first time, Plasma may need a restart (log out and back in) to see the add-on: `status` says so.

On Windows the daemon puts one window per monitor behind the desktop icons; clicks go to the icons and to the desktop as usual. Windows has two desktop layouts and the daemon detects which one is there: `status` shows `windows_raised` (Windows 11 24H2 and later) or `windows_classic` (Windows 10, Windows 11 up to 23H2). If File Explorer restarts, the wallpaper comes back by itself. If the wallpaper does not show, the variable `MIRAISHIN_WORKERW` changes what the daemon asks Explorer to prepare: `auto` (default), `none`, or two numbers such as `0,0` or `0xD,1`; it is a diagnostic aid while the Windows version is being tested.

Add `--json` to print the daemon's raw answer. Exit codes: `0` ok, `1` error, `2` the daemon is not running (not a failure). The texts follow the language in the settings or, if it is `system`, `LANG`.

## Files

| What | Linux | Windows |
|---|---|---|
| Settings | `~/.config/net.decatron.MiraiShin/settings.toml` | `%APPDATA%\net.decatron.MiraiShin\settings.toml` |
| Library | `~/.local/share/net.decatron.MiraiShin/library.db` | `%APPDATA%\net.decatron.MiraiShin\library.db` |
| Backups before migrating (last 5) | `…/backups/` | `…\backups\` |
| Logs (one per day, last 7) | `…/logs/` | `…\logs\` |
| IPC | `$XDG_RUNTIME_DIR/app/net.decatron.MiraiShin/daemon.sock` (only your user) | named pipe `\\.\pipe\net.decatron.MiraiShin-<user>` |

The library also keeps what was chosen for each monitor.

The library is only an index: the wallpaper folders are the truth. If the library file gets damaged, it is set aside as `library.db.danada-<date>` and rebuilt from the folders.

## Settings (`settings.toml`)

```toml
wallpapers_dir = "/home/ana/Wallpapers"  # optional; default: the folder above
language = "system"                       # system, es, en
log_level = "info"                        # error, warn, info, debug, trace
```

Every key is optional. An invalid value uses its default and is noted in the log. Keys this version does not know are kept when the settings are saved. `RUST_LOG` overrides `log_level` for debugging.
