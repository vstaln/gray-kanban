<p align="center">
  <img src="assets/gray-logo.svg" alt="gray" width="96">
</p>
<h1 align="center">gray-kanban</h1>
<p align="center">A self-contained per-project kanban board — tool plus `/kanban` command.</p>
<p align="center">
  <a href="https://github.com/vstaln/gray-kanban/blob/main/LICENSE"><img alt="MIT License" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
  <img alt="gray plugin" src="https://img.shields.io/badge/gray-plugin-7aa2f7.svg">
  <img alt="rust" src="https://img.shields.io/badge/built%20with-rust-orange.svg">
</p>

Self-contained per-project kanban board
(backlog/todo/doing/review/done).

A sidecar plugin for [gray](https://github.com/vstaln/gray).

## What it does

- `kanban` tool: `{action: "boards"|"create"|"add"|"move"|"done"|"show"|"rm", board?, title?, col?, id?, note?}`
- `/kanban [board]` renders the board as text columns; `/kanban boards` lists boards
- `/kanban move [board]` — interactive card move (needs the `host.ask`
  capability): step 1 picks a card, step 2 picks the target column, and the
  move runs through the same path as the tool's `move` action. Without an
  ask channel it just renders the board.
- Boards are stored under `~/.gray/kanban/<cwd-hash>/<board>.json`, keyed by the
  session's working directory — each project gets its own set of boards.
- Columns: `backlog | todo | doing | review | done`. Card ids: `k1`, `k2`, …
  Default board is `main`; `add` auto-creates it.

## Wire methods

`plugin/manifest`, `tool/call`, `command/run`, `plugin/shutdown`, plus
sidecar→host `host/ask` for `/kanban move` — protocol 1.1. Capability:
`host.ask` (the command degrades to the board render without it).

## Install

```sh
gray plugin install kanban
gray plugin capabilities kanban --all   # grants host.ask → /kanban move picker
```

## Develop

```sh
cargo test
gray account check      # entry point + manifest handshake
gray account publish    # check → build → release → publish to the gray registry
```

---

Part of the [gray](https://github.com/vstaln/gray) plugin ecosystem —
the open-source AI agent harness. <https://gray.alignment.id>
