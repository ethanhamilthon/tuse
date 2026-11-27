# tui

## Purpose
Terminal UI behavior.

## Requirements

### Requirement: Task details
Enter on a task in `/tasks` SHALL open a panel with id, name, status, started, ended, elapsed time (live while running), the source of the task's function, and the last 200 log lines. When the source is unknown it shows `(source unavailable)`. `task_list`, `task_get`, `task_stop`, `tasks.list`, `tasks.get`, and `tasks.stop` SHALL share the same base fields `{ id, name, status, started, ended, log, code }` (with `task_get` and `tasks.get` adding `output`).

#### Scenario: Running task
- **WHEN** the agent ran `spawn(function() sh("sleep 60") end, "nap")` and the user presses Enter on it
- **THEN** the panel shows `nap`, `running`, the elapsed time, and `function() sh("sleep 60") end`

#### Scenario: Task field parity across Lua and JSON-RPC
- **WHEN** a task exists and both Lua `task_get(id)` and JSON-RPC `tasks.get` are called
- **THEN** both return identical fields `{ id, name, status, started, ended, log, code, output }`

### Requirement: Task stop shortcut supports primary modifier
In `/tasks`, both `Ctrl+X` and `Cmd+X` (the primary modifier) SHALL stop the highlighted background task and refresh the task list.

#### Scenario: Stop task with Ctrl+X or Cmd+X
- **WHEN** `/tasks` is open with a running task highlighted and the user presses `Ctrl+X` or `Cmd+X`
- **THEN** the selected task is stopped and the `/tasks` list refreshes

### Requirement: Model and effort selection uses the shared model effort resolver
`/models` and JSON-RPC `model.set` SHALL resolve available efforts through the same shared model effort resolver (`config.models[].efforts` when declared, else `low`, `medium`, `high`, `xhigh`, `max`, with explicitly empty `efforts = {}` meaning no reasoning parameter). In `/models`:
- Picking a model with explicitly empty `efforts = {}` SHALL skip the effort picker and immediately select the model with `effort = ""` (and the status bar omits the effort segment).
- Picking any other model SHALL open the effort picker listing only that model's resolved efforts with no `"unsupported"` entries (highlighting the current effort if allowed, else the model's fallback: `"medium"` if present, else the first declared effort).
In JSON-RPC `model.set`:
- `model: ""` SHALL continue to clear the selected model.
- For a non-empty model with `efforts = {}`, omitting `effort` or passing `effort: ""` SHALL succeed and set `effort = ""`, while passing any non-empty effort MUST fail with `-32000`.
- For a non-empty model with a non-empty resolved effort list `L`, omitting `effort` SHALL select the fallback (`"medium"` if in `L`, else `L[0]`), and passing any effort not in `L` (including retired `"default"`) MUST fail with `-32000`.

#### Scenario: Effort picker shows only resolved model efforts
- **WHEN** `config.models` declares `{ name = "gemini/gemini-2.5-pro", efforts = { "low", "medium", "high" } }` and the user selects that model in `/models`
- **THEN** the effort picker lists only `low`, `medium`, and `high` with no `"unsupported"` items

#### Scenario: Effort picker skipped for explicitly empty efforts
- **WHEN** `config.models` declares `{ name = "openai/gpt-4o", efforts = {} }` and the user selects that model in `/models`
- **THEN** no effort picker opens, the session model becomes `openai/gpt-4o` with `effort = ""`, and requests omit reasoning parameters

#### Scenario: Undeclared model offers all five shared default efforts
- **WHEN** a model has no `efforts` override in `config.models` and the user selects it in `/models`
- **THEN** the effort picker lists `low`, `medium`, `high`, `xhigh`, and `max` regardless of provider API type

#### Scenario: model.set validates against resolver, applies fallback, and preserves clearing
- **WHEN** a client calls `model.set` with `{"model": "gemini/gemini-2.5-pro", "effort": "max"}` or `{"model": "gemini/gemini-2.5-pro", "effort": "default"}`
- **THEN** the call fails with `-32000` and does not update the session or saved defaults
- **WHEN** `config.models` declares `{ name = "p/fast", efforts = { "high", "low" } }` and a client calls `model.set` with `{"model": "p/fast"}`
- **THEN** `effort` resolves to `"high"` (the first declared effort because `"medium"` is absent)
- **WHEN** `config.models` declares `{ name = "openai/gpt-4o", efforts = {} }` and a client calls `model.set` with `{"model": "openai/gpt-4o"}`
- **THEN** the call succeeds with `effort = ""`, whereas `{"model": "openai/gpt-4o", "effort": "low"}` fails with `-32000`
- **WHEN** a client calls `model.set` with `{"model": ""}`
- **THEN** the selected model is cleared

### Requirement: JSON-RPC v2 protocol, validation, and path expansion
JSON-RPC SHALL speak `protocolVersion: 2` (`initialize` defaults omitted `protocolVersion` to `2` and rejects `1` or any other version with `-32602`). All Tuse domain fields across JSON-RPC methods and events SHALL use canonical `snake_case` (`input_id`, `elapsed_ms`, `title`, `config_path`, `context`, `context_window`, `cache_read`, `cache_write`, `llm_cost`, `requests`, `llm_requests`, `model_info`). `initialize` SHALL return `{ protocolVersion: 2, tuse, session, capabilities: { methods } }`, `session.new` SHALL return `{ session }`, and `session.fork` SHALL return `{ session, draft }` using the unified lightweight `SessionState` object. `turn.submit` SHALL validate every `data:` entry in `images` with the same `data:image/<mime>,<data>` rule enforced by `set_session`, failing invalid data URLs with `-32000`, and SHALL expand `~/…` paths in `files` and `images` consistently with TUI `@~/…` attachments. `tasks.get` and `session.list` SHALL reject non-null wrong-typed optional parameters (`lines` not a non-negative integer, `cwd` not a string) with `-32602`, and `tasks.get` SHALL clamp `lines: 0` to `1` consistently with Lua `task_get`.

#### Scenario: Protocol version 2 handshake
- **WHEN** a client calls `initialize` with `{"protocolVersion": 1}`
- **THEN** the server fails with error code `-32602`
- **WHEN** a client calls `initialize` with `{}` or `{"protocolVersion": 2}`
- **THEN** the server replies with `protocolVersion: 2` and a lightweight `session` object using `snake_case` fields

#### Scenario: Invalid image data URL rejected
- **WHEN** a client calls `turn.submit` with `{"text": "hi", "images": ["data:bad"]}`
- **THEN** the call fails with error code `-32000` and no user record is written

#### Scenario: Home-relative paths in turn.submit
- **WHEN** a client calls `turn.submit` with `files: ["~/note.txt"]` and that file exists in the user's home directory
- **THEN** the file is resolved under the home directory and attached in the user context

#### Scenario: Wrong-typed optional RPC parameters
- **WHEN** a client calls `tasks.get` with `{"id": "t1", "lines": "abc"}` or `{"id": "t1", "lines": -1}`, or calls `session.list` with `{"cwd": 123}`
- **THEN** each call fails with error code `-32602`
- **WHEN** a client calls `tasks.get` with `{"id": "t1", "lines": 0}`
- **THEN** it returns the last 1 line of the task log rather than empty output

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
`/history` SHALL list the session history from the database, minus roles and message levels in `history.hide` and minus `agent` entries with no text and no reasoning. `trace` is no longer a built-in command and `history` is a reserved command name. Typing an unknown standalone single-segment slash command at column 0 with no second `/` (such as `/trace`) and pressing Enter MUST fail immediately on the first Enter with an unknown-command error, preserve the draft, and MUST NOT send the text as a user message to the model. JSON-RPC `session.trace` and `session.records` are replaced by `session.history`, which returns the same structured records as `session().history`, and `session.list` returns only started sessions.

#### Scenario: Open history
- **WHEN** the user types `/history`
- **THEN** a list of every history record opens, starting with `system`

#### Scenario: Tool-only reply hidden
- **WHEN** the agent replied with only a `lua` call and the user opens `/history`
- **THEN** only the `lua` entry with the code is listed for that call

#### Scenario: Old command
- **WHEN** the user types `/trace` and presses Enter once
- **THEN** an `Unknown command /trace` error is shown, `/trace` stays in the editor, and no turn is sent to the model

#### Scenario: Old method
- **WHEN** a client calls `session.trace` or `session.records`
- **THEN** it gets a method-not-found error

### Requirement: Resume lists started sessions
The `/resume` picker SHALL list only sessions of the current directory with `started = true`, newest first.

#### Scenario: Unstarted session
- **WHEN** a session was opened and closed without a message
- **THEN** `/resume` does not list it

### Requirement: TUI and JSON-RPC parity
In `protocolVersion: 2`, the TUI and JSON-RPC SHALL expose a clean split between lightweight session state (`session.state`) and full history (`session.history`), alongside sending, withdrawing, and stopping input (`turn.*`), session creation, listing, and forking (`session.new`, `session.list`, `session.fork`), model listing and selection (`model.list`, `model.set`), config view and reload (`config.get`, `config.reload`), tasks (`tasks.*`), commands (`commands.*`), `pick`/`ask`/`show` (`ui.*`), and quit (`shutdown`). Two intentional asymmetries apply:
- Neither frontend SHALL have a way to edit history directly; `set_session` is a Lua host function only, reachable from both through commands, hooks, and agent `lua` calls (`session.set` and `config.set` return `-32601`).
- In-process session resume is TUI-only (`/resume`) and Lua-only (`resume(id)`): JSON-RPC v2 removes `session.resume` (`-32601`) while retaining `session.list` so external clients discover saved session IDs and resume them by launching `tuse json -r <id>` (or via Lua `resume(id)`, which emits `session_switched`).
The retired methods `session.info`, `session.stats`, `session.snapshot`, `session.resume`, and `model.get` MUST return method-not-found (`-32601`).

#### Scenario: Lightweight state split from history
- **WHEN** a client calls `session.state`
- **THEN** it receives the unified `snake_case` session state (including `title`, `model`, `effort`, `busy`, `turn`, `context`, `context_window`, `usage`, `model_info`, `requests`, `llm_requests`, `steps`, `turns`, `calls`) without serializing `history`
- **WHEN** the client needs history records
- **THEN** it calls `session.history`

#### Scenario: Retired methods removed while session.list remains
- **WHEN** a client calls `session.info`, `session.stats`, `session.snapshot`, `session.resume`, or `model.get`
- **THEN** each returns error code `-32601` (method not found)
- **WHEN** a client calls `session.list`
- **THEN** it receives the started sessions for the requested or current folder, which can be resumed via `tuse json -r <id>`

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
Every input style SHALL distinguish provider wait with the existing spinner, foreground Lua execution with the Lua block's animated star, and model streaming with a fixed-width dot that pulses through progressively larger and smaller glyphs. The elapsed timer SHALL remain available with the activity. While background tasks are running, the activity indicator SHALL be followed by ` · n tasks` in the accent color, using `task` for one, and the task count SHALL NOT add a second spinner. Only tasks whose status is `running` count. Streaming SHALL begin at the first displayable text, reasoning, or Lua code update; empty announcements alone MUST NOT activate the pulse or the Lua execution star. A pause between chunks MUST NOT switch the indicator back to waiting. Activity changes MUST NOT insert rows into the chat. The TUI SHALL preserve steering and retry notices and MUST NOT keep showing an old retry notice over a new attempt. Ending, stopping, or switching sessions MUST clear stale activity.

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
When no activity indicator or notice is shown, no turn is running, and no background task is running, the composer SHALL show the session working directory in the `muted` color without an icon, with the home directory written as `~`, in the place of the indicator in every input style. When background tasks are running, that place SHALL show the task indicator instead of the directory: an animated spinner in the `icons` color followed by the task count (`1 task` or `n tasks`) in the `accent` color.

#### Scenario: Idle and working
- **WHEN** no turn is running, no background task is running, and no notice is shown
- **THEN** the status shows the directory
- **WHEN** a turn starts or a notice appears
- **THEN** the indicator or notice replaces the directory

#### Scenario: Idle with tasks
- **WHEN** no turn is running and two background tasks are running
- **THEN** the status shows an animated spinner in `icons` and `2 tasks` in `accent` instead of the directory
- **WHEN** the last task ends
- **THEN** the directory returns

#### Scenario: Distinct icon and accent colors
- **WHEN** `ui.theme.icons = "#FF0000"`, `ui.theme.accent = "#00FF00"`, no turn is running, and one background task is running
- **THEN** the spinner uses `#FF0000` and `1 task` uses `#00FF00`

### Requirement: Lists open below the input
A picker, question, or panel SHALL open below the input, never between the chat and the input. With `lines` and `boxed` its rows SHALL follow the last row of the composer. With `fill` and `bar` its rows SHALL sit between the editor block and the status row, followed by one row drawn as a rule in the `border` color. Its height SHALL stay limited by `ui.lines.list` and the viewport, and the chat SHALL keep the rows that remain. The wheel over the list and `PageUp`/`PageDown` paging (without `Shift`) SHALL operate on any open picker—including built-in modal pickers and titled or untitled Lua pickers—using its visible choice rows, clamping selection to `0..n-1` without wrapping around.

#### Scenario: Picker with boxed input
- **WHEN** the style is `boxed` and the user types `/`
- **THEN** the command list is drawn directly under the bottom border of the box and the chat above is not covered

#### Scenario: Picker with fill input
- **WHEN** the style is `fill` and the user types `/`
- **THEN** the order from the top is the filled block, the command list, one border rule row, and the status row

#### Scenario: Page through modal picker
- **WHEN** `/models`, `/history`, `/resume`, `/fork`, `/tasks`, or an untitled Lua picker is open with more items than fit and the user presses `PageDown`
- **THEN** the picker advances by its visible choice rows and the chat scroll does not move

#### Scenario: Page bounds clamp without wraparound
- **WHEN** a picker is near the bottom of its list and the user presses `PageDown`, or near the top and presses `PageUp`
- **THEN** the selection clamps to the last or first item instead of wrapping around

### Requirement: Blank row above the composer
Every input style SHALL draw one empty row above the first row of the composer: the top rule, the top border, the filled block, or the first bar row. The row holds no text and no color other than the terminal background. It MUST NOT count toward `ui.lines.input`. When the viewport is too small, this row SHALL be the first one dropped.

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
- **THEN** the roles start at one column and the summaries at one column, for example `0   system  …` and `10  lua     …`

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
The `Models` section of the `/config` panel SHALL pad every model name to the widest name in the section and start the details (`display=…`, `window=…`, `efforts=…`, `prices=…`) at one column after two spaces. The other sections of `/config` and the `/session` panel keep their current alignment.

#### Scenario: Models of different length
- **WHEN** the config has `openai/gpt-6.2-sol` and `claude/claude-sonnet-5-5`
- **THEN** both `window=…` details start at the same column

### Requirement: Command descriptions and words
The `/` picker SHALL show the `desc` of a custom command as its detail. Built-in and custom commands—including commands with `args` and positional words—SHALL be usable anywhere in a TUI draft (beginning, middle, or end), with no column-0-only restriction.
- **Open command picker (`Kind::Command`):** `Enter` (or `Tab`) when a valid command item is selected SHALL accept it immediately: for a built-in command or custom command without `args`, it SHALL remove only the command token span, preserve all surrounding draft text (unless the command itself switches sessions, such as `/new`, `/resume`, or `/fork`), and immediately execute the command; for a custom command with `args`, it SHALL replace only the command token with `/<name> `, transition to `Kind::CommandArg(<name>)`, and preserve surrounding draft text. When a `/...` token contains a second `/` (such as `/usr/bin`), `Kind::Command` MUST NOT open.
- **Open argument picker (`Kind::CommandArg`):** Typing a space after `/<name>` anywhere in the draft SHALL open `Kind::CommandArg(<name>)` filtered by the typed text. Choosing a valid arg SHALL remove only the `/<name> <arg>` span, preserve surrounding draft text, and run `<name>(arg)`. Pressing `Enter` on an unmatched/invalid arg MUST NOT run the command, MUST NOT send a message, SHALL show the validation error, and SHALL preserve the full draft. Pressing `Esc` SHALL close the arg picker and leave `/<name> ` in place without discarding surrounding draft text.
- **Space after zero-argument command:** Typing a space after a zero-argument command (such as `/new ` or a custom `function()` command `/diff `) SHALL disengage the command picker. Ordinary `Enter` with no active command picker MUST NOT trim trailing whitespace into an executable command: standalone `"/new "` or `"/new foo"` (and standalone `"/diff "` or `"/diff x"`) MUST NOT execute the command, MUST NOT send to the model, SHALL show `/<name> takes no words`, and SHALL preserve the draft.
- **Ordinary `Enter` with no active command picker:**
  - Mid-draft prose (not starting at column 0 with `/`, such as `"explain /new works"` or `" /new"` or `"check /usr/bin/env"`) and column-0 paths containing a second `/` (such as `"/usr/bin/env"` or `"/etc/hosts is broken"`) SHALL be sent as ordinary user message text without executing any command or raising an unknown-command error.
  - Standalone exact zero-arg commands without leading or trailing whitespace (`"/new"`, `"/diff"`), standalone custom commands with `args` (`"/deploy "` after `Esc` -> `run(nil)`, `"/deploy prod"` -> `run("prod")`), and standalone custom commands with positional words (`"/mv a.txt b.txt"`) SHALL execute after validating words; any invalid word list or unclosed quote MUST NOT execute, MUST NOT send to the model, SHALL show the error, and SHALL preserve the draft.
  - Standalone single-segment unknown commands at column 0 with no second `/` (`"/trace"`) SHALL fail immediately on the first `Enter` with `Unknown command /<name>`, preserve the draft, and MUST NOT send to the model.
- JSON-RPC `commands.run` SHALL accept `{name, argv?}` with a list of words, apply the same checks, and fail with `-32000` when they fail; `commands.list` SHALL return `[{name, desc, args, params}]`.

#### Scenario: Built-in command executed immediately with open picker
- **WHEN** the user types `/new` so the command picker is open with `/new` selected and presses `Enter`
- **THEN** `/new` executes immediately and opens a new session
- **WHEN** the draft holds `draft /reload after` with the cursor after `/reload`, the command picker is open with `/reload` selected, and the user presses `Enter`
- **THEN** `/reload` executes immediately, only `/reload` is removed, and `draft  after` is preserved in the editor

#### Scenario: Space after zero-argument command prevents firing on Enter
- **WHEN** the user types `/new` followed by a space (`"/new "`)
- **THEN** the command picker disengages
- **WHEN** the user then presses ordinary `Enter` on `"/new "`
- **THEN** whitespace is NOT trimmed into an executable `/new`, `/new` does NOT execute, no message is sent to the model, `/new takes no words` is shown, and `"/new "` stays in the editor

#### Scenario: Built-in command with trailing words preserves draft
- **WHEN** the user types `/new foo` or `/models bar` and presses `Enter`
- **THEN** `/new takes no words` or `/models takes no words` is shown, `/new` and `/models` do not execute, no message is sent to the model, and the draft stays in the editor

#### Scenario: Mid-draft custom command with arguments preserves surrounding text
- **WHEN** the draft holds `fix bug /dep later` with the cursor after `/dep`, the user presses `Enter` to pick `/deploy` (opening the `args` picker mid-draft), and then picks `prod`
- **THEN** `run("prod")` is called, only the `/deploy prod` span is removed, and the surrounding draft (`fix bug  later`) stays in the editor

#### Scenario: Mid-draft built-in modal command preserves surrounding text
- **WHEN** the draft holds `draft /models after` with the cursor after `/models` and the user presses `Enter` with `/models` selected in the open picker
- **THEN** the `/models` picker opens, and after closing or `Esc` the surrounding draft (`draft  after`) is restored

#### Scenario: Esc then Enter on standalone arg command
- **WHEN** the user picks `/deploy`, presses `Esc` in the arg picker (leaving `"/deploy "`), and presses `Enter`
- **THEN** `run(nil)` is called

#### Scenario: Type positional words standalone
- **WHEN** the user types `/mv a.txt b.txt` and presses `Enter`
- **THEN** `run("a.txt", "b.txt")` is called and the text is not sent to the agent

#### Scenario: Invalid arguments preserve draft standalone and mid-draft
- **WHEN** the user presses `Enter` on `/deploy nope`, `fix /deploy nope` (in the arg picker), `/mv a b c`, or `/commit "unclosed`
- **THEN** nothing runs, no message is sent to the model, an error naming the command is shown, and the entire editor draft is preserved untouched

#### Scenario: Plain prose and file paths send without accidental execution
- **WHEN** no command picker is open and the user submits `explain /new works`, ` /new`, `check /usr/bin/env and /etc/hosts`, or `/etc/hosts is broken`
- **THEN** the full text is sent to the model as a normal user message, no command executes, and no unknown-command error is raised

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
User and agent message rows SHALL reserve two display columns on each side for padding, inside the chat width and separate from the outer `ui.space.padding` margin. Messages SHALL wrap at the remaining content width. The top-level text of every chat entry SHALL start at the same left content column after any entry marker, on both first and continuation rows. This applies to messages, reasoning, Lua code and output, diff blocks, notices, errors, turn summaries, pinned and unpinned `show()` blocks, and recorded picker and question results. Intentional indentation within an entry, such as Markdown nesting or code indentation, SHALL be preserved. When the chat is too narrow to fit the padding and content, the padding SHALL yield as needed without overflowing the row.

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
`/fork` SHALL open a picker of the `user` and `agent` messages of the current session, newest first, each with its role and a one-line preview, except `agent` messages with no text and no reasoning. Picking one SHALL open a new session forked at that message. For an agent message the history ends with that message. For a user message the history ends just before it, and its text is placed in the input as the draft. With no such messages, `/fork` SHALL do nothing and show a notice. `fork` is a reserved command name. JSON-RPC `session.fork` SHALL do the same for a `session.history` index, counted from 0, and return `{ session, draft }` (`draft` empty for an agent message), and an index that is not a `user` or `agent` record returns an error.

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
- **WHEN** `ui.space.messages = 0` and a user message and the next agent message are displayed
- **THEN** the user message and the next agent message have no blank row between them

### Requirement: Namespace parameter for JSON-RPC config reload
In protocol version 2, `config.reload` SHALL accept optional `params.ns` as a non-empty string naming the active namespace to reload. Omitting `ns` or setting it to `null` SHALL perform a full reload. Other types and the empty string SHALL fail with `-32602`. Scoped reload SHALL use the existing response contract: `null` on success and `-32000` with the reason when reload fails. A successful scoped reload SHALL rebuild the session system prompt, trigger `config_load`, and emit the existing `reloaded` event. A failed reload SHALL NOT emit `reloaded`. TUI `/reload` and Lua `reload()` SHALL continue to perform full reloads.

#### Scenario: Reload one namespace over JSON-RPC
- **WHEN** a client calls `config.reload` with `{"ns":"work"}` and the reload succeeds
- **THEN** it receives `null`, `config.get` reflects the resulting merged config, the system prompt is rebuilt, `config_load` is triggered, and the usual `reloaded` event is emitted

#### Scenario: Optional parameter preserves full reload
- **WHEN** a client calls `config.reload` without `ns` or with `{"ns":null}`
- **THEN** it performs the same full reload as before

#### Scenario: Invalid namespace parameter
- **WHEN** a client passes `{"ns":42}` or `{"ns":""}`
- **THEN** the server returns `-32602` without running a reload

#### Scenario: Unknown namespace
- **WHEN** a client passes a non-empty namespace name not present in the active config
- **THEN** the server returns `-32000` with a reason and does not emit `reloaded`
