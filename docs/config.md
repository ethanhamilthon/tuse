# Config

Tuse configuration is a Lua 5.4 file that returns a table (or `nil`, treated as `{}`). Defaults and the built-in system prompt live in the binary; `tuse update` never modifies user config files. For a step-by-step walkthrough, see [setup.md](setup.md).

Configuration files execute as Lua code with the user's permissions. Only load project configs and modules you trust.

## Config Lookup

Tuse selects the config file in this order:

1. `--config PATH` (`-c PATH`), resolved relative to the current working directory (`~/` expands to the home directory). If the explicit path cannot be read, startup fails rather than falling back.
2. `./tuse.lua` in the session's working directory, if it exists as a file.
3. Global config: `~/.config/tuse.lua`, else `~/.config/tuse/init.lua` (first existing file).
4. None: Tuse runs on built-in defaults (`session().configpath` is `nil`, `config.get` reports `path: ""`).

When resuming a saved session without `--config`, Tuse switches to the session's saved working directory (if that directory still exists) before looking up `./tuse.lua`.

`tuse init` creates `./tuse.lua` (failing if it already exists) with a template that extends the global config via `pcall(require, "global")`.

```lua
local t = require("tuse")              -- host functions and t.session()
local ok, g = pcall(require, "global") -- ~/.config/tuse.lua or ~/.config/tuse/init.lua
g = ok and g or {}
local shared = require("shared")       -- shared.lua or shared/init.lua next to this file
local interactive = t.session().mode == "tui"

g.settings = g.settings or {}
g.settings.retries = 3
g.systems = g.systems or {}
g.systems[#g.systems + 1] = interactive and "Answer interactively." or "Keep responses concise."
g.prompts = g.prompts or {}
g.prompts.review = shared.review
g.open = g.open or {}
g.open.md = function(path)
  t.sh('open -a Typora "$P"', { env = { P = path } })
end
return g
```

## Execution Environment

Config files, plugins, trusted `require`d modules, and callbacks defined in them run in Tuse's trusted Lua environment (see [lua.md](lua.md)):

- Built-in host functions are **not** globals; import the unmodified originals with `require("tuse")`.
- Configured `functions` are agent-only and are never visible in trusted code (share code via `local` functions or `require`d helper modules).
- `os.exit` and the `debug` library are unavailable (`nil`). `os.getenv("THIS_PATH")` returns the calling Lua file's path. Other environment variables come straight from the process environment.
- During top-level config and plugin evaluation (both on startup and on `/reload`), `require("tuse").session()` returns a pre-session snapshot containing `{ mode, configpath, cwd, home }` with `id` and all session-only fields set to `nil`.
- **Startup vs reload**: on initial startup, no session exists yet, so session-dependent host functions (`write`, `append`, `delete`, `edit`, `grep`, `spawn`, `task_*`, `show`, `pick`, `ask`, `llm`, `reload`, `set_title`, `docs`) fail if called at top level (while `read`, `sh`, `sleep`, `json`, and `inspect` work, with `read` using `cwd` and default `max_image_bytes`). On reload, although `t.session()` still returns the pre-session snapshot, the live session already exists in the process; top-level side effects (file writes, shell commands, UI calls, global mutations) execute immediately and are **not** rolled back if config validation fails. Keep top-level config evaluation pure and put runtime actions in callbacks.

## Modules and `require`

`package.path` and `package.cpath` are empty. Tuse provides a custom `require(name)`:

1. If `package.loaded[name]` is truthy, returns that single cached value immediately (this is how `require("tuse")` resolves).
2. If `package.preload[name]` is a function, calls it with `(name, ":preload:")`, caches the result (storing `true` if it returned `nil`), and returns `(value, ":preload:")` on the initial load (subsequent cached calls return only `value`).
3. Otherwise resolves files relative to the calling Lua file's directory (or the session's `cwd` for inline `lua` tool calls):

| Call | Loads |
| --- | --- |
| `require("tuse")` | Host functions: `session`, `sh`, `sleep`, `spawn`, `read`, `write`, `append`, `delete`, `edit`, `grep`, `docs`, `set_title`, `task_get`, `task_list`, `task_stop`, `show`, `pick`, `ask`, `reload`, `llm`, `json`, `inspect`. |
| `require("global")` | Global config: `~/.config/tuse.lua`, else `~/.config/tuse/init.lua`. |
| `require("name")` | `name.lua`, else `name/init.lua`, relative to the calling file (`a.b` maps to `a/b`). |
| `require("./name")` | `./name.lua`, else `./name/init.lua`, relative to the calling file (`../name` works too). |
| `require("~/path")` | `~/path.lua`, else `~/path/init.lua`. Absolute `/path` works the same. |
| `require("~/path/file.lua")` | That exact file. A relative `dir/file.lua` is relative to the calling file. |

File modules are cached by their canonical path in the calling environment's isolated `package.loaded` (trusted config/plugin modules and agent `lua(code)` modules do not share module caches), so identically named files in different folders do not collide and different relative paths to the same file share one module instance. Each file module runs in the calling environment (trusted or agent) with arguments `(name, canonical_path)`; returning `nil` caches `true`. An initial file load returns two values `(value, canonical_path)`; subsequent cached loads return only `value`. Cyclic `require` chains raise `cyclic require: '<path>'`. Missing files raise an error listing every candidate path checked.

On reload, Tuse forgets all trusted file-backed module cache entries before re-evaluating the config and restores them if reload fails.

## Top-Level Keys

Every key is optional. Unknown keys are rejected. Empty tables `{}` are accepted for list fields.

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `settings` | table | defaults below | Runtime and UI settings. |
| `theme` | table | built-in dark palette | 10 base color tokens; see [theme.md](theme.md). |
| `models` | list of tables | `{}` | Model metadata keyed by `name = "provider/model"`. |
| `systems` | list of strings | `{}` | Extra instructions appended to the built-in system prompt. |
| `prompts` | map (`name -> string`) | `{}` | Named prompts for the `#` picker ([prompts.md](prompts.md)) and JSON-RPC `turn.submit`. |
| `functions` | map (`name -> function \| false`) | `{}` | Agent-only Lua functions, built-in overrides, and built-in/plugin disables (`false`); see [functions.md](functions.md). |
| `hooks` | map (`name -> table`) | `{}` | Named event hooks; see [hooks.md](hooks.md). |
| `commands` | map (`name -> function`) | `{}` | Custom slash commands (`/name` in TUI, `commands.run` in JSON-RPC); see [lua.md](lua.md). |
| `open` | map (`ext -> function`) | `{}` | Local file opener callbacks for clicked paths in TUI (`open.<ext>`); see [lua.md](lua.md). |
| `plugins` | list of specs | `{}` | Plugins merged under this config (forbidden inside plugins); see [plugins.md](plugins.md). |

### `settings`

| Key | Type & range | Default | Meaning |
| --- | --- | --- | --- |
| `settings.compact_at` | integer `>= 0` (or unset) | unset (auto) | Context threshold in tokens that triggers compaction. Unset uses `floor(window * 6 / 10)` when the active model's `window` is positive, else disables compaction. `0` disables compaction globally unless overridden by a per-model `compact_at` (a per-model `compact_at` always takes precedence, even over a global `0`). |
| `settings.max_image_bytes` | integer `>= 1` | `5242880` (5 MiB) | Maximum image file size `read()` will encode and send to the model. |
| `settings.retries` | integer `0..=100` | `5` | Maximum retries for HTTP 429, HTTP 5xx, and network errors on agent turns and `llm()`. Validation accepts `0..=100`, while actual retries are capped at 5 (delays: 2, 5, 10, 20, 30 s). |
| `settings.ui` | table | defaults below | TUI rendering, truncation, and layout controls. |

### `settings.ui`

See [theme.md](theme.md) for visual details.

| Key | Type & range | Default | Meaning |
| --- | --- | --- | --- |
| `fold_strings` | boolean | `true` | Folds multiline Lua long strings (`[[...]]`) in chat tool calls to one line (`/trace` always shows full code). |
| `tool_line` | integer `>= -1` | `-1` | Max tool call code lines shown in chat (`0` hides, `-1` unlimited). |
| `diff_line` | integer `>= -1` | `12` | Max diff lines shown per file in chat (`0` hides the entire diff block, `-1` all stored diff lines). |
| `output_line` | integer `>= -1` | `1` | Max tool output lines shown in chat (`0` hides, `-1` unlimited). |
| `reasoning_line` | integer `>= -1` | `-1` | Max reasoning lines shown in chat (`0` hides, `-1` unlimited). |
| `scroll_step` | integer `1..=65535` | `3` | Lines scrolled per mouse wheel tick or arrow scroll step in panels. |
| `pad_x` | integer `0..=65535` | `0` | Horizontal padding (columns) on each side of the TUI. |
| `message_gap` | integer `0..=65535` | `1` | Blank lines between chat entries. |
| `list_max_height` | integer `>= 1` | `16` | Maximum visible rows for pickers and overlay panels. |
| `input_max_height` | integer `-1` or `>= 1` | `-1` | Maximum input box height in rows (`-1` unlimited; `0` is rejected). |

### `theme`

All 10 tokens default to the built-in dark palette and override only the keys specified: `bg` (`#121212`), `surface` (`#1E1E1E`), `fg` (`#DEDEDE`), `dim` (`#BDBDBD`), `muted` (`#929292`), `border` (`#333333`), `accent` (`#BDDEAB`), `ok` (`#94B979`), `err` (`#E27373`), `info` (`#97BEDC`).

Valid color values (case-insensitive during validation, surrounding whitespace ignored): `#rgb`, `#rrggbb`, ANSI 256 index `"0"`–`"255"`, named colors (`black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`, `grey`/`gray`, `dark_grey`/`dark_gray`, `bright_red`, `bright_green`, `bright_yellow`, `bright_blue`, `bright_magenta`, `bright_cyan`, `bright_white`), `"accent"` (inherits the theme's `accent` token; write lowercase `"accent"` because TUI rendering matches this alias case-sensitively), or `"none"` (terminal default).

### `models`

Each entry in `models` is a table (unknown keys rejected):

| Key | Type | Default | Meaning |
| --- | --- | --- | --- |
| `name` | string (required) | — | Full model identifier `"provider/model"` (non-empty provider and model parts, no whitespace). |
| `display` | string | `""` | Display label for TUI status bar, turn summaries, `/models`, and `/session` (falls back to `name` when empty). |
| `window` | non-negative integer | `0` | Context window size in tokens (`0` means unknown). |
| `compact_at` | non-negative integer | unset | Per-model compaction threshold in tokens; overrides `settings.compact_at` (`0` disables compaction for this model). |
| `prices` | table | unset | USD per 1,000,000 tokens: `input`, `output`, `cache_read`, `cache_write` (each defaults to `0.0` when `prices` is present; must be a finite non-negative number). |

If multiple entries share the same `name`, the **last** entry in the merged list wins.

```lua
-- init.lua (or tuse.lua)
return { models = require("models") }
```

```lua
-- models.lua
return {
  {
    name = "my-provider/my-model",
    display = "My Model",
    window = 200000,
    compact_at = 120000,
    prices = { input = 2, output = 10, cache_read = 0.2, cache_write = 2.5 },
  },
}
```

### `commands` and `open`

- `commands`: maps a custom slash command name to a Lua function `function() ... end`. Names must be non-empty, must not start with `/`, must not contain whitespace or control characters, and cannot override any built-in command (`config`, `new`, `resume`, `models`, `tasks`, `session`, `trace`, `reload`, `quit`).
- `open`: maps a file extension (without a leading dot, e.g. `md`) to `function(path) ... end`. Keys must be non-empty, must not start with `.`, and must not contain `/`, `\`, whitespace, or control characters. Matching is case-insensitive and applies only to clicked local file paths in the TUI (URLs use the OS opener; `tuse json` has no clicks).

## Merge Rules

When `plugins` are declared, plugins load and validate in list order and the user config merges on top:

- `systems` and `models`: concatenated (plugins in list order, then user config). For `models`, the last definition for a given `name` wins.
- `prompts`, `functions`, `hooks`, `commands`, `open`: merged by key. If two plugins define the same key in one of these maps, loading fails. The user config silently overrides any plugin key.
- `settings` and `theme`: merged deeply table-by-table; later plugins override earlier plugins on overlapping keys, and user values win over all plugins.
- `plugins`: allowed only in the user config; plugins cannot declare `plugins`.

See [plugins.md](plugins.md) for plugin entries, options, and checkout paths.

## Validation

Tuse validates the user config, every enabled plugin, and the merged result strictly on startup, reload, and session switch (`/new`, `/resume`, `session.new`, `session.resume`).

- Returning a non-table value (other than `nil`) fails with `<source>: the config must return a table`.
- Unknown keys, wrong types, out-of-range integers, negative or non-finite prices, invalid colors, malformed `provider/model` names, non-list tables for list fields, empty or unknown hook events, invalid or reserved function/command/`open` names (including Lua keywords and base/stdlib globals in `functions`), non-function/`false` values in `functions` (including `json = function(...)`), `false` on an unknown function name, and missing or non-function hook `run`/command/`open` callbacks all fail before any change takes effect.
- Diagnostics report `<source>: <path>: <message>` with dot-separated keys and 1-based list indices, e.g. `tuse.lua: models[2].prices.input: expected a finite non-negative number`.

## Providers and SQLite State

Providers (`name`, `base_url`, `api_key`) and the default `model`/`effort` for new sessions live in SQLite (`~/.tuse/tuse.db`), not in Lua config files. Manage providers with `tuse connect`, `tuse providers`, and `tuse disconnect`; select the model and effort via `/models` or JSON-RPC `model.set`.

When loading or reloading config, Tuse reads providers from SQLite and expands API keys written as `$VAR` or `${VAR}` (where `VAR` contains only ASCII letters, digits, and `_`) from the process environment (unset variables expand to `""`). Running sessions check provider availability against this loaded snapshot, so external `tuse connect` / `tuse disconnect` changes take effect only after a reload or reopening the session. Resumed sessions keep their own saved model and effort selection.

Compaction thresholds resolve as: per-model `compact_at` (which overrides `settings.compact_at` even when `settings.compact_at = 0`), else `settings.compact_at`, else `floor(window * 6 / 10)` when `window > 0`, else `0` (disabled). Compaction only affects messages sent to the model, not durable session records.

## Inspecting Config (`/config`, `config.get`, `tuse models`)

- **`/config`** (TUI): opens a scrollable styled panel listing `Config file` (clickable path or `(none, defaults)`), `Settings`, `Theme` (all 10 tokens with color swatches), `System prompts` (numbered, first non-empty line), `Prompts` (`#name`, first non-empty line), `Functions` (`<name>`, `<name>  [override]`, or `<name>  [disabled]`), `Hooks` (name and events), `Commands` (`/name`), `Open` (`.ext`), and `Plugins` (name, optional `@ref`, 7-char commit or `local`, and clickable folder path). Provider credentials and Lua function bodies are never shown.
- **`config.get`** (JSON-RPC): returns the resolved config JSON (`settings`, `theme`, `models`, `systems`, `prompts`, `functions`, `hooks`, `commands`, `open`, `path`, `plugins`), omitting provider secrets and Lua bodies.
- **`tuse models [--config PATH]`** (CLI): evaluates the effective config (in `tui` mode) and prints the saved default model/effort plus configured model metadata.

## Reloading (`/reload`, `config.reload`, `reload()`)

Reloads are serialized so only one reload runs at a time. Each reload follows this sequence:

1. Re-resolves the config file path from the original `--config` argument (if any) and session `cwd`.
2. Clears cached trusted file modules, evaluates the user config under pre-session `session()` semantics, validates user keys and `plugins` entries (including URL and `ref` conflict checks), synchronizes enabled Git plugins (cloning missing checkouts and updating moving refs while respecting immutable commit pins and local-work safety; see [plugins.md](plugins.md)), loads and validates all plugins and the merged result, and reloads providers from SQLite.
3. **On failure**: restores the previous trusted module cache and leaves the previous config, helpers, hooks, commands, and `open` handlers active (already-synced sibling Git repositories on disk are not reverted so concurrent Tuse sessions are unaffected, and arbitrary top-level Lua side effects cannot be undone). TUI shows `Reload failed: <reason>`; JSON-RPC returns `-32000`; Lua `reload()` raises an error (or emits an `error` event when deferred from a hook).
4. **On success**:
   - Cancels pending interactive UI requests from hooks and denies new hook UI requests during the transition.
   - Runs `close` (`reason = "reload"`) on the old hooks and waits for all running hook coroutines to finish.
   - Commits the new config into the VM: updates live-gated agent functions, overrides, and disables (affecting saved aliases immediately), restores default built-ins when overrides or `false` disables are removed, and unbinds removed custom functions from the agent global environment only if the global still refers to the managed proxy (agent-rebound globals and other agent globals stay intact).
   - Swaps the active config, clears cached remote provider models (discarding in-flight model fetches from the prior config generation), and recalculates session usage costs if `models` changed.
   - Re-enables hooks, fires `start` (`reason = "reload"`, `is_new = false`, `restored = false`) on the new hooks, and emits a `reloaded` event.

In TUI, `/reload` restyles the palette, rebuilds the chat view from the session (preserving live `show` blocks and answered UI entries, and immediately applying updated `theme` and `settings.ui` to existing chat entries), and flashes `Config reloaded.`. A reload triggered from Lua `reload()` emits `reloaded`, which restyles the palette and invalidates cached chat entries and open panels so updated `theme` and `settings.ui` rules re-render immediately at the same terminal width. Reload never changes the active session's model or effort, never stops running background tasks, and preserves live UI markdown blocks, UI entries, and non-hook UI prompts.
