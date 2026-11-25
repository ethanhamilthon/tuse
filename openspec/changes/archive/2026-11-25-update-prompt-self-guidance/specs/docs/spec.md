## ADDED Requirements
### Requirement: Prompt points the agent to the docs
When `docs` is enabled, the system prompt SHALL end its rules with three bullets: questions about Tuse are answered from `docs()` and not from memory; a change of agent behavior (prompt, functions, hooks, commands, models) is made by editing the Lua config as `docs("config")` and `docs("examples")` describe, then reloading; a change of the UI (theme, input style, limits) is made with `theme` and `settings.ui` in that config, with keys and slash commands in `docs("usage")`. When `docs` is disabled, the bullets MUST be absent and the prompt MUST NOT mention `docs`.

#### Scenario: Default config
- **WHEN** a session starts with the default config
- **THEN** the system prompt holds the three bullets and `docs/lua.md` shows them verbatim

#### Scenario: Docs disabled
- **WHEN** a session sets `functions = { docs = false }`
- **THEN** the system prompt has none of the three bullets and no `docs(` reference
