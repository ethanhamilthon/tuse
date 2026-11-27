## ADDED Requirements

### Requirement: Global config override via TUSE_GLOBAL_CONFIG
Entry config lookup SHALL use the precedence `-c`/`--config` > `./tuse.lua` > `$TUSE_GLOBAL_CONFIG` > `~/.config/tuse.lua` > `~/.config/tuse/init.lua` > built-in defaults. `require("global")` SHALL resolve against `$TUSE_GLOBAL_CONFIG` when set and non-empty, otherwise `~/.config/tuse.lua` then `~/.config/tuse/init.lua`. Unset, empty, or whitespace-only `TUSE_GLOBAL_CONFIG` values SHALL be treated as unset. A non-empty `TUSE_GLOBAL_CONFIG` SHALL expand `~/…` and resolve relative paths against the session working directory; if the resolved path is a directory, Tuse SHALL load `<dir>/init.lua`, otherwise that file directly. When `TUSE_GLOBAL_CONFIG` is set and non-empty and its target file is missing or unreadable, config loading and `require("global")` MUST fail with an error naming the missing path rather than silently falling back to `~/.config/tuse.lua`.

#### Scenario: Distro override takes precedence over default global config
- **WHEN** `TUSE_GLOBAL_CONFIG=~/distros/work/init.lua` is set, no `-c` flag is passed, and no `./tuse.lua` exists
- **THEN** Tuse loads `~/distros/work/init.lua` instead of `~/.config/tuse.lua`

#### Scenario: Project config and explicit flag win over TUSE_GLOBAL_CONFIG
- **WHEN** `TUSE_GLOBAL_CONFIG` is set and `./tuse.lua` exists in the project folder
- **THEN** `./tuse.lua` is loaded as the entry config, and `require("global")` inside `./tuse.lua` resolves to the `$TUSE_GLOBAL_CONFIG` target
- **WHEN** `-c custom.lua` is also passed
- **THEN** `custom.lua` is loaded as the entry config

#### Scenario: Directory path and empty override
- **WHEN** `TUSE_GLOBAL_CONFIG=/opt/tuse-distro` points to a directory containing `init.lua`
- **THEN** Tuse loads `/opt/tuse-distro/init.lua`
- **WHEN** `TUSE_GLOBAL_CONFIG=""` or `"   "`
- **THEN** Tuse treats it as unset and falls back to `~/.config/tuse.lua` or `~/.config/tuse/init.lua`

#### Scenario: Missing override fails fast without fallback
- **WHEN** `TUSE_GLOBAL_CONFIG=/no/such/distro.lua` is set and `~/.config/tuse.lua` exists
- **THEN** loading without `-c` or `./tuse.lua` fails with an error naming `/no/such/distro.lua` instead of loading `~/.config/tuse.lua`

### Requirement: Model effort availability
The shared default effort list for all models and all providers SHALL be `low`, `medium`, `high`, `xhigh`, and `max`. Legacy `"default"` is retired and MUST fail validation wherever an effort name is accepted. A `models` entry MAY declare an optional `efforts` dense list of unique valid effort names:
- When `efforts` is omitted (or when a model has no `models` entry), the shared default list `["low", "medium", "high", "xhigh", "max"]` SHALL apply.
- When `efforts` is explicitly empty (`efforts = {}`), the model SHALL be treated as a non-reasoning model with no available efforts, and Tuse MUST NOT send any reasoning parameter on the wire; explicitly empty `efforts = {}` MUST NOT be conflated with omitted `efforts`.
- When `efforts` is explicitly non-empty, that declared list SHALL be the sole source of truth for that model's available efforts.
Unknown or duplicate effort names (including `"default"`) in `models[i].efforts` MUST fail config validation with their path. When an effort is omitted or unset (or when recovering a legacy/invalid saved effort), the fallback effort SHALL be `medium` if `medium` is in the model's resolved list, else the first declared effort in that list, and no effort (`""`) when the resolved list is explicitly empty (`efforts = {}`). Every public selection and validation entrypoint (`/models`, JSON-RPC `model.set`, Lua `set_session`, Lua `llm`, saved selections, and turn validation) SHALL use this same model effort resolver rather than hardcoded provider-API effort allowlists. `config.get.models[]` SHALL omit `efforts` when unset and serialize `"efforts": []` when explicitly empty; `model_info.efforts`, `/config`, and `tuse models` SHALL expose the model's effort availability.

#### Scenario: Model declares a restricted effort list
- **WHEN** `models` configures `{ name = "gemini/gemini-2.5-pro", efforts = { "low", "medium", "high" } }`
- **THEN** the resolver returns `["low", "medium", "high"]` for `gemini/gemini-2.5-pro`, fallback is `"medium"`, and `xhigh` or `max` is rejected for that model

#### Scenario: Model omits efforts and uses shared default list
- **WHEN** a model entry omits `efforts` or the model is not listed in `models`
- **THEN** the resolver returns `["low", "medium", "high", "xhigh", "max"]` regardless of provider API type, fallback is `"medium"`, and `config.get.models[]` omits `efforts`

#### Scenario: Explicitly empty efforts disables reasoning parameters
- **WHEN** `models` configures `{ name = "openai/gpt-4o", efforts = {} }`
- **THEN** `config.get.models[]` serializes `"efforts": []`, the resolver returns `[]` with no fallback effort (`""`), and requests to `openai/gpt-4o` omit reasoning parameters

#### Scenario: Fallback uses first declared effort when medium is excluded
- **WHEN** `models` configures `{ name = "p/fast", efforts = { "high", "low" } }`
- **THEN** the fallback effort for `p/fast` is `"high"` (the first declared effort)

#### Scenario: Invalid or retired default effort fails config validation
- **WHEN** a config sets `models = { { name = "p/m", efforts = { "low", "default" } } }` or `{ "low", "turbo" }` or `{ "low", "low" }`
- **THEN** loading fails with an error identifying `models[1].efforts[2]`

### Requirement: Reload clears preloaded config modules
When a config loads a module registered in trusted `package.preload`, `/reload`, `reload()`, and `config.reload` SHALL clear that cached module before re-evaluating the config so an updated preloader runs again. If the reload fails, the previous cached module and preloader state SHALL be restored.

#### Scenario: Preloaded module reloads
- **WHEN** a config registers `package.preload.helper`, requires `"helper"`, and the user updates the preloader and runs `/reload`
- **THEN** the updated `package.preload.helper` loader runs and its new result is used

#### Scenario: Failed reload restores preloaded module
- **WHEN** a subsequent `/reload` fails after modifying `package.preload.helper`
- **THEN** the previous config and previously loaded `helper` module remain active

## MODIFIED Requirements

### Requirement: History filter setting
`history.hide` SHALL be a list of history roles hidden from the TUI `/history` list. Valid roles are those of the session history: `system`, `user`, `agent`, `lua`, `notification`, `message`, `usage`, `meta`, `turn_end`, `show`, `pick`, `ask`, `task`, `command`, `open`. It SHALL also accept the message levels `flash`, `notice`, `error`, which hide only `message` entries of that level. Unknown names, including the removed `tool`, `hook`, `model`, `title`, `turn`, MUST fail validation with the element path `history.hide[N]` without duplicating the `history.hide:` prefix. `session().history` and `session.history` stay complete.

#### Scenario: Hiding usage
- **WHEN** `history = { hide = { "usage", "turn_end" } }` and the user opens `/history`
- **THEN** no `usage` or `turn_end` entry is listed, while `session().history` still has them

#### Scenario: Hiding a message level
- **WHEN** `history = { hide = { "flash" } }` and the user opens `/history`
- **THEN** `message` entries with level `flash` are not listed, while `error` messages are

#### Scenario: Removed role
- **WHEN** `history = { hide = { "hook" } }`
- **THEN** loading fails with ``tuse.lua: history.hide[1]: unknown history role `hook` `` and MUST NOT repeat `history.hide:` twice

#### Scenario: Old key
- **WHEN** a config sets `ui.hide_history` or `settings.ui.hide_history`
- **THEN** loading fails with an unknown-key or moved error

### Requirement: Prompts with args
A `prompts` value SHALL be a string or a table `{ args?, run }`. `args` is an optional dense list (including empty `{}`) of unique non-empty strings without whitespace; non-integer keys or holes in `args` MUST fail validation with the path. `run(arg)` gets the chosen arg or `nil` and MUST return a string, which becomes the prompt body, sent as `<prompt name="<name>" arg="<arg>">`. Only that returned string MUST reach the model, never the function or the `args` list.

#### Scenario: Invalid args
- **WHEN** `args = { "a b" }` or `run` is missing
- **THEN** loading fails with `prompts.<name>.args[1]: must not contain whitespace` or `prompts.<name>.run: is required`

#### Scenario: Empty or non-dense args
- **WHEN** `prompts.p = { args = {}, run = f }`
- **THEN** loading succeeds with `args = []`
- **WHEN** `prompts.p = { args = { "a", extra = "b" }, run = f }`
- **THEN** loading fails with an error naming `prompts.p.args`

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
`/config` and `config.get` SHALL show every resolved config field, including `providers` (all active saved and Lua providers with only `name`, `type`, and `base` — never keys, header names or values, or extra metadata), `models` (`name`, `display`, `window`, `efforts`, `prices`), `open`, `ui` (`lines`, `input`, `space`, `theme`), `history.hide`, and prompt kinds and args. Secrets and Lua bodies MUST stay hidden.

#### Scenario: Active providers shown without secrets
- **WHEN** a session has both a saved provider from `tuse connect` and a Lua `providers` entry with API keys and custom headers
- **THEN** `config.get` exposes `providers` as `[{ "name": ..., "type": ..., "base": ... }]` for all active providers with no keys, headers, or extra fields, and `/config` renders a `Providers` section with only `name`, `type`, and `base`

#### Scenario: Models shown
- **WHEN** the config has `models` (including custom `efforts` or `efforts = {}`) and the user opens `/config`
- **THEN** a `Models` section lists each model with its metadata including `efforts` (`efforts=none` for `efforts = {}`)

#### Scenario: No field forgotten
- **WHEN** a field is added to `Config`, `Ui`, or `History` but not to `/config`
- **THEN** `cargo test` fails

### Requirement: Custom commands with args and description
A `commands` value SHALL be a table `{ args?, desc?, run }`. A bare function MUST fail with `commands.<name>: expected a table`. `args` is an optional dense list (including empty `{}`) of unique non-empty strings without whitespace; non-integer keys or holes in `args` MUST fail validation with the path. `desc` is an optional string. `run` is required and runs as a Lua job without a deadline, and its return value is ignored. Without `args`, `run` SHALL get the words typed after the command name, split on whitespace, as positional parameters (a word wrapped in `"…"` or `'…'` keeps its spaces, `\"` or `\'` inside the same quotes is a literal quote, and an unclosed quote is an error): at most as many as `run` declares, or any number when `run` is variadic; missing words are `nil`. With `args`, `run` SHALL get one word from `args` as its first parameter, or `nil` when none is given. Command names keep the existing rules: not empty, no leading `/`, no whitespace or control characters, and not a built-in name. The resolved config SHALL expose `desc`, `args`, and `params` (the declared parameter count, `null` when variadic) per command and never the function.

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

#### Scenario: Empty or non-dense args
- **WHEN** `commands.c = { args = {}, run = f }`
- **THEN** loading succeeds with `args = []`
- **WHEN** `commands.c = { args = { "a", extra = "b" }, run = f }`
- **THEN** loading fails with an error naming `commands.c.args`

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

### Requirement: Config layout
The config table SHALL accept the keys `ui`, `history`, `open`, `providers`, `models`, `systems`, `prompts`, `functions`, `hooks`, and `commands`. `ui` SHALL accept `lines`, `input`, `space`, and `theme`. `history` SHALL accept `hide`. The keys `settings` and `theme` at the top level MUST fail validation, with `settings: moved to ui, history, and open` and `theme: moved to ui.theme`. Unknown keys and unsupported Lua value types (including functions, userdata, and threads outside the validated callback slots `hooks.*.run`, `commands.*.run`, `prompts.*.run`, `functions.*`, and `open.*`) MUST fail validation with their path rather than being silently dropped. Validation errors under `providers.*` MUST continue to avoid echoing secret values.

#### Scenario: Old settings
- **WHEN** a config returns `{ settings = { ui = {} } }`
- **THEN** loading fails with `tuse.lua: settings: moved to ui, history, and open`

#### Scenario: Old theme
- **WHEN** a config returns `{ theme = { accent = "red" } }`
- **THEN** loading fails with `tuse.lua: theme: moved to ui.theme`

#### Scenario: New layout
- **WHEN** a config returns `{ ui = { lines = { diff = 0 } }, history = { hide = { "usage" } } }`
- **THEN** it loads and diffs are hidden

#### Scenario: Unknown function-valued key
- **WHEN** a config returns `{ on_start = function() end }` or `{ ui = { typo = function() end } }`
- **THEN** loading fails with an error naming `on_start` or `ui.typo`

#### Scenario: Extra function inside a hook entry
- **WHEN** a config returns `{ hooks = { h = { events = { "user_message" }, run = f, extra = f } } }`
- **THEN** loading fails with an error naming `hooks.h.extra`

### Requirement: Theme tokens
`ui.theme` SHALL accept the colors `bg`, `accent`, `fg`, `user`, `user_bg`, `muted`, `input`, `input_bg`, `border`, `icons`, and `error`, and the tables `code` and `markdown`. `fg` colors agent messages, `user` colors user messages, `user_bg` is the background of user messages, `muted` colors secondary text, `input` colors the input text, `input_bg` is the background or frame of the input, `border` colors other borders, `icons` colors every icon and spinner, and `error` colors error text. `code` SHALL accept `keyword`, `storage`, `constant`, `operator`, `punctuation`, `number`, `string`, `escape`, `comment`, `function`, `builtin`, `method`, `field`, `parameter`, `variable`, `added`, and `removed`. `markdown` SHALL accept `heading`, `link`, `code`, `quote`, and `list`. A value SHALL be a color or the name of a top-level color token, except itself. Omitted tokens SHALL take the defaults of the design. The old colors `surface`, `dim`, `ok`, `err`, `info` and the table `lua` MUST fail as unknown keys. Link cycles, unknown tokens, and invalid colors MUST fail validation. `config.get` SHALL expose `ui.theme` by token key preserving raw token links (not resolving links away into hex/color strings), and `/config` SHALL render each token's resolved color swatch beside its raw token value or link.

#### Scenario: Partial override
- **WHEN** `ui.theme.code.operator = "accent"`
- **THEN** operators use the accent color and every other token keeps its default

#### Scenario: Linked token exposed raw in config.get
- **WHEN** `ui.theme = { accent = "#FFB86C", icons = "accent" }`
- **THEN** icons render using `#FFB86C`, `config.get` exposes `ui.theme.icons == "accent"` (raw link preserved), and `/config` renders a `#FFB86C` swatch next to `accent`

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
When `models` has entries, `/models` SHALL list only those models. When `models` is empty or omitted, `/models` SHALL list every available model. `scope_models` MUST fail as an unknown key. `model.list` and `model.set` MUST ignore the model-list scoping restriction (while `model.set` still validates effort against the shared model effort resolver).

#### Scenario: Listed models only
- **WHEN** `models` has two entries and the user opens `/models`
- **THEN** only those two models are listed

#### Scenario: No models set
- **WHEN** the config has no `models`
- **THEN** `/models` lists every available model

#### Scenario: Old key
- **WHEN** a config sets `ui.scope_models`
- **THEN** loading fails with an unknown-key error

## REMOVED Requirements

### Requirement: Trace filter setting`
**Reason**: Removes the corrupted leftover `RENAMED` merge fragment at `openspec/specs/config/spec.md:97-100` (``### Requirement: Trace filter setting` ``, `- TO: \``, and duplicate ``### Requirement: History filter setting` ``), preserving the canonical `### Requirement: History filter setting` at lines 41–58.
