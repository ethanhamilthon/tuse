# Terminal UI (TUI)

Running `tuse` opens a full-screen terminal UI. See [commands.md](commands.md) for slash command summaries, [theme.md](theme.md) for colors and `settings.ui`, and [prompts.md](prompts.md) for `#`, `@`, and pasted image atoms.

## Terminal lifecycle and signals

- **Startup:** Enables raw mode, alternate screen, bracketed paste, mouse capture, steady bar cursor, and Kitty keyboard disambiguation.
- **Shutdown & watchdog:** Restores cursor, mouse/paste/keyboard modes, alternate screen, and raw mode on normal exit, panic, or `SIGINT` / `SIGTERM` / `SIGHUP`. A background watchdog polls signal flags and lost TTY stdin every 250 ms: if the main loop has **not yet claimed shutdown** within 2 seconds of a signal or lost TTY, the watchdog claims shutdown, closes the session, restores the terminal, and exits 0 (without printing a session ID). Once the main loop claims shutdown, the watchdog exits and does **not** impose a timeout on `close` hooks. (`SIGKILL` cannot be intercepted; run `reset` in the terminal afterward.)
- **Rendering cadence:** Streams text, reasoning, and tool deltas at up to ~30 fps (33 ms frame pacing); redraws immediately on input, resize, and discrete session events; ticks the busy spinner every 100 ms; wraps frame output in synchronized terminal updates.

## Screen layout

From top to bottom, padded horizontally on both edges by `settings.ui.pad_x`:

1. **Chat history:** Scrollable viewport anchored at the bottom, separating non-empty entries by `settings.ui.message_gap` blank lines. Pinned `show(name, text, true)` blocks always render at the very bottom after new messages; unpinned `show` blocks stay in chronological order.
2. **Optional overlay (above the input):** At most one active picker, detail panel, or `ask()` question panel, framed by a top `border` rule and capped at `settings.ui.list_max_height` rows (and available chat height).
3. **Top input rule:** `↑N ` scroll mark on the left when scrolled up; 5-second flash notice, busy working indicator (`<spinner> <elapsed>` in `accent`), or queued steering indicator on the right ([theme.md](theme.md)).
4. **Input editor:** `› ` prefix on the first visible row (`accent` when active, `border` when disabled by `pick()`), word-wrapped to the inner width, growing up to `settings.ui.input_max_height` (`-1` = terminal height minus rules) and scrolling vertically with the cursor.
5. **Bottom input rule:** Model, effort, context usage, and cumulative cost on the right.

## Chat entry rendering

- **User message:** Full-width `surface` background block with no prefix; displays submitted text (with leading and trailing whitespace trimmed on submit). If images were attached without `[Image` in the text, appends `\n[N image(s)]`. Not linkified.
- **Assistant message:** Renders a GFM Markdown subset: headings (all levels styled identically in `info` bold), emphasis, strong, strikethrough (`dim`), inline `code`, links, blockquotes (`│ `), bullet (`• `) and numbered lists, task lists (`[x]` / `[ ]`), horizontal rules, fenced/indented code blocks (with Lua syntax highlighting for `lua` fences), responsive tables (aligned columns or `Header: value` cards when narrow), and raw HTML shown as plain text. Markdown images, footnotes, and other extensions are not specially rendered. Interactive links are enabled everywhere except fenced `lua` code blocks; Markdown links with unsupported destinations (such as `#anchor` or non-openable schemes) still render in `info` + underline without being clickable or hover-highlighted.
- **Reasoning:** Prefixed with `∴ ` in `muted` italic; shows the **last** `settings.ui.reasoning_line` wrapped lines (`0` hides, `-1` shows all).
- **Tool call (`lua`):**
  - Source code prefixed with `* `, Lua syntax highlighted (or colored `err` when failed), folding multiline long strings when `settings.ui.fold_strings = true`, capped at `settings.ui.tool_line` wrapped lines (+ `…` in `border`).
  - Output in `muted`, capped at `settings.ui.output_line` lines (+ `… N more lines` in `border`).
  - Per-file diffs: file path header in `border`, `+`/`-`/context lines (`ok`/`err`/`dim`), with paired `-`/`+` horizontal windowing so edits near the end of long lines stay visible, capped at `settings.ui.diff_line` body lines (+ `… N more lines` in `border`). Setting `diff_line = 0` hides the entire diff block (no file path header, diff body lines, or footer); `-1` shows all stored diff lines.
  - Tool calls, tool output, and diffs are not linkified. Tabs expand to 4 spaces and control characters other than `\n`/`\t` are stripped.
- **Notices, errors, turn summaries:** `✓ ` in `dim` (notice), `✗ ` in `err` (error), `✓ [cancelled · ]<duration>[ · <display>/<effort>]` in `muted` (turn end).
- **Lua UI entries:** `show()` renders with `◆ ` in `accent` plus Markdown (links enabled); completed `pick()` and `ask()` render with `? ` in `accent`, prompt text (unlinked Markdown for `ask`), and `› ` in `accent` with the user's answer.
- **Task notifications:** Not rendered in chat (visible in `/trace`).

## Keyboard shortcuts

Shortcuts also work with the Russian keyboard layout when `Ctrl`, `Alt`, or `Super`/`Cmd` is held (Cyrillic letters `йцукенгшщзфывапролдячсмить` map to their QWERTY Latin equivalents when the terminal reports modified Cyrillic keys, such as under the Kitty keyboard protocol). Unmodified typing remains Cyrillic.

### Global and chat navigation

| Key | Action |
| --- | --- |
| `PageUp` / `PageDown` | Scroll chat history by one page, even when a picker or panel is open. |
| `Shift+PageUp` / `Shift+PageDown` | Scroll the open panel or `ask` question panel by one page (otherwise scroll chat history). |
| `Esc` | Close active picker -> cancel active `ask` -> close open panel (returning to `/trace` list if opened from `/trace`) -> cancel active turn, hooks, custom commands, and steering. |
| `Ctrl+C` / `Cmd+C` | Copy active chat or input selection (flashes `Copied.`); otherwise clear non-empty input; otherwise quit (ignored for quitting inside modal pickers). |
| `Ctrl+D` | Quit when the input is empty and no modal picker is open (including when a detail panel or `ask()` question panel is open with an empty editor). |

### Input editing

| Key | Action |
| --- | --- |
| `Enter` | Send trimmed message or run an exact built-in/custom `/command`; submit answer inside `ask`; choose highlighted item in a picker. |
| `Shift+Enter`, `Alt+Enter`, `Ctrl+J` | Insert a newline (also inside `ask`). |
| `Left` / `Right` | Move cursor left/right (steps over `#`/`@`/image atoms whole; in a `/trace` detail panel with empty input, step to the previous/next filtered entry). |
| `Up` / `Down` | Move across visual wrapped rows in the editor (or navigate picker; or scroll open panel/`ask` by `scroll_step` when input is empty). |
| `Home` / `End`, `Ctrl+E` | Move to start / end of current logical line. |
| `Ctrl+Left/Right`, `Alt+Left/Right`, `Alt+B`, `Alt+F` | Move by word. |
| `Shift` + arrows / `Shift+Home` / `Shift+End` | Extend input selection (`Shift+Up/Down` overrides token pickers). |
| `Ctrl+A` / `Cmd+A` | Select all input. |
| `Ctrl+X` / `Cmd+X` | Cut selected input to clipboard (in `/tasks` picker: `Ctrl+X` stops the selected task). |
| `Ctrl+V` / `Cmd+V` | Paste from clipboard (no-op when `pick()` disables input): checks clipboard image first, writes PNG to `~/.tuse/<id>/images/image-<k>.png`, and inserts `[Image #N] ` (in modal search or `ask()`, atom insertion is discarded even though the PNG file may already have been written); otherwise inserts clipboard text. Bracketed paste inserts text (newlines become spaces in modal search). |
| `Backspace` / `Delete` | Delete previous / next character or whole atom (or delete selection). |
| `Ctrl+W`, `Alt+Backspace`, `Ctrl+Backspace` | Delete previous word (or delete selection). |
| `Ctrl+U` / `Ctrl+K` | Kill to start / end of current logical line (or delete selection). |

### Pickers and overlays

| Key | Action |
| --- | --- |
| `Up` / `Down`, `Ctrl+P` / `Ctrl+N` | Move picker selection (wraps around at top/bottom). |
| `Left` / `Right` | In a `/trace` detail panel with empty input and no modifiers: open the previous / next entry in the current filtered `/trace` list (stops at the ends, resets scroll to top). |
| `Enter` | Choose item (in `@` picker: attaches highlighted file or directory as an atom). |
| `Tab` | In `@` picker: completes a directory (`@dir/`) or attaches a file; in other pickers (except `pick()`): same as `Enter`. |
| `Esc` | Dismiss token picker (stays suppressed until token text changes), close modal picker (restoring draft), or cancel `pick()` (returning `nil`). |
| `Ctrl+X` | In `/tasks` picker: stop the selected running task. |

## Mouse interactions

- **Wheel:** Scrolls the picker (clamped, no wrap), panel/`ask` (by `scroll_step`), or chat history (by `scroll_step`) depending on whether the pointer is over the overlay or chat area.
- **Click in input:** Moves the editor cursor to the clicked visual cell (accounting for wide characters and atoms).
- **Drag:** Dragging inside the input selects input text; dragging elsewhere selects rendered screen cells across lines (trailing spaces per line trimmed), highlighted in `bg` on `accent`. Press `Ctrl/Cmd+C` to copy.
- **Interactive links:**
  - Active in: agent Markdown messages (outside `lua` code fences), `show()` blocks, `/config`, `/session`, and `system`/`user`/`agent` detail panels in `/trace`.
  - Not active in: user chat messages, `ask()` panels, Lua code, tool output, diffs, or plain-text `/trace` panels.
  - Detects bare `http://`/`https://` URLs (trimming trailing punctuation while preserving balanced parentheses), existing local file paths (absolute, `~/`, `./`, `../`, relative with `/` or `name.ext`, with optional `:line[:col]`), and Markdown `[label](dest)` links (`http://`, `https://`, `mailto:`, `file://`, or local path).
  - Resting clickable links render in `accent` (underlined); hovering or pressing the left mouse button highlights all visible spans for the same link target in `fg` (bold, underlined), returning to `accent` when the pointer leaves or press-only state clears (with no visited color). Scrolling, resizing, or dragging clears stale link hover.
  - Left-clicking (press and release on the same cell without dragging) opens URLs (`http://`, `https://`, `mailto:`, under 2,000 characters) via the OS opener, and opens existing local paths via a matching `open.<ext>` Lua callback (case-insensitive extension) or the OS default (`open` on macOS, `xdg-open` on Linux, `cmd /C start ""` on Windows). Open failures emit a chat error.

## Pickers, panels, and slash workflows

All pickers filter item **labels** only (never secondary detail columns) using case-insensitive substring matching.

- **Token pickers (inline, keep your draft):**
  - `/` lists all 9 built-in commands plus custom `commands` from config; selecting executes immediately and removes the `/token`.
  - `#` lists configured `prompts` (label `#name`, first-line preview in detail); selecting inserts `#name `.
  - `@` lists up to 20,000 entries of the target folder (skipping `.git`); supports relative paths, `/`, and `~/`.
- **Modal searchable pickers (input becomes a search box matching labels; draft saved and restored on close):**
  - `/resume`: other named sessions in the current `cwd` sorted by `updated DESC` (label = title, detail = timestamp). Flashes `No other sessions in this folder.` if none exist.
  - `/models`: lists available models (label = display name or `provider/model`, detail = raw `name` when display differs, `unavailable` if disconnected, window, prices) plus the current model even when unavailable or empty, and triggers a background provider model fetch that refreshes the list live while preserving your query and selection. Selecting an available model opens the Effort picker (`default`, `low`, `medium`, `high`, `xhigh`, `max`).
  - `/tasks`: lists background tasks (label = `<id> <name>`, detail = `<status>`), updating live on task changes. `Enter` opens a scrollable panel with the last 200 log lines; `Ctrl+X` stops the selected task. Flashes `No tasks. The agent starts them with \`spawn()\`.` if empty.
  - `/trace`: lists `index  role  summary` for every trace entry. `Enter` opens the full entry in a scrollable panel (`system`/`user`/`agent` as Markdown with interactive links, `lua` as full unfolded syntax-highlighted source, all other roles as plain text). Inside the detail panel, unmodified `Left` and `Right` when the input editor is empty step to the previous or next entry in the current filtered `/trace` list without wrapping (keeping the current entry and scroll position at the ends, resetting scroll to the top on switch, clearing stale mouse selection and link hover, and preserving the search query; typing into the editor or holding modifiers keeps normal editor cursor movement). `Esc` returns to the `/trace` picker with the search query preserved and the last-viewed entry selected and scrolled into view; a second `Esc` closes `/trace` and restores your draft.
- **Detail panels:**
  - `/session`: scrollable view of `ID`, `Name`, `Mode`, `CWD`, `Home`, `Config`, `Provider`, `Model`, `Effort`, `Model info`, `Context`, `Turns`, `Usage`, and `Cost` (`CWD`, `Home`, `Config` are clickable when present).
  - `/config`: scrollable view of `Config file`, `Settings`, `Theme` (with color swatches `■`), `System prompts`, `Prompts`, `Functions` (marking built-in overrides as `[override]` and `false` entries as `[disabled]`), `Hooks`, `Commands`, and `Open`. Never shows secrets or Lua function bodies.
- **Lua UI prompts (`pick` and `ask`):**
  - `pick(options)` opens a non-searchable modal picker and disables the editor until the user picks (`Enter`) or cancels (`Esc` -> `nil`).
  - `ask(text)` opens a scrollable unlinked Markdown question panel and clears the editor for the answer (saving/restoring the user's draft); `Enter` returns the typed answer without sending it to the model, and `Esc` returns `nil`.
- **Steering while busy:** Pressing `Enter` during a running turn queues the trimmed message, dims the input text (`dim`), and switches the top bar to `steering…`. Moving the cursor keeps it queued; any text modification withdraws it from the queue without interrupting the turn so you can edit and press `Enter` again. When the agent consumes the message, the input clears automatically.
- **Reloading (`/reload` vs Lua `reload()`):**
  - TUI `/reload` validates and reloads config in the same Lua VM; on success, restyles the palette, **rebuilds the chat view** from the session (preserving live `show()` and UI result entries so updated `settings.ui` rules apply to existing entries), and flashes `Config reloaded.`. On failure, prints `Reload failed: <error>` and keeps the previous working config active.
  - A reload triggered from Lua (`reload()`) emits a `reloaded` event, which restyles the palette and invalidates cached chat entries and open panels so updated `theme` and `settings.ui` rules re-render immediately at the same terminal width.
