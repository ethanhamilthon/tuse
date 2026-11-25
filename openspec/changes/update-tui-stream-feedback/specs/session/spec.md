## ADDED Requirements

### Requirement: Distinguish foreground turn activity
The session SHALL make the current main model attempt, retry backoff, and foreground Lua execution distinguishable to the TUI, including the start and end of each phase. Provider wait SHALL start at request dispatch, not when user input is queued. Foreground Lua execution MUST be distinct from receiving Lua arguments and MUST end when the call finishes, is stopped, or is handed off. Activity state SHALL be transient and MUST NOT change the order of persisted records or the JSON-RPC event schema.

#### Scenario: Before and after the provider request
- **WHEN** a user hook runs before request dispatch or an agent hook runs after the provider response finishes
- **THEN** the hook does not count as waiting for the provider or running a foreground Lua tool

#### Scenario: Steering hands off an executing tool
- **WHEN** a foreground Lua call transfers to a background task because of steering
- **THEN** it no longer counts as foreground execution even if the task continues running
