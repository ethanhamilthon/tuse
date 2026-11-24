# Planned: Navigate `/trace` Entries with Left and Right (TODO #1)

Status: Planned (not yet implemented).

## Goal

Allow stepping directly to the previous or next entry from inside a `/trace` detail panel using `Left` and `Right` without closing the panel and returning to the `/trace` picker.

## Target behavior

While a `/trace` detail panel is open (entered by pressing `Enter` on an item in the `/trace` picker, which moves the search query aside and leaves the input editor empty):

1. **`Left`:** Opens the previous entry in the `/trace` picker's current filtered list (`selected - 1`).
2. **`Right`:** Opens the next entry in the `/trace` picker's current filtered list (`selected + 1`).
3. **Stop at ends (no wrap):**
   - Pressing `Left` on the first filtered entry (`index == 0`) is a no-op.
   - Pressing `Right` on the last filtered entry (`index == len - 1`, including a single-item filtered list) is a no-op.
   - **Decision:** When stopped at either end, keep the current entry and its current scroll position unchanged (do not jump back to the top).
4. **Reset scroll on switch:** Whenever `Left` or `Right` switches to a different entry, start the newly opened entry scrolled to the top (`scroll = 0`).
5. **Role-aware rendering:** Rebuild the detail panel using the exact same rendering rules as opening from `/trace`:
   - `system`, `user`, and `agent`: Markdown with interactive links enabled (outside fenced `lua` code blocks).
   - `lua`: full unfolded syntax-highlighted Lua source (not linkified).
   - All other roles (`thinking`, `output`, `tool`, `notification`, `notice`, `error`, `hook`): plain text (not linkified).
6. **Preserve search query and synchronize selection:**
   - Keep the `/trace` search query intact while stepping between entries.
   - Update the underlying `/trace` picker's selection to the newly viewed filtered entry (and keep it scrolled into view when returning to the picker).
   - Pressing `Esc` closes the detail panel and returns to the `/trace` picker with the original search query active and the last-viewed entry highlighted (visible within the picker viewport).
   - Pressing `Esc` a second time closes `/trace` and restores the user's pre-`/trace` input draft.
7. **Input gating and unchanged arrow behavior elsewhere:**
   - **Decision (deliberate empty-editor gating):** Because opening a `/trace` detail panel starts with an empty editor and panels still allow typing a message into the editor if the user begins typing, only unmodified `Left` and `Right` when the input editor is **empty** navigate trace entries (matching how `Up` and `Down` scroll panels only when the input editor is empty). If the user has typed text into the editor while the detail panel is open, or holds `Shift`/`Ctrl`/`Alt`, `Left`/`Right` keep their normal editor cursor/selection behavior rather than switching entries. (Alternative: capture unmodified `Left`/`Right` unconditionally in `/trace` details if editing while viewing trace details is not desired.)
   - Outside a `/trace` detail panel (`/session`, `/config`, `/tasks` log panel, `ask()` question panel, all pickers, and normal chat input), `Left` and `Right` behavior is completely unchanged.

## Defaults

No new config keys or settings. Behavior is always enabled in the TUI.

## Edge cases

- **Filtered list with non-contiguous trace indices:** If the search filter matches trace entries `[0, 4, 7]`, pressing `Right` from entry `0` opens entry `4` (not `1`), and pressing `Left` from `7` opens `4`.
- **Single-entry or empty filtered list:** `Left` and `Right` are no-ops.
- **Scroll viewport on return to picker:** If the user steps past the bottom (or above the top) of the picker's previous visible window and presses `Esc`, the picker adjusts its scroll offset so the newly selected entry is visible.
- **Mouse selection / link hover:** Switching entries clears any stale mouse text selection and updates hover state on the next draw so links from the previous entry cannot be accidentally clicked or highlighted.
- **Switching across formats:** Stepping between Markdown, Lua, and plain-text entries re-renders and re-wraps cleanly for the new entry's format.

## Error reporting

None. Reaching either end of the filtered list stops silently without notices, flashes, or errors. Out-of-bounds indices are guarded as no-ops rather than panicking.

## Baseline docs to update before implementation

Before handing off implementation (docs-first gate), update and review:

- [../commands.md](../commands.md) — `/trace` bullet in the TUI section (mention `Left`/`Right` navigation in the detail panel).
- [../tui.md](../tui.md) — Keyboard shortcuts (add `Left`/`Right` in `/trace` detail panel when input is empty) and the `/trace` entry under Pickers, panels, and slash workflows.

## Acceptance tests

1. Unfiltered `/trace`: open entry `0`, press `Right` -> opens entry `1` at scroll `0`; press `Right` -> opens entry `2`; press `Left` -> returns to entry `1` at scroll `0`.
2. Filtered `/trace`: filter so non-adjacent entries match (e.g. only `lua` entries), open the first match, press `Right` -> opens the second matching `lua` entry (skipping non-matching entries); press `Esc` -> returns to `/trace` picker with the same search query text in the editor and the second match selected.
3. Scroll reset: scroll down inside entry `1`, press `Right` to open entry `2` -> entry `2` starts at scroll `0`; press `Left` back to entry `1` -> entry `1` starts at scroll `0`.
4. List ends: scroll down inside entry `0`, press `Left` -> stays on entry `0` at the same scroll offset; on the last entry, scroll down and press `Right` -> stays on the last entry at the same scroll offset (no wrap).
5. Picker viewport follow on `Esc`: with more filtered entries than `list_max_height`, step `Right` past the initial picker viewport and press `Esc` -> the selected entry is visible inside the picker window.
6. Second `Esc`: pressing `Esc` again from the `/trace` picker restores the user's original draft.
7. Role rendering: stepping across `system` (Markdown), `lua` (highlighted unfolded source), and `output` (plain text) renders each entry in its proper format.
8. Empty-editor gating & isolation: typing into the editor while a `/trace` detail panel is open causes `Left`/`Right` to move the editor cursor instead of switching entries; clearing the editor re-enables `Left`/`Right` trace navigation; modified `Left`/`Right` and arrows in `/session`, `/config`, `/tasks` log panels, `ask()` panels, pickers, and normal chat editing behave unchanged.
