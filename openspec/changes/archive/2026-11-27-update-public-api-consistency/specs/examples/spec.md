## MODIFIED Requirements

### Requirement: Examples folder
The repo SHALL have an `examples/` folder with self-contained Lua config modules covering at least: skills, MCP, themes, rename, notify, tmux, cmux, herdr, orca, zellij, rex, websearch, sandbox, commands, open, openspec, project, and compaction. Every example MUST load with the current config schema and use only current hook events, current v2 `session()` field names (`title`, `context`, `context_window` — not retired `name`, `ctx`, `ctx_size`), current history roles (never removed roles such as `tool`), and current host functions (`set_session`, not `set_title`). Multiplexer examples that write title and agent state to the same single field (`tmux.lua`, `zellij.lua`, `rex.lua`, `orca.lua`) SHALL combine the session title and state in a single command so the session title is never immediately overwritten.

#### Scenario: Example loads
- **WHEN** a test evaluates each `examples/*.lua` as a config module and validates the result
- **THEN** none fails

#### Scenario: Compaction example
- **WHEN** `compaction.lua` is loaded and `context` passes its threshold of `context_window` after a reply
- **THEN** it summarizes older records with `llm` and replaces them through `set_session{ history = ... }`, keeping the `system` record and the recent records

#### Scenario: Compaction example excludes removed roles
- **WHEN** `examples/compaction.lua` filters history records for summarization
- **THEN** it checks only valid v0.13+ roles (`user`, `agent`, `notification`, `lua`) and does not reference removed role `tool`

#### Scenario: Title from session
- **WHEN** a multiplexer example handles `user_message` or `agent_message`
- **THEN** it reads the title from `session().title` instead of retired `session().name` or `name_change`

#### Scenario: Multiplexer examples preserve title with status
- **WHEN** `tmux.lua`, `zellij.lua`, `rex.lua`, or `orca.lua` handles `user_message` or `agent_message`
- **THEN** it executes a single rename/comment command containing `session().title` together with the working/finished state instead of issuing a second command that overwrites the title
