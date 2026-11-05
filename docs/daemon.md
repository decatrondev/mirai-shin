# Daemon and `miraishin-ctl`

> In development: the daemon shows still images on KDE Plasma (through its own Plasma wallpaper add-on), Sway and Hyprland (Wayland with `wlr-layer-shell`), and on Windows (being tested). Videos play on KDE Plasma, on Sway and Hyprland, and on Windows (Sway, Hyprland and Windows being tested). X11 and GNOME come in upcoming versions.

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
| `status` | Daemon version, uptime, wallpapers folder, how many wallpapers are valid or have problems, license, whether this desktop can show the wallpaper (and why not), and whether the computer is plugged in or on battery |
| `list` | Every wallpaper in the library; the ones with problems show their [codes](wallpaper-format.md#problems) |
| `rescan` | Reads the wallpapers folder again (a wallpaper on screen that changed is drawn again) |
| `outputs` | Monitors (name, size, scale) and what each one shows |
| `set <folder> [--monitor <name>] [--mode <mode>]` | Sets a wallpaper. Without `--monitor`, the same one on all monitors (and drops what each monitor had). Modes: `fill` (default, crops), `fit` (borders), `stretch`, `center` (real size) |
| `clear [--monitor <name>]` | Removes the wallpaper. With `--monitor`, that monitor stays without one even if there is one for all |
| `pause [--monitor <name>]` | Pauses the video wallpaper (the last frame stays on screen) until `resume` or until the daemon restarts. Images do not change. `outputs` shows `paused: by you` |
| `resume [--monitor <name>]` | Plays again what `pause` stopped. Resuming one monitor while all were paused leaves the others paused |
| `ping` | Checks that the daemon answers |
| `shutdown` | Stops the daemon |

The folder is the one shown by `list`; the monitor name, the one shown by `outputs`. A new monitor gets the wallpaper chosen for all. If an image cannot be read, that monitor shows a solid graphite color and `outputs` shows the problem code.

A video wallpaper loops without sound and uses the same modes. Where videos are not available yet, the monitor shows the graphite color and `outputs` says so (`video_unsupported`, not an error); if the video file is missing, the code is `video_unreadable`.

On Sway and Hyprland videos are played by libmpv (the Flatpak includes it; elsewhere install your distribution's `mpv` package) and drawn with the graphics card through EGL. Monitors showing the same video with the same mode share one player, so the video is decoded once. Codes: `mpv_unavailable` (libmpv missing), `video_no_gpu` (EGL not available), `video_no_decoder` (format or codec not available, try WebM), `video_unplayable` (damaged file or not a video). `MIRAISHIN_MPV_LOG=v` sends mpv's detailed messages to the log, for diagnosis.

Where the wallpaper cannot be shown yet (X11, GNOME), `set` still saves the choice and `status` explains why; it is not an error.

On KDE Plasma the daemon installs its wallpaper add-on in `~/.local/share/plasma/wallpapers/` and switches each screen to it; `clear` gives the screen back the wallpaper type it had before. The first time, Plasma may need a restart (log out and back in) to see the add-on: `status` says so. Videos are played by Plasma inside the add-on (Qt Multimedia), with the decoders your distribution installs. When you change videos, the previous one stays on screen until the new one has its first frame. The add-on tells the daemon whether the video is really showing, and `outputs` explains it when it is not: `video_no_decoder` (Plasma has no decoder for that format, for example HEVC on some distributions; the screen shows the graphite color), `video_no_frames` (the video opened but no frame reached the screen in 10 seconds; graphite color) and `video_too_slow` (fewer than 80 % of the frames and fewer than 24 per second reach the screen, so it stutters; common in virtual machines). None of these is an error in the app: a lower-resolution video (for example 720p) or WebM usually works.

On Windows the daemon puts one window per monitor behind the desktop icons; clicks go to the icons and to the desktop as usual. Windows has two desktop layouts and the daemon detects which one is there: `status` shows `windows_raised` (Windows 11 24H2 and later) or `windows_classic` (Windows 10, Windows 11 up to 23H2). Videos are drawn by libmpv (`libmpv-2.dll`, next to the daemon) with Direct3D 11; without it, a video shows the fallback color and `outputs` says why. If File Explorer restarts, the wallpaper comes back by itself, videos included. If the wallpaper does not show, the variable `MIRAISHIN_WORKERW` changes what the daemon asks Explorer to prepare: `auto` (default), `none`, or two numbers such as `0,0` or `0xD,1`; it is a diagnostic aid while the Windows version is being tested.

A paused video stops decoding: on Sway and Hyprland mpv pauses (if two monitors share the same video and only one is paused, mpv keeps playing for the other and the paused one stops drawing); on KDE Plasma the add-on pauses its player. `outputs` shows why each monitor is paused. Windows cannot pause yet: `pause` says so and the video keeps playing (not an error).

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
video_saver = "auto"                      # auto, on, off (see "Compatible copies")
pause_on_fullscreen = true                # pause the video when an app is in full screen on that monitor
pause_on_maximized = true                 # … when a window is maximized on that monitor
pause_on_battery = true                   # … when the computer runs on battery
pause_on_lock = true                      # … when the session is locked
pause_on_screen_off = true                # … when that monitor's screen is off
```

The `pause_on_*` keys are read now but act only as each signal arrives. **Battery** works on Linux: the daemon asks UPower (the Flatpak has permission to talk to it) and pauses the videos as soon as the charger is unplugged, and plays them again when it is plugged back in; `outputs` shows `paused: on battery`. A pause made with `pause` wins: plugging in does not undo it. If the system has no UPower, `status` says the power source is unknown and videos are not paused on battery. **Full screen** works on KDE Plasma, Sway and Hyprland, and pauses only the monitor where the window is (a maximized window does the same, unless `pause_on_maximized = false`; a normal window on top does not pause). On KDE the wallpaper add-on sees the windows of its screen, on the virtual desktop and activity being shown, and pauses by itself; minimized windows do not count. On Sway and Hyprland the daemon reads the window list (`wlr-foreign-toplevel`): a full-screen window pauses its monitor while it is the active window, or while the active window is on another monitor (a game on one monitor stays paused when you click on the other); a full-screen window left on a hidden workspace does not pause anything. If you switch to an empty workspace, the video plays again. `outputs` shows `paused: app in full screen`. Lock and screen off come in the next steps of phase 3; on Windows, battery and full screen come with them.

Every key is optional. An invalid value uses its default and is noted in the log. Keys this version does not know are kept when the settings are saved. `RUST_LOG` overrides `log_level` for debugging.

## Compatible copies of videos

If this computer cannot play a video well, the daemon makes a copy of it once, in VP9 (WebM, a free format every distribution includes), and plays that copy. The original is never changed. It goes up one step at a time: the original, then a copy at the size of the largest monitor, then a power-saving copy (720p, 30 frames per second). On KDE Plasma it steps up when Plasma reports `video_no_decoder`, `video_no_frames` or `video_too_slow` (it asks 3, 10 and 22 seconds after showing the video). While the copy is being made, the previous wallpaper stays and `outputs` shows the progress; when it is ready it replaces the old one without a gap. On a computer with a fast processor a copy takes less time than the video lasts (a Ryzen 5 3500U converts 20 seconds of 4K video in about 14 seconds).

`video_saver`: `auto` (default) starts with the power-saving copy when the computer has no real graphics card (a virtual machine, or no graphics card at all) and otherwise steps up only if needed; `on` always uses the power-saving copy; `off` never uses it (at most the copy at monitor size). When playback would start with the power-saving copy (`auto` with no real graphics card, or `on`) but the video already is like that (up to 1280×720 and 30 frames per second, in H.264, VP8, VP9 or AV1), the original plays without converting it, and the copy is made only if KDE Plasma reports that it does not play. HEVC video is always copied, because some distributions do not ship its decoder. Copies live in `video-copies/` inside the app's data folder; they can be deleted, and they are made again when needed. When a wallpaper's video changes, its old copy is deleted as soon as the new one is made; copies of removed wallpapers are deleted the next time the daemon starts. Codes: `video_damaged` (the video file is damaged or incomplete, for example a download that was cut short: `ffmpeg` cannot open it; download or copy it again), `copy_failed` (it could not be converted for another reason, for example the disk is full; choosing the wallpaper again tries once more) and `copy_unavailable` (there is no `ffmpeg`; the Flatpak includes it).
