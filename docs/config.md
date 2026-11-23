# Config

Tuse runs `--config PATH`, else `./tuse.lua`, else the global config: `~/.config/tuse.lua`, else
`~/.config/tuse/init.lua`. The file returns a table. Defaults and the built-in system prompt live
in the binary. `tuse update` never touches your config.

Configuration files execute as Lua code with your user permissions. Only load project configs and
modules you trust. Host functions are globals only in the agent's `lua` tool. In config files,
Lua modules, and callbacks defined in them, use `require("tuse")`. Functions that need a live
session, such as `spawn` or `ask`, belong in callbacks.

```lua
local t = require("tuse")              -- host functions and t.session()
local ok, g = pcall(require, "global") -- global config (~/.config/tuse.lua or ~/.config/tuse/init.lua)
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

## Modules and require

- `require` loads a Lua module relative to the file that calls it (or the session's working
  directory for inline code in the `lua` tool) and returns its value:

  | Call | Loads |
  | --- | --- |
  | `require("tuse")` | Host functions (`session`, `sh`, `sleep`, `spawn`, `read`, `write`, `append`, `delete`, `edit`, `grep`, `docs`, `set_title`, `task_*`, `show`, `pick`, `ask`, `reload`, `llm`, `json`, `inspect`). |
  | `require("name")` | `name.lua`, else `name/init.lua`, relative to the calling file (`a.b` is `a/b`). |
  | `require("./name")` | `./name.lua`, else `./name/init.lua`, relative to the calling file (`../name` works too). |
  | `require("global")` | The global config: `~/.config/tuse.lua`, else `~/.config/tuse/init.lua`. |
  | `require("~/path")` | `~/path.lua`, else `~/path/init.lua`. An absolute `/path` works the same. |
  | `require("~/path/file.lua")` | That file. A relative `dir/file.lua` is relative to the calling file. |

  File modules are cached by canonical path, so files in different folders do not collide and
  different relative paths to the same file share one module. `/reload` forgets file modules and
  runs everything again (restoring the previous modules if reload fails).

## Keys

| Key | Meaning |
| --- | --- |
| `settings.compact_at` | Context size (tokens) that triggers compaction. Unset: 60% of the model's `window` when known; `0` disables it. A per-model `compact_at` wins. |
| `settings.max_image_bytes` | Largest image `read` sends to the model. Default 5242880. |
| `settings.retries` | Retries for 429, 5xx and network errors (0-100). Default 5. |
| `settings.ui` | UI behavior; see [theme](theme.md). |
| `theme` | The 10 base colors; see [theme](theme.md). |
| `models` | Model metadata: a list of tables keyed by `name = "provider/model"`; see below. |
| `systems` | List of strings appended to the built-in system prompt. |
| `prompts` | Map of names to strings for [the `#` picker](prompts.md). |
| `functions` | Map of named Lua helpers with `run`, `description`, optional `args` and `show_agent`. See [functions](functions.md). |
| `hooks` | Map of named hooks `{events = {...}, run = function(e) ... end}`. See [hooks](hooks.md). |
| `commands` | Map of custom slash command names to Lua functions (`commands = { hello = function() ... end }`). Built-in commands cannot be overridden. |
| `open` | Map of file extensions (without the dot, e.g. `md = function(path) ... end`) to Lua callbacks for clicked local file paths (TUI-only: `tuse json` has no clicks). URLs are handled separately by the OS default opener. |
| `plugins` | List of plugins from GitHub or local folders, merged under this config. See [plugins](plugins.md). |

## Validation

Tuse validates both the user config and each plugin strictly before applying changes on startup
and `/reload`. Unknown keys, wrong types, out-of-range settings, invalid theme colors, and
malformed `hooks`, `commands`, or `open` handlers fail with diagnostics that include the source
file or plugin and the full key path (lists count from 1, like Lua), for example
`tuse.lua: models[2].prices.input: expected a finite non-negative number`. A list field accepts
`{}` as an empty list. If `/reload` fails, the previous working config, modules, and callbacks remain
active.

## Providers, models, and history

Providers and the last model/effort selection live in SQLite (`~/.tuse/tuse.db`). Use
`tuse connect --base URL --key '$ENV' --name NAME`, `tuse providers`, and `tuse disconnect NAME`
to manage providers. Pick the model and effort in `/models`; resumed sessions keep their own
selection.

Model metadata lives in the `models` list in your Lua config. The old 0.9.0 SQLite metadata is
no longer read. Put the list inline or keep it in `models.lua` next to your config:

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

Only `name` is required. `window` and `compact_at` are in tokens, and `prices` are USD per
million tokens. Run `tuse models` to inspect the effective config or `/reload` to apply changes
to a running session.

Compaction only changes what the model receives, not the saved log. It keeps recent context
intact, shortens older tool outputs, and keeps only the user/assistant text further back.
