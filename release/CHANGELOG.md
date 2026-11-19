# Changelog

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
