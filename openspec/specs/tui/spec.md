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

### Requirement: Streaming renders incrementally
While a reply, reasoning, or `lua` tool call streams, the TUI SHALL re-render only the unfinished tail and reuse the lines of the finished part. The streamed view MUST equal a full render of the same text at every point. Once the reply ends, the entry SHALL be rendered once in full.

#### Scenario: Long list or code block
- **WHEN** the agent streams a long list, a long fenced code block, or long tool code
- **THEN** each frame renders only the last item or the last code lines, and the output matches a full render

#### Scenario: Path probes
- **WHEN** streamed text mentions file paths
- **THEN** each path is checked on disk at most once per second

### Requirement: Markdown images
A Markdown image `![alt](target)` SHALL render as a link to `target` with `alt` as its text, or with `target` as its text when `alt` is empty.

#### Scenario: Image in a reply
- **WHEN** a reply contains `![Photo](/tmp/a.png)`
- **THEN** the chat shows `Photo` as a clickable link that opens `/tmp/a.png`

### Requirement: Lua syntax highlighting
Lua code in tool calls, panels, and `lua` code blocks SHALL be colored by the `theme.lua` tokens using the code's structure. Function definition and call names SHALL be `function`, names after `:` SHALL be `method`, names after `.` and table keys SHALL be `field`, names in a function's parameter list and their uses inside that function SHALL be `parameter`, standard library and Tuse functions SHALL be `builtin`, and other names SHALL be `variable`. `local` and `function` SHALL be `storage`; `nil`, `true`, and `false` SHALL be `constant`; `and`, `or`, `not`, and symbol operators SHALL be `operator`; brackets, `,`, `;`, `.`, and `:` SHALL be `punctuation`; escapes inside strings SHALL be `escape`. Long strings and long comments SHALL continue across lines.

#### Scenario: Assignment and calls
- **WHEN** the code is `x = read("a"):upper() .. y`
- **THEN** `=` and `..` are `operator`, `read` is `builtin`, `upper` is `method`, and `x` and `y` are `variable`

#### Scenario: Parameters and table keys
- **WHEN** the code defines `function M.go(self, n) return n end` and a table `{ run = function() local y = 1 end }`
- **THEN** `go` is `function`, `self` and both `n` are `parameter`, `run` is `field`, and `y` is `variable`

### Requirement: User message color
User messages and the input text SHALL use `theme.user`; agent text SHALL use `fg`.

#### Scenario: Different colors
- **WHEN** `theme.user` differs from `theme.fg`
- **THEN** a user message and the draft in the input differ in color from an agent reply

### Requirement: Wake the TUI on streaming updates
The TUI SHALL wake when session events arrive, without waiting for terminal input or a periodic poll. The first displayable streaming update of each model attempt MUST be drawn at the next render opportunity, without waiting for a frame-rate timer, a later delta, or the final reply. Later updates MAY share a frame, but the last pending update MUST be drawn even if the stream pauses. Stream processing MUST preserve event order and leave opportunities to handle terminal input.

#### Scenario: First delta in an idle terminal
- **WHEN** a provider sends one displayable delta, then holds the connection open without sending another event, and the user does not press a key
- **THEN** the TUI draws the delta without waiting for the frame pacing deadline or the response to finish

#### Scenario: Burst followed by a pause
- **WHEN** many deltas arrive, the user presses a key, and then the provider pauses
- **THEN** the TUI handles input during the burst and renders the accumulated content in order, including the last delta, without waiting for another event

### Requirement: Show the foreground activity in the composer
Both input styles SHALL distinguish provider wait with the existing spinner, foreground Lua execution with the Lua block's animated star, and model streaming with a fixed-width dot that pulses through progressively larger and smaller glyphs. The elapsed timer SHALL remain available with the activity. Streaming SHALL begin at the first displayable text, reasoning, or Lua code update; empty announcements alone MUST NOT activate the pulse or the Lua execution star. A pause between chunks MUST NOT switch the indicator back to waiting. Activity changes MUST NOT insert rows into the chat. The TUI SHALL preserve steering and retry notices and MUST NOT keep showing an old retry notice over a new attempt. Ending, stopping, or switching sessions MUST clear stale activity.

#### Scenario: Wait, stream, run Lua
- **WHEN** the model request is sent, the provider announces a tool without code, then sends displayable code, and the foreground Lua function starts after the reply
- **THEN** the spinner remains through the empty announcement, the dot pulses while code streams, and the indicator changes to a star only when Lua executes

#### Scenario: Reasoning and text arrive intermittently
- **WHEN** displayable reasoning or reply text arrives and the provider pauses mid-response
- **THEN** the pulse begins with the first displayed content and continues through the pause without changing the composer height

#### Scenario: Retry and stop
- **WHEN** a partial response is retried and the new attempt is then stopped
- **THEN** the retry notice replaces stale activity during the wait for retry, the provider spinner returns for the new attempt, and stopping clears its indicator

### Requirement: Show the Lua preview only with code
The TUI MUST NOT reserve a chat row or show an inline Lua star for a tool announcement or whitespace-only code. The inline star and the first displayable Lua code MUST appear together in the same frame, whether the code first arrives in a delta or at tool start. A hidden preview discarded on reset, cancellation, or session switch MUST NOT flash on screen.

#### Scenario: Metadata and whitespace arrive before code
- **WHEN** the provider announces a Lua call, sends whitespace-only code, and then sends its first displayable code chunk
- **THEN** the announcement and whitespace create no chat row, gap, or star, and the inline star first appears beside code in the same frame

#### Scenario: No displayable code streams
- **WHEN** a Lua call has no displayable streaming delta but its tool start carries displayable code
- **THEN** the TUI shows the inline star and code together at tool start, without a prior placeholder

#### Scenario: Reset before code arrives
- **WHEN** the attempt is reset before a pending Lua preview gets displayable code
- **THEN** the pending preview is discarded without ever appearing in chat

### Requirement: Boxed input composer
When `settings.ui.input_style` is `boxed` and the viewport fits, the composer SHALL consist of a top border, the editor rows, one blank row, a status row, and a bottom border. Editor rows SHALL have no prompt arrow and no left padding, so the first column inside the border is shared by the editor text and the status row. The editor SHALL grow with the draft, limited by `input_max_height` and the viewport, and scroll beyond that with the cursor visible. Exactly one blank row SHALL separate the last editor row from the status row. Side borders SHALL enclose all interior rows. The status row SHALL place activity and notices on the left and the model/context label on the right; labels MUST be clipped rather than overlap. The lines composer SHALL be used when the box cannot fit, without changing the configured style or losing the draft. All styles SHALL retain editing, completion, mouse and selection behavior.

#### Scenario: Boxed composer with a multiline draft
- **WHEN** boxed input is configured in a terminal with sufficient space and the draft wraps to more than one row
- **THEN** the box grows by one row per draft row, one blank row stays above the status row, and the status row keeps activity on the left and model/context on the right

#### Scenario: Resize or reload
- **WHEN** the terminal becomes too small for the box, grows again, or `/reload` changes the input style
- **THEN** the layout and input hit testing update together, and the draft and logical cursor position are preserved

### Requirement: Lines input composer
When `settings.ui.input_style` is `lines`, or a framed style does not fit, the composer SHALL be a rule with the activity on its right, the editor rows with the prompt arrow on the first row, and a rule with the model/context label on its right.

#### Scenario: Small terminal
- **WHEN** the style is `boxed` or `fill` and the terminal is too small for the frame
- **THEN** the lines composer is drawn and the configured style is unchanged

### Requirement: Fill input composer
When `settings.ui.input_style` is `fill` and the viewport fits, the composer SHALL draw one blank row, the editor rows, and one blank row, all with the `border` color as background across the full width. The first editor row SHALL start with the prompt arrow. One row without background SHALL follow, then the status row with the activity on the left and the model/context label on the right.

#### Scenario: Fill composer with a multiline draft
- **WHEN** fill input is configured in a terminal with sufficient space and the draft wraps to more than one row
- **THEN** the filled block grows by one row per draft row, one unfilled row separates it from the status row, and the cursor and mouse hit testing match the editor rows

### Requirement: Idle status shows the project directory
When no activity indicator or notice is shown and no turn is running, the composer SHALL show the session working directory in the `muted` color without an icon, with the home directory written as `~`, in the place of the indicator in every input style.

#### Scenario: Idle and working
- **WHEN** no turn is running and no notice is shown
- **THEN** the status shows the directory
- **WHEN** a turn starts or a notice appears
- **THEN** the indicator or notice replaces the directory
