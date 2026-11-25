# config

## Purpose
Config keys and their validation.

## Requirements

1

### Requirement: Open handlers under settings
`open` SHALL be read from `settings.open` (`ext -> function(path)`). A top-level `open` key MUST fail with `open: moved to settings.open`.

#### Scenario: Old key
- **WHEN** a config returns `{ open = { md = f } }`
- **THEN** loading fails with `tuse.lua: open: moved to settings.open`

#### Scenario: New key
- **WHEN** a config returns `{ settings = { open = { md = f } } }` and a `.md` path is clicked
- **THEN** `f` is called with the path

### Requirement: History filter setting
`settings.ui.hide_history` SHALL be a list of history roles hidden from the TUI `/history` list. Unknown roles MUST fail validation. `session().history` and `session.history` stay complete. `hide_trace` is removed.

#### Scenario: Hiding hooks
- **WHEN** `hide_history = { "hook", "usage" }` and the user opens `/history`
- **THEN** no `hook` or `usage` entry is listed, while `session().history` still has them

#### Scenario: Unknown role
- **WHEN** `hide_history = { "hooks" }`
- **THEN** loading fails with ``settings.ui.hide_history[1]: unknown history role `hooks` ``

#### Scenario: Old key
- **WHEN** a config sets `settings.ui.hide_trace`
- **THEN** loading fails with an unknown-key error

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
`/config` and `config.get` SHALL show every resolved config field, including `models` (name, display, window, prices), `settings.open`, `hide_history`, `scope_models`, and prompt kinds and args. Secrets and Lua bodies MUST stay hidden.

#### Scenario: Models shown
- **WHEN** the config has `models` and the user opens `/config`
- **THEN** a `Models` section lists each model with its metadata

#### Scenario: No field forgotten
- **WHEN** a field is added to `Config`, `Settings`, or `UiSettings` but not to `/config`
- **THEN** `cargo test` fails

### Requirement: No compaction settings
`compact_at` MUST NOT be accepted in `models` entries or in `settings`.

#### Scenario: Old key
- **WHEN** a config sets `settings.compact_at = 100000`
- **THEN** loading fails with an unknown-key error for `settings.compact_at`

### Requirement: Trace filter setting`
- TO: `

### Requirement: History filter setting`
