## ADDED Requirements

### Requirement: No compaction settings
`compact_at` MUST NOT be accepted in `models` entries or in `settings`.

#### Scenario: Old key
- **WHEN** a config sets `settings.compact_at = 100000`
- **THEN** loading fails with an unknown-key error for `settings.compact_at`

## RENAMED Requirements
- FROM: `### Requirement: Trace filter setting`
- TO: `### Requirement: History filter setting`

## MODIFIED Requirements

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

### Requirement: Config view is complete
`/config` and `config.get` SHALL show every resolved config field, including `models` (name, display, window, prices), `settings.open`, `hide_history`, `scope_models`, and prompt kinds and args. Secrets and Lua bodies MUST stay hidden.

#### Scenario: Models shown
- **WHEN** the config has `models` and the user opens `/config`
- **THEN** a `Models` section lists each model with its metadata

#### Scenario: No field forgotten
- **WHEN** a field is added to `Config`, `Settings`, or `UiSettings` but not to `/config`
- **THEN** `cargo test` fails
