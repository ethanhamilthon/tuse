# Config

How Tuse finds and runs its Lua config, every key it accepts, and how reload works. Tuse runs fine without any config. Defaults and the system prompt are built into the binary.

## Where the config comes from

Tuse loads one entry file, the first one that exists:

1. `-c PATH` / `--config PATH`, relative to the current folder (`~/` expands). If it can't be read, Tuse stops with an error.
2. `./tuse.lua` in the session folder.
3. `~/.config/tuse.lua`.
4. `~/.config/tuse/init.lua`.
5. Nothing: built-in defaults (`session().configpath` is `nil`).

The file is Lua 5.4 code that returns a table (or `nil`, which counts as `{}`). It runs with your permissions, so only load project configs you trust. `tuse init` writes a starter `./tuse.lua`:

```lua
local ok, g = pcall(require, "global")
g = ok and g or {}
return g
```

A fuller example of a project config that builds on the global one:

```lua
local t = require("tuse")
local ok, g = pcall(require, "global")
g = ok and g or {}

g.systems = g.systems or {}
g.systems[#g.systems + 1] = "Run `make check` before you finish."
g.prompts = g.prompts or {}
g.prompts.review = require("prompts/review")
g.settings = { open = { md = function(path) t.sh('open -a Typora "$P"', { env = { P = path } }) end } }
return g
```

## Modules and `require`

`package.path` is empty. `require(name)` resolves like this:

| Call | Loads |
| --- | --- |
| `require("tuse")` | The host functions: `session`, `sh`, `spawn`, `read`, `write`, `append`, `delete`, `edit`, `grep`, `docs`, `set_session`, `task_get`, `task_list`, `task_stop`, `show`, `pick`, `ask`, `reload`, `llm`, `json`, `inspect`. |
| `require("global")` | `~/.config/tuse.lua`, else `~/.config/tuse/init.lua`. |
| `require("name")`, `require("a.b")` | `name.lua` or `name/init.lua` (`a/b.lua`, …) next to the calling file. |
| `require("./x")`, `require("../x")` | The same, relative to the calling file. |
| `require("~/x")`, `require("/abs/x")` | `x.lua` or `x/init.lua` at that path. |
| `require("dir/file.lua")` | That exact file. |

Modules are cached by their real path. A cycle raises `cyclic require: '<path>'`, and a missing module lists every path that was tried. `package.preload` works as in normal Lua.

In config code the host functions are **not** globals: get them from `local t = require("tuse")`. While the config itself is being evaluated, `t.session()` only has `mode` (`"tui"` or `"json"`), `configpath`, `cwd`, and `home`. Use `mode` to build a config that differs per front end. At this point only `read`, `sh`, `json`, and `inspect` are safe to call. Put everything else into callbacks (hooks, commands, functions). `os.getenv("THIS_PATH")` gives the path of the current Lua file. `os.exit` and `debug` are not available.

## Keys

All keys are optional. Unknown keys, wrong types, and out-of-range values fail with a path, like `tuse.lua: models[2].prices.input: expected a finite non-negative number`. On startup that stops Tuse. On reload the old config stays.

| Key | Type | Meaning |
| --- | --- | --- |
| `settings` | table | `ui` and `open` (below). |
| `theme` | table | Colors (below). |
| `models` | list | Model metadata ([Connect](connect.md#model-metadata)). |
| `systems` | list of strings | Text appended to the system prompt, in order ([Lua](lua.md#what-the-agent-sees)). |
| `prompts` | `name -> string \| { args?, run }` | Prompts for the `#` picker and JSON-RPC `turn.submit` (below). |
| `functions` | `name -> function \| false` | Extra agent functions, overrides, and disabled built-ins (below). |
| `hooks` | `name -> { events, run }` | Lifecycle hooks ([Lua](lua.md#hooks)). |
| `commands` | `name -> function` | Custom slash commands (below). |

### Theme and UI

`theme` has 10 colors. Set only the ones you want to change:

| Token | Default | Used for |
| --- | --- | --- |
| `bg` | `#121212` | Background. |
| `surface` | `#1E1E1E` | Your messages. |
| `fg` | `#DEDEDE` | Main text, hovered links. |
| `dim` | `#BDBDBD` | Secondary text, notices, diff context. |
| `muted` | `#929292` | Tool output, reasoning, status bar. |
| `border` | `#333333` | Rules, truncation marks, diff headers. |
| `accent` | `#BDDEAB` | Prompt marker, spinner, selection, links, atoms. |
| `ok` | `#94B979` | Added lines, inline code, Lua strings. |
| `err` | `#E27373` | Errors, removed lines, Lua keywords. |
| `info` | `#97BEDC` | Headings, Lua functions. |

A color is `#rgb`, `#rrggbb`, an ANSI index `"0"`–`"255"`, or a name: `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`, `grey`, `dark_grey`, and `bright_*`. It can also be `"accent"` (the accent color) or `"none"` (the terminal default).

`settings.ui` controls the TUI. `-1` means unlimited and `0` hides the item:

| Key | Default | Meaning |
| --- | --- | --- |
| `fold_strings` | `true` | Fold multiline Lua long strings in tool calls to one line. |
| `tool_line` | `-1` | Lines of tool code shown. |
| `output_line` | `1` | Lines of tool output shown. |
| `diff_line` | `12` | Diff lines per file shown. `0` hides the diff. |
| `reasoning_line` | `-1` | Last lines of reasoning shown. |
| `scroll_step` | `3` | Lines per wheel tick (`>= 1`). |
| `pad_x` | `0` | Side padding in columns. |
| `message_gap` | `1` | Blank lines between entries. |
| `list_max_height` | `16` | Max rows for pickers and panels (`>= 1`). |
| `input_max_height` | `-1` | Max input rows (`-1` or `>= 1`). |
| `hide_history` | `{}` | History roles left out of `/history`, such as `{ "usage", "hook" }` ([Session](session.md#history-and-session-data)). Unknown roles fail. |
| `scope_models` | `{}` | `provider/model` or `provider/*` patterns. When set, `/models` lists only matching models plus the current one. JSON-RPC ignores it. |

```lua
return {
  theme = { accent = "#FFB86C", info = "bright_blue" },
  settings = { ui = { output_line = 3, pad_x = 1 } },
}
```

### `prompts`

A prompt is a string, or a table with an optional `args` list and a `run` function:

```lua
return {
  prompts = {
    review = "Check the change for bugs.",
    today = { run = function() return "Today is " .. os.date("%F") end },
    openspec = {
      args = { "explore", "propose", "apply" },
      run = function(arg) return "OpenSpec " .. (arg or "explore") .. ": …" end,
    },
  },
}
```

- `args` are unique strings without spaces. In the TUI you pick one after the prompt (`#openspec propose`). JSON-RPC sends `"openspec propose"`, and an arg not in the list fails.
- When the message is sent, `run(arg)` runs (`arg` is `nil` if none was picked) and must return a string. The model only sees that string, as `<prompt name="openspec" arg="propose">…</prompt>`, never the function.
- If `run` fails or does not return a string, the message is not sent and the error is shown.

### `functions`

Gives the agent extra functions, replaces built-ins, or removes them:

```lua
local t = require("tuse")
return {
  systems = { "You also have `websearch(query)`, which returns result text." },
  functions = {
    websearch = function(q)
      return t.sh('curl -fsS "https://example.com/?q=$Q"', { env = { Q = q } }).out
    end,
    sh = function(cmd, opts)
      opts = opts or {}
      opts.timeout = opts.timeout or 30
      return t.sh(cmd, opts)
    end,
    delete = false,
  },
}
```

- Keys are Lua identifiers. Lua keywords and standard globals (`print`, `string`, …) are rejected.
- A function value becomes a global, and an override of the same name on `require("tuse")`, but only in the agent's `lua` tool. Config code, hooks, commands, and other functions never see these functions. Use `t.<name>` to call the original.
- `false` removes a built-in from the agent. It disappears from the globals, from `require("tuse")`, from the system prompt, and from `docs()`. `false` on an unknown name is an error. `json = false` works, but `json = function` does not.
- Custom functions are not announced to the agent. Describe them in `systems` or `prompts`.
- This limits what the agent sees. It is not a sandbox: the agent still has `io` and `os`.

### `commands` and `settings.open`

- `commands = { name = function() ... end }` adds `/name` to the TUI and JSON-RPC `commands.run`. Commands run as Lua jobs with no deadline. `print` is not shown, and host functions are not globals here, so use `t.show()`. Esc cancels them. Built-in names (`config`, `new`, `resume`, `models`, `tasks`, `session`, `history`, `reload`, `stop`, `quit`) are reserved.
- `settings = { open = { md = function(path) ... end } }` handles clicks on local files with that extension (case-insensitive) in the TUI. Without a handler, the OS opener is used. URLs always go to the OS opener. A top-level `open` key fails with `open: moved to settings.open`.

## Reload

`/reload`, Lua `reload()`, and JSON-RPC `config.reload` run the config file again in the same VM:

1. Evaluate and validate the new config, and re-read providers from SQLite.
2. If that fails, the error is shown (`Reload failed: …`) and the old config, hooks, and functions stay. Side effects of top-level code can't be undone, so keep top-level code pure.
3. If it works, the new config takes over. Functions, overrides, and disables update in place, even through saved references. Tuse rebuilds the first `system` history record and runs `config_load` hooks.

A reload keeps the session's model, its running tasks, `show()` blocks, and the agent's own globals. `/config` (or `config.get`) shows what is active.
