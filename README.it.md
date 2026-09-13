# cafe

[English](README.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | Italiano | [Português (BR)](README.pt-BR.md) | [Русский](README.ru.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.74%2B-orange.svg)](https://www.rust-lang.org)
[![macOS](https://img.shields.io/badge/platform-macOS%2011%2B-blue.svg)](https://www.apple.com/macos)
[![Made with objc2](https://img.shields.io/badge/built%20with-objc2-9cf)](https://docs.rs/objc2)

> Il tuo Mac non sonnecchia più — ecco perché si chiama **cafe**. ☕

Un piccolissimo strumento per la barra dei menu di macOS che impedisce il riposo
durante le sessioni di coding con agenti.

Stanco che il tuo Mac si addormenti mentre un agente di codice lavora a lungo?
**cafe** mette una tazza di caffè nella barra dei menu. Clicca, scegli una
modalità e il tuo Mac resta sveglio finché non la disattivi. Il colore dell'
icona ti dice lo stato a colpo d'occhio — niente più domande su se `caffeinate`
sta ancora girando.

Costruito in Rust puro sopra il `caffeinate` di sistema, **senza vista web** e
**senza dipendenze** oltre a macOS — solo un binario di ~400 KB che vive nella
barra dei menu.

---

## Funzionalità

- **Tre modalità** (mutuamente esclusive, nel menu):
  | Modalità | Flag `caffeinate` | Colore icona | Effetto |
  |----------|-------------------|--------------|---------|
  | **Off** | — | grigio | nessuna prevenzione del riposo |
  | **Solo inattività** | `-i` | giallo caldo | previene il riposo per inattività; lo schermo può attenuarsi |
  | **Inattività + schermo** | `-di` | arancio intenso | previene il riposo **e** mantiene lo schermo acceso |
- **Sessioni a tempo** — tieni sveglio il Mac per 30 min / 1 h / 2 h, oppure
  scegli **Personalizza…** (1–1440 minuti). Il timer si applica alla **modalità
  attualmente selezionata** (o all'ultima armata). Mentre gira, il tempo
  rimanente scorre in tempo reale accanto alla tazza (`12:34` o `1:23:45`) e
  nell'intestazione del menu; l'app si disarma da sola alla scadenza.
- **Luci agenti online** — il menu mostra quali agenti di codice sono in
  esecuzione, ciascuno con il suo punto colorato: 🟠 Claude · 🟢 Codex ·
  🔵 WorkBuddy · 🟣 ZCode · 🟡 OpenCode. Online = acceso; offline = la riga è
  nascosta del tutto. Rilevamento scansionando la tabella dei processi
  (confronto sul basename, senza distinzione maiuscole/minuscole) — aggiornato
  a ogni apertura del menu, rinfrescato in background ogni 60 secondi.
- **Scorciatoia globale** — `Ctrl+Alt+C` cicla le tre modalità da qualsiasi app.
- **Auto: osserva gli agenti** *(opt-in)* — attiva Inattività + schermo mentre
  uno dei cinque agenti osservati (Claude, Codex, WorkBuddy, ZCode, OpenCode)
  è in esecuzione, e lo disattiva poco dopo che sono tutti usciti.
- **Apri al login** — interruttore nel menu (basato su LaunchAgent).
- **Bilingue cinese/inglese** — la voce di menu “语言:中文 / Language: English”
  cambia l'interfaccia tra cinese e inglese con un clic (menu, tooltip,
  countdown); la scelta viene salvata. L'UI dell'app supporta attualmente
  cinese/inglese.
- **Icona codificata a colori** — la tazza di caffè (SF Symbol
  `cup.and.saucer.fill`) è tinta tramite una configurazione simbolica
  gerarchica, visibile e affidabile su tutte le versioni di macOS (incluso
  macOS 26 / Tahoe).
- **Nessun processo orfano.** Il figlio `caffeinate` viene tracciato e ucciso a
  ogni cambio di modalità e all'uscita/panico (garanzia `Drop`). I figli
  ricevano anche `-w <cafe pid>`, quindi nemmeno un `kill -9` di cafe lascia
  prevenzione del riposo orfana.
- **UI onesta.** Aprendo il menu il figlio viene riverificato; se qualcuno ha
  ucciso `caffeinate` esternamente, l'icona torna a Off invece di mentire.
- **Solo barra dei menu.** Gira come accessory — nessuna icona nel Dock,
  nessuna finestra principale.
- **Binario universale** — un `.app` per Apple Silicon e Intel Mac.

## Requisiti

- macOS 11.0 (Big Sur) o superiore
- Rust 1.74+ (solo per compilare dai sorgenti)

## Installazione

### Opzione A — compila il bundle `.app` (consigliata)

```sh
git clone https://github.com/Anthemty/cafe.git
cd cafe
./make-app.sh          # produce dist/cafe.app
open dist/cafe.app
```

Al primo avvio macOS potrebbe mostrare "cafe non può essere aperto perché lo
sviluppatore non può essere verificato" (non firmato). Per aggirare: clic
destro sull'app → **Apri** → **Apri** nella finestra di dialogo. Questo
consenso viene richiesto una sola volta.

### Opzione B — esegui il binario direttamente

```sh
cargo build --release
./target/release/cafe
```

Appare una tazza di caffè nella barra dei menu. Clicca, scegli una modalità e
il tuo Mac resta sveglio finché non torni a **Off** o esci.

> 💡 Il binario diretto non ha l'icona dell'app; per l'esperienza completa
> (icona, nome corretto in Forza chiusura, comportamento LSUIElement pulito),
> usa `make-app.sh`.

## Come funziona

cafe crea e supervisiona un unico processo figlio `caffeinate`, riavviandolo
con flag diversi a ogni cambio di modalità:

```
cambio modalità ──► Supervisor::enter(mode) ──► uccide il figlio precedente ──► ne avvia uno nuovo
                                                  │
               (flag caffeinate da Mode::caffeinate_args)
```

Il supervisor è Rust puro senza dipendenze da AppKit ed è interamente coperto
da test unitari che usano `sleep` al posto di `caffeinate`. Il livello GUI è
sottile: un `NSStatusItem` + `NSMenu` le cui azioni richiamano il supervisor.

## Struttura del progetto

```
src/
  main.rs         Setup NSApp, voce di stato + menu, callback delle azioni
  supervisor.rs   Ciclo di vita del processo figlio caffeinate (spawn / kill / reap)
  state.rs        Enum Mode, persistenza della config, rilevamento agenti
  icon.rs         Tazza SF Symbol + colori per modalità + punti agenti
make-app.sh       Compila il .app e genera l'icona dell'app
resources/        Risorse icona generate (AppIcon.icns)
```

## Sviluppo

```sh
cargo build         # build di debug
cargo test          # test unitari
cargo clippy        # lint
./make-app.sh       # .app di release
```

## Perché non semplicemente `caffeinate -i &`?

Puoi — ma te ne dimenticherai, uscirai per la notte e tornerai a un laptop
rovente con la batteria morta. cafe rende lo stato **visibile** (il colore
dell'icona) e spegnerlo banale.

## Roadmap

Possibili aggiunte future (non ancora implementate):
- [ ] Scorciatoia e lista agenti osservati configurabili
- [ ] Homebrew tap
- [ ] Build firmate/notarizzate

## Contribuire

I contributi sono benvenuti! Apri prima una issue per discutere cosa vuoi
cambiare. Esegui `cargo fmt`, `cargo clippy` e `cargo test` prima di inviare.

## Licenza

Questo progetto è rilasciato sotto [Licenza MIT](LICENSE).

Copyright © 2026 [Anthemty](https://github.com/Anthemty).
