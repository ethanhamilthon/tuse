# session

## Purpose
How a session runs, stops, and records what happens.

## Requirements

### Requirement: Stop everything
Steering SHALL still hand a running `lua` call off to a background task. Esc (with no picker, question, or panel open), `/stop`, and `turn.cancel` SHALL stop the turn and its queued input, the running `lua` call (result `[cancelled]`, no handoff), other unfinished calls (`[interrupted: no output]`), all running background tasks, hooks, commands, and `open` handlers, and close the active `pick`/`ask`.

#### Scenario: Esc during a tool call
- **WHEN** a `lua` call runs `sh("sleep 100")` and the user presses Esc
- **THEN** the process is killed, the result is `[cancelled]`, no task is created, and the turn ends `cancelled`

#### Scenario: Esc stops tasks
- **WHEN** a background task runs and the user presses Esc
- **THEN** its status becomes `stopped`

### Requirement: Task notices wake the agent
A task that ends on its own SHALL queue its `<task>` notice as a `notification` record. A turn that is already running takes the notice at its next step. An idle session SHALL start a turn for it. When a task finishes cleanly without returning a non-empty value, its `<task>` notice MUST NOT report `failed` as the last line (it SHALL omit `Last line:`). Failed tasks with no non-empty error line SHALL continue to fall back to `failed`.

#### Scenario: Idle session
- **WHEN** a task finishes while the session is idle
- **THEN** a turn starts and the agent receives the notice

#### Scenario: Succeeded task with no return value
- **WHEN** `spawn(function() print("ok") end, "job")` finishes cleanly without returning a value
- **THEN** the queued `<task>` notification reports that `job` finished and does not contain `failed`

#### Scenario: Failed task with empty error
- **WHEN** a background task fails with empty error text
- **THEN** the queued `<task>` notification reports `failed`

### Requirement: Shell working directory resolves paths consistently
`sh(cmd, { cwd = path })` SHALL resolve `path` using the same rules as file host functions: absolute paths stay absolute, `~/…` expands to the user's home directory, and relative paths resolve against the session working directory.

#### Scenario: Home-relative shell working directory
- **WHEN** Lua runs `sh("pwd", { cwd = "~/" })`
- **THEN** the command executes in the user's home directory rather than failing on a literal `~` path

### Requirement: Helper llm calls use low effort when switching models
When Lua calls `llm(prompt, model, effort)`:
- If `model` is omitted or empty (or equals the session's current model) and `effort` is omitted or empty, `llm` SHALL use the session's current model and effort (validated against that model's resolved effort list, where empty effort `""` is valid if and only if the model declares `efforts = {}`).
- If `model` is explicitly provided and differs from the session's current model (`other_model`) and `effort` is omitted or empty:
  - When `other_model` declares explicitly empty `efforts = {}`, `llm` SHALL honor that declaration and send no reasoning parameter (`effort = None`).
  - Otherwise, `llm` SHALL use `low` (neither `"default"` nor the session's inherited effort), validated against `other_model`'s resolved effort list (`config.models[].efforts`, else `low`, `medium`, `high`, `xhigh`, `max`). If `other_model`'s non-empty declared `efforts` excludes `low`, `llm` MUST raise an error rather than silently violating the model's declared source of truth.
- If a non-empty `effort` is explicitly provided, `llm` SHALL validate it against the target model's resolved effort list, raising if that list is empty (`efforts = {}`) or excludes the effort (including retired `"default"`).

#### Scenario: Switching models with omitted effort uses low
- **WHEN** the session uses `openai/gpt-5` with `effort = "xhigh"` and Lua calls `llm("summarize", "gemini/gemini-2.5-flash")` (whose efforts are unset or include `low`)
- **THEN** the helper request runs with `effort = "low"` (not `"xhigh"` and not `"default"`)

#### Scenario: Switching to a no-reasoning model with omitted effort sends no reasoning
- **WHEN** `config.models` declares `{ name = "openai/gpt-4o", efforts = {} }` and Lua calls `llm("summarize", "openai/gpt-4o")` without an explicit effort
- **THEN** the helper request succeeds with no reasoning parameter sent on the wire
- **WHEN** Lua calls `llm("summarize", "openai/gpt-4o", "low")`
- **THEN** `llm` raises an error because `openai/gpt-4o` declares `efforts = {}`

#### Scenario: Target model allowlist excludes low
- **WHEN** `config.models` declares `{ name = "p/reasoner", efforts = { "medium", "high" } }` and Lua calls `llm("solve", "p/reasoner")` without an explicit effort
- **THEN** `llm` raises an error naming `p/reasoner` and its allowed efforts, while `llm("solve", "p/reasoner", "medium")` succeeds

#### Scenario: Omitted model inherits session effort
- **WHEN** the session uses `openai/gpt-5` with `effort = "high"` and Lua calls `llm("summarize")`
- **THEN** the helper request uses `openai/gpt-5` with `effort = "high"`

### Requirement: Lua task tables match JSON-RPC task fields
`task_list()`, `task_get(id, lines?)`, and `task_stop(id)` SHALL return task tables containing `{ id, name, status, started, ended, log, code }`, matching JSON-RPC `tasks.list`, `tasks.get`, and `tasks.stop`, with `task_get` additionally including `output`.

#### Scenario: Task log path exposed in Lua
- **WHEN** Lua spawns a background task and calls `task_list()`, `task_get(id)`, and `task_stop(id)`
- **THEN** every returned table includes `log` (the path to the task's `.log` file) and `code`, matching JSON-RPC `tasks.*`

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
`tuse` and `/new` SHALL create the session row at once with `started = false`. Recording the first `user` message—or replacing history via `set_session` with a list containing a `user` record—SHALL set `started = true` in the same transaction. `/resume` and `session.list` SHALL list only started sessions. Unstarted sessions are kept.

#### Scenario: Empty session hidden
- **WHEN** the user starts `tuse` and quits without a message
- **THEN** the session row exists with `started = false` and `/resume` does not list it

#### Scenario: First message
- **WHEN** the first user message is sent
- **THEN** `session().started` is `true` and the session shows in `/resume`

#### Scenario: History replacement starts session
- **WHEN** an unstarted session runs `set_session{ history = { { role = "user", text = "hi" } } }`
- **THEN** `session().started` becomes `true` and the session appears in `/resume` and `session.list`

### Requirement: Session read and write from Lua
`session()` SHALL return the unified `snake_case` session state matching JSON-RPC `session.state` (`mode`, `config_path`, `cwd`, `home`, `id`, `started`, `created`, `title`, `model`, `provider`, `effort`, `busy`, `turn`, `context`, `context_window`, `usage` with `input`/`output`/`cache_read`/`cache_write`/`cost`/`llm_cost`, `model_info` including resolved `efforts`, `requests`, `llm_requests`, `steps`, `turns`, `calls`) plus `history` (structured records with `role`, `summary`, and the record fields). During pre-session config loading only `mode`, `config_path`, `cwd`, and `home` are set. Retired aliases such as `name`, `configpath`, `ctx`, and `ctx_size` MUST NOT be returned. `set_session(t)` SHALL accept any subset of `title`, `model`, `effort`, `history`; any other key fails. It SHALL validate everything in Rust first and, on failure, raise without changing anything. Validation covers known roles and field types, a result for every tool call before the next `user` or `agent` record, data-URL images, `system` only first, a configured model, an effort permitted by the shared model effort resolver for that model (accepting omitted or `""` effort and rejecting any non-empty effort when the target model declares `efforts = {}`; rejecting retired `"default"`), and—when replacing `history` while a `fork(index)` switch is pending in the same tool step—ensuring that the pending fork index still points at a `user` or `agent` record in the new history. `history` replaces the whole list. `usage`, `id`, `cwd`, `started`, and `created` are read-only. `set_title` is removed.

#### Scenario: Unified session fields
- **WHEN** Lua calls `session()` on an active session titled `Fix parser`
- **THEN** `session().title == "Fix parser"`, `session().name == nil`, `session().context` and `session().usage.cache_read` are present, and all fields match JSON-RPC `session.state` plus `history`

#### Scenario: Round trip
- **WHEN** Lua runs `set_session{ history = session().history }`
- **THEN** the history is unchanged, including `replay`

#### Scenario: Model-scoped effort validation and no-reasoning models
- **WHEN** `config.models` declares `{ name = "p/m", efforts = { "low", "medium" } }` and Lua calls `set_session{ model = "p/m", effort = "high" }` or `set_session{ effort = "default" }`
- **THEN** `set_session` raises an error without changing session state
- **WHEN** `config.models` declares `{ name = "p/plain", efforts = {} }` and Lua calls `set_session{ model = "p/plain" }`
- **THEN** `session().model` becomes `"p/plain"` and `session().effort` becomes `""`, while `set_session{ model = "p/plain", effort = "low" }` raises an error

#### Scenario: Invalid history
- **WHEN** the new history has an `agent` tool call without a result
- **THEN** `set_session` raises an error and the stored history is unchanged

#### Scenario: Unknown key
- **WHEN** Lua runs `set_session{ usage = {} }`
- **THEN** it raises an error naming the key

#### Scenario: History replacement invalidating a pending fork
- **WHEN** a tool call runs `fork(4)` and then `set_session{ history = shorter }` where record 4 no longer exists or is not a `user` or `agent` record
- **THEN** `set_session` raises an error without mutating session state and the pending fork remains valid

### Requirement: History replacement waits for tool calls
When `set_session` gets `history` while tool calls of the current step run, the replacement SHALL be kept pending and applied after the last tool result of that step, before a pending session switch, before the next model request, and before any queued user message is recorded. Any `meta` record created by `set_session` (`title`, `model`, or `effort`) in the same call or later in the same tool step MUST survive when the pending history replacement is applied. With no running tool call it SHALL apply at once.

#### Scenario: Self-compaction from a tool call
- **WHEN** the agent's `lua` call runs `set_session{ history = short }`
- **THEN** the call's result is recorded first, then the history becomes `short`, and the next request is built from it

#### Scenario: Steering during the call
- **WHEN** the user sends a message while that call runs
- **THEN** the message is recorded after the replacement

#### Scenario: Meta change combined with deferred history replacement
- **WHEN** a tool call runs `set_session{ title = "Refactored", history = short }` (or calls `set_session{ history = short }` followed by `set_session{ title = "Refactored" }`)
- **THEN** after the tool step finishes, the replaced history retains the `meta` record with `title = "Refactored"`

### Requirement: Edited replies drop replay
When `set_session` replaces history, an `agent` record whose `text` or `tool_calls` differ from the stored record at the same position SHALL lose its `replay`. Unchanged records keep it.

#### Scenario: Edited reply
- **WHEN** Lua changes the `text` of a reply that has Anthropic thinking blocks
- **THEN** the next request sends the new text, not the old blocks

### Requirement: Hook events
Hook events SHALL be exactly: `config_load`, `process_start`, `process_end`, `session_created`, `session_resumed`, `session_close`, `session_updated`, `user_message`, `agent_message`, `turn_error`, `turn_stopped`, `lua_start`, `lua_end`. Vars SHALL hold `event` plus only data that `session()` lacks: `changed` for `session_updated`, `status` for `agent_message`, `error` for `turn_error`, and `id`, `kind`, `name`, `code`, `status`, `duration_ms`, `result`, `error` for `lua_start`/`lua_end`. `session_updated` SHALL fire only after `set_session` changes at least one field (`changed` is non-empty); a no-op `set_session` MUST NOT fire `session_updated`. `config_load` SHALL fire after every successful config load, after `system` is rebuilt. `process_start` SHALL fire once per process, in the TUI and in `tuse json`, and `process_end` only on quit or JSON-RPC `shutdown`. `session_close` SHALL fire before `/new` and `/resume`, not on quit. The Lua VM stays per session. Hooks SHALL run in the background: a turn, a reply, and a launch SHALL NOT wait for any hook. Hook runs SHALL NOT be stored in the database or history.

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

#### Scenario: No-op set_session
- **WHEN** Lua runs `set_session{}` or `set_session{ title = current_title }`
- **THEN** no `session_updated` hook fires

#### Scenario: Removed event
- **WHEN** a hook lists `name_change`
- **THEN** loading fails with the list of valid events

### Requirement: No built-in compaction
Tuse SHALL send the whole stored history to the model and MUST NOT prune, drop, or summarize it on its own. `compact_at` and the `compaction` role are removed.

#### Scenario: Long session
- **WHEN** `context` passes 90% of `context_window`
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
When a session switch started by Lua (`fork(index)` or `resume(id)`) applies in `protocolVersion: 2`, JSON-RPC SHALL send the event `session_switched` with `{ session, draft }` (where `session` is the lightweight `SessionState` object containing `model` and `effort`) after the remaining events of the old session and before the first event of the new one. `draft` is empty unless the switch was a fork at a `user` record. (`session.resume` is removed from JSON-RPC v2; `session.new` and `session.fork` reply directly with `{ session }` and `{ session, draft }`.)

#### Scenario: Lua switch
- **WHEN** a command runs `resume(id)` while a JSON-RPC v2 client is connected
- **THEN** the client gets `session_switched` with `{ session, draft }` holding the new session state, and later events belong to that session

### Requirement: One name per record kind
Each record kind SHALL use one name everywhere: the stored `role` in the database body, `session().history`, `set_session`, JSON-RPC `session.history`, and `history.hide`. Fields SHALL have the exact same names and flat structure in storage and in `session().history`; `session().history` only adds `summary`. In particular, `usage` records SHALL store flat top-level fields (`role`, `model`, `input`, `output`, `cache_read`, `cache_write`, optional `llm`) with no nested `usage` object and no `cached` alias, and deserialization MUST NOT retain a dual-shape legacy reader. The `tool` role and the tool name field are removed.

#### Scenario: Stored usage body matches history
- **WHEN** a model reply records usage
- **THEN** its database body has flat top-level `"role": "usage"`, `model`, `input`, `output`, `cache_read`, `cache_write` (no nested `usage` object and no `cached` key), and `session().history` has the exact same fields plus `summary`

#### Scenario: Stored lua body
- **WHEN** a `lua` call finishes
- **THEN** its database body has `"role": "lua"` with `id`, `code`, `output`, `exit`, and the same fields appear in `session().history`

#### Scenario: Legacy nested usage and old roles rejected
- **WHEN** Lua runs `set_session{ history = { { role = "usage", model = "p/m", usage = { input = 1, output = 1, cached = 0, cache_write = 0 } } } }` or uses `role = "tool"`, `title`, `flash`, `hook`
- **THEN** it raises an error and the history is unchanged

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
Opening a database with schema version below `4` SHALL execute inside an `IMMEDIATE` transaction: for version `0` it drops legacy tables and initializes schema `4`; for versions `1` through `3` it preserves `providers` (upgrading v1 `providers` with `headers` first if needed) and `state`, drops and recreates `sessions` (with `usage_cache_read`) and `history`, and sets `PRAGMA user_version = 4`. No migration of pre-v4 sessions or dual-shape history reader SHALL be kept. If preserved `state.effort` holds retired `"default"` or an effort disallowed for the saved model, session open SHALL recover via the model effort fallback rule (`""` for `efforts = {}`, else `"medium"` if allowed, else the first declared effort). Opening a database with `user_version > 4` MUST fail with an incompatible-schema-version error without modifying the database. Any SQLite error during schema initialization MUST roll back the transaction atomically and fail startup.

#### Scenario: Upgrade from schema v3 to v4
- **WHEN** a v3 database with saved sessions, history, providers, and `state.effort = "default"` is opened
- **THEN** `sessions` and `history` are recreated empty at schema version `4`, saved `providers` and `state` remain intact, and the new session resolves its effort cleanly via the model effort fallback rule

#### Scenario: Future schema version rejected
- **WHEN** a database with `PRAGMA user_version = 5` is opened
- **THEN** opening fails with an incompatible-schema error and does not drop or modify any table

#### Scenario: Failure rolls back atomically
- **WHEN** a database error occurs during v4 schema initialization
- **THEN** the `IMMEDIATE` transaction rolls back without leaving partial tables or bumping `user_version`
