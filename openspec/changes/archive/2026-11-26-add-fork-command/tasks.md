## Tasks

- [x] 1. Add a storage function that creates a session from another one, copying meta and the history records through a given sequence, with usage, cost, and context reset.
- [x] 2. Make the cut valid: when the picked message is an agent message, include its tool calls and results so the copied history passes `set_session` validation.
- [x] 2a. For a user message cut before it and return its text as the draft; the TUI fills the input with it.
- [x] 3. Add `launch::fork` that prepares the new session, closes the current one like `switch`, and starts the new one.
- [x] 4. Add the `/fork` picker and the `fork` reserved name in the TUI.
- [x] 5. Add `session.fork` to JSON-RPC and the method list.
- [x] 6. Update `docs/usage.md` and `docs/json.md`.
- [x] 7. Add minimal tests: cut at a user message, cut at an agent message with tool calls, and `session.fork` over JSON-RPC.
