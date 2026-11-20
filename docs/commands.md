# Commands

## CLI

- `tuse [--config PATH] [--resume ID]` opens the TUI.
- `tuse -p [PROMPT|-]` runs one prompt headless: replies go to stdout, the session ID to stderr,
  and the exit code is 1 after an error. Background tasks are stopped at the end.
- `tuse connect` adds a provider.
- `tuse reset [--yes]` replaces the global config folder with the latest `main.yaml`. It deletes
  everything in that folder.
- `tuse update` updates the binary; `main.yaml` is not touched.

## TUI and slash commands

When the user sends a message in the TUI during a turn, it stays dimmed in the input until
the agent takes it. Editing the waiting message withdraws it without stopping the turn;
Enter sends the edited draft.

`/new`, `/resume`, `/models`, `/theme`, `/tasks`, `/usage`, `/trace`, `/reload`, `/quit`.
Every list (`/resume`, `/models`, `/theme`, `/tasks`, `/trace`, an `ask`, and the `/`, `#` and `@`
pickers) has one look: a rule on top, a title with the position, the rows, all drawn over the
bottom of the chat. Typing filters (in an `ask` it is the answer). `/usage` and a task's output
open as text in the same frame (Esc closes it). `/trace` is a list, one row per item (`system`,
`user`, `agent`, `thinking`, `python`, `output`, `notice`, `error`); Enter opens the item in full
(with Left/Right to step between items and Esc back to the list).

The chat scrolls with the mouse wheel only; the arrows move in the input, and a click in the input
puts the cursor there. With the mouse captured the terminal's own text selection needs a modifier
(Shift, or Option in iTerm2 and Terminal.app).

A session keeps its own model and effort. `/models` changes them in the config and in the
current session; a new session takes them from the config; `/resume` and `/reload` do not
change the model of an old session.

`/config` opens the full resolved YAML configuration, including expanded API keys. Avoid sharing it without redaction. Esc closes the panel.
