## Why
A Lua `pick` can offer choices, but it cannot explain what the user is choosing. The history labels every choice `Picker`, so the answer can lose its context after the prompt closes.

## What Changes
- Add an optional Markdown question to `pick(options, title)`. Show it above the choices, using the same Markdown treatment as `ask` but without a text input.
- Expose the question in the active JSON-RPC picker request as an optional `title` field.
- Save the question and selected answer together in the existing `pick` history entry.
- Keep untitled calls, string options, return values, cancellation, and built-in pickers unchanged.

## Impact
- Specs: `tui`, `session`, and `docs`.
- Implementation: Lua host binding, shared UI requests and results, picker layout and chat rendering. No history schema change.
