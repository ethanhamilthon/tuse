# Planned: Hide Entire Diff Block When `diff_line = 0` (TODO #12)

Status: Planned (not yet implemented).

## Goal

When `settings.ui.diff_line = 0`, suppress the entire diff block for every modified file in TUI chat tool entries—no file path header, no diff body lines, no `… N more lines` footer, and no leftover blank lines—while keeping stored diffs, `/trace`, and ordinary tool output intact.

## Target behavior

1. **`settings.ui.diff_line = 0`:**
   - Render zero lines for every diff attached to a tool call in the TUI chat.
   - Omit the file path header line, all diff body lines, the `… N more lines` footer, and any trailing/extra spacing.
   - Ordinary tool source rendering (`settings.ui.tool_line`) and ordinary tool output (`settings.ui.output_line`, such as `edited path (1 replacement)`) are unchanged.
   - If a tool call has `tool_line = 0`, `output_line = 0`, and `diff_line = 0`, the tool entry renders zero lines and produces no extra `message_gap` blank lines in the chat viewport.
2. **`settings.ui.diff_line > 0` (default `12`):**
   - Unchanged: renders the file path header (`border`), up to `diff_line` body lines (with `+`/`-`/context styling and paired-line horizontal windowing), and `… <remaining> more lines` in `border` when truncated.
3. **`settings.ui.diff_line = -1`:**
   - Unchanged: renders the file path header and **all stored diff lines** with no `… N more lines` footer.
4. **Live updates on reload and resume:**
   - Re-renders already-present chat entries immediately at the **same terminal width** after TUI `/reload` **and** after Lua `reload()` (the `reloaded` event) whenever `settings.ui.diff_line` changes—hiding previously visible diffs (`12` or `-1` -> `0`), restoring **truncated** output (`0` -> `12` on a diff longer than 12 lines), or restoring all **stored** diff lines (`0` -> `-1`).
   - Applies immediately when resuming a session (`tuse -r ID` or `/resume`) using the resumed session's active config.
5. **Storage and non-TUI outputs unchanged:**
   - Stored `tool` record `diffs` continue to be computed and saved in SQLite identically.
   - `/trace` `output` detail entries continue to include full stored diffs.
   - JSON-RPC `tool_end` events and `session.records` continue to emit full stored `diffs`.

## Defaults

- `settings.ui.diff_line` remains `12`.
- Valid values remain integers `-1` or `>= 0`.

## Edge cases

- **Multiple modified files in one `lua` call:** All file diff blocks are omitted when `diff_line = 0`, leaving zero extra lines between the tool output and the next chat entry.
- **Empty diff string (`diff == ""`):** When `diff_line = 0`, also renders zero lines (no file path header).
- **Reload triggered via Lua `reload()` at the same width:** Changing `diff_line` via Lua `reload()` (from a custom command, hook, or `lua` tool call) must update already-displayed chat entries immediately without requiring a terminal resize.
- **Resuming with different configs:** A session created with `diff_line = 0` renders diffs according to the active config when later resumed with `diff_line = 12` or `-1`, and vice versa.

## Error reporting

Config validation is unchanged: non-integer values or integers `< -1` fail validation with the existing range error (`value <N> out of range (-1..=2147483647)`).

## Baseline docs to update before implementation

Before handing off implementation (docs-first gate), update and review:

- [../theme.md](../theme.md) — `settings.ui` table row for `diff_line` (state that `0` hides the entire diff block, matching `tool_line`, `output_line`, and `reasoning_line`).
- [../config.md](../config.md) — `settings.ui` table row for `diff_line` (change `0 suppresses body` to `0 hides the entire diff block` and `-1` to `all stored diff lines`).
- [../tui.md](../tui.md) — `Chat entry rendering` tool call diff bullet (remove the note about `diff_line = 0` still showing header and footer) and `Reloading` note (if Lua `reload()` now re-renders existing chat entries at the same width).

## Acceptance tests

1. Regression test for `diff_line = 0`: a tool call with one or multiple modified file diffs renders tool code and tool output normally, and zero diff lines (no file path, no `+`/`-` lines, no `more lines` footer, no trailing blank lines).
2. `diff_line = 0` combined with `tool_line = 0` and `output_line = 0`: tool entry renders 0 lines and adds no extra `message_gap` between adjacent entries.
3. Positive `diff_line` (e.g. `2` on a 10-line diff): renders path header, 2 diff lines, and `… 8 more lines`.
4. `diff_line = -1` (on a 20-line stored diff): renders path header and all 20 stored lines with no `more lines` footer.
5. Same-width reload tests (both TUI `/reload` and Lua `reload()` without resizing the terminal):
   - Changing `diff_line` from `12` to `0` hides diffs on already-rendered entries (including a 20-line diff).
   - Changing `diff_line` back from `0` to `12` restores **truncated** diff output (path header, 12 lines, and `… 8 more lines`).
   - Changing `diff_line` from `0` to `-1` restores all 20 **stored** diff lines with no truncation footer.
6. Resume test: resuming a session with `diff_line = 0` hides diffs in TUI chat while persisted session records (`session.records`) and `/trace` still contain the full stored diffs.
