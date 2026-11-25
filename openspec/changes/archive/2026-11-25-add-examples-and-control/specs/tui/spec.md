## ADDED Requirements

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
