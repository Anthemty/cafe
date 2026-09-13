# cafe

[English](README.md) | 简体中文 | [日本語](README.ja.md) | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.74%2B-orange.svg)](https://www.rust-lang.org)
[![macOS](https://img.shields.io/badge/platform-macOS%2011%2B-blue.svg)](https://www.apple.com/macos)
[![Made with objc2](https://img.shields.io/badge/built%20with-objc2-9cf)](https://docs.rs/objc2)

> 让你的 Mac 不再犯困——所以它叫 **cafe**。☕

一款为 agent 编程场景打造的小巧 macOS 菜单栏防休眠工具。

受不了 Mac 在 agent 长时间跑任务时打瞌睡?**cafe** 会在你的菜单栏放一杯咖啡。
点开选个模式,Mac 就一直保持清醒,直到你手动关掉。图标颜色一眼可辨状态——
再也不用猜 `caffeinate` 是不是还在跑。

纯 Rust 构建,基于系统自带的 `caffeinate`,**没有 WebView**,除 macOS 外**零依赖**
——只有一个常驻菜单栏的 ~400 KB 二进制。

---

## 功能

- **三种模式**(菜单内互斥):
  | 模式 | `caffeinate` 参数 | 图标颜色 | 效果 |
  |------|--------------------|----------|------|
  | **关闭** | — | 灰色 | 不防休眠 |
  | **仅防休眠** | `-i` | 暖黄 | 防止系统空闲休眠;屏幕可能变暗 |
  | **防休眠 + 常亮** | `-di` | 深橙 | 防止休眠**且**屏幕常亮 |
- **定时会话** — 保持唤醒 30 分钟 / 1 小时 / 2 小时,或选**自定义…**(1–1440 分钟)。
  定时作用于**当前选定的模式**(未激活时用上次手选的模式)。运行期间,剩余时间
  在咖啡杯旁实时跳动(`12:34` 或 `1:23:45`),菜单头部同步显示;到点自动解除。
- **Agent 在线灯** — 菜单实时显示哪些编码 agent 正在运行,各自专属彩点:
  🟠 Claude · 🟢 Codex · 🔵 WorkBuddy · 🟣 ZCode · 🟡 OpenCode。在线点亮,
  离线整行隐藏。通过扫描进程表检测(basename 匹配,不区分大小写)——每次
  打开菜单即时刷新,后台每 60 秒刷新一次。
- **全局快捷键** — `Ctrl+Alt+C` 在三个模式间循环切换,任何应用下都有效。
- **自动:监测 Agent***(可选)* — 五个被监测的 agent(Claude、Codex、WorkBuddy、
  ZCode、OpenCode)任一运行时自动进入防休眠+常亮,全部退出后自动解除。
- **登录时启动** — 菜单一键开关(基于 LaunchAgent)。
- **中英双语** — "语言:中文 / Language: English" 一键切换中英文界面
  (菜单、tooltip、倒计时),选择持久化。应用界面目前支持中/英两种语言。
- **状态一目了然** — 咖啡杯图标(SF Symbol `cup.and.saucer.fill`)通过层级符号
  配置着色,在各 macOS 版本(含 macOS 26 / Tahoe)上都稳定可见。
- **进程绝不泄漏。** 每次切换模式和退出(含 panic 的 `Drop` 兜底)都会跟踪并
  杀掉 `caffeinate` 子进程;子进程还带 `-w <cafe pid>`,即使 cafe 被 `kill -9`
  也不会留下孤儿防休眠进程。
- **诚实的界面。** 打开菜单即复查子进程;`caffeinate` 被外部杀掉时,图标立刻
  回到关闭状态,绝不说谎。
- **纯菜单栏应用。** 以 accessory 方式运行——无 Dock 图标,无主窗口。
- **通用二进制** — 一个 `.app` 同时支持 Apple Silicon 和 Intel Mac。

## 环境要求

- macOS 11.0(Big Sur)或更高
- Rust 1.74+(仅从源码构建时需要)

## 安装

### 方式 A — 构建 `.app` 包(推荐)

```sh
git clone https://github.com/Anthemty/cafe.git
cd cafe
./make-app.sh          # 产出 dist/cafe.app
open dist/cafe.app
```

首次运行时 macOS 可能提示"无法验证开发者"(应用未签名)。绕过方式:右键点击
应用 → **打开** → 在弹窗中再点**打开**。此批准只需一次。

### 方式 B — 直接运行裸二进制

```sh
cargo build --release
./target/release/cafe
```

菜单栏出现咖啡杯。点开选个模式,Mac 就一直保持清醒,直到切回**关闭**或退出。

> 💡 裸二进制没有应用图标;想要完整体验(应用图标、Force Quit 中的正式名称、
> 规范的 LSUIElement 行为)请用 `make-app.sh`。

## 工作原理

cafe 启动并监管单个 `caffeinate` 子进程,切换模式时以不同参数重启它:

```
模式切换 ──► Supervisor::enter(mode) ──► 杀掉旧子进程 ──► 启动新子进程
                                                  │
                        (caffeinate 参数来自 Mode::caffeinate_args)
```

supervisor 是纯 Rust,不依赖 AppKit,并使用 `sleep` 代替 `caffeinate` 做了完整的
单元测试。GUI 层很薄:`NSStatusItem` + `NSMenu`,菜单动作回调进入 supervisor。

## 项目结构

```
src/
  main.rs         NSApp 搭建、状态栏图标 + 菜单、动作回调(define_class!)
  supervisor.rs   caffeinate 子进程生命周期(spawn / kill / reap)
  state.rs        Mode 枚举、配置持久化、agent 进程检测
  icon.rs         SF Symbol 咖啡杯 + 各模式颜色 + agent 在线彩点
make-app.sh       构建 .app 包并生成应用图标
resources/        生成的图标资源(AppIcon.icns)
```

## 开发

```sh
cargo build         # debug 构建
cargo test          # 运行单元测试
cargo clippy        # lint
./make-app.sh       # 完整 release .app
```

## 为什么不直接 `caffeinate -i &`?

可以——但你会忘了它在跑,晚上合盖走人,回来收获一台滚烫、电池见底的笔记本。
cafe 让状态**可见**(图标颜色),关掉也只需一下。

## 路线图

计划中的功能(尚未实现):
- [ ] 可配置快捷键与 agent 监测名单
- [ ] Homebrew tap
- [ ] 签名/公证构建

## 贡献

欢迎贡献!请先开 issue 讨论你想改的内容。提交前请跑 `cargo fmt`、
`cargo clippy` 和 `cargo test`。

## 许可证

本项目基于 [MIT License](LICENSE) 开源。

Copyright © 2026 [Anthemty](https://github.com/Anthemty).
