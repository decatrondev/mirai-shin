# Mirai Shin

> 未来・芯 — *The core of tomorrow.* The product name lives in [`brand.json`](brand.json) and nowhere else.

Animated wallpapers for Linux and Windows, built to use as little as possible. Rust + Tauri. Open source under AGPL-3.0.

**Status:** in development. The plan is done and the foundation is being built. There is nothing to download yet.

Website: <https://miraishin.decatron.net>

## What it will do

- **Pauses on its own** when you play full screen, run on battery or lock the screen.
- **Real Linux support:** native Wayland on KDE, Hyprland and Sway. GNOME and X11 later.
- **One wallpaper per monitor**, or the same one on all of them.

Platforms: Linux first, then Windows. Mac in the future.

## Layout

```
brand.json          product identity (name, kanji, motto, links)
apps/landing/       website (React, Vite, TypeScript)        in progress
apps/daemon/        background process that draws the wallpaper   phase 1
apps/ctl/           console tool to talk to the daemon       phase 1
crates/core/        pure rules: wallpaper format, surface contract, framing
crates/surface-wayland/  wallpaper on Sway/Hyprland (wlr-layer-shell)
crates/surface-plasma/   wallpaper on KDE Plasma (own wallpaper add-on)
crates/surface-windows/  wallpaper on Windows (window behind the desktop icons)
crates/render-image/     still image: decode and fit to each monitor
crates/render-video/     video with libmpv (loaded at start; LGPL build)
crates/store/       local library (SQLite), settings (TOML), paths
crates/ipc/         app <-> daemon protocol (Unix socket / named pipe)
crates/logging/     log files with daily rotation
crates/license/     license interface (real checks come later)
crates/brand/       reads brand.json
services/web/       API for the website's /admin panel
packages/ui/        design tokens and components, shared with the app
packages/i18n/      English and Spanish texts
docs/  docs/es/     documentation (English and Spanish)
```

The Tauri app is added in phase 4. Build and test the Rust part with `cargo build --workspace` and `cargo test --workspace`.

## License

[GNU AGPL-3.0](LICENSE) with an [exception for the Steamworks SDK](LICENSE-EXCEPTION.md). Contributions are accepted under the same terms: see [`CONTRIBUTING.md`](CONTRIBUTING.md).

### Third-party components

The Linux package (Flatpak) includes these libraries, built without changes from their official sources. The exact versions and build options are in [`packaging/flatpak/net.decatron.MiraiShin.yml`](packaging/flatpak/net.decatron.MiraiShin.yml).

| Library | Version | License | What it does here |
|---------|---------|---------|-------------------|
| [mpv](https://mpv.io) (libmpv) | 0.41.0 | LGPL-2.1-or-later (built with `-Dgpl=false`) | Plays video wallpapers on Sway and Hyprland |
| [FFmpeg](https://ffmpeg.org) | 7.1.2 | LGPL-2.1-or-later (decoders, plus the VP9 encoder and WebM output; without GPL or non-free parts) | Decodes the video, and makes a compatible VP9 copy when this computer cannot play the original |
| [libplacebo](https://code.videolan.org/videolan/libplacebo) | 7.387.0 | LGPL-2.1-or-later | Draws the video frames with the graphics card |
| [libass](https://github.com/libass/libass) | 0.17.4 | ISC | Required by mpv |
| [libdisplay-info](https://gitlab.freedesktop.org/emersion/libdisplay-info) | 0.4.0 | MIT | Required by mpv for hardware decoding |

The LGPL libraries are separate shared libraries in `/app/lib`, so they can be replaced by another compatible build. Their license texts travel inside the package, in `/app/share/licenses/net.decatron.MiraiShin/`. The app talks to libmpv through its public headers, which are under the ISC license ([`crates/render-video`](crates/render-video)). Everything else comes from the Freedesktop 25.08 runtime, under its own licenses (among them libvpx, BSD, which FFmpeg uses to encode VP9). On KDE Plasma the video is played by Plasma itself (Qt Multimedia), which is not included in the package.

The Windows package carries the same mpv, FFmpeg, libplacebo and libass versions, built in CI with [`packaging/windows/build-libmpv.sh`](packaging/windows/build-libmpv.sh) (MSYS2). There mpv draws with Direct3D 11 and decodes on the graphics card with D3D11VA, which also needs [shaderc](https://github.com/google/shaderc) and [SPIRV-Cross](https://github.com/KhronosGroup/SPIRV-Cross) (Apache-2.0). They sit next to the daemon as separate DLLs (`libmpv-2.dll`, `avcodec-61.dll`…; libass goes inside `libmpv-2.dll`), so they can be replaced too, along with the DLLs they need from MSYS2 (FreeType, HarfBuzz, FriBidi, libvpx, dav1d, zlib, the GCC runtime and others, all under permissive or LGPL licenses). The `licenses\` folder holds every license text and `VERSIONES.txt` the exact version of each piece; the build stops if a DLL comes from a package outside its allowed list.

---

En español: [`README.es.md`](README.es.md).
