# Design

## Storage
One SQLite file, `~/.tuse/tuse.db`, new schema (no migration):

```
sessions(id PRIMARY KEY, cwd, title, model, effort, started INTEGER,
         usage_input, usage_output, usage_cached, usage_cache_write,
         cost, llm_cost, created, updated)
history(session, seq, body, PRIMARY KEY (session, seq))
providers(...), state(...)   -- unchanged
```

- `seq` is assigned inside the insert transaction (`MAX(seq) + 1`), so hooks, tasks, and the turn can write from any thread.
- `SessionDir` keeps only the id and the lock file. `pending`, `meta`, and the cached `seq` go away; there is no lazy materialization.
- Usage columns are added to on every `usage` record. They are not derived from history, so replacing history keeps cost.

## Memory
`State` keeps only per-run data: `busy`, `generation`, `cancel`, `tool_handoff`, `queue`, `turn_start`, `calls`, `status`, the stream buffer, a pending history replacement, and the read cache.

The cache holds the session row and the decoded history, tagged with the last `seq` (and a revision bumped on replace). It is filled from the database on miss and dropped on every write. The TUI status bar, panels, and JSON-RPC read through it. The model request and `session()` call the same reader, so they also see the database state.

## History records
`Record` stays the stored form, without `Compaction`, plus:

- `System { text }`: always first. Rebuilt (replaced in place at seq of the first record) on session create, resume, and config load.
- `Flash { text }`: retry and other flash messages.
- `Error { text }`: also used for command, `open`, reload, and model-fetch errors.
- `Assistant { cancelled: true }`: the partial reply cut by a stop.

One helper writes a record and then emits the matching event; nothing visible is emitted without it. Every record kind maps to exactly one history role (a missing arm does not compile).

## Lua view
`session().history` is a list of tables: `{ role, summary, ...fields }`. Roles and fields:

| role | fields |
|---|---|
| `system` | `text` |
| `user` | `text`, `context`, `images` |
| `notification` | `text` |
| `agent` | `text`, `reasoning`, `tool_calls` (`{id, name, arguments}`), `replay` |
| `lua` / `tool` | `id`, `name`, `input`, `output`, `code`, `images`, `diffs` |
| `notice`, `error`, `flash`, `title` | `text` |
| `usage` | `model`, `input`, `output`, `cached`, `cache_write`, `llm` |
| `model` | `model`, `effort` |
| `turn` | `elapsed_ms`, `calls`, `model`, `effort`, `cancelled` |
| `hook` | `name`, `event`, `vars`, `error`, `ms` |
| `task`, `command`, `open` | `name`, `code`, `status`, `ms`, `error` |
| `show` | `name`, `text`, `pin`, `removed` |
| `pick`, `ask` | `id`, `prompt`, `value` |

`summary` is display-only and ignored on write.

## set_session
`set_session(t)` takes any subset of `title`, `model`, `effort`, `history`. Any other key fails.

- Validation runs in Rust before anything is written; on failure the call raises and nothing changes. Rules: known roles and field types; every `tool_calls` id has exactly one later `lua`/`tool` result before the next `user` or `agent`; images are data URLs; `system`, if present, is first; `model` exists in a `connect` entry; `effort` is supported by the model.
- `title`, `model`, `effort` apply at once.
- `history` replaces the whole list. If no tool call is running it applies at once; otherwise it is stored as pending and applied after the last tool result of the current step, before the next model request and before any queued user message is recorded.
- For each `agent` record whose `text` or `tool_calls` differ from the stored record at the same position, `replay` is dropped, so adapters never send stale provider blocks. Unchanged records keep their `replay`.
- `usage` records in the new list are stored as history but never touch the session totals.
- `ctx` keeps its last value until the next model reply.
- After a successful call `session_updated` fires with `changed` = the list of keys that changed.

`set_title` is removed. `/model`, `model.set`, and the auto title go through the same internal update but do not fire `session_updated`.

## Sessions
- `tuse` and `/new` insert the session row and take the lock at once, with `started = 0`.
- The first `user` record sets `started = 1` in the same transaction.
- `/resume` and `session.list` filter `started = 1`. Empty sessions are kept.

## Hooks and VM
The Lua VM stays per session; `/new` and `/resume` build a new one.

| event | when | vars besides `event` |
|---|---|---|
| `config_load` | after every successful config load (session start, `/reload`, `config.reload`), after `system` is rebuilt | none |
| `process_start` | once, when the `tuse` process starts (TUI or `tuse --json`) | none |
| `process_end` | once, on quit (TUI) or `shutdown` (JSON-RPC) | none |
| `session_created` | new session (`tuse` without resume, `/new`) | none |
| `session_resumed` | `--resume`, `/resume` | none |
| `session_close` | old session, before `/new` or `/resume` | none |
| `session_updated` | after `set_session` | `changed` |
| `user_message` | a user message is recorded | none |
| `agent_message` | a reply is recorded | `status` |
| `turn_error` | a turn fails | `error` |
| `turn_stopped` | Esc, `/stop`, `turn.cancel` | none |
| `lua_start` | tool, task, command, `open` starts | `id`, `kind`, `name`, `code` |
| `lua_end` | it ends | `id`, `kind`, `status`, `duration_ms`, `result`, `error` |

Order: `tuse` runs `config_load`, `process_start`, then `session_created` or `session_resumed`. `/new` and `/resume` run `session_close` in the old VM, then `config_load` and `session_created`/`session_resumed` in the new one. Quit runs `process_end` only. Globals set by `process_start` do not survive `/new` or `/resume`.

## Removed
`convo/compaction.rs`, `Bounds`, prune and drop, `PRUNED_*`, `Record::Compaction`, the `compaction` role, `compact_at` in `models` and `settings`, `model_info.compact_at`, the compaction note in the system prompt, `set_title`, `TraceItem`, `hide_trace`. `convo::history` is renamed `convo::wire`.

## Frontends
TUI and JSON-RPC stay one to one. `set_session` is Lua only; neither frontend gets a direct history editor. `session.history` replaces both `session.trace` and `session.records` and returns the structured records of `session().history`.
