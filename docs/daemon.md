# Daemon and `miraishin-ctl`

> In development: the daemon does not draw wallpapers yet (that starts in phase 1). Today it keeps the library, writes logs and answers over IPC.

## Daemon

`miraishin-daemon` runs in the background, separate from the app, so the wallpaper keeps going when the app is closed. On start it:

1. Reads the settings.
2. Opens the library (and migrates it if needed, making a backup first).
3. Scans the wallpapers folder.
4. Listens for the app and `miraishin-ctl`.

Only one daemon runs per user. A second one exits with code `3` and leaves the first one untouched.

## `miraishin-ctl`

A console tool to talk to the daemon, shipped with the app for diagnostics and support.

| Command | What it does |
|---|---|
| `status` | Daemon version, uptime, wallpapers folder, how many wallpapers are valid or have problems, license |
| `list` | Every wallpaper in the library; the ones with problems show their [codes](wallpaper-format.md#problems) |
| `rescan` | Reads the wallpapers folder again |
| `ping` | Checks that the daemon answers |
| `shutdown` | Stops the daemon |

Add `--json` to print the daemon's raw answer. Exit codes: `0` ok, `1` error, `2` the daemon is not running (not a failure). The texts follow the language in the settings or, if it is `system`, `LANG`.

## Files

| What | Linux | Windows |
|---|---|---|
| Settings | `~/.config/net.decatron.MiraiShin/settings.toml` | `%APPDATA%\net.decatron.MiraiShin\settings.toml` |
| Library | `~/.local/share/net.decatron.MiraiShin/library.db` | `%APPDATA%\net.decatron.MiraiShin\library.db` |
| Backups before migrating (last 5) | `…/backups/` | `…\backups\` |
| Logs (one per day, last 7) | `…/logs/` | `…\logs\` |
| IPC | `$XDG_RUNTIME_DIR/app/net.decatron.MiraiShin/daemon.sock` (only your user) | named pipe `\\.\pipe\net.decatron.MiraiShin-<user>` |

The library is only an index: the wallpaper folders are the truth. If the library file gets damaged, it is set aside as `library.db.danada-<date>` and rebuilt from the folders.

## Settings (`settings.toml`)

```toml
wallpapers_dir = "/home/ana/Wallpapers"  # optional; default: the folder above
language = "system"                       # system, es, en
log_level = "info"                        # error, warn, info, debug, trace
```

Every key is optional. An invalid value uses its default and is noted in the log. Keys this version does not know are kept when the settings are saved. `RUST_LOG` overrides `log_level` for debugging.
