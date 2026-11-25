# tui

## Purpose
Terminal UI behavior.

## Requirements

1

### Requirement: Task details
Enter on a task in `/tasks` SHALL open a panel with id, name, status, started, ended, elapsed time (live while running), the source of the task's function, and the last 200 log lines. When the source is unknown it shows `(source unavailable)`. `task_get`, `tasks.list`, and `tasks.get` SHALL include `code`.

#### Scenario: Running task
- **WHEN** the agent ran `spawn(function() sh("sleep 60") end, "nap")` and the user presses Enter on it
- **THEN** the panel shows `nap`, `running`, the elapsed time, and `function() sh("sleep 60") end`

### Requirement: Prompt arg input
Picking a prompt with `args` in the `#` picker SHALL insert the prompt atom and open a picker of its `args`. A space typed right after such an atom SHALL reopen that picker, filtered by the typed text. The chosen arg joins the atom, as `#openspec propose`. Esc leaves the atom without an arg. The prompt name and its arg SHALL be drawn in the accent color.

#### Scenario: Pick an arg
- **WHEN** the user picks `#openspec`, then `propose`
- **THEN** the input holds one atom `#openspec propose` in the accent color

#### Scenario: Type an arg
- **WHEN** after the `#openspec` atom the user types ` pro` and presses Enter
- **THEN** the atom becomes `#openspec propose`

### Requirement: Stop command
`/stop` SHALL be a built-in command that stops everything, the same as Esc, and works when the session is idle. `stop` is a reserved command name.

#### Scenario: Idle stop
- **WHEN** no turn runs, two tasks run, and the user types `/stop`
- **THEN** both tasks are `stopped`

### Requirement: Chat scroll follows only at the bottom
The chat SHALL stick to the newest content only while the view is at the bottom. When the user has scrolled up, new messages, streamed text, tool output, and resizes MUST NOT move the visible lines. Scrolling back to the bottom (wheel, `PageDown`, or sending a message) SHALL turn following back on.

#### Scenario: At the bottom
- **WHEN** the view is at the bottom and the agent streams text
- **THEN** the view keeps showing the newest line

#### Scenario: Scrolled up
- **WHEN** the user scrolled up and new messages arrive
- **THEN** the same lines stay on screen

#### Scenario: Back to the bottom
- **WHEN** the user scrolls down to the end
- **THEN** new content is followed again

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
