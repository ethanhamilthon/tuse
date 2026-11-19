# Changelog

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
