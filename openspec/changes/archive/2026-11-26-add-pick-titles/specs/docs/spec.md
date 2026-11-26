## ADDED Requirements

### Requirement: Document Lua picker questions
The Lua reference and agent system prompt SHALL describe `pick(options, title)` with an optional Markdown title. The Lua reference SHALL describe how the question and selected answer appear in history. The JSON-RPC reference SHALL document the optional `title` string in active picker requests.

#### Scenario: Find the picker title contract
- **WHEN** an agent or a JSON-RPC client author looks up picker questions
- **THEN** the Lua reference and agent prompt show how to supply a Markdown title, the Lua reference explains history behavior, and the JSON-RPC reference explains when the raw Markdown field appears in `ui_request` and `ui.state.active`
