## ADDED Requirements
### Requirement: Picker columns are aligned
A picker list SHALL draw each item as columns. Every cell of a column SHALL start at the same display column on every row, padded to the widest cell of that column among all items of the picker, with two spaces between columns. The last column is free text clipped at the row end. A column wider than half of the row width SHALL be clipped with the ellipsis so the next column keeps its room. `/history` rows SHALL have the columns index, role, and summary. Items with a label and a detail SHALL align the detail as the second column. Search SHALL match the cells joined by a space.

#### Scenario: History rows
- **WHEN** `/history` lists records 0 to 14
- **THEN** the roles start at one column and the summaries at one column, for example `0   system  …` and `10  hook    …`

#### Scenario: Command list
- **WHEN** the user types `/`
- **THEN** every command description starts at the same column

#### Scenario: Scrolling and filtering
- **WHEN** the list scrolls or the search narrows it
- **THEN** the column positions stay the same

#### Scenario: Narrow terminal
- **WHEN** the first column is wider than half of the row
- **THEN** it is clipped with the ellipsis and the second column still starts at one column on every row

### Requirement: Config models are aligned
The `Models` section of the `/config` panel SHALL pad every model name to the widest name in the section and start the details (`display=…`, `window=…`, `prices=…`) at one column after two spaces. The other sections of `/config` and the `/session` panel keep their current alignment.

#### Scenario: Models of different length
- **WHEN** the config has `openai/gpt-6.2-sol` and `claude/claude-sonnet-5-5`
- **THEN** both `window=…` details start at the same column
