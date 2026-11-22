# Config

Tuse runs one Lua file: `--config PATH`, else `./tuse.lua`, else `$TUSE_GLOBAL_CONFIG`, else
`~/.config/tuse/main.lua`. The file returns a table. The system prompt and the default settings
are built into the binary, so the table holds only what you change. `tuse reset` installs a fresh
`main.lua`; `tuse update` never touches it.

The file runs in the session's VM, the same one as the agent's code, hooks and tasks, so it can
use everything of `docs("functions")`. Hooks and helpers are plain Lua functions, and locals of
the file are their shared state. `MODE` is `tui` or `json`. Instead of the old `include`, `$VAR`
and `#{{command}}`, use `require("name")` (a `name.lua` next to the config), `dofile(path)`,
`os.getenv("VAR")` and `io.popen(cmd):read("a")` or `sh(cmd).out`. `/reload` runs the file again in
the same VM: the agent's globals stay, the hooks and helpers are replaced.

```lua
local secrets = require("secrets")   -- secrets.lua next to this file
return {
  model = "c/claude-opus-5-5",
  effort = "medium",
  settings = { tool_timeout = 60, ui = { input_design = "box" } },
  systems = { "Answer in Russian." },
  prompts = { review = "Review the diff for bugs." },
  hooks = {
    log = { events = {"user_message"}, run = function(e) print(e.text) end },
  },
}
```

## Keys

| Key | Meaning |
| --- | --- |
| `model` | Default model, `name` or `provider/name`. `/models` remembers a pick in the database, and it wins over this key. |
| `provider` | The provider for `model` when it has no `provider/` prefix (default: the first one). |
| `effort` | `default` (sends no `reasoning_effort`), `low`, `medium`, `high`, `xhigh`, `max`. |
| `models` | List of `{name = "provider/model", display, window, compact_at, prices}`. `prices` are dollars per 1M tokens: `["in"]`, `out`, `cache` (cached reads) and `cache_write`. |
| `settings.compact_at` | Context size that triggers compaction (0 turns it off). Unset: 60% of the model's `window` (off when the window is unknown). A model's `compact_at` wins. |
| `settings.tool_timeout` | Seconds a `lua` call gets when the agent passes no `timeout`. Default 10. |
| `settings.hook_timeout` | Seconds a hook run gets. Default 30. |
| `settings.max_image_bytes` | Largest image `read` sends to the model. Default 5242880. |
| `settings.retries` | Retries for 429, 5xx and network errors, after 2, 5, 10, 20 and 30 s. Default 5. |
| `systems` | Texts appended to the built-in system prompt; each item is a string or `{text, modes}`. |
| `prompts` | `name = text` or `name = {text, modes}`. `#name` picked in the input sends the text along ([prompts.md](prompts.md)). |
| `functions` | Named Lua helpers with `run` (a function), `description`, optional `args` and `modes`, see `docs("functions")`. |
| `hooks` | `name = {events = {...}, run = function(e) ... end, modes?}`, see `docs("hooks")`. |
| `settings.theme` | Name of the active theme (empty or `default`: the built-in one). `/theme` remembers a pick in the database, and it wins over this key. |
| `settings.ui` | UI behavior and layout preferences, see `docs("theme")`. |
| `themes` | `name = {extend = base, bg = ..., ...}`, 10 base colors, see `docs("theme")`. |

## Providers

Providers are not part of the config: `tuse connect` asks for a base URL, an API key and a name
and saves them in `~/.tuse/tuse.db`. `tuse providers` lists them and `tuse disconnect NAME`
removes one. A key written as `$VAR` is read from the environment when it is used.

## Modes

Run modes are `tui` (interactive TUI) and `json` (`tuse json`). Entries in `systems`, `prompts`,
`functions` and `hooks` may set `modes = {"tui"}` (or any combination); omitted or empty enables
the entry in all modes. For anything else, branch on `MODE` in the file.

## Compaction

Records stay intact; a compaction only moves two boundaries that every later request
applies. The latest quarter of `compact_at` is sent unchanged. The quarter before it
keeps tool calls but cuts their outputs to the first line. Everything older keeps
only user and assistant text.

## Files

`~/.tuse/tuse.db` (or `$TUSE_HOME/tuse.db`, SQLite) holds the sessions and their records, the
providers and the last picked model, effort and theme. `~/.tuse/<id>/` holds the session's `lock`,
`tasks/<id>.log` and pasted `images/`. A session can be open in one process only, and a process
holds one session. Sessions of older versions (`session.jsonl` and `meta.json`) move into the
database on the first start.
