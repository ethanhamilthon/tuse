## MODIFIED Requirements

### Requirement: Config layout
Config files (`tuse.lua`, `-c`/`--config PATH`, and the table returned by global `init.lua`) SHALL return a table of named namespaces mapping each namespace name to a contribution table, replacing the legacy flat top-level config layout with no backward compatibility. Each namespace contribution table SHALL accept the keys `ui`, `history`, `open`, `providers`, `models`, `systems`, `prompts`, `functions`, `hooks`, and `commands`. `ui` SHALL accept `lines`, `input`, `space`, and `theme`. `history` SHALL accept `hide`. Top-level `global` and `plugins` SHALL be reserved control keys rather than namespace contribution tables. Legacy flat top-level config tables (including `settings`, `theme`, or bare contribution keys without a namespace) and unknown keys MUST fail validation with their path.

#### Scenario: Namespaced config loads
- **WHEN** a config returns `{ herdr = { functions = { ping = f }, prompts = { hi = "hello" } }, ui_cfg = { ui = { lines = { diff = 0 } }, history = { hide = { "usage" } } } }`
- **THEN** loading succeeds and merges the contributions into the active config

#### Scenario: Legacy flat config rejected
- **WHEN** a config returns `{ ui = { lines = { diff = 0 } } }` or `{ commands = { c = { run = f } } }` or `{ settings = { ui = {} } }` or `{ theme = { accent = "red" } }`
- **THEN** loading fails validation without backward compatibility

#### Scenario: Unknown key inside a namespace
- **WHEN** a config returns `{ my = { unknown_key = 1 } }`
- **THEN** loading fails with an error naming `my.unknown_key`

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

## ADDED Requirements

### Requirement: Primary config evaluation and global config
`-c`/`--config PATH` SHALL behave as the primary local config, identically to `./tuse.lua`. When a primary config is present, Tuse SHALL evaluate it before auto-loading the global config or plugins so opt-outs prevent their execution. The global config target SHALL resolve via `$TUSE_GLOBAL_CONFIG` (if set and non-empty, expanding `~/…`, loading `<dir>/init.lua` for directories, and failing fast if missing), else `~/.config/tuse.lua`, else `~/.config/tuse/init.lua`. `require("global")` SHALL resolve that same global target and return its raw function. Global config SHALL return a function taking a single `opts` argument and returning a namespaced config table; returning a non-function MUST fail validation. When the primary config omits `global` or no primary config exists, Tuse SHALL invoke the global function with `nil`. When the primary config sets `global = false`, Tuse SHALL skip loading and executing the global config. When the primary config sets `global = <opts>`, Tuse SHALL invoke the global function with `<opts>`.

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
