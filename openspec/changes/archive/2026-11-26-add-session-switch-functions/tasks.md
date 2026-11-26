## 1. Host functions

- [x] 1.1 Add `fork(index)` and `resume(id)` to `HOST_FUNCTIONS`, validating position, role, session id, folder, and an already pending switch at call time.
- [x] 1.2 Keep a pending switch and apply it after the last tool result of the step, after a pending history replacement; apply at once when no tool call runs.
- [x] 1.3 Reuse the `/fork` and `/resume` switch code: close hooks, stop old tasks, drop queued messages, no further request in the old session.

## 2. Clients

- [x] 2.1 Make the TUI open the new session and set the draft after a Lua-started switch.
- [x] 2.2 Send the JSON-RPC `session_switched` event with `{session, model, effort, draft}`.

## 3. Documentation and tests

- [x] 3.1 Update `docs/lua.md`, `docs/json.md`, and the agent system prompt.
- [x] 3.2 Cover bad arguments, the wait for tool calls, order with `set_session`, the pending conflict, and the JSON-RPC event.
