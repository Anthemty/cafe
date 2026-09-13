# cafe

[English](README.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | 日本語 | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Italiano](README.it.md) | [Português (BR)](README.pt-BR.md) | [Русский](README.ru.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.74%2B-orange.svg)](https://www.rust-lang.org)
[![macOS](https://img.shields.io/badge/platform-macOS%2011%2B-blue.svg)](https://www.apple.com/macos)
[![Made with objc2](https://img.shields.io/badge/built%20with-objc2-9cf)](https://docs.rs/objc2)

> あなたの Mac を眠らせない。だから、**cafe**。☕

エージェントコーディングのための、小さな macOS メニューバー スリープ防止ツール。

長時間動き続けるコーディングエージェントの横で、Mac が居眠りするのは困りますよね。
**cafe** はメニューバーにコーヒーカップを置きます。クリックしてモードを選べば、
オフにするまで Mac は起き続けます。アイコンの色で状態がひと目でわかる——
`caffeinate` がまだ動いているか推測する必要はもうありません。

システム標準の `caffeinate` の上に純 Rust で構築。**WebView なし**、macOS 以外に
**依存ゼロ**——メニューバーに常駐する約 400 KB のバイナリひとつだけです。

---

## 機能

- **3 つのモード**(メニュー内で排他的):
  | モード | `caffeinate` フラグ | アイコン色 | 効果 |
  |--------|--------------------|------------|------|
  | **オフ** | — | グレー | スリープ防止なし |
  | **アイドル防止のみ** | `-i` | 暖色イエロー | システムのアイドル スリープを防止(画面は暗転可) |
  | **スリープ防止 + 画面常時オン** | `-di` | 濃いオレンジ | スリープ防止**かつ**画面を常にオン |
- **タイマー付きセッション** — 30 分 / 1 時間 / 2 時間、または**カスタム…**
  (1〜1440 分)。タイマーは**現在選択中のモード**に適用されます(未起動なら
  最後に選んだモード)。実行中はカップの横に残り時間がリアルタイム表示
  (`12:34`、`1:23:45`)され、メニューのヘッダーにも同期。時間になると自動解除。
- **エージェント オンライン表示** — どのコーディング エージェントが動いているか
  をメニュー内に表示。各エージェントに専用の色ドット:🟠 Claude · 🟢 Codex ·
  🔵 WorkBuddy · 🟣 ZCode · 🟡 OpenCode。起動中=点灯、停止中=行ごと非表示。
  プロセステーブルの走査で検出(basename 一致、大文字小文字を区別しない)——
  メニューを開くたびに即時更新、バックグラウンドでは 60 秒ごとに更新。
- **グローバル ホットキー** — `Ctrl+Alt+C` で 3 モードをどこからでも切り替え。
- **自動:エージェント監視***(オプトイン)* — 監視対象の 5 エージェント
  (Claude、Codex、WorkBuddy、ZCode、OpenCode)のいずれかが動いている間は
  スリープ防止 + 画面常時オンを自動で有効化し、全員終了すると自動解除。
- **ログイン時に起動** — メニュー内のトグル(LaunchAgent ベース)。
- **日英バイリンガル** — 「言語:中文 / Language: English」メニュー項目で中国語/
  英語 UI をワンクリック切り替え(メニュー・tooltip・カウントダウン)。選択は
  永続化されます。アプリ UI は現在 中国語/英語の 2 言語に対応。
- **色分けアイコン** — コーヒーカップ(SF Symbol `cup.and.saucer.fill`)は
  hierarchical symbol configuration で着色され、macOS の各バージョン
  (macOS 26 / Tahoe を含む)で確実に表示されます。
- **プロセス漏れなし。** モード切替・終了・panic(`Drop` 保証)のたびに
  `caffeinate` 子プロセスを追跡して kill。子プロセスには `-w <cafe pid>` を付け、
  cafe が `kill -9` されても孤児スリープ防止プロセスを残しません。
- **正直な UI。** メニューを開くたびに子プロセスを再確認。`caffeinate` が外部で
  kill されていたら、アイコンは嘘をつかずオフに戻ります。
- **メニューバー専用。** accessory として動作——Dock アイコンもメインウィンドウも
  ありません。
- **ユニバーサルバイナリ** — ひとつの `.app` で Apple Silicon と Intel Mac の
  両方に対応。

## 要件

- macOS 11.0(Big Sur)以降
- Rust 1.74+(ソースからビルドする場合のみ)

## インストール

### 方法 A — `.app` バンドルをビルド(推奨)

```sh
git clone https://github.com/Anthemty/cafe.git
cd cafe
./make-app.sh          # dist/cafe.app を生成
open dist/cafe.app
```

初回起動時に「開発元を検証できないため開けません」と表示されることがあります
(未署名のため)。右クリック → **開く** → ダイアログで**開く**で回避でき、
この許可は一度だけです。

### 方法 B — 生バイナリを直接実行

```sh
cargo build --release
./target/release/cafe
```

メニューバーにコーヒーカップが現れます。クリックしてモードを選べば、
**オフ**に戻すか終了するまで Mac は起き続けます。

> 💡 生バイナリにはアプリアイコンがありません。完全な体験(アプリアイコン、
> Force Quit での正式名称、正しい LSUIElement 動作)には `make-app.sh` を
> 使ってください。

## 仕組み

cafe は `caffeinate` 子プロセスをひとつ起動して監視し、モード切替のたびに
異なるフラグで再起動します:

```
モード切替 ──► Supervisor::enter(mode) ──► 旧子プロセスを kill ──► 新規起動
                                                  │
                    (caffeinate のフラグは Mode::caffeinate_args から)
```

supervisor は AppKit に依存しない純 Rust で、`caffeinate` の代わりに `sleep` を
使ったユニットテストで完全に検証されています。GUI 層は薄く、`NSStatusItem` +
`NSMenu` のアクションが supervisor へコールバックするだけです。

## プロジェクト構成

```
src/
  main.rs         NSApp セットアップ、ステータス項目 + メニュー、アクションコールバック
  supervisor.rs   caffeinate 子プロセスのライフサイクル(spawn / kill / reap)
  state.rs        Mode 列挙、設定の永続化、エージェントプロセス検出
  icon.rs         SF Symbol カップ + モード別カラー + エージェント オンライン ドット
make-app.sh       .app バンドルのビルドとアプリアイコン生成
resources/        生成されたアイコンアセット(AppIcon.icns)
```

## 開発

```sh
cargo build         # デバッグビルド
cargo test          # ユニットテスト
cargo clippy        # lint
./make-app.sh       # リリース用 .app
```

## なぜ `caffeinate -i &` ではだめなのか?

それでも動きます——でもそのうち存在を忘れ、夜のまま出かけ、戻ってきたら
熱いノート PC と空のバッテリー、という経験はありませんか?cafe は状態を
**可視化**(アイコンの色)し、オフにするのもワンクリックです。

## ロードマップ

今後の予定(未実装):
- [ ] ホットキーとエージェント監視リストの設定化
- [ ] Homebrew tap
- [ ] 署名/notarization 済みビルド

## コントリビューション

歓迎します!まず issue を立てて変更内容を相談してください。提出前に
`cargo fmt`、`cargo clippy`、`cargo test` を実行してください。

## ライセンス

このプロジェクトは [MIT License](LICENSE) の下で公開されています。

Copyright © 2026 [Anthemty](https://github.com/Anthemty).
