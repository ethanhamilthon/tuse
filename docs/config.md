# Config

How Tuse finds, composes, and runs its namespaced Lua configs, global config, and installed plugins, every key it accepts, and how reload works. Tuse runs fine without any config. Defaults and the system prompt are built into the binary.

## Global configuration

Tuse reads one Lua file: nonblank `$TUSE_GLOBAL_CONFIG`, then `~/.config/tuse.lua`, then `~/.config/tuse/init.lua`. An override expands `~/`, resolves relative paths against the session directory, and uses `init.lua` when it names a directory. An invalid explicit path fails rather than falling back. With no file, defaults apply. `--no-global` skips the lookup entirely. A project `./tuse.lua` is never auto-discovered.

The global file returns a table. Set `ui`, `systems`, `models`, and the optional `open` callback at the top level. Other entries are namespaces containing `functions`, `prompts`, `commands`, or `hook`:

```lua
return {
  ui = { input = "bar" },
  systems = { rules = "Run the tests before finishing." },
  models = { ["provider/model"] = { window = 128000 } },
  work = {
    prompts = { review = "Review the diff." },
    commands = { hello = function() print("hello") end },
    functions = { ping = function() return "pong" end },
  },
}
```

To reuse a trusted Lua file, load it explicitly, for example `return assert(loadfile("./tuse.lua"))()`. Unlike `require`, a relative `loadfile` path starts from the process working directory. Use `require("tuse").session.get().cwd` to build a path for the session directory when needed. Do not automatically execute an untrusted repository's file.

## Installed plugins

An installed plugin is a directory under `$TUSE_HOME/plugins/` (default `~/.tuse/plugins/`) with a `tuse-plugin.json` manifest. The manifest has required `version`, optional `name`, and `entrypoint` defaulting to `init.lua`. The relative entrypoint must stay within the plugin root. The entrypoint returns a flat table of `functions`, `prompts`, `commands`, and/or `hook`. Tuse uses the plugin name as its namespace: `demo.ping()`, `#demo:review`, `/demo:hello`. A collision with a user namespace fails validation; the tables are never merged.

Management is through `/plugins`, Lua `plugins.*`, and JSON-RPC `plugins.*`. Non-lazy installed plugins activate on startup or a successful full reload, unless `--no-plugins` is set. Installation and updates alone do not change the running config.

Set `"lazy": true` in `tuse-plugin.json` to skip automatic activation. `plugins.get` returns metadata without executing the entrypoint. To load a trusted lazy plugin manually and register its exports under `tools`:

```lua
local plugins = require("tuse").plugins
return { tools = plugins.load("demo") }
```

`plugins.load` executes the entrypoint even with `--no-plugins`, which only disables automatic activation. The call itself does not register anything; returning the export under `tools` registers that namespace. There are no plugin config selectors or entrypoint `opts`.

## Modules and `require`

`package.path` is empty. `require(name)` resolves like this:

| Call | Loads |
| --- | --- |
| `require("tuse")` | The host API: grouped `fs` (`read`, `write`, `append`, `delete`, `edit`, `grep`), `tasks` (`spawn`, `get`, `stop`, `list`), `session` (`get`, `set`, `fork`, `resume`), `ui` (`show`, `pick`, `ask`), `plugins` (`list`, `get`, `add`, `delete`, `update`, `load`), `providers` (`list`, `connect`, `update`, `disconnect`), `json` (`encode`, `decode`), object `docs` (`topics`, `<topic>`), and flat `sh`, `reload`, `llm`. |
| `require("name")`, `require("a.b")` | `name.lua` or `name/init.lua` (`a/b.lua`, …) next to the calling file. |
| `require("./x")`, `require("../x")` | The same, relative to the calling file. |
| `require("~/x")`, `require("/abs/x")` | `x.lua` or `x/init.lua` at that path. |
| `require("dir/file.lua")` | That exact file. |

Modules are cached by their real path. A cycle raises `cyclic require: '<path>'`, and a missing module lists every path that was tried. `package.preload` works as in normal Lua, and `/reload` clears the trusted environment’s preloads and cached modules (restoring them if reload fails).

In config code the host functions are **not** globals: get them from `local t = require("tuse")`. While the config itself is being evaluated, `t.session.get()` only has `mode` (`"tui"` or `"json"`), `config_path`, `cwd`, and `home`. At this point only `fs.read`, `sh`, `docs`, and `json` are safe to call. Put everything else into callbacks (hooks, commands, functions). `os.getenv("THIS_PATH")` gives the path of the current Lua file. `os.exit` and `debug` are not available.

## Global settings and namespace keys

Global settings are `ui`, `models`, `systems`, and `open`. Namespaces accept only `functions` (named callbacks), `prompts` (named literal strings), `commands` (named zero-argument callbacks), and `hook` (event-to-callback table). Unknown keys and wrong types fail with a source path.

### Theme and UI

`ui` controls the TUI and holds the theme. Its keys are `lines`, `input`, `picker`, `space`, and `theme`. Set it at the top level of the global config. Omitted settings use defaults.

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

#### `ui.input` and `ui.picker`

- `ui.input`: `boxed` (the default) draws a border around the growing input and status row, with one blank column inside each side border. `lines` puts a rule above and below the input. `fill` draws the input on an `input_bg` background with the status line below it. `bar` puts an `icons`-colored `▌` before each input row, then a blank row and the status line. `boxed`, `fill`, and `bar` fall back to `lines` if the terminal is too small. Every style leaves one empty row above the input.
- `ui.picker`: `bottom` (default) or `top`. Positions all pickers, questions, forms, and panels below or above the composer across every input style. `bottom` always leaves exactly one blank row between the input and the overlay below it (with `fill` and `bar`, the overlay sits between that blank row and the closing rule above the status line).

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
| `builtin` | `#97BEDC` | Standard and Tuse functions and libraries: `print`, `fs.read`, `string.format`. |
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

### `open`

Optional `open(target, kind)` receives a resolved path or supported URL with kind `file`, `dir`, or `url`. Return `true` if handled; `false` or `nil` delegates to the OS opener. Unsupported targets never reach the callback.

## Reload and `/config`

`/reload`, Lua `reload()`, and JSON-RPC `config.reload` re-evaluate the global config and eligible installed plugins in `$TUSE_HOME/plugins/` (honoring any launch exclusion flags), clearing cached trusted modules and `package.preload` results:

1. If evaluation or validation fails, `Reload failed: …` is shown and the old config, hooks, functions, and cached modules are restored.
2. If it succeeds, the new config takes over, the first `system` history record is rebuilt, and `config_load` hooks run.

`/config` and JSON-RPC `config.get` show every resolved config field: active `plugins` (`[{ name, version }]`), `models` (including `efforts`), raw `ui.theme` links, `ui` (including `picker`), `open`, `systems`, `prompts`, `functions`, `hook`, and `commands`. Saved providers are inspected separately via `providers.list` (`[{ name, type, base }]`), `/providers list`, or `tuse providers --json`. Secrets and Lua function bodies are never exposed.
