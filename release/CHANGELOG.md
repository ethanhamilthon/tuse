# Changelog

## 0.5.4

- A built-in function called as a tool of its own (`ask`, `grep`, `sh`...) now runs as `python` code `name(**args)` instead of failing with "unknown tool".
- A new session creates its folder in `~/.tuse` with the first message; sessions closed before it leave nothing. Tests no longer write to `~/.tuse`.
- An unknown `effort` in the config shows a notice with the valid values (it still falls back to `default`).
- Compaction is on by default: without `compact_at`, it starts at 60% of the model's `window` from `models:`. `compact_at: 0` turns it off.
- Headless mode (`tuse -p`): the system prompt tells the agent there is no user, and `ask`/`draw_ui` raise a clear `RuntimeError`. New `docs("headless")` page; `-c/--config` and `-r/--resume` are documented.
- Faster status line on long sessions: usage totals are kept up to date instead of being recomputed from the whole history on every frame.
- Streaming long replies no longer re-renders the whole Markdown on every frame: finished blocks are cached and only the last one is parsed again (a 30 KB reply: 497 ms → 68 ms of rendering in total). Reasoning wraps only the lines it shows.
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

## 0.4.0 - 2026-11-19

The agent no longer has `bash`. It has four tools: `python`, `read`, `write` and `edit`.

**Breaking:** read the migration notes below before updating a customized config.

- `python` is a persistent REPL on [Monty](https://github.com/pydantic/monty) (a Python subset
  written in Rust). Variables, functions and imports stay between calls until Tuse exits.
  The model gets `print()` output plus the last expression, or a traceback. It has `sh(cmd)`
  (`bash -c`, returns `.out`, `.code`, `.ok`, `.timed_out`), `ask()`, `show()`, `task_run()`,
  `task_get()`, `task_stop()`, `task_list()` and `docs()`, full filesystem access through
  `pathlib` and `open`, and `import` of `.py` files from the config folder.
  A timeout or a cancel (Esc) ends a loop and kills the `sh` it waits for.
- `read` shows text with line numbers in pages, and images (png, jpg, gif, webp) to the model.
  `write` creates or replaces a file atomically. `edit` replaces exact, unique text, handles CRLF
  files and answers with a diff that the TUI colors (`diff_add`, `diff_del`, `diff_line`).
- Hooks run Python instead of bash. Every call gets a fresh interpreter with the agent's access and
  the event data as names (`event`, `tool`, `input`, `output`, `code`, `error`, `cancelled`, ...).
  `settings.hook_timeout` (30 s) bounds a hook; Tuse waits for `session_end` hooks on exit.
- Removed: the session socket and the commands `tuse steer`, `ask`, `print`, `task` and `docs`
  (their work is done by functions in `python`), and `bins`. `tuse -p` runs one prompt to the end
  and exits (exit code 1 after an error).
- New settings: `prelude` (Python run before the REPL and every hook), `hook_timeout`,
  `max_image_bytes`. Renamed with the old names still accepted: `max_bash_output` ->
  `max_output`, `bash_timeout` -> `tool_timeout`, theme token `bash_line` -> `tool_line`.
- Old sessions open as before; their `bash` calls are shown as they were.

Migration: a hook `run: ~/x.sh` becomes `run: sh("~/x.sh")`; `TUSE_TOOL_COMMAND`, `TUSE_TOOL_EXIT`,
`TUSE_ERROR` and `TUSE_CANCELLED` are now the names `input`, `code`, `error` and `cancelled`.
A `bins` entry becomes a function in a `.py` file next to the config, loaded with `prelude`.
A custom `systems` text that talks about `bash` should be updated; `tuse update` brings the new
`default.yaml`.

## 0.3.3 - 2026-11-19

- Edit a waiting steering message to take it back without interrupting the running turn.
  Enter sends the edited draft.

## 0.3.2 - 2026-11-19

- `bar` input: a blank row between the input and the status row, the status row has no indent.
  New tokens `input_top_padding` and `input_bottom_padding` add rows above and below the text;
  the bar runs through them.
- `/trace` is a list (`0 system`, `1 user`, `4 bash`...); Up and Down select, Enter opens an
  item in full, Esc goes back. `trace_role_width` is a theme token.
- Panels have a rule on top that separates them from the chat.
- A session keeps its own model: a new one records the config's model at once, `/resume` and
  `/reload` do not change an old one.

## 0.3.1 - 2026-11-19

- `tuse print` and `tuse steer` keep text that starts with `-` (`--- menu ---`); `tuse print`
  rejects empty text and strips control characters (ESC) and expands tabs, so script output
  cannot break the UI.
- `TUSE_CURRENT_CONFIG`: the path of the config file in use, set for bash calls, tasks and
  hooks. `default.yaml` tells the agent about it, about `tuse print` and about `tuse docs`.
- The input wraps at word boundaries; Up and Down move by the rows you see, not by `\n`.
- A message sent while the agent works stays in the input (dimmed) with `steering…` in place
  of the timer until the agent takes it; Esc keeps the text in the input.
  `steering_label` is a theme token.

## 0.3.0 - 2026-11-19

- The config is `--config PATH`, else `./tuse.yaml`, else `$TUSE_GLOBAL_CONFIG`, else
  `~/.config/tuse/main.yaml`. The app no longer loads `default.yaml` by itself: the shipped
  `main.yaml` includes it.
- `tuse reset` replaces the config folder with the latest `main.yaml` and `default.yaml`;
  `tuse update` updates the binary and `default.yaml`.
- `install.sh` installs the binary only; on a first install it runs `tuse reset`, then `tuse connect`.
- `theme` moved into `settings` (`settings.theme`); a top-level `theme` still works.
- `/usage`, `/trace` and a task's output open in a panel instead of a chat message.
- `/trace` shows the whole session: system prompt, messages, thinking, tool calls and outputs.
- `tuse print` shows plain multiline text in the UI and returns an ID; `--id` replaces that block.
- `tuse docs` prints the docs for the agent.
- Four input designs (`input_design`: `lines`, `box`, `fill`, `bar`), each with a working
  and a model status bar.
- A gradient runs along the input while the agent works; `motion: false` turns it off.
- `input_prefix` theme token (`""` removes it).

## 0.2.0 - 2026-11-19

- `install.sh` installs the binary and `~/.config/tuse/default.yaml`; the system prompt
  about Tuse and the default settings now live there, not in the app.
- `tuse connect`: an inline prompt that adds a provider and fetches its models.
- `/models` lists every model the providers report; `models:` only adds details
  (name, window, prices), which the list shows.
- `/theme`; the chosen theme and model are saved to the active config.
- `themes:` by key, `theme:` picks the active one, `extend:` builds on another theme.
- Theme `bash_line`, `output_line`, `reasoning_line` (`0` hides, `-1` is unlimited)
  replace `tool_command_lines`, `tool_preview_lines` and `reasoning_lines`.
- Ctrl+V and Cmd+V paste; a clipboard image becomes `[Image #n: PATH]` and is sent to the model.
- Files, prompts and images are one unit in the input.
- The `@` picker lists one folder; `@dir/*` opens `dir`.
- The turn timer shows only while the agent works.
- A `VERSION` file; `make check` and the release workflow compare it with `Cargo.toml`.

## 0.1.0 - 2026-11-19

First release.

- Full-screen TUI: Markdown replies, `/` commands, `#` prompts, `@` file picker,
  turn timer and tool-call counter, Esc to stop a turn.
- Headless mode `tuse -p`, driven with `tuse steer`.
- One `bash` tool, run as `bash -c` with a timeout and output limit.
- Sessions in `~/.tuse/<id>/` with a lock and a socket; `/resume` and `--resume`.
- `tuse ask` (blocking or `--async`) and background `tuse task run/stop/get/list`.
- YAML config: `include`, `$VAR`, `#{{command}}`, hooks, `bins`, prompts, systems, a full theme.
- 50/25/25 context compaction, retries with backoff, `/usage` with token costs.
