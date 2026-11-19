# TODO — 0.3.3

## Editing a steering message

In 0.3.2, a message sent while the agent works stays in the input (dimmed) with `steering…`, and the only way to take it back is Esc, which also cancels the turn.

- [x] Any change in the input (typing, delete, paste, a newline, clear) takes the waiting message back: it leaves the core queue, `steering` is reset, the input is no longer dimmed and the text stays for editing. Esc is not needed and the turn keeps running.
  - Needs a core call to drop a queued input (`control.rs` has only `submit` and `cancel`, and `cancel` clears the whole queue).
  - If the agent has already taken the message, there is nothing to take back: the change is just a new draft.
  - Enter on the edited text sends it as a new steering message.
- [x] A test: a queued message is removed from the queue on an edit.
- [x] Update the steering text in `README.md` and `docs/agent.md` (Esc is no longer the way to edit).
