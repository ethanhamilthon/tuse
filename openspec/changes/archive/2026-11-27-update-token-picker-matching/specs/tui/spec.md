## ADDED Requirements

### Requirement: Token picker matching
The `/` and `#` pickers, and the arg pickers of a command or prompt, SHALL list only the items whose name starts with the typed text, ignoring case. The `/` or `#` mark and the description MUST NOT be searched. An empty query SHALL list every item. Items keep the order of the list. The `@` picker SHALL list the files and folders whose name contains the typed text, ignoring case, with the names that start with it first. Inside each group, and for an empty query, the `@` picker SHALL order items by the last modified time, newest first, using the created time when the modified time is missing. The directory listing SHALL be sorted before the picker size cap, so the newest entries are kept.

#### Scenario: Prompt prefix
- **WHEN** the prompts are `imagegen`, `openspec`, `release`, `srt`, and `use-figma` and the user types `#r`
- **THEN** the picker lists only `#release`

#### Scenario: Description is not searched
- **WHEN** the user types `/r` and `/config` has the description `show the resolved config`
- **THEN** `/config` is not listed, and `/resume`, `/reload` are

#### Scenario: Case is ignored
- **WHEN** the user types `#REL`
- **THEN** the picker lists `#release`

#### Scenario: Arg prefix
- **WHEN** the user types `#openspec ` followed by `pr`
- **THEN** the picker lists only `propose`

#### Scenario: File substring with prefix first
- **WHEN** the folder holds `main.rs` (older), `domain.rs` (newest), and `mod.rs`, and the user types `@ma`
- **THEN** the picker lists `main.rs` first, then `domain.rs`

#### Scenario: Newest files first
- **WHEN** the user types `@` and `old.txt` was modified before `new.txt` and the folder `docs/`
- **THEN** the picker lists the most recently modified entry first

#### Scenario: Large folder
- **WHEN** a folder holds more entries than the picker cap
- **THEN** the picker keeps the newest entries, not the first by name

## MODIFIED Requirements

### Requirement: Picker columns are aligned
A picker list SHALL draw each item as columns. Every cell of a column SHALL start at the same display column on every row, padded to the widest cell of that column among all items of the picker, with two spaces between columns. The last column is free text clipped at the row end. A column wider than half of the row width SHALL be clipped with the ellipsis so the next column keeps its room. `/history` rows SHALL have the columns index, role, and summary. Items with a label and a detail SHALL align the detail as the second column. Search in the `/history`, `/resume`, `/fork`, `/models`, and `/tasks` pickers SHALL match the cells joined by a space. The `/`, `#`, `@`, and arg pickers follow the token picker matching requirement.

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

#### Scenario: Row search
- **WHEN** the user types a word that appears only in the summary of a `/history` row
- **THEN** that row is listed
