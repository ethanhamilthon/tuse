## Why

The TUI waits on terminal input polling even when a model delta is ready to display. Its busy spinner also looks the same when waiting for a model, receiving a reply, or running Lua. Users should see received content promptly and be able to tell what the agent is doing without a new status row jumping into the chat.

## What Changes

- Wake the TUI on session events. Draw the first visible streaming update without waiting for terminal input, a frame-rate timer, or another delta. Coalesce later deltas when the stream is busy.
- Keep the existing spinner and elapsed time while waiting for the provider. Show a pulsing dot while receiving content and the Lua block's star while a foreground Lua call executes.
- Add `settings.ui.input_style` with `boxed` (default), `lines` (the former `classic`, still accepted as an alias), and `fill`. The boxed composer has no left padding inside the border. The idle status shows the project directory in `muted`.
- Show the Lua preview and its inline star together on the first displayable code chunk. Metadata-only and whitespace-only updates do not create a chat row.
- Preserve steering, retry notices, and interactive prompts. Do not add a new animation for hooks or background tasks, stream shell output, or change provider transport.

## Impact

- Specs: `tui`, `session`, `config`.
- Code: TUI event loop, status rendering, composer geometry and input mapping; internal session activity; config validation, inspection, and docs.
- Compatibility: `boxed` is the default, `classic` still loads as `lines`; no changes to the JSON-RPC event schema, Lua API, or persisted history format.
