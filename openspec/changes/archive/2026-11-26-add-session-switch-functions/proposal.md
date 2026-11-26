## Why
Forking and resuming a session works only from the TUI and JSON-RPC. Lua commands, hooks, and the agent cannot branch a conversation or jump to another session, so workflows such as "try this in a fork" or "return to the planning session" need manual steps.

## What Changes
- Add host function `fork(index)`: fork the current session at the record at `index` of `session().history`, which must be a `user` or `agent` record.
- Add host function `resume(id)`: switch to an existing session of the current folder.
- Both wait until the tool calls of the current step finish, then switch like `/fork` and `/resume`.
- Add the JSON-RPC event `session_switched` so clients learn about a switch that Lua started.

## Impact
- Specs: `session`, `docs`.
- Implementation: Lua host bindings, a pending session switch applied after the last tool result, the `session_switched` event, system prompt and `docs/lua.md`, `docs/json.md`.
