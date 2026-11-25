## ADDED Requirements

### Requirement: Documentation topics
Tuse SHALL ship exactly seven documentation topics, in this order: `readme`, `usage`, `connect`, `config`, `lua`, `session`, `json`. `readme` SHALL be the content of `README.md` and every other topic the content of `docs/<topic>.md`, embedded at build time. The build MUST fail when a listed page is missing or when `docs/` holds any other `.md` file.

#### Scenario: Listing topics
- **WHEN** the agent calls `docs()`
- **THEN** it gets one `- <topic> — <summary>` line per topic in the fixed order, where the summary is the first paragraph after the page title, followed by the `Call docs("<topic>")` hint

#### Scenario: Reading a topic
- **WHEN** the agent calls `docs(" Config.md ")`
- **THEN** it gets the content of `docs/config.md`

#### Scenario: Unknown or removed topic
- **WHEN** the agent calls `docs("functions")` or `docs("nope")`
- **THEN** it gets `Unknown docs topic "<name>".` followed by the topic list

#### Scenario: Reading the README
- **WHEN** the agent calls `docs("README.md")` or `docs("readme")`
- **THEN** it gets the content of `README.md`

#### Scenario: Stray page
- **WHEN** `docs/extra.md` exists or `docs/json.md` is missing
- **THEN** `cargo build` fails

### Requirement: Topics cover the public surface
The topics SHALL document every public surface: `usage` every CLI subcommand and every built-in slash command, `lua` every host function in `HOST_FUNCTIONS`, `config` every `require("tuse")` export, and `json` every JSON-RPC method.

#### Scenario: Coverage tests
- **WHEN** `cargo test` runs
- **THEN** it fails if a CLI subcommand or slash command is missing from `docs("usage")`, a host function from `docs("lua")`, or a JSON-RPC method from `docs("json")`

### Requirement: Lua docs end with the agent's context
`docs/lua.md` SHALL end with a `## What the agent sees` section that holds, verbatim in `text` code blocks, the system prompt and the `lua` tool description that Tuse produces for the default config. The section MUST also say that disabled functions drop out and that `systems` strings and the compaction note are appended.

#### Scenario: Docs match the code
- **WHEN** the prompt template in the code or the tail of `docs/lua.md` changes so that they no longer match
- **THEN** `cargo test` fails and prints the expected section

#### Scenario: Session-specific rendering
- **WHEN** a session disables `sh` and the agent calls `docs("lua")`
- **THEN** the text starts with `Disabled in this session: sh.` and its `What the agent sees` section contains no `sh(` reference

### Requirement: System prompt template lives in code
The system prompt template SHALL be loaded from a file inside `crates/core/src`, not from `docs/`. The prompt text and the placeholder rules SHALL stay unchanged.

#### Scenario: Prompt unchanged
- **WHEN** a session starts with the default config
- **THEN** its system prompt is identical to the one built from the old `docs/system-prompt.md`
