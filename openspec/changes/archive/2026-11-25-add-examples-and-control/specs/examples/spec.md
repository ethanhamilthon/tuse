## ADDED Requirements

### Requirement: Examples folder
The repo SHALL have an `examples/` folder with self-contained Lua config modules covering at least: skills, MCP, themes, rename, notify, tmux, cmux, herdr, orca, zellij, rex, websearch, sandbox, commands, open, openspec, and project. Every example MUST load with the current config schema.

#### Scenario: Example loads
- **WHEN** a test evaluates each `examples/*.lua` as a config module and validates the result
- **THEN** none fails

### Requirement: Examples page
`docs/examples.md` SHALL have one section per example file with a short description and a link `https://github.com/ethanhamilthon/tuse/blob/main/examples/<file>`.

#### Scenario: Links match files
- **WHEN** `cargo test` runs
- **THEN** it fails if a file in `examples/` has no link or a link points to a missing file

### Requirement: README links examples
README SHALL only gain links, on top of its current edits, to the matching example wherever it mentions skills, MCP, plugins, themes, multiplexers, or titles.

#### Scenario: README link
- **WHEN** a reader opens the `cmux, Orca, Herdr, tmux, Rex…` section
- **THEN** it links the multiplexer examples, including `rex.lua`, on GitHub
