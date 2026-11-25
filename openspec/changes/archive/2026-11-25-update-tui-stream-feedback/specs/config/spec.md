## ADDED Requirements

### Requirement: Input composer style setting
`settings.ui.input_style` SHALL accept `boxed`, `lines`, and `fill`, and SHALL accept `classic` as an alias of `lines`. If omitted, it SHALL resolve to `boxed`. Other values MUST fail config validation. `/config` and `config.get` SHALL expose the resolved value, showing `lines` for `classic`.

#### Scenario: Explicit style
- **WHEN** the config sets `settings.ui.input_style = "fill"` and is loaded or reloaded successfully
- **THEN** the TUI selects the fill composer and `/config` and `config.get` show `fill`

#### Scenario: Default, alias, and invalid styles
- **WHEN** the setting is omitted
- **THEN** it resolves to `boxed`
- **WHEN** the setting is `classic`
- **THEN** it resolves to `lines`
- **WHEN** an unsupported value is supplied
- **THEN** validation rejects it with an error identifying `settings.ui.input_style`
