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
`/history` SHALL list the session history from the database, minus roles and message levels in `history.hide` and minus `agent` entries with no text and no reasoning. `trace` is no longer a built-in command and `history` is a reserved command name. JSON-RPC `session.trace` and `session.records` are replaced by `session.history`, which returns the same structured records as `session().history`, and `session.list` returns only started sessions.

#### Scenario: Open history
- **WHEN** the user types `/history`
- **THEN** a list of every history record opens, starting with `system`

#### Scenario: Tool-only reply hidden
- **WHEN** the agent replied with only a `lua` call and the user opens `/history`
- **THEN** only the `lua` entry with the code is listed for that call

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
Lua code in tool calls, panels, and `lua` code blocks SHALL be colored by the `ui.theme.code` tokens using the code's structure. Function definition and call names SHALL be `function`, names after `:` SHALL be `method`, names after `.` and table keys SHALL be `field`, names in a function's parameter list and their uses inside that function SHALL be `parameter`, standard library and Tuse functions SHALL be `builtin`, and other names SHALL be `variable`. `local` and `function` SHALL be `storage`; `nil`, `true`, and `false` SHALL be `constant`; `and`, `or`, `not`, and symbol operators SHALL be `operator`; brackets, `,`, `;`, `.`, and `:` SHALL be `punctuation`; escapes inside strings SHALL be `escape`. Long strings and long comments SHALL continue across lines.

#### Scenario: Assignment and calls
- **WHEN** the code is `x = read("a"):upper() .. y`
- **THEN** `=` and `..` are `operator`, `read` is `builtin`, `upper` is `method`, and `x` and `y` are `variable`

#### Scenario: Parameters and table keys
- **WHEN** the code defines `function M.go(self, n) return n end` and a table `{ run = function() local y = 1 end }`
- **THEN** `go` is `function`, `self` and both `n` are `parameter`, `run` is `field`, and `y` is `variable`

### Requirement: User message color
User messages SHALL use `ui.theme.user` on `ui.theme.user_bg`; the input text SHALL use `ui.theme.input`; agent text SHALL use `fg`.

#### Scenario: Different colors
- **WHEN** `ui.theme.user` differs from `ui.theme.fg`
- **THEN** a user message differs in color from an agent reply
- **WHEN** `ui.theme.input` differs from `ui.theme.user`
- **THEN** the draft in the input differs in color from a sent user message

### Requirement: Wake the TUI on streaming updates
The TUI SHALL wake when session events arrive, without waiting for terminal input or a periodic poll. The first displayable streaming update of each model attempt MUST be drawn at the next render opportunity, without waiting for a frame-rate timer, a later delta, or the final reply. Later updates MAY share a frame, but the last pending update MUST be drawn even if the stream pauses. Stream processing MUST preserve event order and leave opportunities to handle terminal input.

#### Scenario: First delta in an idle terminal
- **WHEN** a provider sends one displayable delta, then holds the connection open without sending another event, and the user does not press a key
- **THEN** the TUI draws the delta without waiting for the frame pacing deadline or the response to finish

#### Scenario: Burst followed by a pause
- **WHEN** many deltas arrive, the user presses a key, and then the provider pauses
- **THEN** the TUI handles input during the burst and renders the accumulated content in order, including the last delta, without waiting for another event

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
When `ui.input` is `boxed` and the viewport fits, the composer SHALL consist of a top border, the editor rows, one blank row, a status row, and a bottom border. Editor rows SHALL have no prompt arrow. Every editor row and the status row SHALL reserve exactly one blank display column immediately inside each side border. The padding SHALL be inside the existing frame width and separate from the outer `ui.space.padding` margin. The editor SHALL use the remaining content width for wrapping, cursor placement, mouse hit testing, and selection. It SHALL grow with the draft, limited by `ui.lines.input` and the viewport, and scroll beyond that with the cursor visible. Exactly one blank row SHALL separate the last editor row from the status row. Side borders SHALL enclose all interior rows and use the `input_bg` color. The status row SHALL place activity and notices at the left edge of the content area and the model/context label at its right edge; labels MUST be clipped to fit without overlapping or using the padding. The box SHALL require room for the two side borders, the two padding columns, and at least two content columns after accounting for `ui.space.padding`. The `lines` composer SHALL be used when the box cannot fit, without changing the configured style or losing the draft. All styles SHALL retain editing, completion, mouse and selection behavior.

#### Scenario: Boxed composer with a multiline draft
- **WHEN** boxed input is configured in a terminal with sufficient space and the draft wraps to more than one row at the padded content width
- **THEN** every editor row keeps one blank column just inside each side border, including when the draft fills the content width
- **AND** the box grows by one row per draft row within its height limits, and one blank row stays above the status row
- **AND** the cursor and mouse selection align with the inset text on the wrapped rows

#### Scenario: Long status labels
- **WHEN** the left status and right model/context labels together exceed the content width in a visible box
- **THEN** the labels are clipped within the content area without overlapping, and the column next to each side border stays blank

#### Scenario: Resize or reload
- **WHEN** the terminal becomes too small for the padded box, grows again, or `/reload` changes the input style
- **THEN** insufficient space uses `lines`, and the padded box returns when it fits again if `boxed` remains configured
- **AND** layout and input hit testing update together, preserving the draft and logical cursor position

### Requirement: Lines input composer
When `ui.input` is `lines`, or a framed style does not fit, the composer SHALL be a rule with the activity on its right, the editor rows with the prompt arrow on the first row, and a rule with the model/context label on its right.

#### Scenario: Small terminal
- **WHEN** the style is `boxed` or `fill` and the terminal is too small for the frame
- **THEN** the lines composer is drawn and the configured style is unchanged

### Requirement: Fill input composer
When `ui.input` is `fill` and the viewport fits, the composer SHALL draw one blank row, the editor rows, and one blank row, all with the `input_bg` color as background across the full width. The first editor row SHALL start with the prompt arrow. One row without background SHALL follow, then the status row with the activity on the left and the model/context label on the right. While a picker, question, or panel is open, its rows and one border rule row SHALL take the place of that unfilled row.

#### Scenario: Fill composer with a multiline draft
- **WHEN** fill input is configured in a terminal with sufficient space and the draft wraps to more than one row
- **THEN** the filled block grows by one row per draft row, one unfilled row separates it from the status row, and the cursor and mouse hit testing match the editor rows

#### Scenario: Fill composer with an open list
- **WHEN** fill input is configured and a picker is open
- **THEN** the list is drawn between the filled block and the status row, and one border rule row closes it

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

### Requirement: Lists open below the input
A picker, question, or panel SHALL open below the input, never between the chat and the input. With `lines` and `boxed` its rows SHALL follow the last row of the composer. With `fill` and `bar` its rows SHALL sit between the editor block and the status row, followed by one row drawn as a rule in the `border` color. Its height SHALL stay limited by `list_max_height` and the viewport, and the chat SHALL keep the rows that remain. The wheel over the list and paging SHALL use the rows where the list is drawn.

#### Scenario: Picker with boxed input
- **WHEN** the style is `boxed` and the user types `/`
- **THEN** the command list is drawn directly under the bottom border of the box and the chat above is not covered

#### Scenario: Picker with fill input
- **WHEN** the style is `fill` and the user types `/`
- **THEN** the order from the top is the filled block, the command list, one border rule row, and the status row

### Requirement: Blank row above the composer
Every input style SHALL draw one empty row above the first row of the composer: the top rule, the top border, the filled block, or the first bar row. The row holds no text and no color other than the terminal background. It MUST NOT count toward `input_max_height`. When the viewport is too small, this row SHALL be the first one dropped.

#### Scenario: All styles
- **WHEN** the style is `lines`, `boxed`, `fill`, or `bar`
- **THEN** one blank row separates the chat from the composer

#### Scenario: Tiny terminal
- **WHEN** the viewport has no room for the blank row
- **THEN** the composer is drawn without it and the draft is kept

### Requirement: Bar input composer
When `ui.input` is `bar` and the viewport fits, the composer SHALL draw each editor row as `▌` in the `icons` color at column 0 with no padding before it, one space, then the text, without a prompt arrow. The height SHALL follow the draft, limited by `ui.lines.input` and the viewport, and scroll beyond that with the cursor visible. One blank row SHALL follow the editor rows, then the status row as in `fill`, with the activity and notices on the left and the model/context label on the right. The lines composer SHALL be used when the bar cannot fit, without changing the configured style or losing the draft.

#### Scenario: Multiline draft
- **WHEN** bar input is configured and the draft wraps to three rows
- **THEN** three rows each start with `▌ `, one blank row follows, and then the status row

#### Scenario: Small terminal
- **WHEN** the terminal is too small for the bar
- **THEN** the lines composer is drawn and the configured style is unchanged

### Requirement: Token picker matching
The `/` and `#` pickers, and the arg pickers of a command or prompt, SHALL list only the items whose name starts with the typed text, ignoring case. The `/` or `#` mark and the description MUST NOT be searched. An empty query SHALL list every item. Items keep the order of the list. The `@` picker SHALL list the files and folders whose name contains the typed text, ignoring case, with the names that start with it first. Inside each group, and for an empty query, the `@` picker SHALL order items by the last modified time, newest first, using the created time when the modified time is missing. The directory listing SHALL be sorted before the picker size cap, so the newest entries are kept.

#### Scenario: Prompt prefix
- **WHEN** the prompts are `imagegen`, `openspec`, `release`, `srt`, and `use-figma` and the user types `#r`
- **THEN** the picker lists only `#release`

#### Scenario: Description is not searched
- **WHEN** the user types `/r` and `/config` has the description `show the resolved config`
- **THEN** `/config` is not listed, and `/resume`, `/reload` are

#### Scenario: Case is ignored
- **WHEN** the user types `#REL`
- **THEN** the picker lists `#release`

#### Scenario: Arg prefix
- **WHEN** the user types `#openspec ` followed by `pr`
- **THEN** the picker lists only `propose`

#### Scenario: File substring with prefix first
- **WHEN** the folder holds `main.rs` (older), `domain.rs` (newest), and `mod.rs`, and the user types `@ma`
- **THEN** the picker lists `main.rs` first, then `domain.rs`

#### Scenario: Newest files first
- **WHEN** the user types `@` and `old.txt` was modified before `new.txt` and the folder `docs/`
- **THEN** the picker lists the most recently modified entry first

#### Scenario: Large folder
- **WHEN** a folder holds more entries than the picker cap
- **THEN** the picker keeps the newest entries, not the first by name

### Requirement: Picker columns are aligned
A picker list SHALL draw each item as columns. Every cell of a column SHALL start at the same display column on every row, padded to the widest cell of that column among all items of the picker, with two spaces between columns. The last column is free text clipped at the row end. A column wider than half of the row width SHALL be clipped with the ellipsis so the next column keeps its room. `/history` rows SHALL have the columns index, role, and summary. Items with a label and a detail SHALL align the detail as the second column. Search in the `/history`, `/resume`, `/fork`, `/models`, and `/tasks` pickers SHALL match the cells joined by a space. The `/`, `#`, `@`, and arg pickers follow the token picker matching requirement.

#### Scenario: History rows
- **WHEN** `/history` lists records 0 to 14
- **THEN** the roles start at one column and the summaries at one column, for example `0   system  …` and `10  hook    …`

#### Scenario: Command list
- **WHEN** the user types `/`
- **THEN** every command description starts at the same column

#### Scenario: Scrolling and filtering
- **WHEN** the list scrolls or the search narrows it
- **THEN** the column positions stay the same

#### Scenario: Narrow terminal
- **WHEN** the first column is wider than half of the row
- **THEN** it is clipped with the ellipsis and the second column still starts at one column on every row

#### Scenario: Row search
- **WHEN** the user types a word that appears only in the summary of a `/history` row
- **THEN** that row is listed

### Requirement: Config models are aligned
The `Models` section of the `/config` panel SHALL pad every model name to the widest name in the section and start the details (`display=…`, `window=…`, `prices=…`) at one column after two spaces. The other sections of `/config` and the `/session` panel keep their current alignment.

#### Scenario: Models of different length
- **WHEN** the config has `openai/gpt-6.2-sol` and `claude/claude-sonnet-5-5`
- **THEN** both `window=…` details start at the same column

### Requirement: Command descriptions and words
The `/` picker SHALL show the `desc` of a custom command as its detail. Picking a command with `args` SHALL put `/name ` in the input and open a picker of its `args`; a space typed after `/name` SHALL reopen that picker, filtered by the typed text. Choosing an arg SHALL clear the input and run the command with it. Esc in that picker SHALL leave `/name ` in the input, and Enter then SHALL run the command with no arg. Enter on `/name` followed by words SHALL run the command with those words as the config spec describes, and a word list the command does not accept MUST NOT run it and MUST NOT be sent as a message. Enter on the exact `/name`, or picking a command without `args` from the `/` picker, SHALL run it with no words. JSON-RPC `commands.run` SHALL accept `{name, argv?}` with a list of words, apply the same checks, and fail with `-32000` when they fail; `commands.list` SHALL return `[{name, desc, args, params}]`.

#### Scenario: Pick an arg
- **WHEN** the user picks `/deploy` and then `prod`
- **THEN** `run("prod")` is called and the input is cleared

#### Scenario: Esc then Enter
- **WHEN** the user picks `/deploy`, presses Esc in the arg picker, and presses Enter
- **THEN** `run(nil)` is called

#### Scenario: Type words
- **WHEN** the user types `/mv a.txt b.txt` and presses Enter
- **THEN** `run("a.txt", "b.txt")` is called and the text is not sent to the agent

#### Scenario: Rejected words
- **WHEN** the user sends `/deploy nope`
- **THEN** nothing runs and an error naming the command is shown

#### Scenario: Description in the picker
- **WHEN** the user types `/`
- **THEN** `/deploy` is listed with `ship it` as its description

#### Scenario: JSON-RPC
- **WHEN** a client calls `commands.run` with `{"name": "mv", "argv": ["a.txt", "b.txt"]}`
- **THEN** `run("a.txt", "b.txt")` is called; `{"name": "deploy", "argv": ["nope"]}` fails with `-32000`

### Requirement: Frames stay in sync with the terminal
The TUI SHALL write each frame with a single write. It SHALL turn the terminal's automatic line wrap off while it runs and on again when it exits or panics, so text that overflows a row is clipped and MUST NOT scroll the screen. It SHALL repaint every row, not only the changed ones, at least every five seconds.

#### Scenario: One write per frame
- **WHEN** the TUI draws a frame
- **THEN** the whole frame, from the synchronized-update start to its end, reaches the output in one write

#### Scenario: Stray text
- **WHEN** another writer leaves text on a row that the renderer considers unchanged
- **THEN** the row is repainted within five seconds

#### Scenario: Line wrap
- **WHEN** the TUI starts
- **THEN** it sends the sequence that turns line wrap off, and it sends the one that turns it on when it exits or panics

### Requirement: Aligned chat entry text
User and agent message rows SHALL reserve two display columns on each side for padding, inside the chat width and separate from the outer `pad_x` margin. Messages SHALL wrap at the remaining content width. The top-level text of every chat entry SHALL start at the same left content column after any entry marker, on both first and continuation rows. This applies to messages, reasoning, Lua code and output, diff blocks, notices, errors, turn summaries, pinned and unpinned `show()` blocks, and recorded picker and question results. Intentional indentation within an entry, such as Markdown nesting or code indentation, SHALL be preserved. When the chat is too narrow to fit the padding and content, the padding SHALL yield as needed without overflowing the row.

#### Scenario: Mixed chat history
- **WHEN** the chat shows user and agent messages alongside Lua blocks, events, `show()` blocks, and picker or question results
- **THEN** their top-level text starts two display columns past the outer left margin, with any entry marker in those two columns
- **AND** user and agent messages leave two columns of right padding even when a row fills the available content width
- **AND** wrapped lines keep the same left inset, while nested lists and indented code keep their internal indentation

#### Scenario: Narrow terminal
- **WHEN** the chat width is too small for both two-column insets and message text
- **THEN** the insets shrink so the row stays within the viewport
- **AND** widening the terminal restores the two-column insets

#### Scenario: Restore history
- **WHEN** a session is reopened
- **THEN** restored entries use the same text alignment and wrapping as live entries at the same width

### Requirement: Selection is limited to one text target
Ctrl+A and Cmd+A SHALL clear any chat or panel selection and select the entire editable input, including draft lines outside the viewport. Mouse selection SHALL be limited to the input, a single user or agent message, a single `show()` block (pinned or not), a single `pick` or `ask` result in the chat, or the open panel (`/config`, `/session`, a `/history` entry, task details, and the Lua `pick` and `ask` panels, including their question text and choices). A drag moving outside its starting target SHALL stop at that target's boundary and MUST NOT add text from another target. Starting a new selection SHALL replace the previous one. Prompt atoms in user messages SHALL be selectable as their displayed text. The marker icon and the padding of a `show()` block or a `pick` or `ask` result MUST NOT be selected or copied. Reasoning, Lua tool blocks, event entries, built-in pickers, and layout decorations MUST NOT be selectable. The highlight SHALL match the selected text rather than the whole screen row. Closing a panel or stepping to another entry SHALL clear the panel selection. Scrolling clears the selection.

#### Scenario: Select all with a message selected
- **WHEN** a chat message is selected and the user presses Ctrl+A or Cmd+A with an editable multiline draft
- **THEN** the chat selection clears and only the entire draft is selected, including any hidden editor lines

#### Scenario: Drag across messages or into the input
- **WHEN** the user starts a drag in an agent message and drags through a different message, a Lua block, a UI result, or the input
- **THEN** only the range within the original agent message is highlighted, in either drag direction

#### Scenario: Drag out of the input
- **WHEN** the user starts a drag in the input and moves into the chat or a panel
- **THEN** the selection stays in the input and no other text is highlighted

#### Scenario: Select in a panel
- **WHEN** `/config` is open and the user drags across several lines of it
- **THEN** only that text is highlighted, and Ctrl+C copies it

#### Scenario: Drag out of a panel
- **WHEN** the user starts a drag in a panel and moves into the chat or the input
- **THEN** the selection stays in the panel and no other text is highlighted

#### Scenario: Select in a Lua question
- **WHEN** a Lua `pick` or `ask` panel is open and the user drags across its question text or choices
- **THEN** that text is highlighted and Ctrl+C copies it, and nothing is picked

#### Scenario: Link click in a panel
- **WHEN** the user clicks a URL or file path in a panel without dragging
- **THEN** it opens as before and nothing is selected

#### Scenario: Prompt atom in a message
- **WHEN** the user selects a user message that contains the atom `#openspec propose`
- **THEN** the atom is highlighted and copies as `#openspec propose`

#### Scenario: Select all with a panel selected
- **WHEN** a panel text is selected and the user presses Ctrl+A or Cmd+A
- **THEN** the panel selection clears and only the entire draft is selected

#### Scenario: Select a show block
- **WHEN** Lua calls `show("report", "line one\n\nline two")` and the user drags across the block and presses Ctrl+C
- **THEN** only the block text is highlighted and the clipboard holds `line one\n\nline two`, without the marker icon or padding

#### Scenario: Select a pinned show block
- **WHEN** a pinned `show()` block sits at the end of the chat and the user drags across it
- **THEN** its text is highlighted and copies the same way as an unpinned block

#### Scenario: Select a pick result
- **WHEN** the chat shows the result of `pick(options, title)` and the user drags across it
- **THEN** the displayed question and answer are highlighted and copy as displayed, without the marker icon

#### Scenario: Drag between a show block and a message
- **WHEN** the user starts a drag in a `show()` block and moves into an agent message, or starts in the message and moves into the block
- **THEN** only the range inside the starting target is highlighted

#### Scenario: Drag from non-selectable UI
- **WHEN** the user starts a selection drag on reasoning, Lua output, an event, or a built-in picker and moves into a message, a `show()` block, or a panel
- **THEN** neither that UI text nor the target is selected

### Requirement: Copy logical text without layout whitespace
Ctrl+C and Cmd+C with a selection SHALL copy only the selected text. Chat and panel selections SHALL copy plain text as displayed, without styles, layout padding, message gaps, or newlines introduced solely by visual wrapping. A soft wrap SHALL preserve the original word separation without inserting a space inside a wrapped word. Logical line breaks, paragraph breaks, and intentional indentation within the selected range SHALL be preserved. Editor selections SHALL copy the exact selected draft text, unaffected by wrapping or composer style.

#### Scenario: Copy a visually wrapped range
- **WHEN** the user selects a message or panel paragraph that wraps at a space and also wraps a long word across rows
- **THEN** copying produces one logical line with the original word separation and no extra whitespace inside the long word
- **AND** the same logical range copies identically at a different terminal width

#### Scenario: Copy intentionally multiline text
- **WHEN** the user selects across a paragraph break or across indented lines of a code block in an agent message or panel
- **THEN** copying preserves the paragraph break or code line breaks and indentation, without adding leading or trailing padding from the highlighted rows

#### Scenario: Copy a draft
- **WHEN** the user selects a wrapped draft containing explicit newlines, spaces, or indentation
- **THEN** copying returns exactly that part of the draft, without the composer prompt, borders, or visual wraps

### Requirement: Lua picker questions
Lua `pick(options, title)` SHALL accept an optional Markdown question as its second argument. A missing, `nil`, empty, or whitespace-only title SHALL be treated as untitled. The TUI SHALL render a nonblank title above the choices with the same Markdown and link treatment as `ask`, without a text input. Only the choices SHALL be selectable. `pick` SHALL still return the selected option string unchanged, or `nil` on cancellation or an empty options list. Built-in pickers SHALL be unaffected.

#### Scenario: Ask a multiple-choice question
- **WHEN** Lua calls `pick({"Retry", "Skip"}, "**Build failed.**\n\nWhat next?")`
- **THEN** the question is rendered as Markdown above the two choices, there is no answer editor, and selecting `Retry` returns `"Retry"`

#### Scenario: Untitled picker
- **WHEN** Lua calls `pick({"Retry", "Skip"})` or passes `nil` or a blank title
- **THEN** the choices appear without a question or an extra row

#### Scenario: Empty options or cancellation
- **WHEN** Lua passes an empty options list, or the user cancels an active titled picker
- **THEN** `pick` returns `nil`, and an empty options list does not open a picker

### Requirement: Titled picker layout and navigation
The title and choices SHALL share the picker overlay height budget. The title SHALL stay above the choices when they scroll. If the whole title does not fit, the TUI SHALL display only the leading title rows that fit while reserving at least one choice row whenever the overlay is visible. Paging and choice scrolling SHALL use the available choice rows, not the combined height.

#### Scenario: One available row
- **WHEN** only one picker row fits below the composer
- **THEN** the selected choice is visible and the title is hidden

#### Scenario: Paging beneath the question
- **WHEN** a multi-line title wraps and the user pages through a long list of choices
- **THEN** the visible title rows stay fixed and the page size is based only on the visible choice rows

### Requirement: JSON-RPC picker questions
An active titled picker SHALL include the original Markdown source in a `title` string in both the `ui_request` event and `ui.state.active`. Untitled pickers SHALL omit `title`. Replies SHALL continue to accept only an offered option string.

#### Scenario: Observe and answer a titled picker remotely
- **WHEN** a titled picker becomes active over JSON-RPC and a client calls `ui.state`
- **THEN** both the event and the active snapshot include the same Markdown `title` and unchanged `options`, and `ui.reply` still validates the answer against `options`

#### Scenario: Untitled request shape
- **WHEN** an untitled picker becomes active over JSON-RPC
- **THEN** neither the request event nor the active snapshot includes `title`

### Requirement: Fork command
`/fork` SHALL open a picker of the `user` and `agent` messages of the current session, newest first, each with its role and a one-line preview, except `agent` messages with no text and no reasoning. Picking one SHALL open a new session forked at that message. For an agent message the history ends with that message. For a user message the history ends just before it, and its text is placed in the input as the draft. With no such messages, `/fork` SHALL do nothing and show a notice. `fork` is a reserved command name. JSON-RPC `session.fork` SHALL do the same for a `session.history` index, counted from 0, and return the `draft` text (empty for an agent message), and an index that is not a `user` or `agent` record returns an error.

#### Scenario: Fork at a user message
- **WHEN** the user types `/fork` and picks an earlier user message
- **THEN** a new session opens in the same folder whose history ends just before that message
- **AND** the text of that message is in the input
- **AND** the original session is unchanged and listed in `/resume`

#### Scenario: Fork at an agent message
- **WHEN** the user picks an agent message with text that made tool calls
- **THEN** the new history ends with that message and the results of its tool calls

#### Scenario: Tool-only reply
- **WHEN** the agent replied with only a `lua` call and the user opens `/fork`
- **THEN** that reply is not listed

#### Scenario: Empty session
- **WHEN** the session has no user or agent messages and the user types `/fork`
- **THEN** no picker opens and a notice says there is nothing to fork

#### Scenario: Bad index
- **WHEN** a client calls `session.fork` with the index of a `system` record
- **THEN** it gets an error and no session is created

### Requirement: Folded Lua strings
A multiline Lua long string in a tool call SHALL be shown with at most `ui.lines.strings` lines. With `0` it SHALL be shown as `…` alone. With `N > 0` it SHALL show its first N lines and end with `…` when it has more. With `-1` it SHALL show every line.

#### Scenario: Default
- **WHEN** a tool call holds a long string of three lines and `ui.lines.strings` is `1`
- **THEN** the first line followed by `…` is shown

#### Scenario: Hidden
- **WHEN** `ui.lines.strings` is `0`
- **THEN** the string is shown as `…`

### Requirement: Themed colors
The TUI SHALL take colors only from `ui.theme`. The prompt marker, spinners, the inline Lua star, and the bar mark SHALL use `icons`. The input text SHALL use `input`, and the input background or frame SHALL use `input_bg`. User messages SHALL use `user` on `user_bg`. Pickers, panels, and rules SHALL use `border`. Error text SHALL use `error`. Diff lines SHALL use `code.added`, `code.removed`, `muted` for context, and `border` for headers. Markdown SHALL use `markdown.heading`, `markdown.link`, `markdown.code`, `markdown.quote`, and `markdown.list`; hovered links use `fg`.

#### Scenario: Icon color
- **WHEN** `ui.theme.icons = "#FF0000"`
- **THEN** the spinner, the prompt marker, and the inline Lua star are drawn in `#FF0000`

#### Scenario: Markdown tokens
- **WHEN** `ui.theme.markdown.heading` differs from `fg` and an agent reply has a heading
- **THEN** the heading uses that color

#### Scenario: Diff colors
- **WHEN** a diff has an added line, a removed line, and a context line
- **THEN** they use `code.added`, `code.removed`, and `muted`

### Requirement: Spacing between entries
Blank rows between consecutive tool calls and reasoning blocks SHALL equal `ui.space.steps`. Blank rows above and below user and agent messages SHALL equal `ui.space.messages`. Side padding SHALL equal `ui.space.padding`.

#### Scenario: Steps gap
- **WHEN** `ui.space.steps = 2` and two tool calls follow each other
- **THEN** two blank rows separate them

#### Scenario: Messages gap
- **WHEN** `ui.space.messages = 0`
- **THEN** a user message and the next agent message have no blank row between them
