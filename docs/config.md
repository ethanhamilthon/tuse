# Config

How Tuse finds, composes, and runs its namespaced Lua configs, global config, plugins, every key it accepts, and how reload works. Tuse runs fine without any config. Defaults and the system prompt are built into the binary.

## Where the config comes from

Tuse composes three layers: **primary local config**, **global config**, and **installed plugins**.

1. **Primary local config** (evaluated first): `-c PATH` / `--config PATH` (relative to the current folder, `~/` expands; missing/unreadable fails), else `./tuse.lua` in the session folder.
2. **Global config**: resolved from `$TUSE_GLOBAL_CONFIG` (when set and non-empty after trimming whitespace; `~/` expands, relative paths resolve against the session working directory, and a directory loads `<dir>/init.lua`; a missing or unreadable target fails fast without falling back), else `~/.config/tuse.lua`, else `~/.config/tuse/init.lua`. Unset, empty, or whitespace-only `TUSE_GLOBAL_CONFIG` falls through to `~/.config/tuse.lua` / `~/.config/tuse/init.lua`.
3. **Installed plugins**: discovered under `~/.tuse/plugins/` (any immediate subdirectory whose root contains `tuse-plugin.json`).

Overall lookup precedence is `-c` > `./tuse.lua` > `$TUSE_GLOBAL_CONFIG` > `~/.config/tuse.lua` > `~/.config/tuse/init.lua` > built-in defaults. `session().config_path` points to the primary local config file, else the global config file, else `nil`.

### Namespaced layout

Primary configs (`./tuse.lua` or `-c PATH`) return a table of **named namespaces**, plus optional reserved top-level control keys `global` and `plugins`. Flat top-level config tables are rejected without backward compatibility:

```lua
local t = require("tuse")

return {
  global = { profile = "work" },
  plugins = { without = { "noise" } },
  project = {
    systems = { "Run `make check` before you finish." },
    prompts = { review = require("prompts/review") },
    open = { md = function(path) t.sh('open -a Typora "$P"', { env = { P = path } }) end },
  },
}
```

`tuse init` creates a minimal starter namespaced `./tuse.lua` that automatically composes with your global config and installed plugins.

### Global config function

The global config file (`$TUSE_GLOBAL_CONFIG`, `~/.config/tuse.lua`, or `~/.config/tuse/init.lua`) MUST return a function `function(opts) ... return { <namespaces> } end`:

```lua
return function(opts)
  opts = opts or {}
  return {
    base = {
      ui = { input = "bar", theme = { accent = opts.accent or "#FFB86C" } },
      history = { hide = { "usage", "flash" } },
    },
  }
end
```

- Because the primary local config evaluates first, setting `global = false` skips loading and executing the global config file entirely.
- Setting `global = <opts>` calls the global function with `<opts>`.
- Omitting `global` (or running with no primary local config) calls the global function with `nil`.
- Returning a non-function from the global config file fails validation.

### Plugins and `tuse-plugin.json`

Installed plugins live in `~/.tuse/plugins/<name>/` and are managed with `tuse plugins` ([Usage](usage.md#command-line)). A directory under `~/.tuse/plugins/` is recognized as an installed plugin if and only if its root contains `tuse-plugin.json`:

```json
{
  "name": "demo",
  "version": "1.0.0",
  "entrypoint": "init.lua"
}
```

- `version` (non-empty string) is required.
- `entrypoint` (string, default `"init.lua"`) must stay inside the plugin root directory (no absolute paths, `..`, or symlink escapes).
- `name` is optional; when installing, `<name>` defaults to the repository or subdirectory basename.
- A plugin entrypoint returns either a flat contribution table (`{ functions = ..., commands = ..., ... }`) or `function(opts) -> table`.

Control automatic plugin loading with top-level `plugins`:

- Omitting `plugins` or setting `plugins = {}` (or running with no config file) auto-loads all installed plugins.
- `plugins = { only = {} }` disables automatic plugin loading while keeping explicit `require("plugin:<name>")` and `require("plugins")` working. `plugins = false` is rejected.
- `plugins = { only = { "a", "b" }, opts = { a = { flag = true } } }` allowlists plugins and passes per-plugin `opts`.
- `plugins = { without = { "b" } }` excludes named plugins from auto-loading.
- Setting both `only` and `without`, or naming an uninstalled plugin in `only`, `without`, or `opts`, fails validation.
- If both global and local configs set `plugins`, the local `plugins` selector wins. Excluded plugins are never executed unless explicitly required.

### Merging and duplicate rules

Contributions from `global`, auto-loaded `plugins`, and `local` namespaces merge deterministically:

- **Duplicate error:** duplicate keys in `functions` (including `false` vs a function override), `commands`, `prompts`, `open`, `providers`, `hooks`, and duplicate `models[].name` fail validation with an error naming both sources. This includes re-exporting a function from `require("plugin:<name>")` while that plugin is still auto-loaded; exclude it with `plugins = { without = { "<name>" } }` or `plugins = { only = {} }`.
- **`ui`:** deep-merged by leaf field with layer priority `local > global > plugins`. Conflicting leaves within the same layer fail validation.
- **`systems` and `history.hide`:** concatenated in deterministic order: `global` -> `plugins` (sorted by name) -> `local` (sorted by namespace name).

## Modules and `require`

`package.path` is empty. `require(name)` resolves like this:

| Call | Loads |
| --- | --- |
| `require("tuse")` | The host functions: `session`, `sh`, `spawn`, `read`, `write`, `append`, `delete`, `edit`, `grep`, `docs`, `set_session`, `fork`, `resume`, `task_get`, `task_list`, `task_stop`, `show`, `pick`, `ask`, `reload`, `llm`, `json`, `inspect`. |
| `require("global")` | `$TUSE_GLOBAL_CONFIG` when set and non-empty (failing fast if missing), else `~/.config/tuse.lua` then `~/.config/tuse/init.lua`, returning its raw global function. |
| `require("plugin:<name>")` | Trusted config only. Loads only the installed plugin `<name>` from `~/.tuse/plugins` and returns its raw entrypoint export without invoking functions. |
| `require("plugins")` | Trusted config only. Loads all installed plugins (invoking function entrypoints with `nil`), validates duplicates, and returns a merged flat table (`.functions`, `.commands`, `.prompts`, etc.) independent of any returned `plugins` selector or `opts`. |
| `require("name")`, `require("a.b")` | `name.lua` or `name/init.lua` (`a/b.lua`, …) next to the calling file. |
| `require("./x")`, `require("../x")` | The same, relative to the calling file. |
| `require("~/x")`, `require("/abs/x")` | `x.lua` or `x/init.lua` at that path. |
| `require("dir/file.lua")` | That exact file. |

Modules are cached by their real path. A cycle raises `cyclic require: '<path>'`, and a missing module lists every path that was tried. `package.preload` works as in normal Lua, and `/reload` clears cached preloaded, global, and plugin modules (restoring them if reload fails).

In config code the host functions are **not** globals: get them from `local t = require("tuse")`. While the config itself is being evaluated, `t.session()` only has `mode` (`"tui"` or `"json"`), `config_path`, `cwd`, and `home`. At this point only `read`, `sh`, `json`, and `inspect` are safe to call. Put everything else into callbacks (hooks, commands, functions). `os.getenv("THIS_PATH")` gives the path of the current Lua file. `os.exit` and `debug` are not available.

## Namespace keys

Every namespace contribution table accepts these optional keys. Unknown keys, stray functions/userdata/threads outside known callback slots (`hooks.*.run`, `commands.*.run`, `prompts.*.run`, `functions.*`, `open.*`), wrong types, and out-of-range values fail with a path. Validation errors under `providers.*` stay generic so secrets are never echoed. On startup that stops Tuse; on reload the old config stays.

| Key | Type | Meaning |
| --- | --- | --- |
| `ui` | table | `lines`, `input`, `space`, and `theme` (below). |
| `history` | table | `hide` (below). |
| `open` | `key -> function(path)` | Click handlers for files and links (below). |
| `providers` | `name -> { base, key, api?, headers? }` | Session-only API connections ([Connect](connect.md#providers-in-lua-config)). |
| `models` | list | Model metadata including optional tri-state `efforts` ([Connect](connect.md#model-metadata)). |
| `systems` | list of strings | Text appended to the system prompt, in order ([Lua](lua.md#what-the-agent-sees)). |
| `prompts` | `name -> string \| { args?, run }` | Prompts for the `#` picker and JSON-RPC `turn.submit` (below). |
| `functions` | `name -> function \| false` | Extra agent functions, overrides, and disabled built-ins (below). |
| `hooks` | `name -> { events, run }` | Lifecycle hooks ([Lua](lua.md#hooks)). `events` needs at least one known event, and `run` is required. |
| `commands` | `name -> { args?, desc?, run }` | Custom slash commands (below). |

### Theme and UI

`ui` controls the TUI and holds the theme. `history` holds `hide`. Set only the keys you want to change inside a namespace:

```lua
return {
  look = {
    ui = {
      lines = { output = 3, diff = 20, strings = 2 },
      input = "bar",
      space = { padding = 1, steps = 0 },
      theme = { accent = "#FFB86C", code = { keyword = "accent" } },
    },
    history = { hide = { "usage", "flash" } },
  },
}
```

#### `ui.lines`

`-1` means unlimited and `0` hides the item, except for `strings`:

| Key | Default | Meaning |
| --- | --- | --- |
| `tool` | `-1` | Lines of tool code shown. |
| `output` | `1` | Lines of tool output shown. |
| `diff` | `12` | Diff lines per file shown. `0` hides the diff. |
| `reasoning` | `-1` | Last lines of reasoning shown. |
| `strings` | `1` | Lines of a multiline Lua long string shown in a tool call. `0` shows only `…`, `N` shows the first N lines and `…` when more follow, `-1` shows all. |
| `input` | `10` | Max visible input rows (`-1` or `>= 1`). |
| `list` | `16` | Max rows for pickers and panels (`>= 1`). |

#### `ui.input`

`boxed` (the default) draws a border around the growing input and status row, with one blank column inside each side border. `lines` puts a rule above and below the input. `fill` draws the input on an `input_bg` background with the status line below it. `bar` puts an `icons`-colored `▌` before each input row, then a blank row and the status line. `boxed`, `fill`, and `bar` fall back to `lines` if the terminal is too small. Every style leaves one empty row above the input. Pickers, questions, and panels open below the input: under the box or rules with `lines` and `boxed`, and between the input and the status line, closed by a rule, with `fill` and `bar`.

#### `ui.space`

| Key | Default | Meaning |
| --- | --- | --- |
| `padding` | `0` | Side padding in columns. |
| `steps` | `1` | Blank rows between tool calls and reasoning blocks. |
| `messages` | `1` | Blank rows above and below your messages and the agent's. |

The mouse wheel always scrolls 2 lines per tick.

#### `ui.theme`

| Token | Default | Used for |
| --- | --- | --- |
| `bg` | `#121212` | Background. |
| `accent` | `#BDDEAB` | Selection, atoms, accents, task counts. |
| `fg` | `#DEDEDE` | The agent's messages, hovered links. |
| `user` | `"fg"` | Text of your messages. |
| `user_bg` | `#1E1E1E` | Background of your messages. |
| `muted` | `#929292` | Secondary text, notices, tool output, reasoning, status bar, diff context. |
| `input` | `"user"` | Text in the input. |
| `input_bg` | `#333333` | Background or frame of the input. |
| `border` | `#333333` | Other borders: pickers, panels, rules, diff headers. |
| `icons` | `"accent"` | Prompt marker, `bar` mark (`▌`), spinners, the tool star. |
| `error` | `#E27373` | Error text. |
| `code` | table | Lua and diff colors (below). |
| `markdown` | table | Markdown colors (below). |

A color is `#rgb`, `#rrggbb`, an ANSI index `"0"`–`"255"`, or a name: `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`, `grey` (or `gray`), `dark_grey` (or `dark_gray`), and `bright_red`, `bright_green`, `bright_yellow`, `bright_blue`, `bright_magenta`, `bright_cyan`, `bright_white`. It can also be `"none"` (the terminal default).

Every token can also name a color token (`bg`, `accent`, `fg`, `user`, `user_bg`, `muted`, `input`, `input_bg`, `border`, `icons`, `error`) instead of a color. The link follows that token when you change it. A token cannot link to itself, and a cycle fails loading. `config.get` preserves raw token links (such as `"icons": "accent"`), while `/config` displays each resolved swatch `■` beside the raw token value or link.

#### `ui.theme.code`

| Token | Default | Colors |
| --- | --- | --- |
| `keyword` | `#E27373` | `if`, `then`, `else`, `for`, `while`, `do`, `repeat`, `until`, `in`, `return`, `break`, `goto`, `end`. |
| `storage` | `#E27373` | `local`, `function`. |
| `constant` | `"accent"` | `nil`, `true`, `false`. |
| `operator` | `#BDBDBD` | `=`, `==`, `+`, `..`, `#`, `and`, `or`, `not`, and the other operators. |
| `punctuation` | `"muted"` | `( ) { } [ ] , ; . :`. |
| `number` | `"accent"` | Numbers, including hex and exponents. |
| `string` | `#94B979` | Strings and long strings. |
| `escape` | `#97BEDC` | Escapes inside strings: `\n`, `\x41`, `\u{20}`. |
| `comment` | `"muted"` | Comments. |
| `function` | `#97BEDC` | Function names at definitions and calls. |
| `builtin` | `#97BEDC` | Standard and Tuse functions and libraries: `print`, `read`, `string.format`. |
| `method` | `#97BEDC` | Names after `:`. |
| `field` | `"none"` | Names after `.` and table keys. |
| `parameter` | `#BDBDBD` | Function parameters and their uses in the body. |
| `variable` | `"none"` | Every other name. |
| `added` | `#94B979` | Added diff lines. |
| `removed` | `#E27373` | Removed diff lines. |

#### `ui.theme.markdown`

| Token | Default | Colors |
| --- | --- | --- |
| `heading` | `#97BEDC` | Headings. |
| `link` | `"accent"` | Links. |
| `code` | `#94B979` | Inline code. |
| `quote` | `"muted"` | Block quotes. |
| `list` | `"muted"` | List markers. |

#### `history`

`history.hide` is a list of history roles left out of `/history`, such as `{ "usage", "turn_end" }` ([Session](session.md#history-and-session-data)). It also takes the message levels `flash`, `notice`, and `error`, which hide only `message` entries of that level. Unknown names fail with `history.hide[N]`.

When `models` is set, `/models` lists only those models. JSON-RPC `model.list` ignores that restriction.

### `prompts`

A prompt is a string, or a table with an optional `args` dense list (including empty `{}`) and a `run` function:

```lua
return {
  my = {
    prompts = {
      review = "Check the change for bugs.",
      today = { run = function() return "Today is " .. os.date("%F") end },
      openspec = {
        args = { "explore", "propose", "apply" },
        run = function(arg) return "OpenSpec " .. (arg or "explore") .. ": …" end,
      },
    },
  },
}
```

- A prompt name is not empty and has no whitespace.
- `args` is a dense list of unique strings without spaces. Non-dense tables (holes or extra string keys) fail validation.
- When the message is sent, `run(arg)` runs (`arg` is `nil` if none was picked) and must return a string. The model only sees that string, as `<prompt name="openspec" arg="propose">…</prompt>`, never the function.
- If `run` fails or does not return a string, the message is not sent and the error is shown.

### `functions`

Gives the agent extra functions, replaces built-ins, or removes them:

```lua
local t = require("tuse")
return {
  web = {
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
  },
}
```

- Keys are Lua identifiers. Lua keywords and standard globals (`print`, `string`, …) are rejected.
- A function value becomes a global, and an override of the same name on `require("tuse")`, only in the agent's `lua` tool. Config code, hooks, commands, and other functions never see these functions. Use `t.<name>` to call the original.
- `false` removes a built-in from the agent. It disappears from the globals, from `require("tuse")`, from the system prompt, and from `docs()`. `false` on an unknown name is an error. `json = false` works, but `json = function` does not.
- Custom functions are not announced to the agent. Describe them in `systems` or `prompts`.

### `commands` and `open`

- A command is a table `{ args?, desc?, run }`. A bare function fails loading. It adds `/name` to the TUI and JSON-RPC `commands.run`:

```lua
return {
  tools = {
    commands = {
      diff = { run = function() ... end },
      mv = { run = function(from, to) ... end },
      deploy = {
        desc = "ship it",
        args = { "staging", "prod" },
        run = function(env) ... end,
      },
    },
  },
}
```

- `desc` is shown next to `/name` in the `/` picker.
- Without `args`, words typed after the name (`/mv a.txt b.txt`) are passed to `run` in order, up to the parameter count `run` declares (any number if variadic). Missing words are `nil`. A zero-arg command fails on trailing words or whitespace with `/diff takes no words`. Words in `"…"` or `'…'` keep spaces, `\"`/`\'` escape quotes, and an unclosed quote is an error.
- With `args`, only one word is accepted, it must be in `args`, and `run` gets it as its first parameter. `args` must be a dense list (including empty `{}`) of unique strings without whitespace.
- Commands run as Lua jobs with no deadline. Host functions are not globals here; use `require("tuse")`. Esc cancels them. The return value is ignored.
- A command name is not empty, does not start with `/`, has no whitespace or control characters, and is not a built-in name (`config`, `new`, `resume`, `fork`, `models`, `tasks`, `session`, `history`, `reload`, `stop`, `quit`).
- `open = { md = function(path) ... end }` handles clicks on local files and links in the TUI. A key is an extension without a dot (`md`, case-insensitive), a mask with `*` or `?` (`*.test.lua`, `.env*`, case-insensitive), `*`, `@dir`, or `@url` (`http://`, `https://`, `mailto:`). A file uses its extension handler, then the mask with the most literal characters (ties alphabetically), then `*`. A folder uses `@dir`, then `*`. Without a handler, the OS opener is used.

## Reload and `/config`

`/reload`, Lua `reload()`, and JSON-RPC `config.reload` re-evaluate the primary config, global config, and installed plugins in `~/.tuse/plugins/`, clearing cached trusted modules and `package.preload` results, and re-reading saved providers from SQLite:

1. If evaluation or validation fails, `Reload failed: …` is shown and the old config, hooks, functions, and cached modules are restored.
2. If it succeeds, the new config takes over, the first `system` history record is rebuilt, and `config_load` hooks run.

`/config` and JSON-RPC `config.get` show every resolved config field, including active `providers` (`[{ name, type, base }]` for both saved and Lua providers — never keys or headers), active `plugins` (`[{ name, version }]`), `models` (including `efforts`), raw `ui.theme` links, `ui`, `history`, `open`, `systems`, `prompts`, `functions`, `hooks`, and `commands`.
