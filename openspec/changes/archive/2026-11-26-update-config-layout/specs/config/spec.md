## ADDED Requirements

### Requirement: Config layout
The config table SHALL accept the keys `ui`, `history`, `open`, `providers`, `models`, `systems`, `prompts`, `functions`, `hooks`, and `commands`. `ui` SHALL accept `lines`, `input`, `space`, and `theme`. `history` SHALL accept `hide`. The keys `settings` and `theme` at the top level MUST fail validation, with `settings: moved to ui, history, and open` and `theme: moved to ui.theme`. Unknown keys MUST fail with their path.

#### Scenario: Old settings
- **WHEN** a config returns `{ settings = { ui = {} } }`
- **THEN** loading fails with `tuse.lua: settings: moved to ui, history, and open`

#### Scenario: Old theme
- **WHEN** a config returns `{ theme = { accent = "red" } }`
- **THEN** loading fails with `tuse.lua: theme: moved to ui.theme`

#### Scenario: New layout
- **WHEN** a config returns `{ ui = { lines = { diff = 0 } }, history = { hide = { "usage" } } }`
- **THEN** it loads and diffs are hidden

### Requirement: UI line limits
`ui.lines` SHALL accept `tool`, `output`, `diff`, `reasoning`, `strings`, `input`, and `list`. `tool`, `output`, `diff`, and `reasoning` are integers `>= -1`, where `-1` is unlimited and `0` hides the item, with defaults `-1`, `1`, `12`, and `-1`. `strings` is an integer `>= -1`, default `1`: `0` shows a multiline Lua string in a tool call as `…` alone, `N > 0` shows its first N lines followed by `…` when more lines exist, and `-1` shows all lines. `input` is `-1` or `>= 1` and limits the visible rows of the input, default `10`. `list` is `>= 1` and limits the rows of pickers and panels, default `16`. The old names `fold_strings`, `tool_line`, `output_line`, `diff_line`, `reasoning_line`, `input_max_height`, and `list_max_height` MUST fail as unknown keys.

#### Scenario: Strings folded to the first line
- **WHEN** `ui.lines.strings` is omitted and a tool call has a three-line long string
- **THEN** the first line of the string and `…` are shown

#### Scenario: Strings hidden
- **WHEN** `ui.lines.strings = 0`
- **THEN** the string is shown as `…`

#### Scenario: Strings unlimited
- **WHEN** `ui.lines.strings = -1`
- **THEN** all lines of the string are shown

#### Scenario: Input height
- **WHEN** `ui.lines.input` is omitted and the draft has 15 rows
- **THEN** 10 rows are visible and the editor scrolls with the cursor

#### Scenario: Invalid value
- **WHEN** `ui.lines.list = 0` or `ui.lines.diff = -2`
- **THEN** loading fails with an error naming `ui.lines.list` or `ui.lines.diff`

### Requirement: Fixed wheel scroll
The mouse wheel SHALL scroll the chat by 2 lines per tick. `scroll_step` MUST fail as an unknown key in `ui` and everywhere else.

#### Scenario: Wheel tick
- **WHEN** the user scrolls the wheel one tick over the chat
- **THEN** the chat moves 2 lines

#### Scenario: Old key
- **WHEN** a config sets `ui.scroll_step`
- **THEN** loading fails with an unknown-key error

### Requirement: UI spacing
`ui.space` SHALL accept `padding`, `steps`, and `messages`, all integers `>= 0`. `padding` is the side padding in columns, default `0`. `steps` is the number of blank rows between tool calls and reasoning blocks, default `1`. `messages` is the number of blank rows above and below user and agent messages, default `1`. The old names `pad_x` and `message_gap` MUST fail as unknown keys.

#### Scenario: No gaps
- **WHEN** `ui.space = { steps = 0, messages = 0 }`
- **THEN** consecutive tool calls, reasoning blocks, and messages have no blank rows between them

#### Scenario: Defaults
- **WHEN** `ui.space` is omitted
- **THEN** one blank row separates steps and messages and there is no side padding

### Requirement: Theme tokens
`ui.theme` SHALL accept the colors `bg`, `accent`, `fg`, `user`, `user_bg`, `muted`, `input`, `input_bg`, `border`, `icons`, and `error`, and the tables `code` and `markdown`. `fg` colors agent messages, `user` colors user messages, `user_bg` is the background of user messages, `muted` colors secondary text, `input` colors the input text, `input_bg` is the background or frame of the input, `border` colors other borders, `icons` colors every icon and spinner, and `error` colors error text. `code` SHALL accept `keyword`, `storage`, `constant`, `operator`, `punctuation`, `number`, `string`, `escape`, `comment`, `function`, `builtin`, `method`, `field`, `parameter`, `variable`, `added`, and `removed`. `markdown` SHALL accept `heading`, `link`, `code`, `quote`, and `list`. A value SHALL be a color or the name of a top-level color token, except itself. Omitted tokens SHALL take the defaults of the design. The old colors `surface`, `dim`, `ok`, `err`, `info` and the table `lua` MUST fail as unknown keys. Link cycles, unknown tokens, and invalid colors MUST fail validation. `/config` and `config.get` SHALL expose the resolved tokens.

#### Scenario: Partial override
- **WHEN** `ui.theme.code.operator = "accent"`
- **THEN** operators use the accent color and every other token keeps its default

#### Scenario: Linked token
- **WHEN** `ui.theme = { accent = "#FFB86C", icons = "accent" }`
- **THEN** icons use `#FFB86C`

#### Scenario: User colors
- **WHEN** `ui.theme.user = "#88C0D0"` and `ui.theme.user_bg = "#222222"`
- **THEN** user messages use that text color and background and agent replies keep `fg`

#### Scenario: Removed color
- **WHEN** `ui.theme.surface` or `ui.theme.lua` is set
- **THEN** loading fails with an unknown-key error

#### Scenario: Invalid link
- **WHEN** `ui.theme.user = "user"` or `ui.theme.code.keyword = "nope"`
- **THEN** validation rejects the config

### Requirement: Configured models limit the model list
When `models` has entries, `/models` SHALL list only those models. When `models` is empty or omitted, `/models` SHALL list every available model. `scope_models` MUST fail as an unknown key. `model.list` and `model.set` MUST ignore the restriction.

#### Scenario: Listed models only
- **WHEN** `models` has two entries and the user opens `/models`
- **THEN** only those two models are listed

#### Scenario: No models set
- **WHEN** the config has no `models`
- **THEN** `/models` lists every available model

#### Scenario: Old key
- **WHEN** a config sets `ui.scope_models`
- **THEN** loading fails with an unknown-key error

## MODIFIED Requirements

### Requirement: Open handlers under settings
`open` SHALL be read from the top-level `open` (`key -> function(path)`). A key is an extension without a dot (`md`, case-insensitive), a mask with `*` or `?` matched against the file name (`*.test.lua`, `.env*`, case-insensitive), the catch-all `*`, `@dir` for folders, or `@url` for `http://`, `https://`, and `mailto:` links, which get the URL. Any other key MUST fail validation, and keys MUST NOT contain `/`, `\`, whitespace, or control characters. A file SHALL use the handler of its extension, else the most specific mask (the one with the most literal characters, ties in alphabetical order), else `*`, else the OS opener. A folder SHALL use `@dir`, else `*`, else the OS opener. `settings.open` MUST fail with `settings: moved to ui, history, and open`.

#### Scenario: Old key
- **WHEN** a config returns `{ settings = { open = { md = f } } }`
- **THEN** loading fails with `tuse.lua: settings: moved to ui, history, and open`

#### Scenario: New key
- **WHEN** a config returns `{ open = { md = f } }` and a `.md` path is clicked
- **THEN** `f` is called with the path

#### Scenario: Mask beats catch-all
- **WHEN** `open = { ["*.test.lua"] = a, ["*"] = b }` and `x.test.lua` is clicked
- **THEN** `a` is called, and clicking `y.txt` calls `b`

#### Scenario: Extension beats mask
- **WHEN** `open = { lua = a, ["*.test.lua"] = b }` and `x.test.lua` is clicked
- **THEN** `a` is called

#### Scenario: Folder
- **WHEN** `open = { ["@dir"] = f }` and an existing folder is clicked
- **THEN** `f` is called with the folder path

#### Scenario: URL
- **WHEN** `open = { ["@url"] = f }` and `https://example.com` is clicked
- **THEN** `f` is called with `https://example.com`, and a `javascript:` link is still refused

#### Scenario: Invalid key
- **WHEN** a config sets `open = { ["@file"] = f }` or `open = { [".md"] = f }`
- **THEN** loading fails with an error naming the key

### Requirement: History filter setting
`history.hide` SHALL be a list of history roles hidden from the TUI `/history` list. Unknown roles MUST fail validation. `session().history` and `session.history` stay complete.

#### Scenario: Hiding hooks
- **WHEN** `history = { hide = { "hook", "usage" } }` and the user opens `/history`
- **THEN** no `hook` or `usage` entry is listed, while `session().history` still has them

#### Scenario: Unknown role
- **WHEN** `history = { hide = { "hooks" } }`
- **THEN** loading fails with ``history.hide[1]: unknown history role `hooks` ``

#### Scenario: Old key
- **WHEN** a config sets `ui.hide_history` or `settings.ui.hide_history`
- **THEN** loading fails with an unknown-key or moved error

### Requirement: Config view is complete
`/config` and `config.get` SHALL show every resolved config field, including `models` (name, display, window, prices), `open`, `ui` (lines, input, space, theme), `history.hide`, and prompt kinds and args. Secrets and Lua bodies MUST stay hidden.

#### Scenario: Models shown
- **WHEN** the config has `models` and the user opens `/config`
- **THEN** a `Models` section lists each model with its metadata

#### Scenario: No field forgotten
- **WHEN** a field is added to `Config`, `Ui`, or `History` but not to `/config`
- **THEN** `cargo test` fails

### Requirement: No compaction settings
`compact_at` MUST NOT be accepted in `models` entries or anywhere in the config.

#### Scenario: Old key
- **WHEN** a config sets `models[1].compact_at = 100000`
- **THEN** loading fails with an unknown-key error for `models[1].compact_at`

### Requirement: Input composer style setting
`ui.input` SHALL accept `boxed`, `lines`, `fill`, and `bar`. If omitted, it SHALL resolve to `boxed`. Other values, including `classic`, MUST fail config validation. `/config` and `config.get` SHALL expose the resolved value.

#### Scenario: Explicit style
- **WHEN** the config sets `ui.input = "bar"` and is loaded or reloaded successfully
- **THEN** the TUI selects the bar composer and `/config` and `config.get` show `bar`

#### Scenario: Default and invalid styles
- **WHEN** the setting is omitted
- **THEN** it resolves to `boxed`
- **WHEN** the setting is `classic` or any unsupported value
- **THEN** validation rejects it with an error identifying `ui.input`

### Requirement: Custom commands with args and description
A `commands` value SHALL be a table `{ args?, desc?, run }`. A bare function MUST fail with `commands.<name>: expected a table`. `args` is an optional list of unique non-empty strings without whitespace. `desc` is an optional string. `run` is required and runs as a Lua job without a deadline, and its return value is ignored. Without `args`, `run` SHALL get the words typed after the command name, split on whitespace, as positional parameters (a word wrapped in `"…"` or `'…'` keeps its spaces, `\"` or `\'` inside the same quotes is a literal quote, and an unclosed quote is an error): at most as many as `run` declares, or any number when `run` is variadic; missing words are `nil`. With `args`, `run` SHALL get one word from `args` as its first parameter, or `nil` when none is given. Command names keep the existing rules: not empty, no leading `/`, no whitespace or control characters, and not a built-in name. The resolved config SHALL expose `desc`, `args`, and `params` (the declared parameter count, `null` when variadic) per command and never the function.

#### Scenario: Words without args
- **WHEN** `commands.mv = { run = function(from, to) end }` and the user runs `/mv a.txt b.txt`
- **THEN** `run("a.txt", "b.txt")` runs
- **WHEN** the user runs `/mv a.txt`
- **THEN** `to` is `nil`
- **WHEN** the user runs `/mv a b c`
- **THEN** nothing runs and `/mv takes at most 2 words` is shown

#### Scenario: Quoted words
- **WHEN** `commands.commit = { run = function(msg) end }` and the user runs `/commit "fix the bug"`
- **THEN** `run("fix the bug")` runs
- **WHEN** the user runs `/commit "fix`
- **THEN** nothing runs and `/commit: unclosed quote` is shown

#### Scenario: Command with args
- **WHEN** `commands.deploy = { desc = "ship it", args = { "staging", "prod" }, run = f }` and the user runs `/deploy prod`
- **THEN** `f("prod")` runs
- **WHEN** the user runs `/deploy nope` or `/deploy staging prod`
- **THEN** nothing runs and the error is shown

#### Scenario: No words accepted
- **WHEN** `commands.diff = { run = function() end }` and the user runs `/diff x`
- **THEN** nothing runs and `/diff takes no words` is shown

#### Scenario: Function form
- **WHEN** a config sets `commands.diff = function() end`
- **THEN** loading fails with `commands.diff: expected a table`

#### Scenario: Invalid command
- **WHEN** `run` is missing, `args = { "a b" }`, or `desc = 1`
- **THEN** loading fails with `commands.<name>.run: is required`, `commands.<name>.args[1]: must not contain whitespace`, or `commands.<name>.desc: expected a string`

#### Scenario: Resolved config
- **WHEN** a client calls `config.get` for the `deploy` command above
- **THEN** it holds `{ "desc": "ship it", "args": ["staging", "prod"], "params": 1 }` and no function body

## REMOVED Requirements

### Requirement: Model scope setting
**Reason**: `/models` now lists only the configured `models` when they are set, so a second list is redundant.

### Requirement: Lua syntax theme
**Reason**: Replaced by `ui.theme.code`, defined in "Theme tokens".

### Requirement: User text color
**Reason**: Replaced by `ui.theme.user` and `ui.theme.user_bg`, defined in "Theme tokens".
