# Planned: Simplify the TUI Working Indicator (TODO #5)

Status: Planned (not yet implemented).

## Goal

Remove transient per-call activity labels (such as `reading PATH...`, `writing PATH...`, `running CMD...`) from the TUI working indicator while keeping the spinner, elapsed time, queued steering indicator, flash notices, tool chat rendering, and logs unchanged.

## Target behavior

In the TUI top input rule (right side):

1. **Active turn (no flash notice, no queued steering message):**
   - Render only `<spinner> <elapsed>` in `theme.accent` (for example `◐ 0s`, `◓ 14s`, `◑ 1m5s`, `◒ 1h2m`).
   - Do **not** append ` · <action>` or any transient host-function activity text (`reading ...`, `writing ...`, `appending ...`, `deleting ...`, `editing ...`, `searching ...`, `running ...`).
   - Keep the existing 4-frame spinner (`◐`, `◓`, `◑`, `◒`), 100 ms animation tick, and elapsed-time formatting (`Ns`, `NmNs`, `NhNm`).
2. **Queued steering message:**
   - Unchanged: while a message is queued in the input during a running turn, render `<wave> ` in `theme.accent` followed by `steering…` in `theme.muted` (8-frame `●····` bounce).
3. **Flash notices:**
   - Unchanged: a flash notice (such as `Copied.`, `Config reloaded.`, or retry status `Retry A/M in Ds: ...`) replaces the working indicator in `theme.accent` for 5 seconds, after which the simplified working indicator (if still busy) reappears.
4. **Idle session:**
   - Unchanged: empty (no spinner or elapsed time).
5. **Unchanged surfaces:**
   - Tool call source, output, and diffs in the TUI chat remain unchanged.
   - `/trace` entries and background task logs (`tasks/<id>.log`, `/tasks`) remain unchanged.
   - **Decision:** Retain core `status` session events and JSON-RPC `event` notifications with `type: "status"` unchanged for protocol v1 backward compatibility. Only the TUI working indicator stops rendering transient activity labels.

## Defaults

No new config keys or theme tokens.

## Edge cases

- **Narrow terminal width:** The working indicator still obeys the half-width budget (after subtracting any `↑N ` scroll mark width) and clips cleanly with `…`.
- **Rapid tool calls:** Rapid consecutive file, search, or shell calls display a steady `<spinner> <elapsed>` without transient activity label text or label flicker.
- **Withdrawing a steered message while busy:** Editing the dimmed queued message withdraws it and switches the top rule back to `<spinner> <elapsed>`.

## Error reporting

Not applicable (display simplification; no new error conditions).

## Baseline docs to update before implementation

Before handing off implementation (docs-first gate), update and review:

- [../theme.md](../theme.md) — `Input framing and status bars` section (remove the ` · <action>` description and 100 ms hold note; remove `working action text` from the `muted` token description).
- [../tui.md](../tui.md) — `Screen layout` / top input rule summary (if mentioning activity labels).

## Acceptance tests

1. Busy turn without steering: after transient status updates such as `"reading a.txt..."` and `"running cargo test..."`, the TUI working indicator contains only the spinner glyph and elapsed time in `theme.accent`, with no `·`, `reading`, or `running` text.
2. Elapsed time and spinner ticks continue updating while busy.
3. Busy turn with a queued steering message renders the steering wave in `theme.accent` and `steering…` in `theme.muted`; withdrawing the message returns to `<spinner> <elapsed>`.
4. Flash notice overrides `<spinner> <elapsed>` for 5 seconds, then returns to `<spinner> <elapsed>` if still busy.
5. Tool call blocks in chat, `/trace`, background task logs, and JSON-RPC `status` events continue functioning identically.
