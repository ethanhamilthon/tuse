## MODIFIED Requirements

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

## ADDED Requirements

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
