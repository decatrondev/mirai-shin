# Mirai Shin

> 未来・芯 — *The core of tomorrow.* The product name lives in [`brand.json`](brand.json) and nowhere else.

Animated wallpapers for Windows and Linux, built to use as little as possible. Rust + Tauri. Open source under AGPL-3.0.

**Status:** in development. The plan is done and the foundation is being built. There is nothing to download yet.

Website: <https://miraishin.decatron.net>

## What it will do

- **Pauses on its own** when you play full screen, run on battery or lock the screen.
- **Real Linux support:** native Wayland on KDE, Hyprland and Sway. GNOME and X11 later.
- **One wallpaper per monitor**, or the same one on all of them.

Platforms: Windows and Linux. Mac in the future.

## Layout

```
brand.json          product identity (name, kanji, motto, links)
apps/landing/       website (React, Vite, TypeScript)        in progress
packages/ui/        design tokens and components, shared with the app
packages/i18n/      English and Spanish texts
docs/  docs/es/     documentation (English and Spanish)
```

The desktop app (Rust crates and the Tauri app) is added when its first phase starts.

## License

[GNU AGPL-3.0](LICENSE) with an [exception for the Steamworks SDK](LICENSE-EXCEPTION.md). Contributions are accepted under the same terms: see [`CONTRIBUTING.md`](CONTRIBUTING.md).

---

En español: [`README.es.md`](README.es.md).
