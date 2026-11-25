## MODIFIED Requirements

### Requirement: Examples folder
The repo SHALL have an `examples/` folder with self-contained Lua config modules covering at least: skills, MCP, themes, rename, notify, tmux, cmux, herdr, orca, zellij, rex, websearch, sandbox, commands, open, openspec, project, and compaction. Every example MUST load with the current config schema and use only current hook events and host functions (`set_session`, not `set_title`).

#### Scenario: Example loads
- **WHEN** a test evaluates each `examples/*.lua` as a config module and validates the result
- **THEN** none fails

#### Scenario: Compaction example
- **WHEN** `compaction.lua` is loaded and `ctx` passes its threshold after a reply
- **THEN** it summarizes older records with `llm` and replaces them through `set_session{ history = ... }`, keeping the `system` record and the recent records

#### Scenario: Title from session
- **WHEN** a multiplexer example handles `user_message` or `agent_message`
- **THEN** it reads the title from `session().name` instead of a `name_change` event
