# cafe

[English](README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [Español](README.es.md) | Français | [Deutsch](README.de.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.74%2B-orange.svg)](https://www.rust-lang.org)
[![macOS](https://img.shields.io/badge/platform-macOS%2011%2B-blue.svg)](https://www.apple.com/macos)
[![Made with objc2](https://img.shields.io/badge/built%20with-objc2-9cf)](https://docs.rs/objc2)

> Votre Mac ne somnole plus — c'est pour ça que ça s'appelle **cafe**. ☕

Un tout petit outil pour la barre des menus de macOS qui empêche la mise en
veille pendant vos sessions de codage avec agents.

Fatigué que votre Mac s'assoupisse pendant qu'un agent de code travaille
longtemps ? **cafe** pose une tasse de café dans votre barre des menus.
Cliquez, choisissez un mode, et votre Mac reste éveillé jusqu'à ce que vous
le désactiviez. La couleur de l'icône indique l'état d'un coup d'œil —
fini les devinettes sur le fait de savoir si `caffeinate` tourne encore.

Construit en Rust pur au-dessus du `caffeinate` du système, **sans vue web** et
**sans dépendance** au-delà de macOS — juste un binaire d'environ 400 KB qui
vit dans votre barre des menus.

---

## Fonctionnalités

- **Trois modes** (mutuellement exclusifs, dans le menu) :
  | Mode | Flags `caffeinate` | Couleur de l'icône | Effet |
  |------|--------------------|--------------------|-------|
  | **Désactivé** | — | gris | pas de prévention de veille |
  | **Inactivité seule** | `-i` | jaune chaud | empêche la veille sur inactivité ; l'écran peut s'assombrir |
  | **Inactivité + écran** | `-di` | orange soutenu | empêche la veille **et** garde l'écran allumé |
- **Sessions temporisées** — rester éveillé 30 min / 1 h / 2 h, ou choisir
  **Personnaliser…** (1–1440 minutes). Le minuteur s'applique au **mode
  actuellement sélectionné** (ou au dernier armé). Pendant qu'il tourne, le
  temps restant défile en direct à côté de la tasse (`12:34` ou `1:23:45`) et
  dans l'en-tête du menu ; l'app se désarme toute seule à l'échéance.
- **Voyants agents en ligne** — le menu montre quels agents de code tournent
  actuellement, chacun avec son point de couleur : 🟠 Claude · 🟢 Codex ·
  🔵 WorkBuddy · 🟣 ZCode · 🟡 OpenCode. En ligne = allumé ; hors ligne =
  la ligne est masquée. Détection par balayage de la table des processus
  (correspondance sur le basename, insensible à la casse) — actualisé à
  chaque ouverture du menu, rafraîchi en arrière-plan toutes les 60 s.
- **Raccourci global** — `Ctrl+Alt+C` parcourt les trois modes depuis
  n'importe quelle app.
- **Auto : surveiller les agents** *(opt-in)* — active Inactivité + écran
  tant que l'un des cinq agents surveillés (Claude, Codex, WorkBuddy, ZCode,
  OpenCode) tourne, et le désactive peu après leur sortie.
- **Ouvrir à la connexion** — interrupteur dans le menu (basé sur LaunchAgent).
- **Bilingue chinois/anglais** — l'élément de menu « 语言:中文 / Language:
  English » bascule l'interface entre chinois et anglais en un clic (menu,
  tooltip, compte à rebours) ; le choix est persisté. L'UI de l'app prend
  actuellement en charge le chinois/anglais.
- **Icône codée par couleur** — la tasse (SF Symbol `cup.and.saucer.fill`)
  est teintée via une configuration symbolique hiérarchique, visible et fiable
  sur toutes les versions de macOS (macOS 26 / Tahoe incluse).
- **Aucun processus orphelin.** Le fils `caffeinate` est suivi et tué à chaque
  changement de mode et à la sortie/panique (garantie `Drop`). Les fils
  reçoivent aussi `-w <cafe pid>` : même un `kill -9` de cafe ne laisse aucune
  prévention de veille orpheline.
- **UI honnête.** Ouvrir le menu re-vérifie le fils ; si quelqu'un a tué
  `caffeinate` de l'extérieur, l'icône revient à Désactivé au lieu de mentir.
- **Barre des menus uniquement.** Tourne en accessory — pas d'icône Dock, pas
  de fenêtre principale.
- **Binaire universel** — un `.app` pour Apple Silicon et Intel Mac.

## Prérequis

- macOS 11.0 (Big Sur) ou plus récent
- Rust 1.74+ (uniquement pour compiler depuis les sources)

## Installation

### Option A — compiler le bundle `.app` (recommandée)

```sh
git clone https://github.com/Anthemty/cafe.git
cd cafe
./make-app.sh          # produit dist/cafe.app
open dist/cafe.app
```

Au premier lancement, macOS peut afficher « cafe ne peut pas être ouvert car
son développeur ne peut pas être vérifié » (non signé). Pour contourner :
clic droit sur l'app → **Ouvrir** → **Ouvrir** dans le dialogue. Cette
approbation n'est demandée qu'une fois.

### Option B — exécuter le binaire brut

```sh
cargo build --release
./target/release/cafe
```

Une tasse de café apparaît dans la barre des menus. Cliquez, choisissez un
mode, et votre Mac reste éveillé jusqu'à revenir à **Désactivé** ou quitter.

> 💡 Le binaire brut n'a pas d'icône d'app ; pour l'expérience complète
> (icône, nom correct dans Forcer à quitter, comportement LSUIElement propre),
> utilisez `make-app.sh`.

## Fonctionnement

cafe crée et supervise un unique processus fils `caffeinate`, le relançant
avec des flags différents à chaque changement de mode :

```
changement de mode ──► Supervisor::enter(mode) ──► tue l'ancien fils ──► lance le nouveau
                                                  │
              (flags caffeinate issus de Mode::caffeinate_args)
```

Le supervisor est du Rust pur sans dépendance AppKit, entièrement testé par
unités avec `sleep` en remplacement de `caffeinate`. La couche graphique est
fine : un `NSStatusItem` + `NSMenu` dont les actions rappellent le supervisor.

## Structure du projet

```
src/
  main.rs         Mise en place NSApp, item de statut + menu, callbacks d'action
  supervisor.rs   Cycle de vie du processus fils caffeinate (spawn / kill / reap)
  state.rs        Enum Mode, persistance de la config, détection des agents
  icon.rs         Tasse SF Symbol + couleurs par mode + points agents
make-app.sh       Compile le .app et génère l'icône de l'app
resources/        Ressources d'icône générées (AppIcon.icns)
```

## Développement

```sh
cargo build         # build debug
cargo test          # tests unitaires
cargo clippy        # lint
./make-app.sh       # .app de release
```

## Pourquoi pas simplement `caffeinate -i &` ?

Vous pouvez — mais vous oublierez qu'il tourne, partirez pour la nuit et
reviendrez vers un portable brûlant à batterie morte. cafe rend l'état
**visible** (la couleur de l'icône) et la désactivation triviale.

## Feuille de route

Ajouts futurs possibles (pas encore implémentés) :
- [ ] Raccourci et liste d'agents surveillés configurables
- [ ] Homebrew tap
- [ ] Builds signés/notariés

## Contribuer

Les contributions sont bienvenues ! Ouvrez d'abord une issue pour discuter de
ce que vous voulez changer. Lancez `cargo fmt`, `cargo clippy` et `cargo test`
avant de soumettre.

## Licence

Ce projet est sous [Licence MIT](LICENSE).

Copyright © 2026 [Anthemty](https://github.com/Anthemty).
