## MODIFIED Requirements

### Requirement: Topics cover the public surface
The topics SHALL document every public surface: `usage` every CLI subcommand and every built-in slash command, `lua` every host function in `HOST_FUNCTIONS` and every hook event, `config` every `require("tuse")` export, every top-level config key, every `ui` key, every `ui.theme` token, and the naming rules for prompts, commands, and `open` extensions, `json` every JSON-RPC method, and `session` every history role.

#### Scenario: Coverage tests
- **WHEN** `cargo test` runs
- **THEN** it fails if a CLI subcommand or slash command is missing from `docs("usage")`, a host function or hook event from `docs("lua")`, a top-level key, `ui` key, or `ui.theme` token from `docs("config")`, a JSON-RPC method from `docs("json")`, or a history role from `docs("session")`

#### Scenario: Color aliases
- **WHEN** a color accepted by validation, such as `gray` or `dark_gray`, is looked up in `docs("config")`
- **THEN** it is listed there

### Requirement: Prompt points the agent to the docs
When `docs` is enabled, the system prompt SHALL end its rules with three bullets: questions about Tuse are answered from `docs()` and not from memory; a change of agent behavior (prompt, functions, hooks, commands, models) is made by editing the Lua config as `docs("config")` and `docs("examples")` describe, then reloading; a change of the UI (theme, input style, limits) is made with `ui` in that config, with keys and slash commands in `docs("usage")`. When `docs` is disabled, the bullets MUST be absent and the prompt MUST NOT mention `docs`.

#### Scenario: Default config
- **WHEN** a session starts with the default config
- **THEN** the system prompt holds the three bullets and `docs/lua.md` shows them verbatim

#### Scenario: Docs disabled
- **WHEN** a session sets `functions = { docs = false }`
- **THEN** the system prompt has none of the three bullets and no `docs(` reference
