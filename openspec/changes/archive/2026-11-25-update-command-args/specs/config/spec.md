## ADDED Requirements
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
