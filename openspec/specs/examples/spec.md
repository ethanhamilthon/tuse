# examples

## Purpose
Ready-made Lua configs in `examples/` and the docs page that links them.

## Requirements

### Requirement: Examples folder
The repo SHALL have an `examples/` folder with self-contained Lua config modules covering at least: skills, MCP, themes, rename, notify, tmux, cmux, herdr, orca, zellij, rex, websearch, sandbox, commands, open, openspec, project, and compaction. Every example MUST load under the current namespaced config schema (with no legacy flat config compatibility) and use only current hook events, current v2 `session()` field names (`title`, `context`, `context_window` — not retired `name`, `ctx`, `ctx_size`), current history roles (never removed roles such as `tool`), and current host functions (`set_session`, not `set_title`). Multiplexer examples that write title and agent state to the same single field (`tmux.lua`, `zellij.lua`, `rex.lua`, `orca.lua`) SHALL combine the session title and state in a single command so the session title is never immediately overwritten.

#### Scenario: Example loads
- **WHEN** a test evaluates each `examples/*.lua` under the namespaced config schema and validates the result
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

### Requirement: Examples page
`docs/examples.md` SHALL have one section per example file with a short description and a link `https://github.com/ethanhamilthon/tuse/blob/main/examples/<file>`.

#### Scenario: Links match files
- **WHEN** `cargo test` runs
- **THEN** it fails if a file in `examples/` has no link or a link points to a missing file

### Requirement: README links examples
README SHALL link to the current example modules when describing skills, MCP, themes, titles, or terminal integrations. It SHALL distinguish the example modules from the separately installable official plugins.

#### Scenario: README links terminal integrations
- **WHEN** a reader looks up terminal integrations in the README
- **THEN** it links the relevant examples, including `rex.lua`, and identifies `status` as an official plugin
