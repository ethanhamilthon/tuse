## Why

Mouse selection works only in the input and in user and agent messages. Text in `/config`, `/session`, `/history` entries, task details, and Lua `pick`/`ask` panels cannot be selected or copied, and prompt atoms such as `#openspec propose` in a user message are not reliably part of the copied text.

## What Changes

- Panels become selectable text targets, including the Lua `pick` and `ask` panels. A drag inside an open panel selects within that panel only, and `Ctrl+C`/`Cmd+C` copies it.
- Prompt atoms in user messages are selected and copied as their displayed text.
- Reasoning, Lua tool blocks, events, `show()` blocks, built-in pickers, and layout decorations stay non-selectable.

## Impact

- Specs: `tui` (modifies "Selection is limited to one text target" and "Copy logical text without layout whitespace").
- Code: `crates/tui/src/ui/panel.rs`, `crates/tui/src/ui/mouse.rs`, `crates/tui/src/kit/selection.rs`, `crates/tui/src/ui/screen/compose.rs`.
