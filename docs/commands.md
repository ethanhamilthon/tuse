# Commands

## CLI

- `tuse [--config PATH] [--resume ID]` opens the TUI.
  - `-c`, `--config PATH`: use this config file instead of `./tuse.lua` or the global
    `main.lua` (relative to the working directory).
  - `-r`, `--resume ID`: open the saved session `ID` (the ID Tuse prints to stderr on exit, or
    the folder name in `~/.tuse/`) instead of a new one; `/resume` picks one inside the TUI.
- `tuse json [-c PATH] [-r ID]` serves the session over JSON-RPC 2.0 on stdin/stdout. See
  `docs("json")`.
- `tuse connect` adds a provider.
- `tuse reset [--yes]` replaces the global config folder with the latest `main.lua`. It deletes
  everything in that folder.
- `tuse update` updates the binary; `main.lua` is not touched.
- `tuse connect` adds a provider, `tuse providers` lists them, `tuse disconnect NAME` removes one;
  they live in `~/.tuse/tuse.db`.

## TUI and slash commands

When the user sends a message in the TUI during a turn, it stays dimmed in the input until
the agent takes it. Editing the waiting message withdraws it without stopping the turn;
Enter sends the edited draft.

`/new`, `/resume`, `/models`, `/theme`, `/tasks`, `/childrens`, `/usage`, `/maxsize`, `/trace`, `/config`, `/reload`, `/quit`.
The input status shows context use as `used/max`, e.g. `5.2k/1m`. `/maxsize 200k` (or `1m`) sets the maximum for this run, `/maxsize off` goes back to the model's `window`; with neither it shows `5.2k/?`.
A question that arrives while another list is open waits and appears when that list closes.
Every list (`/resume`, `/models`, `/theme`, `/tasks`, `/childrens`, `/trace`, and the `/`, `#` and `@`
pickers) has one look: a rule on top, a title with the position, the rows, all drawn over the
bottom of the chat. Typing filters most lists: the input becomes the search box (one line, with the usual editing keys and mouse), your draft is set aside and comes back when the list closes. `/usage` and a task's output
open as text in the same frame (Esc closes it). `/trace` is a list, one row per item (`system`,
`user`, `agent`, `thinking`, `lua`, `tool`, `output`, `notification`, `notice`, `error`, `hook`); Enter opens the item in full
(with Left/Right to step between items and Esc back to the list). In the query, `+type` shows only
that type (`+lua+agent`: either), `-type` hides one (`-hook`: all but hooks), and other words
search the rows (`+lua read`). An unknown type shows nothing.

`/childrens` lists all descendants of the session, at any depth, running and finished: sessions whose
meta has `parent` set (from `TUSE_PARENT`, see `env.md`), indented per level. A row shows the
name (title or short ID), `running` (its session lock is held) or `done`, the elapsed time,
and input+output tokens and the spend (only for models with `prices`; the detail view also has the folder, cached tokens and context used/max, `?` when the model's window is unknown). A row with children also shows `all` (subtree) totals. Rows are compact (status, elapsed, total tokens and spend); Enter opens every field of the row (full ID, folder, in/out/cached, context, spend, subtree totals), Esc goes back to the list. A child that sets `TUSE_PARENT` shows up as soon as it starts, before its first prompt. Nothing is locked or changed.

The chat scrolls with the mouse wheel only; the arrows move in the input, and a click in the input
puts the cursor there. Links (Markdown, bare `https://` URLs, OSC 8) light up under the pointer and open on click. Drag to select rendered text; Ctrl/Cmd+C copies it. In the input, Shift+movement extends the selection, Ctrl/Cmd+A selects all, and Ctrl/Cmd+X cuts.

A session keeps its own model and effort. `/models` changes them in the config and in the
current session; a new session takes them from the config; `/resume` and `/reload` do not
change the model of an old session.

`/config` opens the resolved configuration as JSON (functions and providers left out). Esc closes the panel.

Shortcuts also work with the Russian keyboard layout when the terminal reports modified Cyrillic keys (for example, with the kitty keyboard protocol).
