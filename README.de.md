# cafe

[English](README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | Deutsch

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.74%2B-orange.svg)](https://www.rust-lang.org)
[![macOS](https://img.shields.io/badge/platform-macOS%2011%2B-blue.svg)](https://www.apple.com/macos)
[![Made with objc2](https://img.shields.io/badge/built%20with-objc2-9cf)](https://docs.rs/objc2)

> Dein Mac bleibt wach — deshalb heißt es **cafe**. ☕

Ein winziges Menüleisten-Werkzeug für macOS, das das Einschlafen während
Agent-Coding-Sessions verhindert.

Müde davon, dass dein Mac einschläft, während ein Coding-Agent lange
arbeitet? **cafe** stellt eine Kaffeetasse in deine Menüleiste. Klick, Modus
wählen, und dein Mac bleibt wach, bis du ihn wieder ausschaltest. Die Farbe
des Icons zeigt den Zustand auf einen Blick — kein Rätseln mehr, ob
`caffeinate` noch läuft.

In reinem Rust auf dem systemeigenen `caffeinate` gebaut, **ohne WebView** und
**ohne Abhängigkeiten** außer macOS selbst — nur ein ~400-KB-Binary, das in
deiner Menüleiste wohnt.

---

## Funktionen

- **Drei Modi** (exklusiv, im Menü):
  | Modus | `caffeinate`-Flags | Icon-Farbe | Wirkung |
  |-------|--------------------|------------|---------|
  | **Aus** | — | grau | kein Ruhemodus-Schutz |
  | **Nur Leerlauf** | `-i` | warmes Gelb | verhindert Leerlauf-Ruhezustand; Display darf dimmen |
  | **Leerlauf + Display** | `-di` | sattes Orange | verhindert Ruhezustand **und** hält das Display wach |
- **Zeitgesteuerte Sitzungen** — 30 Min. / 1 Std. / 2 Std. wachhalten, oder
  **Benutzerdefiniert…** (1–1440 Minuten). Der Timer gilt für den **aktuell
  gewählten Modus** (oder den zuletzt scharf geschalteten). Während er läuft,
  tickt die Restzeit direkt neben der Tasse (`12:34` oder `1:23:45`) und in
  der Menü-Kopfzeile; die App deaktiviert sich zum Ablauf selbst.
- **Agent-Online-Anzeige** — das Menü zeigt, welche Coding-Agents gerade
  laufen, jeder mit eigenem Farbpunkt: 🟠 Claude · 🟢 Codex · 🔵 WorkBuddy ·
  🟣 ZCode · 🟡 OpenCode. Online = leuchtet; offline = Zeile komplett
  ausgeblendet. Erkennung über einen Scan der Prozesstabelle (Basename-Abgleich,
  Groß-/Kleinschreibung egal) — aktuell bei jedem Menü-Öffnen, im Hintergrund
  alle 60 Sekunden.
- **Globales Tastenkürzel** — `Ctrl+Alt+C` rotiert durch die drei Modi aus
  jeder App heraus.
- **Auto: Agenten beobachten** *(Opt-in)* — schaltet Leerlauf + Display ein,
  während einer der fünf beobachteten Agents (Claude, Codex, WorkBuddy, ZCode,
  OpenCode) läuft, und aus, sobald alle beendet sind.
- **Bei Anmeldung öffnen** — simpler Schalter im Menü (LaunchAgent-basiert).
- **Zweisprachig Chinesisch/Englisch** — der Menüpunkt „语言:中文 / Language:
  English“ schaltet die UI mit einem Klick zwischen Chinesisch und Englisch
  um (Menü, Tooltip, Countdown); die Wahl wird gespeichert. Die App-UI
  unterstützt derzeit Chinesisch/Englisch.
- **Farbcodiertes Icon** — die Kaffeetasse (SF Symbol `cup.and.saucer.fill`)
  wird über eine hierarchische Symbolkonfiguration eingefärbt und ist auf
  allen macOS-Versionen zuverlässig sichtbar (einschließlich macOS 26 / Tahoe).
- **Keine verwaisten Prozesse.** Das `caffeinate`-Kind wird bei jedem
  Moduswechsel und beim Beenden/Panic verfolgt und getötet (`Drop`-Garantie).
  Kinder bekommen zusätzlich `-w <cafe pid>` — selbst ein `kill -9` von cafe
  hinterlässt keinen verwaisten Wachhalter.
- **Ehrliches UI.** Beim Öffnen des Menüs wird das Kind neu geprüft; hat
  jemand `caffeinate` extern getötet, springt das Icon zurück auf Aus, statt
  zu lügen.
- **Nur Menüleiste.** Läuft als Accessory — kein Dock-Icon, kein Hauptfenster.
- **Universal-Binary** — ein `.app` für Apple Silicon und Intel Mac.

## Voraussetzungen

- macOS 11.0 (Big Sur) oder neuer
- Rust 1.74+ (nur für den Build aus dem Quellcode)

## Installation

### Option A — das `.app`-Bundle bauen (empfohlen)

```sh
git clone https://github.com/Anthemty/cafe.git
cd cafe
./make-app.sh          # erzeugt dist/cafe.app
open dist/cafe.app
```

Beim ersten Start zeigt macOS möglicherweise „cafe kann nicht geöffnet werden,
da der Entwickler nicht verifiziert werden kann“ (unsigniert). Umgehen:
Rechtsklick auf die App → **Öffnen** → im Dialog **Öffnen**. Diese Freigabe
gilt dauerhaft.

### Option B — das rohe Binary direkt ausführen

```sh
cargo build --release
./target/release/cafe
```

Eine Kaffeetasse erscheint in der Menüleiste. Klick, Modus wählen — dein Mac
bleibt wach, bis du auf **Aus** zurück schaltest oder beendest.

> 💡 Das rohe Binary hat kein App-Icon; für das volle Erlebnis (Icon, richtiger
> Name in „Sofort beenden“, sauberes LSUIElement-Verhalten) nutze
> `make-app.sh`.

## Funktionsweise

cafe startet und überwacht genau einen `caffeinate`-Kindprozess und startet
ihn bei jedem Moduswechsel mit anderen Flags neu:

```
Moduswechsel ──► Supervisor::enter(mode) ──► altes Kind töten ──► neues Kind starten
                                                  │
               (caffeinate-Flags aus Mode::caffeinate_args)
```

Der Supervisor ist reines Rust ohne AppKit-Abhängigkeit und vollständig mit
Unit-Tests abgedeckt, in denen `sleep` als Stellvertreter für `caffeinate`
dient. Die GUI-Schicht ist dünn: ein `NSStatusItem` + `NSMenu`, dessen Aktionen
in den Supervisor zurückrufen.

## Projektstruktur

```
src/
  main.rs         NSApp-Aufbau, Status-Item + Menü, Action-Callbacks (define_class!)
  supervisor.rs   Lebenszyklus des caffeinate-Kindprozesses (spawn / kill / reap)
  state.rs        Mode-Enum, Config-Persistenz, Agent-Prozess-Erkennung
  icon.rs         SF-Symbol-Tasse + Farben pro Modus + Agent-Online-Punkte
make-app.sh       Baut das .app-Bundle und erzeugt das App-Icon
resources/        Generierte Icon-Assets (AppIcon.icns)
```

## Entwicklung

```sh
cargo build         # Debug-Build
cargo test          # Unit-Tests
cargo clippy        # Lint
./make-app.sh       # Release-.app
```

## Warum nicht einfach `caffeinate -i &`?

Geht — aber du vergisst, dass es läuft, gehst nachts weg und kommst zu einem
heißen Laptop mit leerem Akku zurück. cafe macht den Zustand **sichtbar** (die
Icon-Farbe) und das Ausschalten trivial.

## Roadmap

Mögliche künftige Ergänzungen (noch nicht umgesetzt):
- [ ] Konfigurierbarer Hotkey & Agent-Beobachtungsliste
- [ ] Homebrew tap
- [ ] Signierte/notarisierte Builds

## Mitwirken

Beiträge sind willkommen! Bitte öffne zuerst ein Issue, um Änderungen zu
besprechen. Führe vor dem Einreichen `cargo fmt`, `cargo clippy` und
`cargo test` aus.

## Lizenz

Dieses Projekt steht unter der [MIT-Lizenz](LICENSE).

Copyright © 2026 [Anthemty](https://github.com/Anthemty).
