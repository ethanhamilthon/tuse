## MODIFIED Requirements

### Requirement: Idle status shows the project directory
When no activity indicator or notice is shown, no turn is running, and no background task is running, the composer SHALL show the session working directory in the `muted` color without an icon, with the home directory written as `~`, in the place of the indicator in every input style. When background tasks are running, that place SHALL show the task indicator instead of the directory.

#### Scenario: Idle and working
- **WHEN** no turn is running, no background task is running, and no notice is shown
- **THEN** the status shows the directory
- **WHEN** a turn starts or a notice appears
- **THEN** the indicator or notice replaces the directory

#### Scenario: Idle with tasks
- **WHEN** no turn is running and two background tasks are running
- **THEN** the status shows an animated spinner and `2 tasks` in the accent color instead of the directory
- **WHEN** the last task ends
- **THEN** the directory returns

### Requirement: Show the foreground activity in the composer
Both input styles SHALL distinguish provider wait with the existing spinner, foreground Lua execution with the Lua block's animated star, and model streaming with a fixed-width dot that pulses through progressively larger and smaller glyphs. The elapsed timer SHALL remain available with the activity. While background tasks are running, the activity indicator SHALL be followed by ` · n tasks` in the accent color, using `task` for one, and the task count SHALL NOT add a second spinner. Only tasks whose status is `running` count. Streaming SHALL begin at the first displayable text, reasoning, or Lua code update; empty announcements alone MUST NOT activate the pulse or the Lua execution star. A pause between chunks MUST NOT switch the indicator back to waiting. Activity changes MUST NOT insert rows into the chat. The TUI SHALL preserve steering and retry notices and MUST NOT keep showing an old retry notice over a new attempt. Ending, stopping, or switching sessions MUST clear stale activity.

#### Scenario: Wait, stream, run Lua
- **WHEN** the model request is sent, the provider announces a tool without code, then sends displayable code, and the foreground Lua function starts after the reply
- **THEN** the spinner remains through the empty announcement, the dot pulses while code streams, and the indicator changes to a star only when Lua executes

#### Scenario: Reasoning and text arrive intermittently
- **WHEN** displayable reasoning or reply text arrives and the provider pauses mid-response
- **THEN** the pulse begins with the first displayed content and continues through the pause without changing the composer height

#### Scenario: Retry and stop
- **WHEN** a partial response is retried and the new attempt is then stopped
- **THEN** the retry notice replaces stale activity during the wait for retry, the provider spinner returns for the new attempt, and stopping clears its indicator

#### Scenario: Working with tasks
- **WHEN** a turn has run for 12 seconds and one background task is running
- **THEN** the status shows the activity indicator, `12s`, and `· 1 task` in the accent color, with only the one spinner of the activity

#### Scenario: Notice over tasks
- **WHEN** a notice or flash is shown while tasks are running
- **THEN** the notice replaces the task indicator until it expires

#### Scenario: Finished tasks
- **WHEN** a task has finished, failed, or been stopped
- **THEN** it is not counted and no task indicator is shown for it

#### Scenario: Switching sessions
- **WHEN** the user switches to a session that has no running tasks
- **THEN** no task indicator is shown
