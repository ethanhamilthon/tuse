## Why

The TUI waits on terminal input polling even when a model delta is ready to display. Its busy spinner also looks the same when waiting for a model, receiving a reply, or running Lua. Users should see received content promptly and be able to tell what the agent is doing without a new status row jumping into the chat.

## What Changes

- Wake the TUI on session events. Draw the first visible streaming update without waiting for terminal input, a frame-rate timer, or another delta. Coalesce later deltas when the stream is busy.
- Keep the existing spinner and elapsed time while waiting for the provider. Show a pulsing dot while receiving content and the Lua block's star while a foreground Lua call executes.
- Add `settings.ui.input_style = "boxed"` as an opt-in alternative to the current `"classic"` composer. The box has five rows: top border, two editor rows, a status bar, and bottom border. The status bar is inside the box, with activity on the left and model/context on the right.
- Show the Lua preview and its inline star together on the first displayable code chunk. Metadata-only and whitespace-only updates do not create a chat row.
- Preserve steering, retry notices, and interactive prompts. Do not add a new animation for hooks or background tasks, stream shell output, or change provider transport.

## Impact

- Specs: `tui`, `session`, `config`.
- Code: TUI event loop, status rendering, composer geometry and input mapping; internal session activity; config validation, inspection, and docs.
- Compatibility: classic input remains the default; no changes to the JSON-RPC event schema, Lua API, or persisted history format.
