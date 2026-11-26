## MODIFIED Requirements

### Requirement: History filter setting
`history.hide` SHALL be a list of history roles hidden from the TUI `/history` list. Valid roles are those of the session history: `system`, `user`, `agent`, `lua`, `notification`, `message`, `usage`, `meta`, `turn_end`, `show`, `pick`, `ask`, `task`, `command`, `open`. It SHALL also accept the message levels `flash`, `notice`, `error`, which hide only `message` entries of that level. Unknown names, including the removed `tool`, `hook`, `model`, `title`, `turn`, MUST fail validation. `session().history` and `session.history` stay complete.

#### Scenario: Hiding usage
- **WHEN** `history = { hide = { "usage", "turn_end" } }` and the user opens `/history`
- **THEN** no `usage` or `turn_end` entry is listed, while `session().history` still has them

#### Scenario: Hiding a message level
- **WHEN** `history = { hide = { "flash" } }` and the user opens `/history`
- **THEN** `message` entries with level `flash` are not listed, while `error` messages are

#### Scenario: Removed role
- **WHEN** `history = { hide = { "hook" } }`
- **THEN** loading fails with ``history.hide[1]: unknown history role `hook` ``

#### Scenario: Old key
- **WHEN** a config sets `ui.hide_history` or `settings.ui.hide_history`
- **THEN** loading fails with an unknown-key or moved error
