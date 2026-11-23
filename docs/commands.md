# Commands

## CLI

- `tuse [--config PATH] [--resume ID]` opens the TUI. `--config` (`-c`) picks the config;
  `--resume` (`-r`) opens a saved session. Tuse prints the session ID on exit.
- `tuse json [-c PATH] [-r ID]` serves one session over [JSON-RPC stdio](json.md).
- `tuse connect --base URL --key '$ENV' --name NAME` verifies and saves a provider.
  Quote `$ENV` to store an environment-variable reference rather than passing a literal key.
- `tuse providers` lists providers; `tuse disconnect NAME` removes one.
- `tuse update` updates the binary without touching your config.

## TUI

- `/new` starts a session; `/resume` picks a saved session from the current folder.
- `/models` sets the session model and effort and remembers the pick for new sessions.
  `/reload` and `/resume` do not change an existing session's model.
- `/tasks` lists tasks; Enter shows the log and Ctrl+X stops the selected task.
- `/usage` shows context, tokens and spend. The model window comes from config; unknown is `?`.
- `/trace` opens the full session log in one scrollable panel, starting at the latest entries.
  Use the wheel or Page Up/Down to scroll and Esc to close.
- `/config` shows the resolved config without providers or Lua function bodies.
- `/reload` runs config files again (updating custom commands and keeping live `show_markdown`
  blocks and picker/question results); `/quit` exits.
- Custom slash commands defined in `commands = { name = function() ... end }` appear in `/`
  autocomplete and run as non-blocking Lua jobs (so they can call `show_markdown`, `show_picker`,
  and `show_question`). Built-in commands cannot be overridden.

Pickers match substrings without regard to case. When a picker uses the input as a search
box, your draft comes back on close. A message queued during a turn stays dimmed in the
input until the agent takes it. Editing it withdraws it; Enter sends the edited draft.

Use the mouse wheel to scroll the chat, click an input cell to place the cursor, and drag
to select text. Ctrl/Cmd+C copies it. Click a highlighted link to open it. Shift+movement
extends an input selection; Ctrl/Cmd+A selects all and Ctrl/Cmd+X cuts.
