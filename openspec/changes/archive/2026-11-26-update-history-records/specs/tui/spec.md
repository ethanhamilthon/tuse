## MODIFIED Requirements

### Requirement: History command
`/history` SHALL list the session history from the database, minus roles and message levels in `history.hide` and minus `agent` entries with no text and no reasoning. `trace` is no longer a built-in command and `history` is a reserved command name. JSON-RPC `session.trace` and `session.records` are replaced by `session.history`, which returns the same structured records as `session().history`, and `session.list` returns only started sessions.

#### Scenario: Open history
- **WHEN** the user types `/history`
- **THEN** a list of every history record opens, starting with `system`

#### Scenario: Tool-only reply hidden
- **WHEN** the agent replied with only a `lua` call and the user opens `/history`
- **THEN** only the `lua` entry with the code is listed for that call

#### Scenario: Old command
- **WHEN** the user types `/trace`
- **THEN** it is an unknown command

#### Scenario: Old method
- **WHEN** a client calls `session.trace` or `session.records`
- **THEN** it gets a method-not-found error
### Requirement: Fork command
`/fork` SHALL open a picker of the `user` and `agent` messages of the current session, newest first, each with its role and a one-line preview, except `agent` messages with no text and no reasoning. Picking one SHALL open a new session forked at that message. For an agent message the history ends with that message. For a user message the history ends just before it, and its text is placed in the input as the draft. With no such messages, `/fork` SHALL do nothing and show a notice. `fork` is a reserved command name. JSON-RPC `session.fork` SHALL do the same for a `session.history` index, counted from 0, and return the `draft` text (empty for an agent message), and an index that is not a `user` or `agent` record returns an error.

#### Scenario: Fork at a user message
- **WHEN** the user types `/fork` and picks an earlier user message
- **THEN** a new session opens in the same folder whose history ends just before that message
- **AND** the text of that message is in the input
- **AND** the original session is unchanged and listed in `/resume`

#### Scenario: Fork at an agent message
- **WHEN** the user picks an agent message with text that made tool calls
- **THEN** the new history ends with that message and the results of its tool calls

#### Scenario: Tool-only reply
- **WHEN** the agent replied with only a `lua` call and the user opens `/fork`
- **THEN** that reply is not listed

#### Scenario: Empty session
- **WHEN** the session has no user or agent messages and the user types `/fork`
- **THEN** no picker opens and a notice says there is nothing to fork

#### Scenario: Bad index
- **WHEN** a client calls `session.fork` with the index of a `system` record
- **THEN** it gets an error and no session is created
