## Calls

- `fork(index)`: `index` counts from 1 in `session().history`, like any Lua list. The record must have role `user` or `agent`. A `user` record leaves the history just before it and sets its text as the draft. An `agent` record keeps it and the results of its tool calls. It returns nothing.
- `resume(id)`: `id` is a session id string. The session must exist, be started, and belong to the current folder, like the `/resume` list. It returns nothing.

Both validate their argument at call time and raise without scheduling anything. A valid call is scheduled and returns at once.

## Timing

While tool calls of the current step run, the switch stays pending and applies after the last tool result of that step is recorded, the same moment a pending `set_session` history replacement applies. With no running tool call it applies at once. When both are pending, the history replacement applies first, and a fork copies the replaced history. After the switch the old session gets no further model request, queued user messages are not carried to the new session, and background tasks of the old session are stopped like `/new`.

## One switch at a time

A second `fork` or `resume` while one is pending raises `a session switch is already pending`.

## Clients

The TUI opens the new session exactly like `/fork` and `/resume` do, including the draft. JSON-RPC sends `session_switched` with `{session, model, effort, draft}` after the events of the old session and before any event of the new one.
