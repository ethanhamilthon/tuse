# Config

Tuse runs `--config PATH`, else `./tuse.lua`, else the global config: `$TUSE_GLOBAL_CONFIG`,
else `~/.config/tuse.lua`, else `~/.config/tuse/init.lua` (the older `~/.config/tuse/main.lua`
still works). The file returns a table. The defaults and system prompt are
built into the binary. `tuse update` never touches your config.

Configuration files execute as Lua code with your user permissions. Only load project
configs and included files you trust. Hooks and helpers run in the session's Lua VM.

```lua
local t = require("tuse")              -- host functions and t.env
local shared = require("shared")       -- ./shared.lua or ./shared/init.lua
local interactive = t.env.TUSE_MODE == "tui"

return {
  include = { t.env.TUSE_GLOBAL_CONFIG },
  model = "c/claude-opus-5-5",
  effort = "medium",
  settings = { retries = 3 },
  systems = { interactive and "Answer interactively." or "Keep responses concise." },
  prompts = { review = shared.review },
}
```

## Include and require

- `include` is a list of Lua config files to merge. Paths can be absolute, start with `~/`, or
  be relative to the file that lists them. Files may include other config files. A cycle
  is an error. Use the example above in a project config, not in the global file itself.
- Files merge in list order, then the including file wins conflicts. `systems` and `models`
  append; the last definition of a model name wins. `settings`, `settings.ui` and `theme`
  merge key by key. Named `prompts`, `functions` and `hooks` replace whole entries on conflict.
  Without a global config file `TUSE_GLOBAL_CONFIG` is unset and the example's `include` is empty.
- `require` loads a Lua module and returns its value (it does not merge config tables; use
  `include` for that):

  | Call | Loads |
  | --- | --- |
  | `require("tuse")` | The host functions (`sh`, `sleep`, `spawn`, `read`, `write`, `edit`, `grep`, `docs`, `set_title`, `task_*`, `json`, `inspect`) and `env`. |
  | `require("name")` | `./name.lua`, else `./name/init.lua`, from the session's working directory (`a.b` is `a/b`). |
  | `require("global")` | The global config: `$TUSE_GLOBAL_CONFIG`, else `~/.config/tuse.lua`, else `~/.config/tuse/init.lua`. |
  | `require("~/path")` | `~/path.lua`, else `~/path/init.lua`. An absolute `/path` works the same. |
  | `require("~/path/file.lua")` | That file. A relative `dir/file.lua` is from the working directory. |

  Lua caches modules; `/reload` forgets the ones loaded from files and runs everything again.
- `require("tuse").env.NAME` is `os.getenv(NAME)`, with Tuse's variables (`TUSE_MODE`,
  `TUSE_CWD`, `TUSE_GLOBAL_CONFIG`, ...; see [env](env.md)). `TUSE_MODE` is `tui` or `json`: it
  describes the front end, it does not select one. It is no longer a Lua global. Per-entry
  `modes` are not supported.

## Keys

| Key | Meaning |
| --- | --- |
| `include` | List of Lua config file paths, merged before this file. |
| `model` | Default model, `name` or `provider/name`. A saved `/models` pick wins. |
| `provider` | Provider for a model with no prefix (default: first saved provider). |
| `effort` | `default` (omit `reasoning_effort`), `low`, `medium`, `high`, `xhigh`, `max`. |
| `models` | List of `{name, display, window, compact_at, prices}`. Prices per million tokens: `["in"]`, `out`, `cache`, `cache_write`. |
| `settings.compact_at` | Context size that triggers compaction. Unset: 60% of the model's `window`, when known; `0` disables it. The model override wins. |
| `settings.max_image_bytes` | Largest image `read` sends to the model. Default 5242880. |
| `settings.retries` | Retries for 429, 5xx and network errors, after 2, 5, 10, 20 and 30 s. Default 5. |
| `systems` | List of strings appended to the built-in system prompt. |
| `prompts` | Map of names to strings for [the `#` picker](prompts.md). |
| `functions` | Map of named Lua helpers with `run`, `description`, optional `args` and `show_agent`. See [functions](functions.md). |
| `hooks` | Map of named hooks `{events = {...}, run = function(e) ... end}`. See [hooks](hooks.md). |
| `settings.ui` | UI behavior; see [theme](theme.md). |
| `theme` | The 10 base colors; see [theme](theme.md). |

## Providers and history

Providers come from SQLite, not the Lua config. Run
`tuse connect --base URL --key '$ENV' --name NAME`, `tuse providers` or `tuse disconnect NAME`.

`~/.tuse/tuse.db` (or `$TUSE_HOME/tuse.db`) stores providers, sessions, records and the last
model/effort pick. Session subdirectories hold locks, task logs and images. SQLite history
from 0.7.x remains readable. To import older `session.jsonl` history, open Tuse 0.7.2 with the
same `TUSE_HOME` before moving to this version; 0.7.3 does not import it automatically.

Compaction only changes what the model receives, not the saved log. It keeps recent context
intact, shortens older tool outputs, and keeps only the user/assistant text further back.
