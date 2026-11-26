## ADDED Requirements

### Requirement: OpenAI Responses stream finalization
The OpenAI Responses adapter SHALL finalize the reply after processing `response.completed` or `response.incomplete`, including terminal status and usage, without waiting for the HTTP body to close. `response.failed` SHALL remain an error. EOF without a recognized terminal event SHALL NOT be treated as a successful reply.

#### Scenario: Open body after a terminal event
- **WHEN** the provider sends `response.completed` or `response.incomplete` and leaves the HTTP body open
- **THEN** Tuse returns the reply with its terminal metadata without waiting for another event or EOF

#### Scenario: Premature EOF
- **WHEN** a Responses stream ends after a text delta without a terminal event
- **THEN** Tuse treats the stream as truncated rather than returning a successful reply
