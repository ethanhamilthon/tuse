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

### Requirement: Task notices wait for the user
A task that ends on its own SHALL NOT start a turn. Its `<task>` notice SHALL be queued and sent with the next user message, as `notification` records placed before that `user` record. A turn that is already running still takes the notice at its next step.

#### Scenario: Idle session
- **WHEN** a task finishes while the session is idle
- **THEN** no turn starts, and the next user message carries the notice

### Requirement: Complete trace
The trace SHALL show everything that happens in the session, one role each: `system`, `user`, `thinking`, `agent`, `lua`, `output`, `notification`, `notice`, `error`, `hook`, `task`, `command`, `open`, `compaction`, `usage`, `model`, `turn`, `title`, `show`, `pick`, `ask`. `task`, `command`, and `open` entries SHALL hold name, status, duration, error, and code. Adding a record kind without a trace entry MUST NOT compile.

#### Scenario: Command run
- **WHEN** a custom command `/hello` finishes
- **THEN** the trace has a `command` entry `hello success (<ms> ms)`

#### Scenario: Task run
- **WHEN** a task `tests` fails
- **THEN** the trace has a `task` entry with `failed`, the error, and the function source

#### Scenario: Model and title
- **WHEN** the user picks a new model and the title changes
- **THEN** the trace has a `model` entry with model and effort and a `title` entry with the new title
