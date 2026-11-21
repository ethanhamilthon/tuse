# Commands

## CLI

- `tuse [--config PATH] [--resume ID]` opens the TUI.
  - `-c`, `--config PATH`: use this config file instead of `./tuse.yaml` or the global
    `main.yaml` (relative to the working directory).
  - `-r`, `--resume ID`: open the saved session `ID` (the ID Tuse prints to stderr on exit, or
    the folder name in `~/.tuse/`) instead of a new one; `/resume` picks one inside the TUI.
- `tuse -p [PROMPT|-]` (`--print`) runs one prompt headless, also with `-c` and `-r`. See
  `docs("headless")`.
- `tuse json [-c PATH] [-r ID]` serves the session over JSON-RPC 2.0 on stdin/stdout. See
  `docs("json")`.
- `tuse connect` adds a provider.
- `tuse reset [--yes]` replaces the global config folder with the latest `main.yaml`. It deletes
  everything in that folder.
- `tuse update` updates the binary; `main.yaml` is not touched.

## TUI and slash commands

When the user sends a message in the TUI during a turn, it stays dimmed in the input until
the agent takes it. Editing the waiting message withdraws it without stopping the turn;
Enter sends the edited draft.

`/new`, `/resume`, `/models`, `/theme`, `/tasks`, `/usage`, `/maxsize`, `/trace`, `/config`, `/reload`, `/quit`.
The input status shows context use as `used/max`, e.g. `5.2k/1m`. `/maxsize 200k` (or `1m`) sets the maximum for this run, `/maxsize off` goes back to the model's `window`; with neither it shows `5.2k/?`.
A question that arrives while another list is open waits and appears when that list closes.
Every list (`/resume`, `/models`, `/theme`, `/tasks`, `/trace`, an `ask`, and the `/`, `#` and `@`
pickers) has one look: a rule on top, a title with the position, the rows, all drawn over the
bottom of the chat. Typing filters most lists: the input becomes the search box (one line, with the usual editing keys and mouse), your draft is set aside and comes back when the list closes. In an `ask`, select an option or choose “Write your own answer” to edit a multiline reply without losing your draft. Page Up and Page Down scroll a long question in either answer mode. `/usage` and a task's output
open as text in the same frame (Esc closes it). `/trace` is a list, one row per item (`system`,
`user`, `agent`, `thinking`, `python`, `tool`, `output`, `notification`, `notice`, `error`, `hook`); Enter opens the item in full
(with Left/Right to step between items and Esc back to the list). In the query, `+type` shows only
that type (`+python+agent`: either), `-type` hides one (`-hook`: all but hooks), and other words
search the rows (`+python read`). An unknown type shows nothing.

The chat scrolls with the mouse wheel only; the arrows move in the input, and a click in the input
puts the cursor there. Links (Markdown, bare `https://` URLs, OSC 8 in `draw_ui`) light up under the pointer and open on click. Drag to select rendered text; Ctrl/Cmd+C copies it. In the input, Shift+movement extends the selection, Ctrl/Cmd+A selects all, and Ctrl/Cmd+X cuts.

A session keeps its own model and effort. `/models` changes them in the config and in the
current session; a new session takes them from the config; `/resume` and `/reload` do not
change the model of an old session.

`/config` opens the full resolved YAML configuration, including expanded API keys. Avoid sharing it without redaction. Esc closes the panel.

Shortcuts also work with the Russian keyboard layout when the terminal reports modified Cyrillic keys (for example, with the kitty keyboard protocol).
