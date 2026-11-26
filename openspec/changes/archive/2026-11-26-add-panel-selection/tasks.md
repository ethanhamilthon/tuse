## Tasks

- [x] 1. Give panel rows copy data so each wrapped row maps to its logical text, like chat message rows.
- [x] 2. Add a panel gesture to `Selection`, limited to the open panel and cleared when the panel scrolls.
- [x] 3. Highlight and copy the panel selection without padding or visual wraps; keep link clicks working.
- [x] 3a. Make the Lua `pick` and `ask` panels selectable the same way; nothing is picked by selecting.
- [x] 4. Clear the panel selection when the panel closes, changes entry, or a new selection starts elsewhere.
- [x] 5. Include prompt atoms in the copy rows of user messages.
- [x] 6. Add minimal tests: drag inside a panel, drag leaving a panel, a Lua pick panel, copy of a user message with a prompt atom.
