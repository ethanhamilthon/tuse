# Changelog

## 0.8.6

- `@` picker: Tab completes a folder, Enter attaches the highlighted entry (a folder too) as a token. A folder reaches the model as `<dir path="...">` with its entries.
- `tuse init` creates `./tuse.lua` that starts from the global config.
- Steering shows a bouncing dot before `steering…`.

## 0.8.5

- `/config` starts with the path of the loaded config file. JSON-RPC `config.get` returns it as `path`.

## 0.8.4

- `/config` shows a **Plugins** section: each loaded plugin with its `ref`, commit (or `local`) and folder. JSON-RPC `config.get` returns them as `plugins`.

## 0.8.3

- **Plugins:** `plugins = { "owner/repo", { "owner/repo/sub/path", opts = {...}, ref = "v1" }, { "name", dir = "..." } }` loads `init.lua` (else `tuse.lua`) from GitHub clones in `~/.tuse/plugins` or local folders. A plugin returns a config table or `function(opts)`; tables merge under the user's config (lists concatenate, named entries must not collide between plugins, plugins cannot set `model`, `provider` or `effort`). Startup never touches the network.
- `tuse plugins [list | add NAME [--ref REF] | remove NAME | install | update [NAME] | clean] [-c PATH]`; `add` edits the config Tuse would load.
- `os.getenv("THIS_PATH")` returns the path of the calling Lua file.
- Bundled plugin `ethanhamilthon/tuse/plugins/welcome`: a startup banner with model, folder, git state and version.

## 0.8.2

- **Breaking:** Config files and Lua modules (including hooks, commands, and helpers defined in them) must import host functions with `require("tuse")`. Host functions remain globals in the agent's `lua` tool.
- `require("tuse").read(...)` can read files while the startup config loads.

## 0.8.1

- The mouse wheel scrolls the chat, panel, or picker under the pointer. PageUp/PageDown scroll chat history even when a panel is open; hold Shift to page through the panel.
- Lua question and picker results start with an accent-colored `?`. Pinned and unpinned `show_markdown` blocks start with `◆`.
- End-of-turn timing and model summaries use the `muted` theme color.

## 0.8.0

- **Lua UI:** Added `show_markdown(name, text, pin)`, `show_picker(options)`, and `show_question(text)`.
  - `show_markdown` renders/updates named Markdown blocks (`pin = true` keeps them together at the end of the chat after new messages; `show_markdown(name, "", false)` removes a block).
  - `show_picker` reuses the `/models` picker layout with the input editor disabled (including search) and restores the previous draft on Enter/Esc.
  - `show_question` renders a Markdown panel in the picker slot and uses the editor separately from the previous draft without submitting to the model.
  - Submitted choices/answers leave UI-only chat entries (cancelled prompts leave none); all live UI entries and `show_markdown` blocks survive `/reload` (with pin state) and are excluded from model history and session resume.
  - Picker and question requests share one FIFO queue, pause the calling Lua coroutine's timeout while waiting, keep both front ends and other Lua jobs responsive, never deadlock `/reload` on interactive hooks, and clear pending requests on session close.
- **Custom slash commands:** Added `commands = { name = function() ... end }` in Lua config. Custom `/name` commands appear in `/` autocomplete, update on `/reload`, run as non-blocking Lua jobs, and cannot override built-in commands.
- **Breaking (Lua file helpers):** `read(path)` now returns raw text by default (without line numbers or pagination notes); use `read(path, {lines = true, offset = 1, limit = 2000})` for numbered, paginated lines. Added `append(path, text)` and `delete(path)` (single file only) with diff reporting, extended `grep` for multiple paths and `literal = true` search, and updated the `lua` tool description to prefer file helpers and `sh([=[…]=])`.
- **Trace and config views:** `/trace` now opens a selectable list (`index  role  first line`) whose Enter detail panel renders `system`/`user`/`agent` as Markdown, `lua` as full syntax-highlighted source without preview limits, and other roles as plain text, with Esc returning to the selected row. `/config` now shows a styled, scrollable sectioned view without provider secrets or Lua function bodies while JSON-RPC `config.get` stays structured.
- **TUI and JSON-RPC feature parity:** Added `session.new`, `session.resume`, `commands.list`, `commands.run`, `ui.state`, `ui.reply`, and `ui.cancel` methods, `prompts` support on `turn.submit`, single-call `model.set` validation and persistence, and `ui_markdown`/`ui_request`/`ui_clear`/`ui_result` events. Verified cross-front-end session resume in both directions (JSON-RPC ↔ TUI) sharing the same store and lock.

## 0.7.5

- **Breaking:** Removed config `include`. Use `require(...)` directly (for example `require("global")` in a project `tuse.lua`); configs that still return an `include` key fail with a clear error.
- `require` now resolves relative module names against the file that calls `require` (falling back to the working directory for inline code in the `lua` tool). File modules are cached by canonical path, and cyclic `require` calls raise an error.
- **Breaking:** Global config lookup now checks only `$TUSE_GLOBAL_CONFIG`, `~/.config/tuse.lua`, and `~/.config/tuse/init.lua` (`~/.config/tuse/main.lua` is no longer read as an entry; only `./tuse.lua` is looked up in the working directory).
- Removed the default 1-column left padding on user messages in the TUI.

## 0.7.4

- Tuse no longer keeps running at 100% CPU after its terminal is closed. If the UI does not stop within 2 seconds of a hangup, or after the terminal goes away, Tuse closes the session (with `process_end` hooks) and exits.
- `sh` runs commands in their own session, without a controlling terminal. An interactive shell such as `zsh -ic` can no longer take the terminal and leave Tuse suspended (`suspended (tty input)`) with mouse codes printed in the shell.
- `require("tuse")` gives the host functions (`sh`, `read`, `write`, `edit`, `grep`, `spawn`, ...) and `env`: `require("tuse").env.TUSE_MODE` reads any variable, with Tuse's own ones (`TUSE_MODE`, `TUSE_CWD`, `TUSE_GLOBAL_CONFIG`, ...).
- **Breaking:** `TUSE_MODE` is no longer a Lua global; use `os.getenv("TUSE_MODE")` or `require("tuse").env.TUSE_MODE`. Reading the old global raises an error that says so.
- `require` finds more: `require("name")` loads `./name.lua` or `./name/init.lua`; `require("global")` the global config; `require("~/path")` `~/path.lua` or `~/path/init.lua`; `require("~/path/file.lua")` that file. `/reload` loads changed modules again.
- The global config is `$TUSE_GLOBAL_CONFIG` (a file or a module path), else `~/.config/tuse.lua`, else `~/.config/tuse/init.lua`. `~/.config/tuse/main.lua` still works.
- Files, prompts and images picked into the input are drawn in `accent`; typed `#name` and `@path` stay plain until Enter picks them.
- Lists and panels (`/models`, `/trace`, `/config`, `/usage`, `/resume`, tasks) have no title line, like `Model 3/12`.
- Release scripts are inlined into the workflow; `release/main.lua` is no longer shipped.

## 0.7.3

- Simplified the TUI: one `lines` input design, a scrollable `/trace` log, substring search and normal `@dir/name` paths. Removed `/childrens` and child-session tracking.
- Added `include = { os.getenv("TUSE_GLOBAL_CONFIG") }` for sharing Lua config across projects. `require` looks in the session's working directory. Use `TUSE_MODE` (`tui` or `json`) instead of per-entry `modes`.
- `tuse connect` is non-interactive: `--base URL --key '$ENV' --name NAME`. The installer no longer prompts or edits your config. Removed `tuse reset`.
- Existing SQLite history and providers still work. Use 0.7.2 to import pre-SQLite `session.jsonl` history before updating.
- Faster CI: no duplicate release builds, pinned Rust, and native smoke tests for the four release targets. Linux archives require glibc 2.35 or newer; macOS archives target macOS 11 or newer.

## 0.7.2

- New default theme colors (from the author's Ghostty palette): `bg` `#121212`, `fg` `#DEDEDE`, `accent` `#BDDEAB`, `ok` `#94B979`, `err` `#E27373`, `info` `#97BEDC`, and greys for `surface`, `dim`, `muted`, `border`.
- `@` takes absolute paths: `@/etc/` and `@~/` list those folders as you type, and the picked file is attached from there.
- The status bars are colored by part: spinner, time and the used context (`x` of `x/y`) are `accent`; the action, model, effort, `/y`, cost and `steering…` are `muted`. Flash notices stay `accent`.
- Each status label takes at most half the input width and is cut with `…`.
- Tool output is `muted`; `…` and `… N more lines` under code, output and diffs are `border`.
- The `fill` input is a straight `surface` block (no half-block edges and notched corners). `fill` and `bar` have no `›`, and user messages in the chat are a `surface` block without `›`.
- A wrong `settings.ui.input_design` now names the allowed values in the error.

## 0.7.1

- The `lua` tool is `lua(code)`: `title` and `timeout` are gone. A call is stopped after 120 seconds; `settings.tool_timeout` is removed.
- The working indicator shows what the agent is doing (`reading PATH...`, `writing PATH...`, `editing PATH...`, `searching PATTERN...`, `running CMD...`) instead of a title. Each action stays at least 100 ms.
- Inactive list items use `muted` instead of `border`.
- The working animation and `settings.ui.motion` are removed.
- Hooks have no time limit: `settings.hook_timeout` is removed. Tuse still waits for `process_end` until its runs end.
- The config has one theme, `theme = {...}`, and the UI cannot switch it: `/theme`, `settings.theme`, `themes` and `extend` are removed. `/models` still remembers the model and effort in the database (`config.set` takes `model` or `effort`).
- `/maxsize` is removed: the maximum context shown in the status is the model's `window`.

## 0.7.0

- The agent's tool is `lua(code, title, timeout)`: Lua 5.4 (mlua) instead of Python (Monty). The model picks the timeout of each call, with no upper limit; `settings.tool_timeout` is the default.
- Nothing is cut: tool output, `sh().out` and task logs are kept whole. `settings.max_output` is gone.
- One VM per session runs the config, the agent's code, hooks and tasks as coroutines. `sh`, `sleep` and waits yield, so hooks and tasks keep running while a call waits.
- `spawn(fn, name)` runs a function as a background task in the same VM. `task_run`, `task_add` and process tasks are gone; `sh` kills its process at the timeout instead of handing it over.
- The config is `tuse.lua` / `~/.config/tuse/main.lua`, returning a table. Hooks and helpers are Lua functions; `include`, `$VAR` and `#{{cmd}}` give way to `require`, `os.getenv` and `io.popen`. `/reload` runs the file again in the same VM and keeps the agent's globals.
- Sessions, providers and the last picked model, effort and theme live in SQLite (`~/.tuse/tuse.db`). Old `session.jsonl` sessions move in on the first start. `tuse connect` saves providers there; `tuse providers` and `tuse disconnect NAME` are new.
- Removed: `ask`, `draw_ui`, the `ask` hook event, headless mode (`tuse -p`), and model calls of functions as separate tools.

## 0.6.5

- Config includes accept `only` to import selected top-level keys from a file.
- Hooks, functions, connections, system texts, prompts and includes accept per-entry `modes` (`tui`, `headless`, `json`). Model availability follows the active connections.
- `draw_ui(..., pin=True)` keeps a new block at the bottom of the chat. `draw_ui("", id=...)` removes a block, and redraws and removals preserve the scroll position where possible.
- `/childrens` shows the tree of sessions linked by `TUSE_PARENT`, with token usage, spend and a detail panel.
- `/childrens` stops counting elapsed time when a child session finishes.
- The `agent_message` hook receives `tasks` and `children`, listing only running background tasks and descendant sessions.

## 0.6.4

- `draw_ui` accepts ANSI SGR styling (bold, dim, italic, underline, 16 colors mapped to the theme, 256 and truecolor). Other escape sequences are dropped.
- Links are clickable: hover highlights them and a left click opens them (`open` / `xdg-open`). This covers Markdown links, bare `https://` URLs and OSC 8 hyperlinks in `draw_ui`; only `http`, `https` and `mailto` open.
- Functions in the config accept `show_agent: false`: the agent is not told about the helper, but other functions, hooks and tasks can still call it.
- The `python` tool no longer has a `timeout` argument. A call is stopped after `settings.tool_timeout`; the system prompt tells the agent to run long jobs with `task_run`.

## 0.6.3

- The input status shows context use as `used/max` (`5.2k/1m`) instead of `tok` or a percentage. `/maxsize 200k` sets the maximum, `/maxsize off` returns to the model's window, and without any it shows `5.2k/?`.
- Tool call headers show elapsed time in milliseconds under one second (`302ms/10s`).
- `draw_ui` blocks are drawn in the `accent` color.
- Fix: a question that arrived while another picker (`/tasks`, `/resume`, ...) was open stayed hidden after it closed; it now appears.
- The system prompt has a `Rules` section: prefer short multi-step scripts over one huge call.

## 0.6.2

- Tool call titles use `dim`, elapsed/timeout uses `accent`, and output and diff paths use `border`. Terminal tab OSC sequences have been removed.
- Hooks share the agent host API, can adopt shell tasks and respond to cancellation. Hook questions expire at the hook deadline, and old queues cannot survive a configuration reload.
- Added subsets of `glob`, `shlex` and `shutil`. The remaining Monty limitations are documented in `docs/tool.md`.

## 0.6.1

- Configure `events: [ask]` to run a hook when the main agent needs an answer in the TUI or JSON client. Hooks receive the question, options and ask ID, not the answer.
- The TUI wraps multiline questions above the options and keeps the question visible while writing a custom answer. Page Up and Page Down scroll longer questions.
- Python tool titles appear above their code in live and restored chat. `tool_line: 0` hides the title and code without hiding the result.
- Background task notifications no longer add TUI chat entries. They remain available in `/trace`, session records, the agent context and JSON-RPC.

## 0.6.0

- Picked `#prompts`, `@files` and `[Image #N]` stay as typed in the chat, title and hooks. The model receives their context separately; `/trace` shows it. Text attachments are capped at 1 KiB. Tokens typed by hand stay literal. See `docs("prompts")`.
- Search modal pickers in the input field with normal editing keys. Closing a picker restores the draft. `/models` keeps the filter across refreshes and no longer duplicates unnamed models’ IDs.
- Pickers and panels use the main background; selection uses the accent foreground, inactive rows the border color.
- Tool output fits the configured byte limit, including its omission marker. `/reload` updates the live interpreter’s budget.
- Early session events are buffered for the first subscriber; JSON clients receive the initialize response before startup notices.

- A crashed interpreter (a Monty panic) is recreated: the agent's next `python` call starts a clean one (the failed call says variables and imports are lost), hooks get a fresh one, and a crashed REPL task ends as `exited` instead of staying `idle`.
- Fix the tab state (OSC 9;4) disappearing on long turns and `ask` waits: it is re-sent every 5s while working or asking. Error and paused now send 100% so terminals draw a visible bar. Turning off `settings.ui.tab_status` with `/reload` restores the title and clears the state.
- **Fix**: the TUI now resets the terminal (mouse modes, bracketed paste, alt-screen, raw mode) when it exits on SIGINT, SIGTERM or SIGHUP, and on any early return. Before, mouse events leaked into the shell as `18M65;25;18M64;...`. SIGKILL cannot be handled; run `reset` if it happens.

## 0.5.4

Breaking: hook events and `run:` expansion changed, OSC 7501 was replaced (see below).

- **Hooks have four events**: `process_start` and `process_end` (`reason`: `launch`, `new`, `resume`, `reload`, `exit`), `user_message` (`text`, `is_first`) and `agent_message` (`text`, `status`: `working`, `finished`, `cancelled`). `session_start`, `session_end`, `turn_start`, `turn_end`, `tool_start`, `tool_end` and `error` are removed without aliases, as are the `tool`, `input`, `title`, `output`, `code`, `error` and `cancelled` variables; a hook with an unknown event shows a notice. `/reload`, `/new` and `/resume` now run `process_end` and `process_start`.
- **`tuse json`**: a JSON-RPC 2.0 session over stdin/stdout (NDJSON) with the TUI's full access: submit, cancel, withdraw, answer asks, model, config, tasks, trace and stats; every session event arrives as an `event` notification. See `docs("json")`.
- `/trace` lists hook runs (`hook` items with the event, its variables, the result and the duration); they are saved with the session and never sent to the model.
- `/trace` query filters by type: `+python+agent` keeps those types, `-hook` hides one, other words search the rows.
- **OSC 7501 is replaced by standard sequences**: the tab title (OSC 2, `tuse · <session title>`) and the tab state (OSC 9;4: working, a question, an error). `settings.ui.osc_status` is renamed `settings.ui.tab_status`; the old key is ignored. The previous title is restored on exit.
- **`$VAR` and `#{{cmd}}` are no longer expanded in `hooks.*.run` and `functions.*.run`**, so `$` in Python code (f-strings, templates) stays as written. Use `os.environ` or `sh` there.
- A built-in function called as a tool of its own (`ask`, `grep`, `sh`...) now runs as `python` code `name(**args)` instead of failing with "unknown tool".
- A new session creates its folder in `~/.tuse` with the first message; sessions closed before it leave nothing. Tests no longer write to `~/.tuse`.
- An unknown `effort` in the config shows a notice with the valid values (it still falls back to `default`).
- Compaction is on by default: without `compact_at`, it starts at 60% of the model's `window` from `models:`. `compact_at: 0` turns it off.
- Headless mode (`tuse -p`): the system prompt tells the agent there is no user, and `ask`/`draw_ui` raise a clear `RuntimeError`. New `docs("headless")` page; `-c/--config` and `-r/--resume` are documented.
- Faster status line on long sessions: usage totals are kept up to date instead of being recomputed from the whole history on every frame.
- Streaming long replies no longer re-renders the whole Markdown on every frame: finished blocks are cached and only the last one is parsed again (a 30 KB reply: 497 ms → 68 ms of rendering in total). Reasoning wraps only the lines it shows.
- Resize, `/theme` and `/reload` render only the messages on screen, from the bottom up (4000 messages, 20 resizes: 263 ms → 1.5 ms). `/reload` no longer copies the history.
- Fixed: a notice or a hook record between the tool results of one step no longer splits them with the images message.
- Fixed: assistant text that had already streamed was shown twice when a tool call started in the same step.

## 0.5.3

- Each hook processes events in order in a persistent interpreter. Hooks receive turn context and can set the session title. Configured Python helpers are available to the agent, tasks and hooks.
- Full-screen status is reported via OSC 7501 (disable with `settings.ui.osc_status`). Short notices appear in the status row; the turn footer shows elapsed time and model/effort.
- Russian-layout shortcuts work in supported terminals. Pasted images have compact labels; their paths expand on send.
- Markdown tables wrap within columns or switch to records on narrow screens. The README now includes benchmark results and is available through `docs("README.md")`.

- Tool code now streams into the chat, with the title shown as soon as it arrives.
- Removed `prelude`; import a `.py` file from the config folder or use a hook instead.
- Removed the `VERSION` file; the version now comes only from `Cargo.toml`.

## 0.5.2

- Inspect the effective YAML with `/config`. Expanded credentials are visible. Slash commands now work mid-draft without clearing surrounding text.
- Drag to select and copy chat or panel text. In the editor, use Shift+movement to select, Ctrl/Cmd+A to select all, and Ctrl/Cmd+X to cut.
- Custom `ask` answers use the multiline editor and restore the original draft. Task notifications appear separately from user messages.
- Rename Python `show()` to `draw_ui()` (no alias). The built-in prompt no longer injects environment details or `AGENTS.md`; use `systems` for project instructions.

## 0.5.1

- **Topic-based `docs(topic=None)`.** Split `docs/` into 7 focused topic files (`tool`,
  `functions`, `commands`, `env`, `config`, `hooks`, `theme`), each under 4 KB. `docs()` lists
  available topics; `docs("<topic>")` returns that section; unknown topics return an error message
  with the topic list.
- **Reuse HTTP connection pool.** `Session` keeps a shared `ureq::Agent` across steps via
  `Client::with_agent`, enabling HTTP keep-alive and TLS session reuse.
- **Lower Python thread stack.** Reduced the per-call Python interpreter thread stack from 128 MB
  to 16 MB.
- **TUI rendering & event loop optimizations.** Visible-window chat slicing (`Chat::window`) avoids
  cloning off-screen history; streaming redraws throttle to `FRAME_MS`; line-level screen diff and
  minimal ANSI style transitions skip unchanged rows and redundant resets; event loop caches
  terminal size via `Resize` events and ignores unhandled mouse/focus events.
- **Theme cache invalidation.** `/theme` now invalidates the chat and panel render caches so
  existing messages immediately switch to the new palette.
- **Persistent working indicator `title`.** The active tool `title` stays visible in the working
  indicator until a new `title` arrives (or the turn ends).
- **`tuse update` version output.** Prints `old -> new` (e.g. `0.5.1 -> 0.5.2`) after updating.
- **`/trace` highlighting.** Opened `user`, `agent` and `system` trace items render with Markdown
  highlighting; `python` items render with syntax highlighting matching the chat.
- **`settings.ui.fold_strings` (default `true`).** Folds multiline triple-quoted Python strings in
  chat tool calls to a single line (`"""first line…"""`), while `/trace` shows the full code.

## 0.5.0

Breaking config change: simplified UI configuration.

- **10-color themes + `settings.ui`.** Replaced the 92-token `Theme` with 10 base colors
  (`bg`, `surface`, `fg`, `dim`, `muted`, `border`, `accent`, `ok`, `err`, `info`) with `extend`
  support. Behavior and layout preferences (`input_design`, `motion`, `tool_line`, `diff_line`,
  `output_line`, `reasoning_line`, `scroll_step`, `pad_x`, `message_gap`, `list_max_height`,
  `input_max_height`) moved to `settings.ui` so `/theme` does not overwrite them. Symbols,
  prefixes, status labels and internal constants moved into code.
- **Working indicator shows only tool `title`.** Removed `Host::status` (`reading ...`,
  `writing ...`, `asking user...`, etc.). While the model thinks, the indicator shows only the
  spinner and elapsed time; during a tool call it shows the call's `title` (and `steering…` when a
  queued message waits).
- **`/trace` navigation.** `Left` and `Right` step through opened `/trace` items (with full
  `Trace N/total role` title) when the input is empty. Both the `/trace` list and opened items
  clamp at the start and end instead of wrapping.
- **Styled `/usage` panel.** Multi-color layout using the 10-color palette (`border` keys, `dim`
  labels and session ID, `fg` model and counts, `info` effort and window, bold `accent` context
  size, bold `ok` cost) and no duplicate model name when `display == model`.

## 0.4.1 - 2026-11-19

One tool, a built-in system prompt, one look for every list. **Breaking** for hooks and configs;
read the notes at the end.

- **One tool: `python`.** `read`, `write` and `edit` are functions inside it, next to the new
  `grep(pattern, path, glob, ignore_case, context, limit)` (a regex search that respects
  `.gitignore`). Errors are Python exceptions (`ValueError`, `FileNotFoundError`, ...). The files a
  call changed are shown as a diff at the end of the call (a change at the end of a long line stays
  visible); the model gets a short confirmation. `read` stops at `max_output` and says which
  `offset` continues.
- **`title` and a status in the working indicator.** Every `python` call has a `title`. The
  indicator shows `thinking...` at the start of a turn, then the title, `reading PATH`,
  `writing PATH`, `editing PATH`, `searching PATTERN`, `running CMD`, `asking user...` or
  `reading docs...`; the last status stays until a new one comes. The call counter left the
  indicator (it stays in `/usage`). Texts are theme tokens (`thinking_label`, `read_label`, ...).
- **`sh` does not die at its timeout.** A command still running at its own timeout or the call's
  goes on as a background process task; `.task` is its ID, `.out` has what it printed so far.
- **`task_run(code)` runs Python.** A task is a background REPL (new interpreter, same functions,
  no access to your variables) that stays alive and `idle`. `task_add(id, code)` queues more code in
  it. `task_get` and `task_list` show `.kind`, `.code`, `.status`, `.exit` and `.queue`. The agent
  gets a message each time a REPL goes idle.
- **`ask` always blocks** (no `wait`) and the call's timeout does not run while the user thinks.
  The user can always write their own answer: typing is the answer, Enter without text takes the
  selected option. To ask without waiting, call it from a task.
- **One look for every list.** `/models`, `/resume`, `/theme`, `/tasks`, `/trace`, `ask` and the
  `/`, `#` and `@` pickers share the `/trace` design (a rule, a title with the position, rows),
  drawn over the bottom of the chat. `/trace` filters too. Tokens: `list_*` replace `picker_*`
  and `panel_*`.
- **Python is highlighted** in tool calls and in ```` ```python ```` blocks (`syntax_*` tokens).
- **Mouse.** The wheel scrolls the chat (or the open list); a click in the input puts the cursor
  there. The arrows only move in the input. Select text with Shift (Option in iTerm2 and
  Terminal.app) while the mouse is captured.
- **The system prompt is built in** (`crates/core/prompts/system.md`) and tells the agent the
  real `max_output` and `tool_timeout`. The `systems` of your config come after it. `default.yaml`
  is gone; `tuse update` only replaces the binary.
- **New defaults.** `max_output` 4096, `tool_timeout` 10, `input_max_height` -1 (no limit),
  `tool_line` -1, `output_line` 1, `reasoning_line` -1, `pad_x`, `input_pad_x` and `list_pad_x`
  0, `tool_prefix` `* `, `notice_prefix` `✓ `, spinner `◐ ◓ ◑ ◒`.
- The chat no longer prints `Model ..., effort ...` and `Theme ...` after `/models` and `/theme`.

Notes for an existing install:

- Run `tuse update`, then delete `include: [default.yaml]` from `~/.config/tuse/main.yaml` (or
  run `tuse reset`). Until then `default.yaml` is an empty file, so nothing breaks.
- Removed with no replacement: the old names `max_bash_output`, `bash_timeout`, `bash_line`, the
  top-level `theme` (use `settings.theme`), `bins`, the `wait` argument of `ask`, and `task_run`
  of a shell command (use `task_run('sh("cmd")')`). Sessions written before 0.4 are not shown
  with a tool name.
- Hooks: `tool` is always `python`, `input` is its code, `title` is new. A hook that looked for
  `tool == "edit"` has to look at the code instead.
- `Record::Tool` has `title` and `diffs`; a session written by 0.4.0 still opens.
- The theme tokens `picker_*`, `panel_*` and the two `*_max_height` of lists are renamed; use `list_fg`, `list_bg`, `list_title`, `list_selected_fg`, `list_selected_bg`,
  `list_detail`, `list_selected_prefix`, `list_pad_x`, `list_max_height`.
