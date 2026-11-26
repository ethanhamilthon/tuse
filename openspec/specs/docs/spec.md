# docs

## Purpose
Documentation topics shared by the agent (`docs()`) and the site, and the agent context shown in them.

## Requirements

1

### Requirement: Documentation topics
Tuse SHALL ship exactly eight documentation topics, in this order: `readme`, `usage`, `connect`, `config`, `lua`, `session`, `json`, `examples`. `readme` SHALL be the content of `README.md` and every other topic the content of `docs/<topic>.md`, embedded at build time. The build MUST fail when a listed page is missing or when `docs/` holds any other `.md` file.

#### Scenario: Listing topics
- **WHEN** the agent calls `docs()`
- **THEN** it gets one `- <topic> — <summary>` line per topic in the fixed order, ending with `examples`, followed by the `Call docs("<topic>")` hint

#### Scenario: Reading examples
- **WHEN** the agent calls `docs("examples")`
- **THEN** it gets the content of `docs/examples.md`

#### Scenario: Stray page
- **WHEN** `docs/extra.md` exists or `docs/examples.md` is missing
- **THEN** `cargo build` fails

#### Scenario: Reading a topic
- **WHEN** the agent calls `docs(" Config.md ")`
- **THEN** it gets the content of `docs/config.md`

#### Scenario: Unknown or removed topic
- **WHEN** the agent calls `docs("functions")` or `docs("nope")`
- **THEN** it gets `Unknown docs topic "<name>".` followed by the topic list

#### Scenario: Reading the README
- **WHEN** the agent calls `docs("README.md")` or `docs("readme")`
- **THEN** it gets the content of `README.md`

### Requirement: Topics cover the public surface
The topics SHALL document every public surface: `usage` every CLI subcommand and every built-in slash command, `lua` every host function in `HOST_FUNCTIONS` and every hook event, `config` every `require("tuse")` export, every top-level config key, every `settings.ui` key, every theme color, every `theme.lua` token, and the naming rules for prompts, commands, and `open` extensions, `json` every JSON-RPC method, and `session` every history role.

#### Scenario: Coverage tests
- **WHEN** `cargo test` runs
- **THEN** it fails if a CLI subcommand or slash command is missing from `docs("usage")`, a host function or hook event from `docs("lua")`, a top-level key, `settings.ui` key, theme color, or `theme.lua` token from `docs("config")`, a JSON-RPC method from `docs("json")`, or a history role from `docs("session")`

#### Scenario: Color aliases
- **WHEN** a color accepted by validation, such as `gray` or `dark_gray`, is looked up in `docs("config")`
- **THEN** it is listed there

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

### Requirement: System prompt template lives in code
The system prompt template SHALL be loaded from a file inside `crates/core/src`, not from `docs/`. The template and its placeholder rules SHALL be the source of the rendered prompt.

#### Scenario: Prompt rendered
- **WHEN** a session starts with the default config
- **THEN** its first system history record contains the rendered prompt

### Requirement: Prompt points the agent to the docs
When `docs` is enabled, the system prompt SHALL end its rules with three bullets: questions about Tuse are answered from `docs()` and not from memory; a change of agent behavior (prompt, functions, hooks, commands, models) is made by editing the Lua config as `docs("config")` and `docs("examples")` describe, then reloading; a change of the UI (theme, input style, limits) is made with `theme` and `settings.ui` in that config, with keys and slash commands in `docs("usage")`. When `docs` is disabled, the bullets MUST be absent and the prompt MUST NOT mention `docs`.

#### Scenario: Default config
- **WHEN** a session starts with the default config
- **THEN** the system prompt holds the three bullets and `docs/lua.md` shows them verbatim

#### Scenario: Docs disabled
- **WHEN** a session sets `functions = { docs = false }`
- **THEN** the system prompt has none of the three bullets and no `docs(` reference

### Requirement: Document Lua picker questions
The Lua reference and agent system prompt SHALL describe `pick(options, title)` with an optional Markdown title. The Lua reference SHALL describe how the question and selected answer appear in history. The JSON-RPC reference SHALL document the optional `title` string in active picker requests.

#### Scenario: Find the picker title contract
- **WHEN** an agent or a JSON-RPC client author looks up picker questions
- **THEN** the Lua reference and agent prompt show how to supply a Markdown title, the Lua reference explains history behavior, and the JSON-RPC reference explains when the raw Markdown field appears in `ui_request` and `ui.state.active`
