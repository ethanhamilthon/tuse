## ADDED Requirements

### Requirement: Input composer style setting
`settings.ui.input_style` SHALL accept `classic` and `boxed`. If omitted, it SHALL resolve to `classic`. Other values MUST fail config validation. `/config` and `config.get` SHALL expose the resolved value.

#### Scenario: Explicit boxed style
- **WHEN** the config sets `settings.ui.input_style = "boxed"` and is loaded or reloaded successfully
- **THEN** the TUI selects the boxed composer and `/config` and `config.get` show `boxed`

#### Scenario: Default and invalid styles
- **WHEN** the setting is omitted
- **THEN** it resolves to `classic`
- **WHEN** an unsupported value is supplied
- **THEN** validation rejects it with an error identifying `settings.ui.input_style`
