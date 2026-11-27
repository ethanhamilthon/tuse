# config

## Purpose
Config keys and their validation.

## Requirements

### Requirement: Config layout
Config files (`tuse.lua`, `-c`/`--config PATH`, and the table returned by global `init.lua`) SHALL return a table of named namespaces mapping each namespace name to a contribution table, replacing the legacy flat top-level config layout with no backward compatibility. Each namespace contribution table SHALL accept the keys `ui`, `history`, `open`, `providers`, `models`, `systems`, `prompts`, `functions`, `hooks`, and `commands`. `ui` SHALL accept `lines`, `input`, `space`, and `theme`. `history` SHALL accept `hide`. Top-level `global` and `plugins` SHALL be reserved control keys rather than namespace contribution tables. Legacy flat top-level config tables (including `settings`, `theme`, or bare contribution keys without a namespace), unknown keys, and unsupported Lua value types (including functions, userdata, and threads outside the validated callback slots `hooks.*.run`, `commands.*.run`, `prompts.*.run`, `functions.*`, and `open.*`) MUST fail validation with their path rather than being silently dropped. Validation errors under `providers.*` MUST continue to avoid echoing secret values.

#### Scenario: Namespaced config loads
- **WHEN** a config returns `{ herdr = { functions = { ping = f }, prompts = { hi = "hello" } }, ui_cfg = { ui = { lines = { diff = 0 } }, history = { hide = { "usage" } } } }`
- **THEN** loading succeeds and merges the contributions into the active config

#### Scenario: Legacy flat config rejected
- **WHEN** a config returns `{ ui = { lines = { diff = 0 } } }` or `{ commands = { c = { run = f } } }` or `{ settings = { ui = {} } }` or `{ theme = { accent = "red" } }`
- **THEN** loading fails validation without backward compatibility

#### Scenario: Unknown key inside a namespace
- **WHEN** a config returns `{ my = { unknown_key = 1 } }`
- **THEN** loading fails with an error naming `my.unknown_key`

#### Scenario: Unknown function-valued key
- **WHEN** a config returns `{ my = { on_start = function() end } }` or `{ my = { ui = { typo = function() end } } }`
- **THEN** loading fails with an error naming `my.on_start` or `my.ui.typo`

#### Scenario: Extra function inside a hook entry
- **WHEN** a config returns `{ my = { hooks = { h = { events = { "user_message" }, run = f, extra = f } } } }`
- **THEN** loading fails with an error naming `my.hooks.h.extra`

### Requirement: Global config override via TUSE_GLOBAL_CONFIG
Global config lookup SHALL use `$TUSE_GLOBAL_CONFIG` when set and non-empty, otherwise `~/.config/tuse.lua`, otherwise `~/.config/tuse/init.lua`. `require("global")` SHALL resolve against that same global target and return its raw export. Unset, empty, or whitespace-only `TUSE_GLOBAL_CONFIG` values SHALL be treated as unset. A non-empty `TUSE_GLOBAL_CONFIG` SHALL expand `~/…` and resolve relative paths against the session working directory; if the resolved path is a directory, Tuse SHALL load `<dir>/init.lua`, otherwise that file directly. When `TUSE_GLOBAL_CONFIG` is set and non-empty and its target file is missing or unreadable, config loading (when global config is not opted out via `global = false`) and `require("global")` MUST fail with an error naming the missing path rather than silently falling back to `~/.config/tuse.lua`.

#### Scenario: Distro override takes precedence over default global config
- **WHEN** `TUSE_GLOBAL_CONFIG=~/distros/work/init.lua` is set, no `-c` flag is passed, and no `./tuse.lua` exists
- **THEN** Tuse loads `~/distros/work/init.lua` instead of `~/.config/tuse.lua`

#### Scenario: Project config and explicit flag win over TUSE_GLOBAL_CONFIG
- **WHEN** `TUSE_GLOBAL_CONFIG` is set and `./tuse.lua` exists in the project folder
- **THEN** `./tuse.lua` is evaluated first as the primary config, and `require("global")` inside `./tuse.lua` resolves to the `$TUSE_GLOBAL_CONFIG` target
- **WHEN** `-c custom.lua` is also passed
- **THEN** `custom.lua` is loaded as the primary local config

#### Scenario: Directory path and empty override
- **WHEN** `TUSE_GLOBAL_CONFIG=/opt/tuse-distro` points to a directory containing `init.lua`
- **THEN** Tuse loads `/opt/tuse-distro/init.lua`
- **WHEN** `TUSE_GLOBAL_CONFIG=""` or `"   "`
- **THEN** Tuse treats it as unset and falls back to `~/.config/tuse.lua` or `~/.config/tuse/init.lua`

#### Scenario: Missing override fails fast without fallback
- **WHEN** `TUSE_GLOBAL_CONFIG=/no/such/distro.lua` is set and `~/.config/tuse.lua` exists
- **THEN** loading without `global = false` fails with an error naming `/no/such/distro.lua` instead of loading `~/.config/tuse.lua`

### Requirement: Primary config evaluation and global config
`-c`/`--config PATH` SHALL behave as the primary local config, identically to `./tuse.lua`. When a primary config is present, Tuse SHALL evaluate it before auto-loading the global config or plugins so opt-outs prevent their execution. Global config SHALL return a function taking a single `opts` argument and returning a namespaced config table; returning a non-function MUST fail validation. When the primary config omits `global` or no primary config exists, Tuse SHALL invoke the global function with `nil`. When the primary config sets `global = false`, Tuse SHALL skip loading and executing the global config. When the primary config sets `global = <opts>`, Tuse SHALL invoke the global function with `<opts>`.

#### Scenario: Global opt-out skips execution
- **WHEN** `~/.config/tuse/init.lua` raises an error on load and `./tuse.lua` (or `-c custom.lua`) returns `{ global = false, my = { prompts = { p = "ok" } } }`
- **THEN** loading succeeds without executing `~/.config/tuse/init.lua`

#### Scenario: Passing options to global config
- **WHEN** `~/.config/tuse/init.lua` returns `function(opts) return { base = { prompts = { env = opts.env } } } end` and `./tuse.lua` returns `{ global = { env = "dev" } }`
- **THEN** the global function receives `{ env = "dev" }` and prompt `env` resolves to `"dev"`

#### Scenario: Default global call with nil
- **WHEN** `~/.config/tuse/init.lua` returns `function(opts) return { base = { prompts = { mode = opts == nil and "default" or "custom" } } } end` and no local `./tuse.lua` exists (or `./tuse.lua` omits `global`)
- **THEN** the global function is called with `nil` and prompt `mode` resolves to `"default"`

#### Scenario: Legacy non-function global config rejected
- **WHEN** `~/.config/tuse/init.lua` returns `{ base = { prompts = { a = "b" } } }` instead of a function
- **THEN** loading fails validation

### Requirement: Plugin auto-loading and selection
When `plugins` is omitted or set to `{}` (empty table), or when neither local nor global config exists, Tuse SHALL auto-load all installed plugins discovered under `~/.tuse/plugins`. Setting `plugins = { only = {} }` SHALL disable automatic plugin loading without preventing execution of explicit `require("plugin:<name>")` or `require("plugins")` calls. Setting `plugins = false` MUST fail validation. The `plugins` selector table SHALL accept optional keys `only` (list of plugin names to allowlist), `without` (list of plugin names to exclude), and `opts` (table keyed by plugin name supplying options for matching plugins). Simultaneous `only` and `without`, or referencing an uninstalled plugin name in `only`, `without`, or `opts`, MUST fail validation. When both global and local configs set `plugins`, the local `plugins` selector overrides the global one. Because the primary config is evaluated first, plugins excluded from auto-loading MUST NOT be executed unless explicitly required.

#### Scenario: Omitted or empty plugins auto-loads all
- **WHEN** plugins `alpha` and `beta` are installed under `~/.tuse/plugins` and `./tuse.lua` returns `{}` or `{ plugins = {} }`
- **THEN** both `alpha` and `beta` are auto-loaded and merged

#### Scenario: Disable auto-loading with empty allowlist
- **WHEN** a plugin `broken` is installed under `~/.tuse/plugins` and `./tuse.lua` returns `{ plugins = { only = {} } }`
- **THEN** `broken` is not executed and loading succeeds

#### Scenario: Allowlist, exclusion, and per-plugin options
- **WHEN** plugins `a`, `b`, and `c` are installed and a config sets `plugins = { only = { "a", "b" }, opts = { a = { flag = true } } }`
- **THEN** only `a` and `b` are auto-loaded, `c` is not executed, and `a` receives `{ flag = true }`
- **WHEN** a config sets `plugins = { without = { "b" } }`
- **THEN** `a` and `c` are auto-loaded and `b` is not executed

### Requirement: Duplicate contribution keys fail validation
When merging contributions across local namespaces, global namespaces, and plugins:
- Duplicate keys in `functions` (including `false` vs function override), `commands`, `prompts`, `open`, `providers`, `hooks`, and duplicate `models[].name` entries MUST fail validation with an error rather than applying first-wins or last-wins precedence. This SHALL include duplicates where a local namespace re-exports a value obtained from `require("plugin:<name>")` while that plugin is also auto-loaded.
- `ui` SHALL deep-merge by leaf field with layer priority `local > global > plugins`, while conflicting leaves within the same layer MUST fail validation.
- `systems` and `history.hide` SHALL concatenate in deterministic order `global` -> `plugins` (sorted by plugin name) -> `local` (sorted by namespace name).

#### Scenario: Duplicate function across namespaces
- **WHEN** a config returns `{ ns1 = { functions = { helper = f1 } }, ns2 = { functions = { helper = f2 } } }`
- **THEN** loading fails with a duplicate-key error naming `helper`

#### Scenario: Local re-export collides with auto-loaded plugin
- **WHEN** plugin `cliproxyapi` defines `functions.fn`, auto-loading is enabled, and `./tuse.lua` does `local cl = require("plugin:cliproxyapi"); return { local_ns = { functions = { fn = cl.fn } } }`
- **THEN** loading fails with a duplicate-key error for `functions.fn`
- **WHEN** `./tuse.lua` adds `plugins = { only = {} }` (or `plugins = { without = { "cliproxyapi" } }`)
- **THEN** loading succeeds with `local_ns.functions.fn`

#### Scenario: Duplicate command or prompt
- **WHEN** two merged contributions define the same command name in `commands` or the same prompt name in `prompts`
- **THEN** loading fails with a duplicate-key error naming that command or prompt

### Requirement: Explicit plugin module loading
`require("plugin:<name>")` and `require("plugins")` SHALL be available in trusted config code (`trusted_env`) only. A plugin entrypoint MAY return either a flat contribution table or `function(opts) -> table`. `require("plugin:<name>")` SHALL resolve and load only the installed plugin `<name>` from `~/.tuse/plugins` and return its raw entrypoint export, including returning a function without invoking it. `require("plugins")` SHALL load all installed plugins under `~/.tuse/plugins` (invoking function entrypoints with `nil`) and merge them into a flat table exposing `.functions`, `.commands`, `.prompts`, and other merged sections, independent of the primary config's returned `plugins` selector and `opts`, and SHALL fail on duplicate keys. `/reload` SHALL re-scan `~/.tuse/plugins/`, clear trusted global/plugin caches, and restore prior state on failure. `config.get` and `/config` SHALL include active loaded `plugins` (`[{ name, version }]`).

#### Scenario: Explicit single plugin returns raw export
- **WHEN** installed plugin `demo` has an entrypoint returning a function `function(opts) return { functions = { hi = f } } end` and a config runs `local m = require("plugin:demo")`
- **THEN** `type(m) == "function"` (it is not automatically invoked) and no other installed plugin is loaded

#### Scenario: Explicit merged plugins table
- **WHEN** plugins `a` (defining `functions.fa`) and `b` (defining `prompts.pb`) are installed and a config runs `local p = require("plugins")` before returning `{ plugins = { only = {} }, my = { functions = { fa = p.functions.fa } } }`
- **THEN** `p.functions.fa` and `p.prompts.pb` are accessible regardless of the returned `plugins = { only = {} }` selector, and only `my` is merged into the final config

#### Scenario: Duplicate inside require plugins
- **WHEN** two installed plugins both define `commands.dup` and a config calls `require("plugins")`
- **THEN** `require("plugins")` raises a duplicate-key error for `commands.dup`

### Requirement: Open handlers under settings
`open` SHALL be read from namespace contribution tables (`<namespace>.open`, mapping `key -> function(path)`). A key is an extension without a dot (`md`, case-insensitive), a mask with `*` or `?` matched against the file name (`*.test.lua`, `.env*`, case-insensitive), the catch-all `*`, `@dir` for folders, or `@url` for `http://`, `https://`, and `mailto:` links, which get the URL. Any other key MUST fail validation, and keys MUST NOT contain `/`, `\`, whitespace, or control characters. A file SHALL use the handler of its extension, else the most specific mask (the one with the most literal characters, ties in alphabetical order), else `*`, else the OS opener. A folder SHALL use `@dir`, else `*`, else the OS opener. Legacy `settings.open` MUST fail validation.

#### Scenario: Old key
- **WHEN** a config returns `{ my = { settings = { open = { md = f } } } }` or `{ settings = { open = { md = f } } }`
- **THEN** loading fails validation

#### Scenario: Namespaced open handler
- **WHEN** a config returns `{ my = { open = { md = f } } }` and a `.md` path is clicked
- **THEN** `f` is called with the path

#### Scenario: Mask beats catch-all
- **WHEN** a namespace sets `open = { ["*.test.lua"] = a, ["*"] = b }` and `x.test.lua` is clicked
- **THEN** `a` is called, and clicking `y.txt` calls `b`

#### Scenario: Extension beats mask
- **WHEN** a namespace sets `open = { lua = a, ["*.test.lua"] = b }` and `x.test.lua` is clicked
- **THEN** `a` is called

#### Scenario: Folder
- **WHEN** a namespace sets `open = { ["@dir"] = f }` and an existing folder is clicked
- **THEN** `f` is called with the folder path

#### Scenario: URL
- **WHEN** a namespace sets `open = { ["@url"] = f }` and `https://example.com` is clicked
- **THEN** `f` is called with `https://example.com`, and a `javascript:` link is still refused

#### Scenario: Invalid key
- **WHEN** a namespace sets `open = { ["@file"] = f }` or `open = { [".md"] = f }`
- **THEN** loading fails with an error naming the key

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
- **THEN** loading fails with ``history.hide[1]: unknown history role `hook` `` and MUST NOT repeat `history.hide:` twice

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
`/config` and `config.get` SHALL show every resolved config field, including `providers` (all active saved and Lua providers with only `name`, `type`, and `base` — never keys, header names or values, or extra metadata), `plugins` (active loaded plugins `[{ name, version }]`), `models` (`name`, `display`, `window`, `efforts`, `prices`), `open`, `ui` (`lines`, `input`, `space`, `theme`), `history.hide`, and prompt kinds and args. Secrets and Lua bodies MUST stay hidden.

#### Scenario: Active providers shown without secrets
- **WHEN** a session has both a saved provider from `tuse connect` and a Lua `providers` entry with API keys and custom headers
- **THEN** `config.get` exposes `providers` as `[{ "name": ..., "type": ..., "base": ... }]` for all active providers with no keys, headers, or extra fields, and `/config` renders a `Providers` section with only `name`, `type`, and `base`

#### Scenario: Models shown
- **WHEN** the config has `models` (including custom `efforts` or `efforts = {}`) and the user opens `/config`
- **THEN** a `Models` section lists each model with its metadata including `efforts` (`efforts=none` for `efforts = {}`)

#### Scenario: No field forgotten
- **WHEN** a field is added to `Config`, `Ui`, or `History` but not to `/config`
- **THEN** `cargo test` fails

### Requirement: No compaction settings
`compact_at` MUST NOT be accepted in `models` entries or anywhere in the config.

#### Scenario: Old key
- **WHEN** a config sets `models[1].compact_at = 100000`
- **THEN** loading fails with an unknown-key error for `models[1].compact_at`

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

### Requirement: Reload one active namespace
A scoped reload SHALL target an exact namespace name in the current session's active config. For an active name, it SHALL evaluate the normal config sources in one load pass, including global config and plugins when selected by the usual loader rules. From that pass it SHALL apply only the local and global contributions with the requested name. It SHALL retain all other active contributions, plugin metadata, and saved provider settings. It SHALL apply the existing merge order, precedence, and conflict checks to the resulting config. Scoped reload limits what is applied, not which Lua code executes.

#### Scenario: Refresh a namespace contributed by both layers
- **WHEN** the active local and global configs both contribute to `work`, their files change `work` and other namespaces, and a client reloads `work`
- **THEN** the normal config sources are evaluated in one pass, both new `work` contributions take effect using the existing merge rules, and unrelated namespace and plugin contributions remain unchanged

#### Scenario: Unselected changes are not applied
- **WHEN** a fresh contribution outside the selected namespace would clash with another contribution if applied, but config evaluation succeeds and the selected contribution does not clash with the active config
- **THEN** the scoped reload succeeds without applying the unrelated change

### Requirement: Scoped reload failures preserve active configuration
A scoped reload SHALL fail if the target name is not active, if any previously active local or global contribution for that name is missing from the newly evaluated config, or if evaluation, validation of the selected contributions, or merging the candidate fails. It SHALL leave the previous active config, including its Lua callbacks, in place and preserve the existing failed-reload module-cache and preload restoration semantics. Adding or removing a namespace name SHALL require a full reload.

#### Scenario: Namespace was added on disk
- **WHEN** a client requests a namespace that exists on disk but is not active in the current session
- **THEN** the request fails and the new namespace does not become active

#### Scenario: Namespace was removed from one layer
- **WHEN** an active namespace has both local and global contributions and the fresh config no longer returns its local contribution
- **THEN** scoped reload fails, both active contributions remain, and a full reload is required to apply the removal

#### Scenario: Selected namespace conflicts with an unchanged namespace
- **WHEN** the new selected contribution defines a command already defined by another active namespace
- **THEN** scoped reload fails and the previous config and callbacks remain active
### Requirement: Reload a namespace from Lua
The Lua host function `reload(ns)` SHALL accept an optional non-empty namespace name and use the same scoped reload as JSON-RPC. Omitting the argument or passing `nil` SHALL perform a full reload. Other argument types and an empty string SHALL raise an error without reloading. Within a hook, the requested reload SHALL run after that hook finishes, with reload failures reported to the session. Outside hooks, reload failures SHALL raise a Lua error.

#### Scenario: Agent reloads one namespace
- **WHEN** the agent calls `reload("work")` after changes to `work` and another active namespace
- **THEN** only `work` is refreshed and the other namespace stays unchanged

#### Scenario: Lua reload without a namespace
- **WHEN** Lua calls `reload()` or `reload(nil)`
- **THEN** it reloads the entire config as before

#### Scenario: Hook requests a scoped reload
- **WHEN** a hook calls `require("tuse").reload("work")`
- **THEN** the reload runs after that hook finishes and applies only `work`

#### Scenario: Invalid namespace input from Lua
- **WHEN** Lua calls `reload(123)` or `reload("")`
- **THEN** the call raises an error without reloading
