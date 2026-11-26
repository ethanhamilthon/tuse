## ADDED Requirements

### Requirement: Document session switching from Lua
`docs("lua")` SHALL describe `fork(index)` and `resume(id)`: the 1-based index into `session().history`, the allowed roles, the draft for a `user` record, the wait for running tool calls, and the errors. `docs("json")` SHALL describe the `session_switched` event, and the agent's system prompt SHALL list both functions with the other host functions.

#### Scenario: Coverage
- **WHEN** `cargo test` runs
- **THEN** it fails if `fork` or `resume` is missing from `docs("lua")` or `session_switched` from `docs("json")`
