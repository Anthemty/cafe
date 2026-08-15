# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2026-08-16

### Added
- **中英双语 UI** — 菜单里新增“语言：中文 / Language: English”切换项，点一下即时
  切换全部菜单标题、tooltip 与倒计时文案；选择持久化，重启保留。老版 config
  缺 `lang` 字段会安全回退到 English（已加 `#[serde(default)]` 并加测试覆盖）。
- 翻译范围：模式名、定时预设、登录项、自动监测项、语言项、退出项、状态 tooltip、
  倒计时后缀。

### Fixed
- 新增 config 字段不再让旧版 config 反序列化失败后被整体重置——v0.1.0 的
  `last_mode` 与 v0.2.0 的 `auto_watch` 现在都带 `#[serde(default)]`，跨版本升级
  保留用户偏好。

## [0.2.0] - 2026-08-15

### Added
- **Timed sessions** — "Keep awake for 30 min / 1 h / 2 h" menu items; the icon
  tooltip shows the remaining time and the app auto-disarms at the deadline
  (`caffeinate -t`).
- **Global hotkey** — `Ctrl+Alt+C` cycles Off → Idle Only → Idle + Display from
  anywhere.
- **Auto: watch agents** — opt-in mode that arms sleep prevention (Idle +
  Display) while a coding agent CLI is running (claude, codex, aider, goose,
  gemini, qwen, cursor-agent, opencode, copilot) and disarms when they exit.
  Polled every 5 s.
- **Launch at Login** — menu toggle that installs/removes a LaunchAgent plist.
- Menu now syncs liveness every time it opens: an externally killed caffeinate
  is detected and the icon reverts to Off instead of lying.
- CI (fmt + clippy + test on macOS & Linux) and tag-triggered Release workflow
  that builds the universal `.app` automatically.

### Changed
- Binary is now **universal** (aarch64 + x86_64) — runs on Apple Silicon and
  Intel Macs.
- Spawned `caffeinate` passes `-w <cafe pid>`: even if cafe is SIGKILLed (no
  `Drop` runs), caffeinate terminates itself — no orphaned sleep prevention.
- Icons are rendered once and cached; switching modes no longer re-renders the
  SF Symbol.
- `last_mode` is again informational only; the config now also persists the
  auto-watch preference.

### Fixed
- Menu item handles are captured at build time instead of being recovered by
  index arithmetic, which silently broke if the menu layout changed.

## [0.1.0] - 2026-07-11

### Added
- Menu bar status item with a coffee-cup SF Symbol icon.
- Three keep-awake modes, switchable from the menu:
  - **Off** — no sleep prevention.
  - **Idle Only** (`caffeinate -i`) — prevents idle system sleep; the display may dim. Icon turns warm yellow.
  - **Idle + Display** (`caffeinate -di`) — prevents idle sleep and keeps the display awake. Icon turns deep orange.
- Icon color is driven by an `NSImageSymbolConfiguration` (hierarchical color) so the state is visible at a glance.
- Persists the last-used mode to `~/Library/Application Support/cafe/config.json`.
- Always launches in **Off** for safety (never auto-arms).
- `Supervisor` guarantees no leaked `caffeinate` process: every mode switch and app exit (including `Drop`) kills + reaps the child.
- `.app` bundle packaging via `make-app.sh`, including a generated coffee-cup app icon (`resources/AppIcon.icns`).

[Unreleased]: https://github.com/Anthemty/cafe/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/Anthemty/cafe/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/Anthemty/cafe/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Anthemty/cafe/releases/tag/v0.1.0
