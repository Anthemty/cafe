# cafe

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.74%2B-orange.svg)](https://www.rust-lang.org)
[![macOS](https://img.shields.io/badge/platform-macOS%2011%2B-blue.svg)](https://www.apple.com/macos)
[![Made with objc2](https://img.shields.io/badge/built%20with-objc2-9cf)](https://docs.rs/objc2)

> A tiny macOS menu bar keep-awake tool for agent coding sessions.

Tired of your Mac dozing off while a long-running coding agent does its thing?
**cafe** parks a coffee cup in your menu bar. Click it, pick a mode, and your
Mac stays awake until you switch it off. The icon's color tells you the state
at a glance — no more guessing whether `caffeinate` is still running.

Built in pure Rust on top of the system `caffeinate`, with **no web view** and
**no dependencies** beyond macOS itself — just a ~400 KB binary that lives in
your menu bar.

---

## Features

- **Three modes** (mutually exclusive, in the menu):
  | Mode | `caffeinate` flags | Icon color | Effect |
  |------|--------------------|------------|--------|
  | **Off** | — | gray | no sleep prevention |
  | **Idle Only** | `-i` | warm yellow | prevents idle system sleep; display may dim |
  | **Idle + Display** | `-di` | deep orange | prevents idle sleep **and** keeps the display on |
- **Timed sessions** — keep awake for 30 min / 1 h / 2 h; the tooltip counts
  down and the app disarms itself at the deadline.
- **Global hotkey** — `Ctrl+Alt+C` cycles through the three modes from any app.
- **Auto: watch agents** *(opt-in)* — arms Idle + Display while a coding agent
  CLI is running (`claude`, `codex`, `aider`, `goose`, `gemini`, `qwen`,
  `cursor-agent`, `opencode`, `copilot`), disarms when they exit.
- **Launch at Login** — plain toggle in the menu (LaunchAgent-based).
- **中英双语** — “语言：中文 / Language: English” 菜单项一键切换中英文 UI
  (菜单、tooltip、倒计时)，选择持久化。Bilingual menu/tooltip/countdown with a
  one-click toggle; the choice persists across launches.
- **Color-coded icon** — the coffee cup (SF Symbol `cup.and.saucer.fill`) is
  tinted via a hierarchical symbol configuration, so the color is always visible
  and reliable across macOS versions (including macOS 26 / Tahoe).
- **No leaked processes.** The `caffeinate` child is tracked and killed on every
  mode change and on quit/panic (`Drop` guarantee). Spawned children also get
  `-w <cafe pid>`, so even a `kill -9` of cafe cannot orphan sleep prevention.
- **Honest UI.** Opening the menu re-checks the child; if someone killed
  `caffeinate` externally, the icon reverts to Off instead of lying.
- **Menu bar only.** Runs as an accessory — no Dock icon, no main window.
- **Universal binary** — one `.app` for Apple Silicon and Intel Macs.

## Requirements

- macOS 11.0 (Big Sur) or newer
- Rust 1.74+ (only needed to build from source)

## Install

### Option A — build the `.app` bundle (recommended)

```sh
git clone https://github.com/Anthemty/cafe.git
cd cafe
./make-app.sh          # produces dist/cafe.app
open dist/cafe.app
```

The first time you run it, macOS may show "cafe cannot be opened because the
developer cannot be verified" (it's unsigned). To bypass: right-click the app
→ **Open** → **Open** in the dialog. This one-time approval sticks.

### Option B — run the raw binary

```sh
cargo build --release
./target/release/cafe
```

A coffee cup appears in the menu bar. Click it, pick a mode, and your Mac stays
awake until you switch back to **Off** or **Quit**.

> 💡 The raw binary has no app icon; `make-app.sh` is the way to get the full
> experience (app icon, proper name in Force Quit, clean LSUIElement behavior).

## How it works

cafe spawns and supervises a single `caffeinate` child process, restarting it
with different flags when you switch modes:

```
mode change ──► Supervisor::enter(mode) ──► kill old child ──► spawn new child
                                                  │
                          (caffeinate flags from Mode::caffeinate_args)
```

The supervisor is pure Rust with no AppKit dependency and is fully unit-tested
using `sleep` as a stand-in for `caffeinate`. The GUI layer is thin: an
`NSStatusItem` + `NSMenu` whose actions call back into the supervisor.

## Project layout

```
src/
  main.rs         NSApp setup, status item + menu, action callbacks (define_class!)
  supervisor.rs   caffeinate child-process lifecycle (spawn / kill / reap)
  state.rs        Mode enum + JSON persistence
  icon.rs         SF Symbol + per-mode hierarchical color
make-app.sh       Build the .app bundle and generate the app icon
resources/        Generated icon assets (AppIcon.icns)
```

## Develop

```sh
cargo build         # debug build
cargo test          # run supervisor unit tests
cargo clippy        # lint
./make-app.sh       # full release .app
```

## Why not just `caffeinate -i &`?

You can — but you'll forget it's running, leave for the night, and come back to
a hot laptop with a dead battery. cafe makes the state **visible** (the icon
color) and trivial to toggle off.

## Roadmap

Possible future additions (not yet implemented):
- [ ] Configurable hotkey & agent watch-list
- [ ] Homebrew tap
- [ ] Signed/notarized builds

## Contributing

Contributions are welcome! Please open an issue first to discuss what you'd like
to change. Run `cargo fmt`, `cargo clippy`, and `cargo test` before submitting.

## License

This project is licensed under the [MIT License](LICENSE).

Copyright © 2026 [Anthemty](https://github.com/Anthemty).
