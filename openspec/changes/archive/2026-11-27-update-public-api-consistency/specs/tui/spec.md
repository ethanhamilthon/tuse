## ADDED Requirements

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

## MODIFIED Requirements

### Requirement: Task details
Enter on a task in `/tasks` SHALL open a panel with id, name, status, started, ended, elapsed time (live while running), the source of the task's function, and the last 200 log lines. When the source is unknown it shows `(source unavailable)`. `task_list`, `task_get`, `task_stop`, `tasks.list`, `tasks.get`, and `tasks.stop` SHALL share the same base fields `{ id, name, status, started, ended, log, code }` (with `task_get` and `tasks.get` adding `output`).

#### Scenario: Running task
- **WHEN** the agent ran `spawn(function() sh("sleep 60") end, "nap")` and the user presses Enter on it
- **THEN** the panel shows `nap`, `running`, the elapsed time, and `function() sh("sleep 60") end`

#### Scenario: Task field parity across Lua and JSON-RPC
- **WHEN** a task exists and both Lua `task_get(id)` and JSON-RPC `tasks.get` are called
- **THEN** both return identical fields `{ id, name, status, started, ended, log, code, output }`

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
