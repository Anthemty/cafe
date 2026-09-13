# cafe

[English](README.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [日本語](README.ja.md) | 한국어 | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Italiano](README.it.md) | [Português (BR)](README.pt-BR.md) | [Русский](README.ru.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.74%2B-orange.svg)](https://www.rust-lang.org)
[![macOS](https://img.shields.io/badge/platform-macOS%2011%2B-blue.svg)](https://www.apple.com/macos)
[![Made with objc2](https://img.shields.io/badge/built%20with-objc2-9cf)](https://docs.rs/objc2)

> 당신의 Mac이 졸지 않도록. 그래서 이름이 **cafe**입니다. ☕

에이전트 코딩 세션을 위한 초소형 macOS 메뉴 막대 절전 방지 도구.

오래 실행되는 코딩 에이전트가 작업하는 동안 Mac이 졸드는 게 답답하신가요?
**cafe**는 메뉴 막대에 커피잔 하나를 놓아둡니다. 클릭해서 모드를 고르면,
끌 때까지 Mac은 깨어 있습니다. 아이콘 색깔만 봐도 상태를 알 수 있습니다 —
`caffeinate`가 아직 도는지 추측할 필요가 없죠.

시스템 기본 `caffeinate` 위에 순수 Rust로 빌드했습니다. **웹 뷰 없음**, macOS
자체 외에는 **의존성 없음** — 메뉴 막대에 살던 약 400 KB 바이너리 하나뿐입니다.

---

## 기능

- **세 가지 모드**(메뉴에서 상호 배타적):
  | 모드 | `caffeinate` 플래그 | 아이콘 색 | 효과 |
  |------|--------------------|-----------|------|
  | **끄기** | — | 회색 | 절전 방지 없음 |
  | **유휴 방지만** | `-i` | 따뜻한 노랑 | 시스템 유휴 절전 방지(화면은 어두워질 수 있음) |
  | **절전 방지 + 화면 켜짐** | `-di` | 진한 주황 | 절전 방지**이자** 화면 항상 켜짐 |
- **타이머 세션** — 30분 / 1시간 / 2시간, 또는 **사용자 지정…**(1–1440분).
  타이머는 **현재 선택한 모드**에 적용됩니다(꺼져 있으면 마지막 모드). 실행 중에는
  컵 옆에 남은 시간이 실시간으로 표시되고(`12:34`, `1:23:45`) 메뉴 헤더에도
  동기화됩니다. 시간이 되면 자동으로 해제됩니다.
- **에이전트 온라인 표시등** — 어떤 코딩 에이전트가 실행 중인지 메뉴에 표시합니다.
  각 에이전트에 고유 색 점:🟠 Claude · 🟢 Codex · 🔵 WorkBuddy · 🟣 ZCode ·
  🟡 OpenCode. 실행 중 = 점등, 꺼짐 = 행 전체 숨김. 프로세스 테이블 스캔으로
  감지(basename 일치, 대소문자 무시) — 메뉴를 열 때마다 즉시 갱신, 백그라운드는
  60초마다 갱신.
- **전역 단축키** — `Ctrl+Alt+C`로 어디서든 세 모드를 순환합니다.
- **자동: 에이전트 감시***(옵트인)* — 감시 대상 5개 에이전트(Claude, Codex,
  WorkBuddy, ZCode, OpenCode) 중 하나라도 실행 중이면 절전 방지 + 화면 켜짐을
  자동으로 켜고, 모두 종료하면 자동으로 끕니다.
- **로그인 시 실행** — 메뉴 토글(LaunchAgent 기반).
- **중국어/영어 전환** — "언어:中文 / Language: English" 메뉴 항목으로 UI를
  한 번에 전환(메뉴, tooltip, 카운트다운). 선택은 저장됩니다. 앱 UI는 현재
  중국어/영어 두 언어를 지원합니다.
- **색으로 상태 표시** — 커피잔 아이콘(SF Symbol `cup.and.saucer.fill`)은
  hierarchical symbol configuration으로 착색되어, macOS 26(Tahoe)를 포함한
  모든 버전에서 안정적으로 보입니다.
- **프로세스 누수 없음.** 모드 전환과 종료(panic 시 `Drop` 보장 포함)마다
  `caffeinate` 자식 프로세스를 추적해 종료합니다. 자식에는 `-w <cafe pid>`가
  붙어, cafe가 `kill -9`당해도 고아 절전 방지 프로세스가 남지 않습니다.
- **정직한 UI.** 메뉴를 열 때마다 자식 프로세스를 재확인합니다. `caffeinate`가
  외부에서 죽었으면 아이콘은 거짓말 대신 꺼짐으로 돌아갑니다.
- **메뉴 막대 전용.** accessory로 실행 — Dock 아이콘도 메인 창도 없습니다.
- **유니버설 바이너리** — `.app` 하나로 Apple Silicon과 Intel Mac 모두 지원.

## 요구 사항

- macOS 11.0(Big Sur) 이상
- Rust 1.74+(소스에서 빌드할 때만 필요)

## 설치

### 방법 A — `.app` 번들 빌드(권장)

```sh
git clone https://github.com/Anthemty/cafe.git
cd cafe
./make-app.sh          # dist/cafe.app 생성
open dist/cafe.app
```

처음 실행 시 macOS가 "개발자를 확인할 수 없어서 열 수 없습니다"라고 표시할 수
있습니다(서명 안 됨). 우클릭 → **열기** → 대화상자에서 **열기**로 우회하세요.
이 승인은 한 번만 하면 됩니다.

### 방법 B — 바이너리 직접 실행

```sh
cargo build --release
./target/release/cafe
```

메뉴 막대에 커피잔이 나타납니다. 클릭해서 모드를 고르면 **끄기**로 돌아가거나
종료할 때까지 Mac이 깨어 있습니다.

> 💡 바이너리에는 앱 아이콘이 없습니다. 완전한 경험(앱 아이콘, Force Quit의
> 정식 이름, 올바른 LSUIElement 동작)은 `make-app.sh`를 사용하세요.

## 작동 방식

cafe는 `caffeinate` 자식 프로세스를 하나 생성해 관리하며, 모드를 바꿀 때 다른
플래그로 재시작합니다:

```
모드 변경 ──► Supervisor::enter(mode) ──► 기존 자식 kill ──► 새 자식 생성
                                                  │
                  (caffeinate 플래그는 Mode::caffeinate_args에서)
```

supervisor는 AppKit에 의존하지 않는 순수 Rust이며, `caffeinate` 대신 `sleep`을
쓴 단위 테스트로 완전히 검증되어 있습니다. GUI 계층은 얇습니다. `NSStatusItem` +
`NSMenu`의 액션이 supervisor로 콜백할 뿐입니다.

## 프로젝트 구조

```
src/
  main.rs         NSApp 설정, 상태 표시 항목 + 메뉴, 액션 콜백(define_class!)
  supervisor.rs   caffeinate 자식 프로세스 수명 주기(spawn / kill / reap)
  state.rs        Mode 열거형, 설정 저장, 에이전트 프로세스 감지
  icon.rs         SF Symbol 컵 + 모드별 색 + 에이전트 온라인 점
make-app.sh       .app 번들 빌드 및 앱 아이콘 생성
resources/        생성된 아이콘 에셋(AppIcon.icns)
```

## 개발

```sh
cargo build         # 디버그 빌드
cargo test          # 단위 테스트
cargo clippy        # 린트
./make-app.sh       # 릴리스 .app
```

## 그냥 `caffeinate -i &` 하면 안 되나요?

됩니다 — 하지만 실행 중인 걸 잊어버리고, 밤에 그대로 나갔다가, 뜨거운 노트북과
방전된 배터리로 돌아오게 됩니다. cafe는 상태를 **보이게** 하고(아이콘 색) 끄는 것도
한 번의 클릭입니다.

## 로드맵

계획된 기능(미구현):
- [ ] 단축키 및 에이전트 감시 목록 설정화
- [ ] Homebrew tap
- [ ] 서명/notarization 빌드

## 기여

기여를 환영합니다! 변경 사항을 논의하려면 먼저 issue를 열어 주세요. 제출 전에
`cargo fmt`, `cargo clippy`, `cargo test`를 실행해 주세요.

## 라이선스

이 프로젝트는 [MIT License](LICENSE)로 배포됩니다.

Copyright © 2026 [Anthemty](https://github.com/Anthemty).
