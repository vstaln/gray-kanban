# gray-kanban

Self-contained per-project kanban board (backlog/todo/doing/review/done). Port of hermes' kanban model.

A sidecar plugin for [gray](https://github.com/vstaln/gray).

## What it does

- `kanban` tool: `{action: "boards"|"create"|"add"|"move"|"done"|"show"|"rm", board?, title?, col?, id?, note?}`
- `/kanban [board]` renders the board as text columns; `/kanban boards` lists boards
- Boards are stored under `~/.gray/kanban/<cwd-hash>/<board>.json`, keyed by the
  session's working directory — each project gets its own set of boards.
- Columns: `backlog | todo | doing | review | done`. Card ids: `k1`, `k2`, …
  Default board is `main`; `add` auto-creates it.

## Wire methods

`plugin/manifest`, `tool/call`, `command/run`, `plugin/shutdown` — protocol 1.1, no capabilities needed.

## Install

```sh
gray plugin install kanban
```

## Develop

```sh
cargo test
gray account check      # entry point + manifest handshake
gray account publish    # check → build → release → publish to the gray registry
```
