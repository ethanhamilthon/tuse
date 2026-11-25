## ADDED Requirements

### Requirement: Lua syntax theme
`theme.lua` SHALL accept exactly these tokens: `keyword`, `storage`, `constant`, `operator`, `punctuation`, `number`, `string`, `escape`, `comment`, `function`, `builtin`, `method`, `field`, `parameter`, `variable`. Each value SHALL be a color or the name of a theme color (`bg`, `surface`, `fg`, `dim`, `muted`, `border`, `accent`, `ok`, `err`, `info`). Omitted tokens SHALL default to links onto the base theme. Unknown tokens and invalid colors MUST fail config validation. `/config` and `config.get` SHALL expose the resolved tokens.

#### Scenario: Partial override
- **WHEN** the config sets only `theme.lua.operator = "info"`
- **THEN** operators use the `info` color and every other token keeps its default link

#### Scenario: Invalid token
- **WHEN** the config sets `theme.lua.nope` or `theme.lua.keyword = "user"`
- **THEN** validation rejects the config

### Requirement: User text color
`theme.user` SHALL set the text color of user messages and of the input, and SHALL default to the link `fg`. It SHALL accept a color or the name of another theme color except `user`. The agent's text SHALL keep using `fg`.

#### Scenario: Separate user color
- **WHEN** the config sets `theme.user = "#88C0D0"`
- **THEN** user messages and the input text use that color and the agent's replies keep `fg`
