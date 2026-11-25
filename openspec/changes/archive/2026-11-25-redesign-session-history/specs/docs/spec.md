## MODIFIED Requirements

### Requirement: Lua docs end with the agent's context
`docs/lua.md` SHALL end with a `## What the agent sees` section that holds, verbatim in `text` code blocks, the system prompt and the `lua` tool description that Tuse produces for the default config. The section MUST also say that disabled functions drop out, that `systems` strings are appended, and that the prompt is the first `system` history record, rebuilt on new, resume, and config load.

#### Scenario: Docs match the code
- **WHEN** the prompt template in the code or the tail of `docs/lua.md` changes so that they no longer match
- **THEN** `cargo test` fails and prints the expected section

#### Scenario: Session-specific rendering
- **WHEN** a session disables `sh` and the agent calls `docs("lua")`
- **THEN** the text starts with `Disabled in this session: sh.` and its `What the agent sees` section contains no `sh(` reference

#### Scenario: No compaction note
- **WHEN** the agent calls `docs("lua")`
- **THEN** the section does not mention compaction
