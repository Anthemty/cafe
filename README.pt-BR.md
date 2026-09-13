# cafe

[English](README.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Italiano](README.it.md) | Português (BR) | [Русский](README.ru.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.74%2B-orange.svg)](https://www.rust-lang.org)
[![macOS](https://img.shields.io/badge/platform-macOS%2011%2B-blue.svg)](https://www.apple.com/macos)
[![Made with objc2](https://img.shields.io/badge/built%20with-objc2-9cf)](https://docs.rs/objc2)

> Seu Mac não tira uma soneca — por isso o nome é **cafe**. ☕

Uma ferramenta minúscula na barra de menus do macOS para evitar o sono durante
sessões de codificação com agentes.

Cansado do seu Mac cochilar enquanto um agente de código trabalha por muito
tempo? O **cafe** coloca uma xícara de café na sua barra de menus. Clique,
escolha um modo e o seu Mac fica acordado até você desligar. A cor do ícone
mostra o estado de relance — nunca mais adivinhar se o `caffeinate` ainda está
rodando.

Construído em Rust puro sobre o `caffeinate` do sistema, **sem web view** e
**sem dependências** além do próprio macOS — só um binário de ~400 KB que vive
na sua barra de menus.

---

## Funcionalidades

- **Três modos** (mutuamente exclusivos, no menu):
  | Modo | Flags do `caffeinate` | Cor do ícone | Efeito |
  |------|-----------------------|--------------|--------|
  | **Desligado** | — | cinza | sem prevenção de sono |
  | **Somente inatividade** | `-i` | amarelo quente | evita o sono por inatividade; a tela pode escurecer |
  | **Inatividade + tela** | `-di` | laranja intenso | evita o sono **e** mantém a tela ligada |
- **Sessões com timer** — mantenha acordado por 30 min / 1 h / 2 h, ou escolha
  **Personalizar…** (1–1440 minutos). O timer se aplica ao **modo atualmente
  selecionado** (ou ao último ativado). Enquanto roda, o tempo restante corre
  ao vivo ao lado da xícara (`12:34` ou `1:23:45`) e no cabeçalho do menu; o
  app se desarma sozinho no prazo.
- **Luzes de agentes online** — o menu mostra quais agentes de código estão
  rodando, cada um com seu ponto de cor: 🟠 Claude · 🟢 Codex · 🔵 WorkBuddy ·
  🟣 ZCode · 🟡 OpenCode. Online = aceso; offline = a linha fica oculta.
  Detecção varrendo a tabela de processos (correspondência por basename, sem
  diferenciar maiúsculas) — atualizado a cada abertura do menu, renovado em
  segundo plano a cada 60 s.
- **Atalho global** — `Ctrl+Alt+C` alterna os três modos de qualquer app.
- **Auto: vigiar agentes** *(opcional)* — ativa Inatividade + tela enquanto
  qualquer um dos cinco agentes vigiados (Claude, Codex, WorkBuddy, ZCode,
  OpenCode) estiver rodando, e desativa pouco depois de todos saírem.
- **Abrir no login** — interruptor no menu (baseado em LaunchAgent).
- **Bilíngue chinês/inglês** — o item de menu “语言:中文 / Language: English”
  alterna a interface entre chinês e inglês com um clique (menu, tooltip,
  contagem regressiva); a escolha é salva. A UI do app atualmente suporta
  chinês/inglês.
- **Ícone codificado por cor** — a xícara de café (SF Symbol
  `cup.and.saucer.fill`) é tingida via configuração hierárquica de símbolos,
  visível e confiável em todas as versões do macOS (incluindo macOS 26 / Tahoe).
- **Nenhum processo órfão.** O filho `caffeinate` é rastreado e encerrado a
  cada troca de modo e ao sair/entrar em pânico (garantia de `Drop`). Os
  filhos também recebem `-w <cafe pid>`, então nem um `kill -9` no cafe deixa
  prevenção de sono órfã.
- **UI honesta.** Abrir o menu reverifica o filho; se alguém matou o
  `caffeinate` externamente, o ícone volta para Desligado em vez de mentir.
- **Só barra de menus.** Roda como accessory — sem ícone no Dock, sem janela
  principal.
- **Binário universal** — um `.app` para Apple Silicon e Intel Mac.

## Requisitos

- macOS 11.0 (Big Sur) ou mais recente
- Rust 1.74+ (apenas para compilar do código-fonte)

## Instalação

### Opção A — compilar o bundle `.app` (recomendada)

```sh
git clone https://github.com/Anthemty/cafe.git
cd cafe
./make-app.sh          # produz dist/cafe.app
open dist/cafe.app
```

Na primeira execução, o macOS pode mostrar "cafe não pode ser aberto porque o
desenvolvedor não pode ser verificado" (não assinado). Para contornar: clique
direito no app → **Abrir** → **Abrir** no diálogo. Essa aprovação vale para
sempre.

### Opção B — executar o binário direto

```sh
cargo build --release
./target/release/cafe
```

Uma xícara de café aparece na barra de menus. Clique, escolha um modo e o seu
Mac fica acordado até voltar para **Desligado** ou sair.

> 💡 O binário direto não tem ícone de app; para a experiência completa (ícone,
> nome correto no Forçar Encerrar, comportamento LSUIElement limpo), use
> `make-app.sh`.

## Como funciona

O cafe cria e supervisiona um único processo filho `caffeinate`, reiniciando-o
com flags diferentes a cada troca de modo:

```
troca de modo ──► Supervisor::enter(mode) ──► mata o filho antigo ──► cria um novo
                                                  │
             (flags do caffeinate vindas de Mode::caffeinate_args)
```

O supervisor é Rust puro sem dependência do AppKit e é totalmente coberto por
testes unitários que usam `sleep` no lugar do `caffeinate`. A camada gráfica é
fina: um `NSStatusItem` + `NSMenu` cujas ações chamam de volta o supervisor.

## Estrutura do projeto

```
src/
  main.rs         Configuração do NSApp, item de status + menu, callbacks de ação
  supervisor.rs   Ciclo de vida do processo filho caffeinate (spawn / kill / reap)
  state.rs        Enum Mode, persistência de config, detecção de agentes
  icon.rs         Xícara SF Symbol + cores por modo + pontos de agentes
make-app.sh       Compila o .app e gera o ícone do app
resources/        Recursos de ícone gerados (AppIcon.icns)
```

## Desenvolvimento

```sh
cargo build         # build de debug
cargo test          # testes unitários
cargo clippy        # lint
./make-app.sh       # .app de release
```

## Por que não simplesmente `caffeinate -i &`?

Pode — mas você vai esquecer que ele está rodando, sair para a noite e voltar
para um notebook quente com a bateria morta. O cafe deixa o estado **visível**
(a cor do ícone) e desligar vira trivial.

## Roadmap

Possíveis adições futuras (ainda não implementadas):
- [ ] Atalho e lista de agentes vigiados configuráveis
- [ ] Homebrew tap
- [ ] Builds assinadas/notarizadas

## Contribuir

Contribuições são bem-vindas! Abra primeiro uma issue para discutir o que quer
mudar. Rode `cargo fmt`, `cargo clippy` e `cargo test` antes de enviar.

## Licença

Este projeto é distribuído sob a [Licença MIT](LICENSE).

Copyright © 2026 [Anthemty](https://github.com/Anthemty).
