## ADDED Requirements

### Requirement: Fork command
`/fork` SHALL open a picker of the `user` and `agent` messages of the current session, newest first, each with its role and a one-line preview. Picking one SHALL open a new session forked at that message. For an agent message the history ends with that message. For a user message the history ends just before it, and its text is placed in the input as the draft. With no such messages, `/fork` SHALL do nothing and show a notice. `fork` is a reserved command name. JSON-RPC `session.fork` SHALL do the same for a `session.history` index, counted from 0, and return the `draft` text (empty for an agent message), and an index that is not a `user` or `agent` record returns an error.

#### Scenario: Fork at a user message
- **WHEN** the user types `/fork` and picks an earlier user message
- **THEN** a new session opens in the same folder whose history ends just before that message
- **AND** the text of that message is in the input
- **AND** the original session is unchanged and listed in `/resume`

#### Scenario: Fork at an agent message
- **WHEN** the user picks an agent message that made tool calls
- **THEN** the new history ends with that message and the results of its tool calls

#### Scenario: Empty session
- **WHEN** the session has no user or agent messages and the user types `/fork`
- **THEN** no picker opens and a notice says there is nothing to fork

#### Scenario: Bad index
- **WHEN** a client calls `session.fork` with the index of a `system` record
- **THEN** it gets an error and no session is created
