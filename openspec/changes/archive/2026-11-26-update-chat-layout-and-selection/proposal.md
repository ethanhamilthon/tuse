## Why
User and agent messages start to the left of text in Lua and `show()` entries. Mouse selection currently works on screen rows, so it can cross message boundaries, pick up UI text and padding, and copy visual wraps as extra newlines.

## What Changes
- Inset user and agent messages by two display columns on each side, inside the existing chat width. Align the top-level text of chat entries to the same left column after any marker, without flattening intentional indentation.
- Make Ctrl+A / Cmd+A select only the draft. Mouse selection stays in the editor or in the single user or agent message where the drag started. Events, reasoning, Lua tool blocks, and UI are not selectable.
- Copy the selected message as plain text without layout padding or extra newlines from wrapping. Keep intentional line breaks and indentation, and copy editor selections verbatim.

## Impact
- Specs: `tui`.
- Implementation: chat rendering and viewport metadata, editor and chat hit testing, keyboard shortcuts, clipboard handling, and focused TUI tests.
- Docs: update the selection and copy shortcuts in the usage guide.
