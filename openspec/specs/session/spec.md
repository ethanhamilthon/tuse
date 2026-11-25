# session

## Purpose
How a session runs, stops, and records what happens.

## Requirements

1

### Requirement: Stop everything
Steering SHALL still hand a running `lua` call off to a background task. Esc (with no picker, question, or panel open), `/stop`, and `turn.cancel` SHALL stop the turn and its queued input, the running `lua` call (result `[cancelled]`, no handoff), other unfinished calls (`[interrupted: no output]`), all running background tasks, hooks, commands, and `open` handlers, and close the active `pick`/`ask`.

#### Scenario: Esc during a tool call
- **WHEN** a `lua` call runs `sh("sleep 100")` and the user presses Esc
- **THEN** the process is killed, the result is `[cancelled]`, no task is created, and the turn ends `cancelled`

#### Scenario: Esc stops tasks
- **WHEN** a background task runs and the user presses Esc
- **THEN** its status becomes `stopped`

### Requirement: Task notices wake the agent
A task that ends on its own SHALL queue its `<task>` notice as a `notification` record. A turn that is already running takes the notice at its next step. An idle session SHALL start a turn for it.

#### Scenario: Idle session
- **WHEN** a task finishes while the session is idle
- **THEN** a turn starts and the agent receives the notice

### Requirement: Complete history
The history SHALL show everything that happens in the session, one role each: `system`, `user`, `agent`, `lua`, `tool`, `notification`, `notice`, `flash`, `error`, `hook`, `task`, `command`, `open`, `usage`, `model`, `turn`, `title`, `show`, `pick`, `ask`. Everything shown in the UI SHALL be a history record, including retry flashes, command, `open`, and reload errors, and stopped replies. `task`, `command`, and `open` entries SHALL hold name, status, duration, error, and code. Adding a record kind without a history entry, or emitting a visible event without a record, MUST NOT compile.

#### Scenario: Command run
- **WHEN** a custom command `/hello` finishes
- **THEN** the history has a `command` entry `hello success (<ms> ms)`

#### Scenario: Task run
- **WHEN** a task `tests` fails
- **THEN** the history has a `task` entry with `failed`, the error, and the function source

#### Scenario: Model and title
- **WHEN** the user picks a new model and the title changes
- **THEN** the history has a `model` entry with model and effort and a `title` entry with the new title

#### Scenario: Retry
- **WHEN** a request is retried
- **THEN** the history has a `flash` entry `Retry 1/<n> in <s>s: <error>`

#### Scenario: Command error
- **WHEN** a custom command raises
- **THEN** the history has an `error` entry `/<cmd>: <message>`

### Requirement: Database is the source of truth
Session info and history SHALL be written to `~/.tuse/tuse.db` as soon as they happen. The model request, `session()`, the TUI, and JSON-RPC SHALL read them from the database, through a cache that every write drops. Memory MUST hold only per-run state: busy flag, input queue, cancel flags, tasks, the active `pick`/`ask`, the lock, the stream buffer, a pending history replacement, and the cache.

#### Scenario: Another reader sees writes
- **WHEN** a hook records something and the next model request is built
- **THEN** the request is built from the database rows, including that record

#### Scenario: Crash keeps history
- **WHEN** the process is killed right after a tool result
- **THEN** resuming the session shows that result without any replay of in-memory state

### Requirement: Streaming is display only
Streamed text, reasoning, and tool deltas SHALL only be sent as events. When a reply ends, its usage and its assistant record (with `replay`) SHALL be saved in one transaction before the `assistant` event is sent. A stopped reply SHALL be saved as a cancelled `agent` record with the partial text.

#### Scenario: Reply ends
- **WHEN** a reply finishes streaming
- **THEN** `session().history` already holds the `agent` record when the `assistant` event arrives

#### Scenario: Stop mid-stream
- **WHEN** the user presses Esc while text streams
- **THEN** the partial text is saved as a cancelled `agent` record

### Requirement: Accumulated usage
Session usage totals (`input`, `output`, `cache_read`, `cache_write`, `cost`, `llm_cost`) SHALL be stored in the session row and grow with every model reply and `llm` call. They MUST NOT be derived from history and MUST NOT change when history is replaced.

#### Scenario: History replaced
- **WHEN** `set_session{ history = {} }` runs after replies that cost 0.5
- **THEN** `session().usage.cost` is still 0.5

### Requirement: System prompt in history
The first history record SHALL be `system` with the current system prompt. It SHALL be rebuilt when the session is created, resumed, and on every config load. `set_session` MAY change it until the next rebuild.

#### Scenario: Reload resets the prompt
- **WHEN** `set_session` changed the `system` text and the user runs `/reload`
- **THEN** the `system` record holds the prompt built from the config again

### Requirement: Started sessions
`tuse` and `/new` SHALL create the session row at once with `started = false`. Recording the first `user` message SHALL set `started = true` in the same transaction. `/resume` and `session.list` SHALL list only started sessions. Unstarted sessions are kept.

#### Scenario: Empty session hidden
- **WHEN** the user starts `tuse` and quits without a message
- **THEN** the session row exists with `started = false` and `/resume` does not list it

#### Scenario: First message
- **WHEN** the first user message is sent
- **THEN** `session().started` is `true` and the session shows in `/resume`

### Requirement: Session read and write from Lua
`session()` SHALL return the snapshot with `started` and `history` (structured records with `role`, `summary`, and the record fields). `set_session(t)` SHALL accept any subset of `title`, `model`, `effort`, `history`; any other key fails. It SHALL validate everything in Rust first and, on failure, raise without changing anything. Validation covers known roles and field types, a result for every tool call before the next `user` or `agent` record, data-URL images, `system` only first, a configured model, and a supported effort. `history` replaces the whole list. `usage`, `id`, `cwd`, `started`, and `created` are read-only. `set_title` is removed.

#### Scenario: Round trip
- **WHEN** Lua runs `set_session{ history = session().history }`
- **THEN** the history is unchanged, including `replay`

#### Scenario: Invalid history
- **WHEN** the new history has an `agent` tool call without a result
- **THEN** `set_session` raises an error and the stored history is unchanged

#### Scenario: Unknown key
- **WHEN** Lua runs `set_session{ usage = {} }`
- **THEN** it raises an error naming the key

### Requirement: History replacement waits for tool calls
When `set_session` gets `history` while tool calls of the current step run, the replacement SHALL be kept pending and applied after the last tool result of that step, before the next model request and before any queued user message is recorded. With no running tool call it SHALL apply at once.

#### Scenario: Self-compaction from a tool call
- **WHEN** the agent's `lua` call runs `set_session{ history = short }`
- **THEN** the call's result is recorded first, then the history becomes `short`, and the next request is built from it

#### Scenario: Steering during the call
- **WHEN** the user sends a message while that call runs
- **THEN** the message is recorded after the replacement

### Requirement: Edited replies drop replay
When `set_session` replaces history, an `agent` record whose `text` or `tool_calls` differ from the stored record at the same position SHALL lose its `replay`. Unchanged records keep it.

#### Scenario: Edited reply
- **WHEN** Lua changes the `text` of a reply that has Anthropic thinking blocks
- **THEN** the next request sends the new text, not the old blocks

### Requirement: Hook events
Hook events SHALL be exactly: `config_load`, `process_start`, `process_end`, `session_created`, `session_resumed`, `session_close`, `session_updated`, `user_message`, `agent_message`, `turn_error`, `turn_stopped`, `lua_start`, `lua_end`. Vars SHALL hold `event` plus only data that `session()` lacks: `changed` for `session_updated`, `status` for `agent_message`, `error` for `turn_error`, and `id`, `kind`, `name`, `code`, `status`, `duration_ms`, `result`, `error` for `lua_start`/`lua_end`. `session_updated` SHALL fire only after `set_session`. `config_load` SHALL fire after every successful config load, after `system` is rebuilt. `process_start` SHALL fire once per process, in the TUI and in `tuse --json`, and `process_end` only on quit or JSON-RPC `shutdown`. `session_close` SHALL fire before `/new` and `/resume`, not on quit. The Lua VM stays per session.

#### Scenario: Process start
- **WHEN** the user runs `tuse`
- **THEN** hooks see `config_load`, `process_start`, `session_created`, in that order

#### Scenario: New session
- **WHEN** the user runs `/new`
- **THEN** the old VM sees `session_close`, and the new VM sees `config_load` then `session_created`, but not `process_start`

#### Scenario: Quit
- **WHEN** the user quits or a client calls `shutdown`
- **THEN** hooks see `process_end` and not `session_close`

#### Scenario: Model change
- **WHEN** the user picks a model in `/models`
- **THEN** no `session_updated` fires

#### Scenario: Removed event
- **WHEN** a hook lists `name_change`
- **THEN** loading fails with the list of valid events

### Requirement: No built-in compaction
Tuse SHALL send the whole stored history to the model and MUST NOT prune, drop, or summarize it on its own. `compact_at` and the `compaction` role are removed.

#### Scenario: Long session
- **WHEN** `ctx` passes 90% of `ctx_size`
- **THEN** history is unchanged unless Lua calls `set_session`
