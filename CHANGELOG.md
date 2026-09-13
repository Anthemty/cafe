# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-09-13

### Added
- **Agent 在线指示灯** — 菜单内实时显示当前在线的编码 agent:Claude(珊瑚橙)、
  Codex(绿)、WorkBuddy(蓝)、ZCode(紫)、OpenCode(琥珀)。在线点亮彩点 + 名称,
  不在线整行隐藏;一个都不在线时菜单与旧版完全一致。每次打开菜单即时扫描,
  后台每 60 秒刷新一次。
- **自定义定时** — 定时区新增"自定义…/Custom…",弹窗输入分钟数(1–1440),输入框
  加宽到可用尺寸;定时作用于**当前选定的模式**(未激活时用上次手选的模式)。
- **菜单栏倒计时** — 定时会话期间,咖啡杯图标旁直接显示 `m:ss` / `h:mm:ss` 每秒
  跳动(计时器注册在 `NSRunLoopCommonModes`,菜单打开着也实时刷新),菜单头部
  与 tooltip 同步显示。
- **Panic 日志** — 菜单栏 app 无可见 stderr,panic 钩子把崩溃信息写入
  `~/Library/Application Support/cafe/panic.log`,便于排查。

### Changed
- 自动监测的 agent 名单收敛为上述 5 个(与在线灯一致),移除 gemini/qwen/aider/
  copilot 等误报率较高的通配匹配。
- 后台 agent 扫描频率从 5 秒改为 **60 秒**(打开菜单仍是即时扫描,只影响 auto
  模式的反应延迟)。
- `last_mode` 只记录"武装过"的模式,不会因回到 Off 而被清空,定时自 Off 启动时
  有可靠的默认值。

### Fixed
- **自定义定时闪退** — 旧实现持有 `RefCell::borrow_mut` 期间调用 `runModal()`,
  模态循环中 1 秒 tick 触发 `borrow()` → `BorrowMutError` panic。弹窗逻辑已
  抽为不持有任何状态借用的独立函数。
- **agent 检测永远为空** — 从 pgrep 时代照搬的 `.stdout(Stdio::null())` 把
  `ps` 的输出重定向进了 /dev/null,app 内检测恒为"全离线"(独立测试脚本无此
  设置,故此前未被发现)。已移除并用进程内诊断日志验证。

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

[Unreleased]: https://github.com/Anthemty/cafe/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/Anthemty/cafe/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/Anthemty/cafe/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/Anthemty/cafe/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Anthemty/cafe/releases/tag/v0.1.0
