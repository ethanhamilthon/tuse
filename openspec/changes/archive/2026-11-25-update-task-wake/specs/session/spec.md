## MODIFIED Requirements
### Requirement: Task notices wake the agent
A task that ends on its own SHALL queue its `<task>` notice as a `notification` record. A turn that is already running takes the notice at its next step. An idle session SHALL start a turn for it.

#### Scenario: Idle session
- **WHEN** a task finishes while the session is idle
- **THEN** a turn starts and the agent receives the notice
