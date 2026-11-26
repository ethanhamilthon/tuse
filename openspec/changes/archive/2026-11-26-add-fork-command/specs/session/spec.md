## ADDED Requirements

### Requirement: Forked sessions
A fork SHALL create a new session row with a new id, the same folder, model, and effort, and the title `[fork] ` followed by the original title. Its history SHALL be a copy of the original records through the picked agent message or up to, not including, the picked user message, including the `system` record, and MUST pass the same validation as `set_session{ history = ... }`. `started` SHALL follow the normal rule, so it is true once a `user` record is copied. Usage, cost, and context SHALL start at zero, and `usage` records are not copied. The original session and its history MUST NOT change. Switching to the fork SHALL close the original like `/new` and `/resume`, running `session_close` hooks.

#### Scenario: Copied history
- **WHEN** a session with 10 records is forked at agent record 5
- **THEN** the new session has records 1 to 5 and the original still has 10

#### Scenario: Title
- **WHEN** a session titled `Fix parser` is forked
- **THEN** the fork is titled `[fork] Fix parser`

#### Scenario: Fresh usage
- **WHEN** a session with recorded usage and cost is forked
- **THEN** the fork reports zero usage and zero cost

#### Scenario: Original survives
- **WHEN** the fork is closed and the original is resumed
- **THEN** the original history is exactly as before the fork
