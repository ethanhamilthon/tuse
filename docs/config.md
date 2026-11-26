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
g.open = { md = function(path) t.sh('open -a Typora "$P"', { env = { P = path } }) end }
return g
```

## Modules and `require`

`package.path` is empty. `require(name)` resolves like this:

| Call | Loads |
| --- | --- |
| `require("tuse")` | The host functions: `session`, `sh`, `spawn`, `read`, `write`, `append`, `delete`, `edit`, `grep`, `docs`, `set_session`, `fork`, `resume`, `task_get`, `task_list`, `task_stop`, `show`, `pick`, `ask`, `reload`, `llm`, `json`, `inspect`. |
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
| `ui` | table | `lines`, `input`, `space`, and `theme` (below). |
| `history` | table | `hide` (below). |
| `open` | `key -> function(path)` | Click handlers for files and links (below). |
| `providers` | `name -> { base, key, api?, headers? }` | Session-only API connections ([Connect](connect.md#providers-in-lua-config)). |
| `models` | list | Model metadata ([Connect](connect.md#model-metadata)). |
| `systems` | list of strings | Text appended to the system prompt, in order ([Lua](lua.md#what-the-agent-sees)). |
| `prompts` | `name -> string \| { args?, run }` | Prompts for the `#` picker and JSON-RPC `turn.submit` (below). |
| `functions` | `name -> function \| false` | Extra agent functions, overrides, and disabled built-ins (below). |
| `hooks` | `name -> { events, run }` | Lifecycle hooks ([Lua](lua.md#hooks)). `events` needs at least one known event, and `run` is required. |
| `commands` | `name -> { args?, desc?, run }` | Custom slash commands (below). |

### Theme and UI

`ui` controls the TUI and holds the theme. `history` holds `hide`. Set only the keys you want to change:

```lua
return {
  ui = {
    lines = { output = 3, diff = 20, strings = 2 },
    input = "bar",
    space = { padding = 1, steps = 0 },
    theme = { accent = "#FFB86C", code = { keyword = "accent" } },
  },
  history = { hide = { "usage", "hook" } },
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
| `accent` | `#BDDEAB` | Selection, atoms, accents. |
| `fg` | `#DEDEDE` | The agent's messages, hovered links. |
| `user` | `"fg"` | Text of your messages. |
| `user_bg` | `#1E1E1E` | Background of your messages. |
| `muted` | `#929292` | Secondary text, notices, tool output, reasoning, status bar, diff context. |
| `input` | `"user"` | Text in the input. |
| `input_bg` | `#333333` | Background or frame of the input. |
| `border` | `#333333` | Other borders: pickers, panels, rules, diff headers. |
| `icons` | `"accent"` | Prompt marker, spinners, the tool star. |
| `error` | `#E27373` | Error text. |
| `code` | table | Lua and diff colors (below). |
| `markdown` | table | Markdown colors (below). |

A color is `#rgb`, `#rrggbb`, an ANSI index `"0"`–`"255"`, or a name: `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`, `grey` (or `gray`), `dark_grey` (or `dark_gray`), and `bright_red`, `bright_green`, `bright_yellow`, `bright_blue`, `bright_magenta`, `bright_cyan`, `bright_white`. It can also be `"none"` (the terminal default).

Every token can also name a color token (`bg`, `accent`, `fg`, `user`, `user_bg`, `muted`, `input`, `input_bg`, `border`, `icons`, `error`) instead of a color. The link follows that token when you change it. A token cannot link to itself, and a cycle fails loading.

#### `ui.theme.code`

Colors Lua code in tool calls, `/` panels, and `lua` code blocks, and the lines of diffs. The parser knows which names are definitions, parameters, calls, methods, table keys, and standard functions:

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

`history.hide` is a list of history roles left out of `/history`, such as `{ "usage", "hook" }` ([Session](session.md#history-and-session-data)). Unknown roles fail.

When `models` is set, `/models` lists only those models. JSON-RPC `model.list` ignores that.

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

- A prompt name is not empty and has no whitespace.
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

### `commands` and `open`

- A command is a table `{ args?, desc?, run }` like a prompt, plus `desc`. A bare function fails loading. It adds `/name` to the TUI and JSON-RPC `commands.run`:

```lua
return {
  commands = {
    diff = { run = function() ... end },
    mv = { run = function(from, to) ... end },
    deploy = {
      desc = "ship it",
      args = { "staging", "prod" },
      run = function(env) ... end,
    },
  },
}
```

- `desc` is shown next to `/name` in the `/` picker.
- Without `args`, the words typed after the name (`/mv a.txt b.txt`) are passed to `run` in order, as many as `run` declares (any number if it takes `...`). Missing words are `nil`. Too many words fail with `/mv takes at most 2 words`, and a function without parameters fails with `/diff takes no words`. A word in `"…"` or `'…'` keeps its spaces, `\"` or `\'` inside the same quotes is a literal quote, and an unclosed quote is an error.
- With `args`, only one word is accepted, it must be one of `args`, and `run` gets it as its first parameter. In the TUI, picking the command opens a picker of its `args`, and a space after `/name` opens it again. Choosing an arg runs the command. Esc leaves `/name ` in the input, and Enter then runs it with no arg. An unknown arg fails with `Unknown arg x for /deploy`.
- `args` are unique strings without spaces, `desc` is a string, and `run` is required.
- Commands run as Lua jobs with no deadline. `print` is not shown, and host functions are not globals here, so use `t.show()`. Esc cancels them. The return value is ignored.
- A command name is not empty, does not start with `/`, has no whitespace or control characters, and is not a built-in name (`config`, `new`, `resume`, `fork`, `models`, `tasks`, `session`, `history`, `reload`, `stop`, `quit`).
- `open = { md = function(path) ... end }` handles clicks on local files and links in the TUI. A key is one of:
  - an extension without a dot (`md`), case-insensitive;
  - a mask with `*` or `?` matched against the file name (`*.test.lua`, `.env*`), case-insensitive;
  - `*`, for anything without a more specific handler;
  - `@dir`, for folders;
  - `@url`, for `http://`, `https://`, and `mailto:` links, which get the URL.
- A file uses its extension handler, then the mask with the most literal characters (ties go in alphabetical order), then `*`. A folder uses `@dir`, then `*`. Without a handler, the OS opener is used. Other keys fail loading, and keys have no `/`, `\`, whitespace, or control characters. `settings` and the top-level `theme` fail with `moved to` errors.

## Reload

`/reload`, Lua `reload()`, and JSON-RPC `config.reload` run the config file again in the same VM:

1. Evaluate and validate the new config, and re-read providers from SQLite.
2. If that fails, the error is shown (`Reload failed: …`) and the old config, hooks, and functions stay. Side effects of top-level code can't be undone, so keep top-level code pure.
3. If it works, the new config takes over. Functions, overrides, and disables update in place, even through saved references. Tuse rebuilds the first `system` history record and runs `config_load` hooks.

A reload keeps the session's model, its running tasks, `show()` blocks, and the agent's own globals. `/config` (or `config.get`) shows what is active.
