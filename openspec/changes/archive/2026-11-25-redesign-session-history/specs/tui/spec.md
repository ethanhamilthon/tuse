## ADDED Requirements

### Requirement: History command
`/history` SHALL list the session history from the database, minus roles in `hide_history`; `flash` entries are listed like any other role. `trace` is no longer a built-in command and `history` is a reserved command name. JSON-RPC `session.trace` and `session.records` are replaced by `session.history`, which returns the same structured records as `session().history`, and `session.list` returns only started sessions.

#### Scenario: Open history
- **WHEN** the user types `/history`
- **THEN** a list of every history record opens, starting with `system`

#### Scenario: Old command
- **WHEN** the user types `/trace`
- **THEN** it is an unknown command

#### Scenario: Old method
- **WHEN** a client calls `session.trace` or `session.records`
- **THEN** it gets a method-not-found error

### Requirement: Resume lists started sessions
The `/resume` picker SHALL list only sessions of the current directory with `started = true`, newest first.

#### Scenario: Unstarted session
- **WHEN** a session was opened and closed without a message
- **THEN** `/resume` does not list it

### Requirement: TUI and JSON-RPC parity
The TUI and JSON-RPC SHALL offer the same features, one to one: sending, withdrawing, and stopping input (`turn.*`), the session panel and history (`session.snapshot`, `session.history`), new and resume (`session.new`, `session.list`, `session.resume`), models (`model.get`, `model.set`, `model.list`), config view and reload (`config.get`, `config.reload`), tasks (`tasks.*`), commands (`commands.*`), `pick`/`ask`/`show` (`ui.*`), and quit (`shutdown`). Neither SHALL have a way to edit history directly; `set_session` is a Lua host function only, reachable from both through commands, hooks, and agent `lua` calls.

#### Scenario: Edit through a command
- **WHEN** a config command calls `set_session{ history = ... }` and it is run with `/name` in the TUI or `commands.run` over JSON-RPC
- **THEN** both change the history the same way

#### Scenario: No direct write
- **WHEN** a client calls `session.set`
- **THEN** it gets a method-not-found error

#### Scenario: config.set removed
- **WHEN** a client calls `config.set`
- **THEN** it gets a method-not-found error; `model.set` covers model and effort
