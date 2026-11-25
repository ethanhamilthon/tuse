## Event-driven rendering

Wait on terminal input and session activity through a common wake path, with deadlines for any pending redraw, animation, or flash expiry. Session activity must wake the loop even when the terminal is idle; lowering the polling interval is not a substitute. A first visible delta bypasses frame pacing; later dirty frames may be coalesced but must be flushed at their deadline even if no further events arrive. Keep input responsive under sustained traffic with a bounded drain, preserving event order. Rebind the wake source on session change so an old session cannot overwrite the new session's view. Keep the existing terminal diff and synchronized update.

## Activity

Drive provider wait, streaming, retry and foreground Lua from actual session transitions, not from time since the last token or from the existence of a Lua preview. Carry activity as transient state with wakeups, independent of persisted records and without adding a JSON-RPC notification type. Associate transitions with the current turn/attempt and clear them on cancel or session switch. In particular, a provider pause between deltas is still streaming; a Lua preview is not running code. No provider spinner or Lua star during a post-response hook unless the corresponding activity is actually underway.

Use the existing spinner frames for provider wait, the same star frames as the Lua block for execution, and a fixed-width terminal glyph sequence (for example `· • ● •`) for the streaming pulse. Animation ticks do not delay rendering deltas. Keep existing flash/steering affordances; retry copy must not mask new activity once its next attempt starts. `pick`/`ask` keep their existing modal affordances, without a new animated phase. Background tasks do not claim the foreground tool indicator.

Keep pending Lua call metadata and leading whitespace off-screen until there is displayable code. Create the chat entry with its inline star and code in the same frame. If the first displayable code arrives with `ToolStart` instead of a delta, use the same rule; never create a star-only placeholder. Discard hidden preview state on retry, cancellation, or session switch. This inline star is separate from the composer star, which indicates execution.

## Boxed geometry

Use one geometry calculation for the frame, wrapped editor rows, cursor, selections and hit testing. The box takes the editor rows plus four: top border, blank row, status row, bottom border. Editor rows grow with the draft up to `input_max_height` and the viewport, then scroll. Boxed rows have a one-column left padding instead of the prompt arrow. At insufficient width or height, use the classic layout without changing the configured style. Size changes and config reload recalculate geometry without dropping the draft.
