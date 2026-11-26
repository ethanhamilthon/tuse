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
The history SHALL show everything that happens in the session, one role each: `system`, `user`, `agent`, `lua`, `notification`, `message`, `usage`, `meta`, `turn_end`, `show`, `pick`, `ask`, `task`, `command`, `open`. Everything shown in the UI SHALL be a history record, including retry flashes, command, `open`, and reload errors, and stopped replies. `message` SHALL hold `level` (`flash`, `notice`, or `error`) and `text`. `meta` SHALL hold any of `title`, `model`, `effort` that changed. `task`, `command`, and `open` entries SHALL hold name, status, duration, error, and code. Hook runs MUST NOT create records. Adding a record kind without a history entry, or emitting a visible event without a record, MUST NOT compile.

#### Scenario: Command run
- **WHEN** a custom command `/hello` finishes
- **THEN** the history has a `command` entry `hello success (<ms> ms)`

#### Scenario: Task run
- **WHEN** a task `tests` fails
- **THEN** the history has a `task` entry with `failed`, the error, and the function source

#### Scenario: Model and title
- **WHEN** the user picks a new model and the title changes
- **THEN** the history has a `meta` entry with model and effort and a `meta` entry with the new title

#### Scenario: Retry
- **WHEN** a request is retried
- **THEN** the history has a `message` entry with level `flash` and text `Retry 1/<n> in <s>s: <error>`

#### Scenario: Command error
- **WHEN** a custom command raises
- **THEN** the history has a `message` entry with level `error` and text `/<cmd>: <message>`

#### Scenario: Hooks leave no records
- **WHEN** a `user_message` hook succeeds
- **THEN** neither the database nor `session().history` has a record for it

#### Scenario: Failing hook
- **WHEN** a hook `rename` raises `boom`
- **THEN** the history has only a `message` entry with level `notice` and text `hook rename: boom`

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
When `set_session` gets `history` while tool calls of the current step run, the replacement SHALL be kept pending and applied after the last tool result of that step, before a pending session switch, before the next model request, and before any queued user message is recorded. With no running tool call it SHALL apply at once.

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
Hook events SHALL be exactly: `config_load`, `process_start`, `process_end`, `session_created`, `session_resumed`, `session_close`, `session_updated`, `user_message`, `agent_message`, `turn_error`, `turn_stopped`, `lua_start`, `lua_end`. Vars SHALL hold `event` plus only data that `session()` lacks: `changed` for `session_updated`, `status` for `agent_message`, `error` for `turn_error`, and `id`, `kind`, `name`, `code`, `status`, `duration_ms`, `result`, `error` for `lua_start`/`lua_end`. `session_updated` SHALL fire only after `set_session`. `config_load` SHALL fire after every successful config load, after `system` is rebuilt. `process_start` SHALL fire once per process, in the TUI and in `tuse --json`, and `process_end` only on quit or JSON-RPC `shutdown`. `session_close` SHALL fire before `/new` and `/resume`, not on quit. The Lua VM stays per session. Hooks SHALL run in the background: a turn, a reply, and a launch SHALL NOT wait for any hook. Hook runs SHALL NOT be stored in the database or history.

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

### Requirement: Distinguish foreground turn activity
The session SHALL make the current main model attempt, retry backoff, and foreground Lua execution distinguishable to the TUI, including the start and end of each phase. Provider wait SHALL start at request dispatch, not when user input is queued. Foreground Lua execution MUST be distinct from receiving Lua arguments and MUST end when the call finishes, is stopped, or is handed off. Activity state SHALL be transient and MUST NOT change the order of persisted records or the JSON-RPC event schema.

#### Scenario: Before and after the provider request
- **WHEN** a user hook runs before request dispatch or an agent hook runs after the provider response finishes
- **THEN** the hook does not count as waiting for the provider or running a foreground Lua tool

#### Scenario: Steering hands off an executing tool
- **WHEN** a foreground Lua call transfers to a background task because of steering
- **THEN** it no longer counts as foreground execution even if the task continues running

### Requirement: Persist picker questions with answers
A completed titled `pick` SHALL save the original Markdown question in `prompt` and the selected string in `value` in the existing `pick` history entry. The result SHALL be available through both TUI and JSON-RPC history, including after session resume. TUI chat SHALL render the question as Markdown like an `ask` question and the answer as plain text. An untitled picker SHALL retain the `Picker` prompt. A cancelled picker SHALL NOT create a result entry.

#### Scenario: Read a picker result after resuming
- **WHEN** the user answers `Retry` to a picker titled `**Build failed.** What next?` and resumes the session
- **THEN** the `pick` history entry retains that exact Markdown source and `Retry`, and TUI chat renders the formatted question above the plain-text answer

#### Scenario: Untitled or cancelled picker
- **WHEN** a user answers an untitled picker and then cancels a titled picker
- **THEN** only the first picker produces a history result, with `Picker` as its prompt

### Requirement: Forked sessions
A fork SHALL create a new session row with a new id, the same folder, model, and effort, and the title `[fork] ` followed by the original title. Its history SHALL be a copy of the original records through the picked agent message or up to, not including, the picked user message, including the `system` record, and MUST pass the same validation as `set_session{ history = ... }`. `started` SHALL follow the normal rule, so it is true once a `user` record is copied. Usage, cost, and context SHALL start at zero, and `usage` records are not copied. The original session and its history MUST NOT change. Switching to the fork SHALL close the original like `/new` and `/resume`, running `session_close` hooks.

#### Scenario: Copied history
- **WHEN** a session with 10 records is forked at agent record 5
- **THEN** the new session has records 1 to 5 and the original still has 10

#### Scenario: Title
- **WHEN** a session titled `Fix parser` is forked
- **THEN** the fork is titled `[fork] Fix parser`

#### Scenario: Fresh usage
- **WHEN** a session with recorded usage and cost is forked
- **THEN** the fork reports zero usage and zero cost

#### Scenario: Original survives
- **WHEN** the fork is closed and the original is resumed
- **THEN** the original history is exactly as before the fork

### Requirement: Fork and resume from Lua
The host SHALL provide `fork(index)` and `resume(id)`. `fork(index)` SHALL take a 1-based position in `session().history` of a `user` or `agent` record and create the same session as `/fork` at that record, setting the text of a `user` record as the draft. `resume(id)` SHALL switch to the started session `id` of the current folder, like `/resume`. Both SHALL validate their argument when called and raise without changing anything on a bad position, a record of another role, an unknown or unstarted session, a session of another folder, or a session switch already pending. A valid call SHALL schedule the switch and return at once. While tool calls of the current step run, the switch SHALL wait until the last tool result of that step is recorded, after a pending history replacement and before the next model request. With no running tool call it SHALL apply at once. After the switch, the old session MUST get no further model request, queued user messages MUST NOT move to the new session, and hooks SHALL run as for `/new`: `session_close`, then the new session event.

#### Scenario: Fork from a tool call
- **WHEN** the agent's `lua` call runs `fork(3)` on a session whose third record is an `agent` message
- **THEN** the call's result is recorded in the old session first, then a new session opens with records 1 to 3 and the old session sends no further request

#### Scenario: Fork at a user record
- **WHEN** a command runs `fork(2)` and record 2 is a `user` message
- **THEN** the new history ends before that message and its text is the draft

#### Scenario: Resume
- **WHEN** a command runs `resume(id)` for another started session of this folder
- **THEN** that session opens as with `/resume`

#### Scenario: Bad arguments
- **WHEN** `fork(1)` points at the `system` record, `fork(99)` is out of range, `resume("nope")` is unknown, or `resume(id)` names a session of another folder
- **THEN** each raises an error and the current session is unchanged

#### Scenario: Switch already pending
- **WHEN** a tool call runs `fork(3)` and then `resume(id)` in the same step
- **THEN** `resume` raises `a session switch is already pending` and the fork still applies

#### Scenario: History replacement first
- **WHEN** a tool call runs `set_session{ history = short }` and then `fork(2)`
- **THEN** the replacement applies first and the fork copies the replaced history

#### Scenario: Queued message
- **WHEN** the user sends a message while the call runs
- **THEN** the message stays with the old session and is not recorded in the new one

### Requirement: Switch event over JSON-RPC
When a session switch started by Lua applies, JSON-RPC SHALL send the event `session_switched` with `{session, model, effort, draft}` after the remaining events of the old session and before the first event of the new one. `draft` is empty unless the switch was a fork at a `user` record. `session.new`, `session.resume`, and `session.fork` replies stay unchanged.

#### Scenario: Lua switch
- **WHEN** a command runs `resume(id)` while a JSON-RPC client is connected
- **THEN** the client gets `session_switched` with the new session id, and later events belong to that session

### Requirement: One name per record kind
Each record kind SHALL use one name everywhere: the stored `role` in the database body, `session().history`, `set_session`, JSON-RPC `session.history`, and `history.hide`. Fields SHALL have the same names in storage and in `session().history`; `session().history` only adds `summary`. The `tool` role and the tool name field are removed.

#### Scenario: Stored body
- **WHEN** a `lua` call finishes
- **THEN** its database body has `"role": "lua"` with `id`, `code`, `output`, `exit`, and the same fields appear in `session().history`

#### Scenario: Old role rejected
- **WHEN** Lua runs `set_session{ history = { {role = "tool", ...} } }` or uses `title`, `flash`, `hook`
- **THEN** it raises an unknown-role error and the history is unchanged

### Requirement: Lua calls show once
An `agent` entry's summary and detail SHALL hold only its text, reasoning, and the cancelled marker, never its tool calls. The `lua` entry SHALL show the code. `agent.tool_calls` SHALL still be stored and sent to the provider.

#### Scenario: Tool-only reply
- **WHEN** the agent replies with only a `lua` call
- **THEN** the `agent` entry summary is empty and the `lua` entry summary is the first line of the code

### Requirement: Show removal by empty text
A `show` record SHALL hold `name`, `text`, `pin`. An empty `text` with `pin` false SHALL mean the block was removed.

#### Scenario: Remove block
- **WHEN** Lua runs `show("plan", "", false)`
- **THEN** the history has a `show` entry `plan` with empty `text` and the block disappears

### Requirement: History reset
Opening a database with schema version below 3 SHALL drop all sessions and history and set the version to 3. Providers and state MUST stay unchanged.

#### Scenario: Upgrade to 0.13.0
- **WHEN** a database from 0.12 with saved sessions and providers is opened
- **THEN** `/resume` lists no sessions and `/connect` providers are still there
