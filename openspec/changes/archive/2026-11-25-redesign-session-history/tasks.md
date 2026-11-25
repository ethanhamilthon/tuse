# Tasks

## 1. Storage
- [x] 1.1 New schema: `sessions` with columns (title, model, effort, started, usage totals, created, updated) and `history(session, seq, body)`; drop `records` and `meta` JSON; no migration
- [x] 1.2 Insert with `seq = MAX(seq) + 1` in one transaction; `started = 1` with the first `user` record
- [x] 1.3 `SessionDir`: create row and lock at once; remove `pending`, `meta`, cached `seq`
- [x] 1.4 Usage totals updated on every `usage` record and `llm` call
- [x] 1.5 `Meta::list` filters `started = 1`

## 2. State and cache
- [x] 2.1 Remove `records`, `meta`, `bounds`, `context`, `stats` from `State`
- [x] 2.2 Read cache (session row + history, tagged by `seq`/revision), dropped on every write
- [x] 2.3 Model request, `session()`, TUI status/panels, JSON-RPC read through the cache
- [x] 2.4 Resume reads model, effort, ctx, UI pins from the database, no replay loop

## 3. Records and history
- [x] 3.1 Add `System`, `Flash`, cancelled `Assistant`; remove `Compaction`
- [x] 3.2 One helper that records then emits; route retry flash, command, `open`, reload, model-fetch errors through it
- [x] 3.3 Save usage + assistant in one transaction before the `assistant` event; save partial reply on stop
- [x] 3.4 Rebuild `system` record on create, resume, config load
- [x] 3.5 Rename trace to history: `HistoryItem`, `HISTORY_ROLES`, `session().history`, `session.history`, `/history`, `hide_history`; rename `convo::history` to `convo::wire`
- [x] 3.6 Structured history view (role, summary, fields) for Lua and JSON
- [x] 3.7 Replace `session.records` and `session.trace` with `session.history`; remove `config.set`; update the parity test

## 4. Compaction removal
- [x] 4.1 Delete `convo/compaction.rs`, `Bounds`, `PRUNED_*`, the `compaction` role
- [x] 4.2 Remove `compact_at` from config types, `model_info`, `/config`
- [x] 4.3 Remove the compaction note from the system prompt and `docs/lua.md`

## 5. Lua
- [x] 5.1 `set_session{title, model, effort, history}` with full Rust validation; unknown keys fail
- [x] 5.2 Pending history replacement applied after the step's last tool result, before queued user input
- [x] 5.3 Drop `replay` on changed `agent` records
- [x] 5.4 Remove `set_title`; `/model`, `model.set`, auto title use the shared update without `session_updated`
- [x] 5.5 Update `HOST_FUNCTIONS`, prompt function list, `docs/lua.md`

## 6. Hooks
- [x] 6.1 New `HOOK_EVENTS` list; remove `start`, `close`, `model_change`, `name_change`
- [x] 6.2 Minimal vars per event
- [x] 6.3 Fire `config_load`, `process_start`/`process_end` (CLI and RPC entry), `session_created`/`session_resumed`/`session_close`, `turn_error`, `turn_stopped`, `session_updated`
- [x] 6.4 Order tests: `tuse`, `/new`, `/resume`, quit

## 7. Docs and examples
- [x] 7.1 Update `docs/session.md`, `docs/config.md`, `docs/json.md`, `docs/usage.md`, `docs/lua.md`
- [x] 7.2 Update examples: `rename.lua` (`set_session`), multiplexers (no `name_change`), `project.lua` (`hide_history`)
- [x] 7.3 Add `examples/compaction.lua` and its section in `docs/examples.md`

## 8. Check
- [x] 8.1 `cargo test` and `cargo clippy` pass
- [x] 8.2 Production code stays under 20000 lines
- [x] 8.3 Merge deltas into `openspec/specs/` and archive the change
