# cafe

[English](README.md) | [简体中文](README.zh-CN.md) | 繁體中文 | [日本語](README.ja.md) | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Italiano](README.it.md) | [Português (BR)](README.pt-BR.md) | [Русский](README.ru.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.74%2B-orange.svg)](https://www.rust-lang.org)
[![macOS](https://img.shields.io/badge/platform-macOS%2011%2B-blue.svg)](https://www.apple.com/macos)
[![Made with objc2](https://img.shields.io/badge/built%20with-objc2-9cf)](https://docs.rs/objc2)

> 讓你的 Mac 不再打瞌睡——所以它叫 **cafe**。☕

一款為 agent 編程場景打造的小巧 macOS 選單列防休眠工具。

受不了 Mac 在 agent 長時間跑任務時打瞌睡?**cafe** 會在你的選單列放一杯咖啡。
點開選個模式,Mac 就一直保持清醒,直到你手動關掉。圖示顏色一眼可辨狀態——
再也不用猜 `caffeinate` 是不是還在跑。

純 Rust 建置,基於系統內建的 `caffeinate`,**沒有 WebView**,除 macOS 外**零依賴**
——只有一個常駐選單列的 ~400 KB 執行檔。

---

## 功能

- **三種模式**(選單內互斥):
  | 模式 | `caffeinate` 參數 | 圖示顏色 | 效果 |
  |------|--------------------|----------|------|
  | **關閉** | — | 灰色 | 不防休眠 |
  | **僅防休眠** | `-i` | 暖黃 | 防止系統閒置休眠;螢幕可能變暗 |
  | **防休眠 + 常亮** | `-di` | 深橙 | 防止休眠**且**螢幕常亮 |
- **定時工作階段** — 保持清醒 30 分鐘 / 1 小時 / 2 小時,或選**自訂…**(1–1440 分鐘)。
  定時作用於**目前選定的模式**(未啟用時用上次手選的模式)。執行期間,剩餘時間
  在咖啡杯旁即時跳動(`12:34` 或 `1:23:45`),選單標題同步顯示;時間到自動解除。
- **Agent 線上燈** — 選單即時顯示哪些編程 agent 正在執行,各自專屬彩點:
  🟠 Claude · 🟢 Codex · 🔵 WorkBuddy · 🟣 ZCode · 🟡 OpenCode。線上點亮,
  離線整行隱藏。透過掃描程序表偵測(basename 比對,不分大小寫)——每次
  打開選單即時重新整理,背景每 60 秒重新整理一次。
- **全域快速鍵** — `Ctrl+Alt+C` 在三個模式間循環切換,任何應用程式下都有效。
- **自動:監測 Agent***(可選)* — 五個被監測的 agent(Claude、Codex、WorkBuddy、
  ZCode、OpenCode)任一執行時自動進入防休眠+常亮,全部結束後自動解除。
- **登入時啟動** — 選單一鍵開關(基於 LaunchAgent)。
- **中英雙語** — 「語言:中文 / Language: English」一鍵切換中英文介面
  (選單、tooltip、倒數計時),選擇會保存。應用程式介面目前支援中/英文。
- **狀態一目了然** — 咖啡杯圖示(SF Symbol `cup.and.saucer.fill`)透過階層式符號
  設定著色,在各 macOS 版本(含 macOS 26 / Tahoe)上都穩定可見。
- **程序絕不外漏。** 每次切換模式和結束(含 panic 的 `Drop` 兜底)都會追蹤並
  結束 `caffeinate` 子程序;子程序還帶 `-w <cafe pid>`,即使 cafe 被 `kill -9`
  也不會留下孤兒防休眠程序。
- **誠實的介面。** 打開選單即複查子程序;`caffeinate` 被外部刪除時,圖示立刻
  回到關閉狀態,絕不說謊。
- **純選單列應用程式。** 以 accessory 方式執行——無 Dock 圖示,無主視窗。
- **通用二進位** — 一個 `.app` 同時支援 Apple Silicon 和 Intel Mac。

## 系統需求

- macOS 11.0(Big Sur)或以上
- Rust 1.74+(僅從原始碼建置時需要)

## 安裝

### 方式 A — 建置 `.app` 包(建議)

```sh
git clone https://github.com/Anthemty/cafe.git
cd cafe
./make-app.sh          # 產出 dist/cafe.app
open dist/cafe.app
```

首次執行時 macOS 可能提示「無法驗證開發者」(應用程式未簽署)。繞過方式:右鍵
點選應用程式 → **打開** → 在彈窗中再點**打開**。此核准只需一次。

### 方式 B — 直接執行裸執行檔

```sh
cargo build --release
./target/release/cafe
```

選單列出現咖啡杯。點開選個模式,Mac 就一直保持清醒,直到切回**關閉**或結束。

> 💡 裸執行檔沒有應用程式圖示;想要完整體驗(應用程式圖示、Force Quit 中的
> 正式名稱、規範的 LSUIElement 行為)請用 `make-app.sh`。

## 運作原理

cafe 啟動並監管單一 `caffeinate` 子程序,切換模式時以不同參數重新啟動它:

```
模式切換 ──► Supervisor::enter(mode) ──► 結束舊子程序 ──► 啟動新子程序
                                                  │
                        (caffeinate 參數來自 Mode::caffeinate_args)
```

supervisor 是純 Rust,不依賴 AppKit,並使用 `sleep` 代替 `caffeinate` 做了完整的
單元測試。GUI 層很薄:`NSStatusItem` + `NSMenu`,選單動作回呼進入 supervisor。

## 專案結構

```
src/
  main.rs         NSApp 建置、狀態列圖示 + 選單、動作回呼(define_class!)
  supervisor.rs   caffeinate 子程序生命週期(spawn / kill / reap)
  state.rs        Mode 列舉、設定保存、agent 程序偵測
  icon.rs         SF Symbol 咖啡杯 + 各模式顏色 + agent 線上彩點
make-app.sh       建置 .app 包並產生應用程式圖示
resources/        產生的圖示資源(AppIcon.icns)
```

## 開發

```sh
cargo build         # debug 建置
cargo test          # 執行單元測試
cargo clippy        # lint
./make-app.sh       # 完整 release .app
```

## 為什麼不直接 `caffeinate -i &`?

可以——但你會忘了它在跑,晚上闔上螢幕走人,回來收穫一台滾燙、電池見底的筆電。
cafe 讓狀態**可見**(圖示顏色),關掉也只需一下。

## 路線圖

計畫中的功能(尚未實作):
- [ ] 可設定快速鍵與 agent 監測名單
- [ ] Homebrew tap
- [ ] 簽署/公證建置

## 貢獻

歡迎貢獻!請先開 issue 討論你想修改的內容。提交前請跑 `cargo fmt`、
`cargo clippy` 和 `cargo test`。

## 授權條款

本專案基於 [MIT License](LICENSE) 開源。

Copyright © 2026 [Anthemty](https://github.com/Anthemty).
