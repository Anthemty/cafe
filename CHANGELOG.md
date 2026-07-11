# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/Anthemty/cafe/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Anthemty/cafe/releases/tag/v0.1.0
