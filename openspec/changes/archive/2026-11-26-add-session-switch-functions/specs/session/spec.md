## ADDED Requirements

### Requirement: Fork and resume from Lua
The host SHALL provide `fork(index)` and `resume(id)`. `fork(index)` SHALL take a 1-based position in `session().history` of a `user` or `agent` record and create the same session as `/fork` at that record, setting the text of a `user` record as the draft. `resume(id)` SHALL switch to the started session `id` of the current folder, like `/resume`. Both SHALL validate their argument when called and raise without changing anything on a bad position, a record of another role, an unknown or unstarted session, a session of another folder, or a session switch already pending. A valid call SHALL schedule the switch and return at once. While tool calls of the current step run, the switch SHALL wait until the last tool result of that step is recorded, after a pending history replacement and before the next model request. With no running tool call it SHALL apply at once. After the switch, the old session MUST get no further model request, queued user messages MUST NOT move to the new session, and hooks SHALL run as for `/new`: `session_close`, then the new session event.

#### Scenario: Fork from a tool call
- **WHEN** the agent's `lua` call runs `fork(3)` on a session whose third record is an `agent` message
- **THEN** the call's result is recorded in the old session first, then a new session opens with records 1 to 3 and the old session sends no further request

#### Scenario: Fork at a user record
- **WHEN** a command runs `fork(2)` and record 2 is a `user` message
- **THEN** the new history ends before that message and its text is the draft

#### Scenario: Resume
- **WHEN** a command runs `resume(id)` for another started session of this folder
- **THEN** that session opens as with `/resume`

#### Scenario: Bad arguments
- **WHEN** `fork(1)` points at the `system` record, `fork(99)` is out of range, `resume("nope")` is unknown, or `resume(id)` names a session of another folder
- **THEN** each raises an error and the current session is unchanged

#### Scenario: Switch already pending
- **WHEN** a tool call runs `fork(3)` and then `resume(id)` in the same step
- **THEN** `resume` raises `a session switch is already pending` and the fork still applies

#### Scenario: History replacement first
- **WHEN** a tool call runs `set_session{ history = short }` and then `fork(2)`
- **THEN** the replacement applies first and the fork copies the replaced history

#### Scenario: Queued message
- **WHEN** the user sends a message while the call runs
- **THEN** the message stays with the old session and is not recorded in the new one

### Requirement: Switch event over JSON-RPC
When a session switch started by Lua applies, JSON-RPC SHALL send the event `session_switched` with `{session, model, effort, draft}` after the remaining events of the old session and before the first event of the new one. `draft` is empty unless the switch was a fork at a `user` record. `session.new`, `session.resume`, and `session.fork` replies stay unchanged.

#### Scenario: Lua switch
- **WHEN** a command runs `resume(id)` while a JSON-RPC client is connected
- **THEN** the client gets `session_switched` with the new session id, and later events belong to that session

## MODIFIED Requirements

### Requirement: History replacement waits for tool calls
When `set_session` gets `history` while tool calls of the current step run, the replacement SHALL be kept pending and applied after the last tool result of that step, before a pending session switch, before the next model request, and before any queued user message is recorded. With no running tool call it SHALL apply at once.

#### Scenario: Self-compaction from a tool call
- **WHEN** the agent's `lua` call runs `set_session{ history = short }`
- **THEN** the call's result is recorded first, then the history becomes `short`, and the next request is built from it

#### Scenario: Steering during the call
- **WHEN** the user sends a message while that call runs
- **THEN** the message is recorded after the replacement
