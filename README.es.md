# cafe

[English](README.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | Español | [Français](README.fr.md) | [Deutsch](README.de.md) | [Italiano](README.it.md) | [Português (BR)](README.pt-BR.md) | [Русский](README.ru.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.74%2B-orange.svg)](https://www.rust-lang.org)
[![macOS](https://img.shields.io/badge/platform-macOS%2011%2B-blue.svg)](https://www.apple.com/macos)
[![Made with objc2](https://img.shields.io/badge/built%20with-objc2-9cf)](https://docs.rs/objc2)

> Tu Mac no se echa una siesta. Por eso se llama **cafe**. ☕

Una diminuta herramienta de menú de macOS para evitar el reposo durante sesiones
de codificación con agentes.

¿Cansado de que tu Mac se quede dormida mientras un agente de código trabaja
un buen rato? **cafe** coloca una taza de café en tu barra de menús. Haz clic,
elige un modo y tu Mac permanece despierta hasta que lo apagues. El color del
icono te dice el estado de un vistazo — nunca más adivinar si `caffeinate`
sigue corriendo.

Construido en Rust puro sobre el `caffeinate` del sistema, **sin vista web** y
**sin dependencias** más allá de macOS — solo un binario de ~400 KB que vive en
tu barra de menús.

---

## Características

- **Tres modos** (excluyentes, en el menú):
  | Modo | Flags de `caffeinate` | Color del icono | Efecto |
  |------|-----------------------|-----------------|--------|
  | **Apagado** | — | gris | sin prevención de reposo |
  | **Solo inactividad** | `-i` | amarillo cálido | evita el reposo por inactividad; la pantalla puede atenuarse |
  | **Inactividad + pantalla** | `-di` | naranja intenso | evita el reposo **y** mantiene la pantalla encendida |
- **Sesiones temporizadas** — mantén despierto el Mac 30 min / 1 h / 2 h, o
  elige **Personalizar…** (1–1440 minutos). El temporizador se aplica al
  **modo actualmente seleccionado** (o al último que activaste). Mientras
  corre, el tiempo restante parpadea en vivo junto a la taza (`12:34` o
  `1:23:45`) y en la cabecera del menú; la app se desactiva sola al llegar
  al límite.
- **Luces de agentes en línea** — el menú muestra qué agentes de código están
  corriendo, cada uno con su punto de color: 🟠 Claude · 🟢 Codex · 🔵
  WorkBuddy · 🟣 ZCode · 🟡 OpenCode. En línea = encendido; fuera de línea =
  la fila se oculta por completo. Se detecta escaneando la tabla de procesos
  (coincidencia por basename, sin distinguir mayúsculas) — fresco cada vez que
  abres el menú, refrescado en segundo plano cada 60 s.
- **Atajo global** — `Ctrl+Alt+C` alterna los tres modos desde cualquier app.
- **Auto: vigilar agentes** *(opcional)* — activa Inactividad + pantalla
  mientras cualquiera de los cinco agentes vigilados (Claude, Codex, WorkBuddy,
  ZCode, OpenCode) esté corriendo, y lo desactiva poco después de que terminen
  todos.
- **Abrir al iniciar sesión** — interruptor en el menú (basado en LaunchAgent).
- **Bilingüe chino/inglés** — el elemento de menú “语言:中文 / Language: English”
  cambia la interfaz entre chino e inglés con un clic (menú, tooltip,
  cuenta atrás); la elección persiste. La UI de la app admite actualmente
  chino/inglés.
- **Icono codificado por color** — la taza de café (SF Symbol
  `cup.and.saucer.fill`) se tiñe mediante una configuración jerárquica de
  símbolos, visible y fiable en todas las versiones de macOS (incluida
  macOS 26 / Tahoe).
- **Sin procesos huérfanos.** El hijo `caffeinate` se rastrea y se mata en cada
  cambio de modo y al salir/entrar en pánico (garantía de `Drop`). Los hijos
  también reciben `-w <cafe pid>`, así que ni un `kill -9` de cafe deja la
  prevención de reposo huérfana.
- **UI honesta.** Al abrir el menú se re-verifica el hijo; si alguien mató
  `caffeinate` externamente, el icono vuelve a Apagado en lugar de mentir.
- **Solo barra de menús.** Corre como accessory — sin icono en el Dock ni
  ventana principal.
- **Binario universal** — un `.app` para Apple Silicon e Intel Mac.

## Requisitos

- macOS 11.0 (Big Sur) o superior
- Rust 1.74+ (solo para compilar desde el código fuente)

## Instalación

### Opción A — compilar el `.app` (recomendada)

```sh
git clone https://github.com/Anthemty/cafe.git
cd cafe
./make-app.sh          # produce dist/cafe.app
open dist/cafe.app
```

La primera vez, macOS puede mostrar "cafe no se puede abrir porque el
desarrollador no se puede verificar" (no está firmado). Para evitarlo: clic
derecho en la app → **Abrir** → **Abrir** en el diálogo. Esta aprobación solo
se pide una vez.

### Opción B — ejecutar el binario directamente

```sh
cargo build --release
./target/release/cafe
```

Aparece una taza de café en la barra de menús. Haz clic, elige un modo y tu
Mac se mantiene despierta hasta volver a **Apagado** o salir.

> 💡 El binario directo no tiene icono de app; para la experiencia completa
> (icono, nombre correcto en Force Quit, comportamiento LSUIElement limpio),
> usa `make-app.sh`.

## Cómo funciona

cafe crea y supervisa un único proceso hijo `caffeinate`, reiniciándolo con
distintos flags al cambiar de modo:

```
cambio de modo ──► Supervisor::enter(mode) ──► mata el hijo previo ──► crea uno nuevo
                                                  │
                 (flags de caffeinate desde Mode::caffeinate_args)
```

El supervisor es Rust puro sin dependencia de AppKit y está completamente
probado con `sleep` como sustituto de `caffeinate`. La capa gráfica es fina:
un `NSStatusItem` + `NSMenu` cuyas acciones llaman de vuelta al supervisor.

## Estructura del proyecto

```
src/
  main.rs         Configuración de NSApp, ítem de estado + menú, callbacks
  supervisor.rs   Ciclo de vida del proceso hijo caffeinate (spawn / kill / reap)
  state.rs        Enum Mode, persistencia de config, detección de agentes
  icon.rs         Taza SF Symbol + colores por modo + puntos de agentes
make-app.sh       Compila el .app y genera el icono de la app
resources/        Recursos de icono generados (AppIcon.icns)
```

## Desarrollo

```sh
cargo build         # build de depuración
cargo test          # tests unitarios
cargo clippy        # lint
./make-app.sh       # .app de release
```

## ¿Por qué no simplemente `caffeinate -i &`?

Puedes — pero lo olvidarás, te irás por la noche y volverás a un portátil
caliente con la batería muerta. cafe hace el estado **visible** (el color del
icono) y apagarlo trivial.

## Hoja de ruta

Funciones futuras posibles (aún no implementadas):
- [ ] Atajo y lista de agentes vigilados configurables
- [ ] Homebrew tap
- [ ] Builds firmadas/notarizadas

## Contribuir

¡Las contribuciones son bienvenidas! Abre primero un issue para discutir qué
quieres cambiar. Ejecuta `cargo fmt`, `cargo clippy` y `cargo test` antes de
enviar.

## Licencia

Este proyecto está bajo la [Licencia MIT](LICENSE).

Copyright © 2026 [Anthemty](https://github.com/Anthemty).
