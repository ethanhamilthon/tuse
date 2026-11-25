# Redesign session history

## Why
Session data lives in two places: `State` keeps `records`, `meta`, `bounds`, `context`, and `stats` in memory, and the database gets a copy, lazily, only after the first user message. Readers (the model request, `session()`, TUI, JSON-RPC) read memory, so the database is a backup, not the truth. Some things the user sees (retry flashes, command and `open` errors, the stream cut by Esc) are never recorded. Compaction is built in and can't be changed, and Lua can only read a formatted trace and set a title. Hooks pass data that `session()` already has, and their names don't match session lifecycle.

## What Changes
- **BREAKING** The database is the source of truth. History and session info are written right away; only per-run state (busy, queue, cancel, tasks, active `pick`/`ask`, lock) and a read cache stay in memory. The model request, `session()`, TUI, and JSON-RPC read through the database. New schema; old `tuse.db` data is not migrated.
- Streaming is for the UI only. When a reply ends, its usage and the assistant record are saved in one transaction before any event.
- Everything shown in the UI or history view is a history record, including retry flashes, command, `open`, and reload errors, and the partial reply cut by a stop.
- The system prompt is the first history record. It is rebuilt on new, resume, and config reload.
- **BREAKING** `trace` becomes `history` everywhere: `session().history`, `session.history`, `/history`, `settings.ui.hide_history`.
- `session().history` holds structured records that can be written back.
- New host function `set_session{title, model, effort, history}`, Lua only: neither the TUI nor JSON-RPC gets a direct history editor, so both stay one to one and reach it through commands, hooks, and agent calls. **BREAKING** `set_title` is removed.
- **BREAKING** JSON-RPC `session.records` and `session.trace` merge into `session.history`, and `config.set` is removed (`model.set` covers it), so JSON-RPC has nothing the TUI lacks.
- Usage accumulates in the session and can't be changed by `set_session`.
- **BREAKING** Built-in compaction is removed (`compact_at`, prune and drop zones, the `compaction` role, the compaction note in the system prompt). Users can build it with `ctx`, `ctx_size`, `llm`, and `set_session`.
- Every `tuse` and `/new` creates a session row right away with `started = false`. The first user message sets `started = true`. `/resume` and `session.list` show only started sessions.
- **BREAKING** Hook events become `config_load`, `process_start`, `process_end`, `session_created`, `session_resumed`, `session_close`, `session_updated`, `user_message`, `agent_message`, `turn_error`, `turn_stopped`, `lua_start`, `lua_end`. `start`, `close`, `model_change`, and `name_change` are removed. Hook vars carry only what `session()` does not have.

## Impact
- Specs: `session`, `config`, `docs`, `tui`, `examples` (modified).
- Code: `crates/core` (storage, convo, session state, turn, hooks, Lua host, history view, prompt), `crates/config` (hook events, roles, `compact_at`, `hide_history`), `crates/rpc` (`session.history`, `session.list`, events), `crates/tui` (`/history`, status bar and panels via cache).
- Repo: `docs/lua.md`, `docs/session.md`, `docs/config.md`, `docs/json.md`, `docs/usage.md`, `docs/examples.md`, `examples/` (`rename.lua`, multiplexer hooks, `project.lua`, new compaction example).
- Production code is ~16.4k lines without tests. Removing compaction, the pending buffer, and resume replay roughly pays for the history converter and validation.
