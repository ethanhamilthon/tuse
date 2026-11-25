## MODIFIED Requirements
### Requirement: Topics cover the public surface
The topics SHALL document every public surface: `usage` every CLI subcommand and every built-in slash command, `lua` every host function in `HOST_FUNCTIONS` and every hook event, `config` every `require("tuse")` export, every top-level config key, every `settings.ui` key, every theme color, every `theme.lua` token, and the naming rules for prompts, commands, and `open` extensions, `json` every JSON-RPC method, and `session` every history role.

#### Scenario: Coverage tests
- **WHEN** `cargo test` runs
- **THEN** it fails if a CLI subcommand or slash command is missing from `docs("usage")`, a host function or hook event from `docs("lua")`, a top-level key, `settings.ui` key, theme color, or `theme.lua` token from `docs("config")`, a JSON-RPC method from `docs("json")`, or a history role from `docs("session")`

#### Scenario: Color aliases
- **WHEN** a color accepted by validation, such as `gray` or `dark_gray`, is looked up in `docs("config")`
- **THEN** it is listed there
