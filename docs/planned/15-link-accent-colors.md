# Planned: Use Accent Colors for Interactive Links (TODO #15)

Status: Planned (not yet implemented).

## Goal

Render clickable URLs, local file paths, and Markdown links in `theme.accent` by default and `theme.fg` on hover or click, returning to `theme.accent` when the pointer leaves, with no persistent visited color and no changes to link opening behavior.

## Target behavior

1. **Default (resting) link style:**
   - Every **clickable** link—auto-detected bare URLs (`http://`, `https://`), auto-detected existing local paths (with optional `:line[:col]`), clickable path fields in `/session` and `/config`, and openable Markdown `[label](dest)` links—renders with foreground `theme.accent` and `underline`.
   - **Decision (preserve existing non-openable Markdown link appearance):** Markdown links whose destination cannot be resolved to an openable target (such as `#anchor` or an unsupported scheme like `javascript:...`) are not clickable; keep today's visual styling (`theme.info` and `underline`, with no hover or click behavior) rather than altering non-clickable Markdown appearance outside the scope of TODO #15.
2. **Hover state:**
   - When the mouse pointer hovers over any cell belonging to a clickable link, **all** visible spans for that link target—including every segment of a link wrapped across multiple lines—switch foreground to `theme.fg`.
   - **Decision (target-level hover scope):** Keep target-level hover highlighting (matching today's behavior): when the same URL or path appears multiple times on screen, hovering one copy highlights all visible copies of that target together, and leaving returns all of them to `theme.accent`. (Alternative: per-occurrence highlighting if independent highlighting of duplicate targets is preferred.)
   - **Decision (hover emphasis):** Keep `underline` and `bold` on hover/click (matching today's hover emphasis, preserving visual contrast even in themes where `accent` and `fg` are close).
3. **Click (press) state and pointer leave:**
   - Pressing the left mouse button on a clickable link cell activates the `theme.fg` highlight immediately (even if the terminal did not deliver a prior mouse-move hover event).
   - Dragging away from the link cell clears link hover/active styling and switches to normal text selection (`theme.bg` on `theme.accent`).
   - Releasing the left mouse button on the same cell opens the link using the existing URL/path opener logic (`open.<ext>` or OS default opener) without modification.
   - When the pointer moves off the link (or the viewport scrolls/changes so the link is no longer under the pointer), the link immediately returns to `theme.accent` (resting style).
   - **No visited state:** Tuse never stores or displays a separate visited color after a link is clicked.
4. **Interactive surfaces unchanged:**
   - Active in: agent Markdown messages (outside fenced `lua` code blocks), `show()` Markdown blocks, `/config`, `/session`, and `system`/`user`/`agent` detail panels in `/trace`.
   - Not linkified: user chat messages, `ask()` question panels, Lua tool calls, tool output, diffs, fenced `lua` code blocks, and plain-text `/trace` entries.

## Defaults

- Uses existing `theme.accent` (`#BDDEAB`) for resting clickable links and `theme.fg` (`#DEDEDE`) for hovered/clicked clickable links.
- No new theme tokens or config keys; user overrides of `theme.accent` and `theme.fg` apply automatically (including after `/reload`).

## Edge cases

- **Wrapped links:** A URL, path, or Markdown link split across 2+ visual lines highlights every wrapped segment in `theme.fg` when any segment is hovered or pressed, and returns every segment to `theme.accent` on leave.
- **Duplicate targets on screen:** Under target-level hover, hovering any occurrence of `https://example.com` highlights all visible occurrences in `theme.fg`; moving off restores all occurrences to `theme.accent`.
- **Click without prior hover events:** On terminals that report press/release without continuous motion events, pressing sets the active link highlight to `theme.fg` and releasing clears the press-only highlight (while motion-tracked hover stays synced with pointer position) so links never stay stuck in `theme.fg` after clicking.
- **Scroll or content shift under stationary pointer:** Scrolling chat/panel or resizing clears stale hover state until the next mouse event so a different link at the old screen position is not falsely highlighted.
- **Theme values set to `"none"`:** When `theme.accent` or `theme.fg` is `"none"`, links fall back cleanly to the terminal default color with underline/bold attributes preserved.

## Error reporting

Unchanged. Unopenable URLs, missing files, OS opener failures, and `open.<ext>` Lua errors continue to emit chat errors identically.

## Baseline docs to update before implementation

Before handing off implementation (docs-first gate), update and review:

- [../theme.md](../theme.md) — `The 10 tokens` table (`accent` gains resting clickable links; `fg` gains hovered/clicked clickable links; `info` retains only non-openable Markdown links if preserved, or drops clickable links).
- [../tui.md](../tui.md) — `Mouse interactions` -> `Interactive links` (update resting clickable color from `info` to `accent` and hover/click color from `accent` to `fg`).

## Acceptance tests

1. Default color: clickable bare URLs, existing local paths (including `:line[:col]`), explicit path rows in `/session` and `/config`, and openable Markdown links all render with `fg = theme.accent` and `underline = true` (not `theme.info`).
2. Hover color (wrapped & repeated targets): hovering any cell of a clickable link renders all spans for that target—including wrapped segments across multiple lines and duplicate visible occurrences of the same target—in `fg = theme.fg` (`underline = true`, `bold = true`).
3. Leave behavior: moving the pointer off the link restores all spans back to `fg = theme.accent` with no persistent visited color.
4. Click behavior: left-button down on a link highlights it in `theme.fg`; releasing on the same cell opens the target identically to today; after the pointer leaves (or press-only state clears), the link renders in `theme.accent`.
5. Dragging starting on a link clears link hover and activates text selection (`theme.bg` on `theme.accent`) without opening the link.
6. Non-clickable surfaces (user messages, Lua tool calls, tool output, diffs, fenced `lua` code, plain-text `/trace`, `ask()` panels) remain unlinked, and non-openable Markdown links (e.g. `#anchor`) remain non-interactive in `theme.info` + `underline`.
7. `/reload` with changed `theme.accent` or `theme.fg` updates resting and hover link colors accordingly.
