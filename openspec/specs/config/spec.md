# config

## Purpose
Config keys and their validation.

## Requirements

1

### Requirement: Open handlers under settings
`open` SHALL be read from `settings.open` (`key -> function(path)`). A key is an extension without a dot (`md`, case-insensitive), a mask with `*` or `?` matched against the file name (`*.test.lua`, `.env*`, case-insensitive), the catch-all `*`, `@dir` for folders, or `@url` for `http://`, `https://`, and `mailto:` links, which get the URL. Any other key MUST fail validation, and keys MUST NOT contain `/`, `\`, whitespace, or control characters. A file SHALL use the handler of its extension, else the most specific mask (the one with the most literal characters, ties in alphabetical order), else `*`, else the OS opener. A folder SHALL use `@dir`, else `*`, else the OS opener. A top-level `open` key MUST fail with `open: moved to settings.open`.

#### Scenario: Old key
- **WHEN** a config returns `{ open = { md = f } }`
- **THEN** loading fails with `tuse.lua: open: moved to settings.open`

#### Scenario: New key
- **WHEN** a config returns `{ settings = { open = { md = f } } }` and a `.md` path is clicked
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

### Requirement: Input composer style setting
`settings.ui.input_style` SHALL accept `boxed`, `lines`, `fill`, and `bar`, and SHALL accept `classic` as an alias of `lines`. If omitted, it SHALL resolve to `boxed`. Other values MUST fail config validation. `/config` and `config.get` SHALL expose the resolved value, showing `lines` for `classic`.

#### Scenario: Explicit style
- **WHEN** the config sets `settings.ui.input_style = "bar"` and is loaded or reloaded successfully
- **THEN** the TUI selects the bar composer and `/config` and `config.get` show `bar`

#### Scenario: Default, alias, and invalid styles
- **WHEN** the setting is omitted
- **THEN** it resolves to `boxed`
- **WHEN** the setting is `classic`
- **THEN** it resolves to `lines`
- **WHEN** an unsupported value is supplied
- **THEN** validation rejects it with an error identifying `settings.ui.input_style`

### Requirement: Custom commands with args and description
A `commands` value SHALL be a function or a table `{ args?, desc?, run }`; a function means `{ run = f }`. `args` is an optional list of unique non-empty strings without whitespace. `desc` is an optional string. `run` runs as a Lua job without a deadline and its return value is ignored. Without `args`, `run` SHALL get the words typed after the command name, split on whitespace, as positional parameters (a word wrapped in `"…"` or `'…'` keeps its spaces, `\"` or `\'` inside the same quotes is a literal quote, and an unclosed quote is an error): at most as many as `run` declares, or any number when `run` is variadic; missing words are `nil`. With `args`, `run` SHALL get one word from `args` as its first parameter, or `nil` when none is given. Command names keep the existing rules: not empty, no leading `/`, no whitespace or control characters, and not a built-in name. The resolved config SHALL expose `desc`, `args`, and `params` (the declared parameter count, `null` when variadic) per command and never the function.

#### Scenario: Words without args
- **WHEN** `commands.mv = function(from, to) ... end` and the user runs `/mv a.txt b.txt`
- **THEN** `run("a.txt", "b.txt")` runs
- **WHEN** the user runs `/mv a.txt`
- **THEN** `to` is `nil`
- **WHEN** the user runs `/mv a b c`
- **THEN** nothing runs and `/mv takes at most 2 words` is shown

#### Scenario: Quoted words
- **WHEN** `commands.commit = function(msg) ... end` and the user runs `/commit "fix the bug"`
- **THEN** `run("fix the bug")` runs
- **WHEN** the user runs `/commit "fix`
- **THEN** nothing runs and `/commit: unclosed quote` is shown

#### Scenario: Command with args
- **WHEN** `commands.deploy = { desc = "ship it", args = { "staging", "prod" }, run = f }` and the user runs `/deploy prod`
- **THEN** `f("prod")` runs
- **WHEN** the user runs `/deploy nope` or `/deploy staging prod`
- **THEN** nothing runs and the error is shown

#### Scenario: Function form
- **WHEN** `commands.diff = function() end` and the user runs `/diff`
- **THEN** it runs; `/diff x` fails with `/diff takes no words`

#### Scenario: Invalid command
- **WHEN** `run` is missing, `args = { "a b" }`, or `desc = 1`
- **THEN** loading fails with `commands.<name>.run: is required`, `commands.<name>.args[1]: must not contain whitespace`, or `commands.<name>.desc: expected a string`

#### Scenario: Resolved config
- **WHEN** a client calls `config.get` for the `deploy` command above
- **THEN** it holds `{ "desc": "ship it", "args": ["staging", "prod"], "params": 1 }` and no function body
