## Why
History records are messy. A `lua` call shows twice (`agent: lua: {"code": ...}` and the `lua` result). Stored tags differ from the roles in `session().history` (`assistant`/`agent`, `turn_end`/`turn`, `settings`/`model`, `ui_markdown`/`show`, `ui_result`/`pick`+`ask`, `job`/`task`+`command`+`open`), which needs about 100 lines of mapping both ways. Three text-only kinds (`flash`, `notice`, `error`) have the same shape. Every hook run writes a `hook` record, so hooks fill most of the history.

## What Changes
- **BREAKING** One name per record kind, the same in the database, `session().history`, JSON-RPC, and `history.hide`: `system`, `user`, `agent`, `lua`, `notification`, `message`, `usage`, `meta`, `turn_end`, `show`, `pick`, `ask`, `task`, `command`, `open`.
- **BREAKING** Remove the `tool` role and the `tool`/`name` field: the only tool is `lua`.
- **BREAKING** Merge `flash`, `notice`, `error` into `message` with `level` = `flash` | `notice` | `error`.
- **BREAKING** Merge `title` and `model` into `meta` with optional `title`, `model`, `effort`.
- **BREAKING** `show` drops `removed`: an empty `text` removes the block.
- **BREAKING** Remove `hook` records. Hooks still run; a failing hook still leaves a `message` with level `notice`.
- `agent` entries no longer list their tool calls in their summary or detail; the `lua` entry shows the code. the `lua` entry is the only place where a call shows.
- Existing sessions and history are dropped on first start (schema reset); providers and state are kept. Released as 0.13.0.
- `history.hide` also accepts message levels `flash`, `notice`, `error`.
- `/history` and `/fork` hide `agent` entries with no text and no reasoning.

## Impact
- Specs: `session`, `tui`, `config`.
- Code: `convo/record.rs`, `session/view/history.rs`, `storage/db.rs`, hook lanes, TUI chat and history, `HISTORY_ROLES`, tests.
- Docs and examples: `docs/session.md`, `docs/json.md`, `docs/config.md`, `docs/lua.md`, `examples/project.lua`.
- User configs with `history.hide = { "hook" }` or other removed roles fail to load until updated.
