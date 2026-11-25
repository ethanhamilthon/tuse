# config

## Purpose
Config keys and their validation.

## Requirements

### Requirement: Open handlers under settings
`open` SHALL be read from `settings.open` (`ext -> function(path)`). A top-level `open` key MUST fail with `open: moved to settings.open`.

#### Scenario: Old key
- **WHEN** a config returns `{ open = { md = f } }`
- **THEN** loading fails with `tuse.lua: open: moved to settings.open`

#### Scenario: New key
- **WHEN** a config returns `{ settings = { open = { md = f } } }` and a `.md` path is clicked
- **THEN** `f` is called with the path

### Requirement: Trace filter setting
`settings.ui.hide_trace` SHALL be a list of trace roles hidden from the TUI `/trace` list. Unknown roles MUST fail validation. `session().trace` and `session.trace` stay complete.

#### Scenario: Hiding hooks
- **WHEN** `hide_trace = { "hook", "usage" }` and the user opens `/trace`
- **THEN** no `hook` or `usage` entry is listed, while `session().trace` still has them

#### Scenario: Unknown role
- **WHEN** `hide_trace = { "hooks" }`
- **THEN** loading fails with ``settings.ui.hide_trace[1]: unknown trace role `hooks` ``

### Requirement: Model scope setting
`settings.ui.scope_models` SHALL be a list of `provider/model` or `provider/*` patterns. When it is non-empty, `/models` SHALL list only matching models plus the current one. `model.list` and `model.set` MUST ignore it.

#### Scenario: Scoped picker
- **WHEN** `scope_models = { "anthropic/*", "openai/gpt-5" }` and the user opens `/models`
- **THEN** only Anthropic models and `openai/gpt-5` are listed, plus the current model

### Requirement: Prompts with args
A `prompts` value SHALL be a string or a table `{ args?, run }`. `args` is an optional list of unique non-empty strings without whitespace. `run(arg)` gets the chosen arg or `nil` and MUST return a string, which becomes the prompt body, sent as `<prompt name="<name>" arg="<arg>">`. Only that returned string MUST reach the model, never the function or the `args` list.

#### Scenario: Invalid args
- **WHEN** `args = { "a b" }` or `run` is missing
- **THEN** loading fails with `prompts.<name>.args[1]: must not contain whitespace` or `prompts.<name>.run: is required`

#### Scenario: Prompt with arg
- **WHEN** `prompts.openspec = { args = { "propose" }, run = f }` and the user sends `#openspec propose`
- **THEN** `f("propose")` runs and its result is attached as `<prompt name="openspec" arg="propose">…</prompt>`

#### Scenario: Run fails
- **WHEN** `run` errors or returns a non-string
- **THEN** the message is not sent, the draft stays, and the error is shown

#### Scenario: JSON-RPC arg
- **WHEN** `turn.submit` gets `prompts: ["openspec propose"]`
- **THEN** `run("propose")` builds the body; an arg not in `args` fails with `-32000`

### Requirement: Config view is complete
`/config` and `config.get` SHALL show every resolved config field, including `models` (name, display, window, compact_at, prices), `settings.open`, `hide_trace`, `scope_models`, and prompt kinds and args. Secrets and Lua bodies MUST stay hidden.

#### Scenario: Models shown
- **WHEN** the config has `models` and the user opens `/config`
- **THEN** a `Models` section lists each model with its metadata

#### Scenario: No field forgotten
- **WHEN** a field is added to `Config`, `Settings`, or `UiSettings` but not to `/config`
- **THEN** `cargo test` fails
