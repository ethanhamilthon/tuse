# Commands

## CLI

- `tuse [--config PATH] [--resume ID]` opens the TUI. `--config` (`-c`) picks the config;
  `--resume` (`-r`) opens a saved session. Tuse prints the session ID on exit.
- `tuse json [-c PATH] [-r ID]` serves one session over [JSON-RPC stdio](json.md).
- `tuse connect --base URL --key '$ENV' --name NAME` verifies and saves a provider.
  Quote `$ENV` to store an environment-variable reference rather than passing a literal key.
- `tuse providers` lists providers; `tuse disconnect NAME` removes one.
- `tuse models` lists model metadata from the effective Lua config and the default model/effort from SQLite.
- `tuse init` creates `./tuse.lua` that starts from the global config (`require("global")`).
- `tuse plugins ...` manages [plugins](plugins.md).
- `tuse update` updates the binary without touching your config.

## TUI

- `/new` starts a session; `/resume` picks a saved session from the current folder.
- `/models` sets the session model and effort and remembers the pick in SQLite for new sessions.
  `/reload` and `/resume` do not change an existing session's model.
- `/tasks` lists tasks; Enter shows the log and Ctrl+X stops the selected task.
- `/session` shows every field of `require("tuse").session()` except `trace`: ID, name, mode, CWD,
  home, config path, provider, model, effort, model info, context, turns/steps, usage, and cost.
- `/trace` opens a selectable list of trace entries (`index  role  first line`). Press Enter to
  view the full entry in a scrollable detail panel (`system`, `user`, and `agent` as Markdown;
  `lua` as full syntax-highlighted source; other roles as plain text) and Esc to return to the
  list with your selection intact.
- `/config` shows a styled, scrollable view of the resolved config: the config file path first,
  then settings, theme, system prompts, prompts, functions, hooks, commands, `open` handlers, and
  plugins, without provider secrets or Lua function bodies.
- `/reload` validates and runs config files again (if validation fails, the previous working
  config and callbacks remain active); `/quit` exits.
- Custom slash commands defined in `commands = { name = function() ... end }` appear in `/`
  autocomplete and run as non-blocking Lua jobs (so they can call `show_markdown`, `show_picker`,
  and `show_question` through `require("tuse")`). Built-in commands cannot be overridden.

Pickers match substrings without regard to case. When a picker uses the input as a search box,
your draft comes back on close. A message queued during a turn stays dimmed in the input until the
agent takes it. Editing it withdraws it; Enter sends the edited draft.

Use the mouse wheel to scroll the chat, click an input cell to place the cursor, and drag to
select text. Ctrl/Cmd+C copies it. URLs, local file paths (absolute, `~/`, relative, with optional
`:line[:col]`), and Markdown links are interactive in agent messages, `show_markdown` blocks,
`/config`, `/session`, and the agent, system, and user entries in `/trace`. Lua code, tool output,
and diffs are not linkified. Hover to highlight and click to open. Local paths use a matching `open.<ext>` Lua callback if configured, otherwise the OS
default opener (`open` on macOS, `xdg-open` on Linux, `cmd /C start` on Windows); open failures
are reported in the UI. Shift+movement extends an input selection; Ctrl/Cmd+A selects all and
Ctrl/Cmd+X cuts.
