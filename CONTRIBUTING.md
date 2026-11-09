# Contributing

Thanks for your interest. Mirai Shin is at an early stage: the plan is done and the foundation is being built.

## License of contributions

By submitting a contribution (code, docs, translations or assets), you agree that it is licensed under the **GNU AGPL version 3 with the Steamworks SDK exception** described in [`LICENSE-EXCEPTION.md`](LICENSE-EXCEPTION.md), the same terms as the rest of the project.

This matters: without the exception on every contribution, the Steam version could not be built.

## Dependencies

Only dependencies with compatible licenses are accepted: permissive (MIT, Apache-2.0, BSD) or LGPL linked dynamically. No third-party GPL or AGPL code unless it carries a compatible exception.

## Product name

The product name lives only in [`brand.json`](brand.json). Never write it by hand in code or interface text.

## Languages

Every visible text is written in English and Spanish from the start, using translation keys.

## Power use

If a change touches video, pausing or the drawing loop, run `scripts/consumo-pausa.sh` before pushing it. It starts a headless Sway, plays a 60 fps video and fails if, while paused (by hand or with the screen off), the daemon uses more than 5 CPU ticks every 5 s, or if the frame limit saves nothing. CI has no screen or GPU, so this does not run there. It needs `sway`, `ffmpeg` and libmpv.

---

En español: [`CONTRIBUTING.es.md`](CONTRIBUTING.es.md).
