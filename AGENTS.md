# AGENTS.md

This file provides guidance to coding agents working in this repository.

CardBili is an unofficial Bilibili client for the M5Stack Cardputer Zero (aarch64 Linux, 320×170 RGB565 framebuffer, physical keyboard), written in Rust + Slint. The same code also builds as a desktop preview app. The Rust crate lives in `cardbili/`; repo-level packaging/CI scripts live in `scripts/`.

## Commands

All cargo commands run from `cardbili/` and CI always passes `--locked`.

```sh
cargo run --locked --features desktop          # desktop preview (minifb window; needs system FFmpeg, Linux also needs pacat)
cargo check --locked --features desktop
cargo test --locked                             # device-config tests
cargo test --locked --features desktop          # CI runs both
cargo test --locked <name_substring>            # single test, e.g. `cargo test --locked wbi`
```

From the repo root:

```sh
bash scripts/check.sh                 # shellcheck, py_compile, actionlint + gitleaks (Docker), check-release.py
bash scripts/build-device.sh          # cross-compile aarch64-unknown-linux-musl (clang/llvm-ar/rust-lld); SKIP_FFMPEG=1 skips the Docker FFmpeg build
python3 scripts/package-release.py    # builds dist/*.deb, source tarball, SHA256SUMS
python3 scripts/check-release.py --deb dist/cardbili_*_arm64.deb
bash scripts/install-device.sh [--skip-build] [--with-ffmpeg] [--host=IP]   # build+package+install on the real device
```

Device builds need `rustup target add aarch64-unknown-linux-musl` and `rustup component add rust-docs`. Device connection settings come from `.secrets/device.env` (copy `.secrets/device.env.example`).

Useful env vars at runtime: `CARDBILI_LANG=en|zh` (override UI language), `CARDBILI_DEBUG=1` (force debug log, mirrored to stderr).

## Architecture

**Single-threaded UI loop + worker threads via `mpsc`.** `main.rs` defines the `Msg` enum, which is the only way background work reports back. `app::run` (`src/app.rs`) owns a hand-rolled ~16 ms loop: `App::tick()`, drain `Msg`s into `App::handle_message` (`src/app/messages.rs`), handle keys, render. Network/decoding work is spawned as plain `std::thread`s in `src/tasks.rs` (and `tasks/playback.rs`) that call `api::*` and send a `Msg`. There is no async runtime.

**Stale-result guarding.** Most `Msg` variants carry a request/generation id (`u64`) as their first field. `App` (`src/app/state.rs`) holds the current `*_request` counters, and handlers drop messages whose id no longer matches. When adding a new async load, follow this pattern: bump the counter, pass it into the task, and compare it in the handler.

**`App` is split across `src/app/*.rs`** as `impl App` blocks per feature (detail, comments, playback, login, favorites, live sections, …). `src/app/view.rs`/`components/` push data into Slint models; `navigation.rs`/`input.rs` handle page routing and keys.

**Platform layer (`src/app/platform.rs`).** Implements a Slint `Platform` around `MinimalSoftwareWindow` with the software renderer to an RGB565 buffer. Device build: mmaps the framebuffer and reads keys from evdev. `desktop` feature: renders into a `minifb` window and maps keys (`desktop_key`) to the same strings evdev produces, so the input code is shared. Desktop-only behavior is gated with `#[cfg(feature = "desktop")]`; Windows audio uses `cpal` (`player/audio_desktop.rs`).

**Playback (`src/player.rs`, `src/player/`).** Video/audio are decoded by an external `ffmpeg` child process (device: a static musl FFmpeg built by `cardbili/ffmpeg/Dockerfile` and shipped in the .deb; desktop: system FFmpeg). The filter graph scales/pads to 320×170 @ 30fps and applies playback speed via `setpts`; raw frames are consumed by `player/frames.rs`, audio is piped to a separate output process on Linux. Pause/resume uses signals to the child.

**Bilibili API (`src/api.rs`, `src/api/`).** Blocking `ureq` HTTP. `api/wbi.rs` implements WBI request signing; `net/session/` handles QR login, cookies and encrypted on-disk session storage (`chacha20poly1305`). `src/cache.rs` provides a disk cache; `tasks::cached_then_refresh` publishes cached data first, then fresh data. Live danmaku uses a websocket (`live_danmaku.rs`, `tungstenite`).

**UI (`cardbili/ui/`).** `app.slint` is the root, compiled by `build.rs`; `state.slint` holds globals (e.g. `I18n`, `Player`, `Detail`, `Meta`) that Rust reads/writes via `ui.global::<crate::X>()`. Pages in `ui/pages/`, reusable pieces in `ui/components/`. Slint is pinned to `~1.8` (and `i-slint-core = 1.8.0`), so newer Slint features like `@tr` are unavailable.

**Fonts are pre-rasterized at build time.** `build.rs` + `build/*.rs` turn the pixel fonts in `fonts/` (Fusion Pixel, Mona emoji) into bitmap caches for UI, danmaku and emoji; `src/fonts.rs` registers them. Editing font files or `build/*.rs` regenerates these caches.

## Conventions

- **i18n:** The Chinese source string *is* the translation key. Rust uses `i18n::tr` / `i18n::trf` (with `{}` placeholders), Slint uses `I18n.tr`. English lives in `cardbili/i18n/en.po`; missing entries fall back to Chinese, so new strings don't require a translation. Translations must keep the same number of `{}` placeholders (enforced by `cargo test`).
- Code comments, log messages and commit messages are in Chinese; commits use conventional prefixes (`feat:`, `fix:`, `chore:`).
- Unit tests live in sibling `tests.rs` files (`mod tests;` at the bottom of the module, e.g. `src/api/wbi/tests.rs`, `src/app/tests/`).
- **Version bump:** update `app-builder.json`, `cardbili/Cargo.toml` and `cardbili/Cargo.lock` together; `check-release.py` fails if they differ. Pushing a `v*` tag triggers the release + store publish workflows.
- The device binary bundles Slint and is distributed as GPL-3.0-only (store manifest license); original code is MIT. Keep `THIRD_PARTY_NOTICES.md` in sync when adding bundled assets/fonts.

## Device work

`.agents/skills/cardputer-zero/SKILL.md` is the reference for SSH access (`scripts/r.sh`), deploying (`scripts/deploy.sh`), framebuffer screenshots (`scripts/fbcap.py`), key layout, APPLaunch (a `systemctl --user` service that must be stopped before writing to the framebuffer directly) and debugging. `.agents/skills/cardputer-zero-application/` and `.agents/skills/cardputer-app-publish/` cover the APPLaunch package contract and store pre-publish checks.
