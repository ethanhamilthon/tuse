# Functions

Available as globals in the agent's `lua` tool. In config files and modules, including hooks,
commands, `open` handlers, and helpers, import them with `require("tuse")`. Errors are Lua errors
with a plain message: catch them with `pcall`. Options are passed as a table, like
`read(path, {lines = true, offset = 40})`.

Prefer `read` for file contents, `grep` for search, and `write`/`edit`/`append`/`delete` for file
changes so the user sees diffs. Use `sh` for running programs and processing their output.

## `session()`

`require("tuse").session()` (or global `session()` in the `lua` tool) is the single source of
runtime session data. Every call returns a **fresh snapshot** table (mutating it has no effect on
Tuse) and never exposes provider credentials or API keys.

| Field | Type | Unit / meaning |
| --- | --- | --- |
| `mode` | string | Front end: `"tui"` or `"json"`. Always set. |
| `configpath` | string \| nil | Path of the loaded config file, or `nil` when running on defaults. |
| `cwd` | string | Session working directory. Always set. |
| `home` | string | Tuse state directory (`~/.tuse`). Always set. |
| `id` | string \| nil | Session ID (`nil` before a session exists). |
| `name` | string \| nil | Session title/name, or `nil` when unnamed. |
| `model` | string \| nil | Active model (`provider/model`), or `nil` when unset. |
| `provider` | string \| nil | Active provider prefix, or `nil` when unset. |
| `effort` | string \| nil | Reasoning effort (`"default"`, `"low"`, `"medium"`, `"high"`, `"xhigh"`, `"max"`). |
| `ctx` | integer \| nil | Latest request context size in tokens (`input + output` of the last usage record, or estimated tokens right after compaction). |
| `ctx_size` | integer \| nil | Context window size in tokens from configured model metadata (`nil` when unknown). |
| `usage` | table \| nil | Cumulative session totals (including resumed history): `input` (prompt tokens, including cached/cache-write), `output` (completion tokens), `cache_read` (cached prompt tokens read), `cache_write` (prompt tokens written to cache), and `cost` (USD, or `nil` if pricing is unknown). |
| `model_info` | table \| nil | Saved metadata for the active model (`name`, `display`, `window` in tokens, `compact_at` override in tokens, `prices = {input, output, cache_read, cache_write}` in USD per 1M tokens), or `nil` when unset. |
| `steps` | integer \| nil | Completed model request/response steps in the session (including resumed history). |
| `turns` | integer \| nil | Completed user turns in the session (including resumed history). |
| `trace` | list \| nil | Full session trace in chronological order (`{role, summary, text}`), including resumed history, with no UI truncation. |

**Before a session exists (initial config load and `/reload`):** Top-level config and plugin
evaluation runs with consistent pre-session semantics on both initial startup and `/reload`:
`session()` returns `{ mode, configpath, cwd, home }` with `id` and all session-only fields set
to `nil`. Inside hooks (including `start` and `close`), commands, helpers, `open` handlers, tasks,
and `lua` tool calls, `session()` returns the full live session snapshot.

## Built-in functions

- `read(path)` / `read(path, {lines=true, offset=1, limit=2000})`: returns raw file text by default
  without line numbers or pagination notes. Pass `{lines=true}` for numbered lines in pages (the
  end says which `offset` continues). An image (png, jpg, gif, webp) is shown to you with the
  result. A binary file gives a short message. Images over `settings.max_image_bytes` raise an error.
- `write(path, content)`: creates the file (and missing folders) or replaces it. The write is
  atomic and keeps the file's permissions.
- `append(path, text)`: appends text to the file (creating it and missing folders if needed)
  atomically.
- `delete(path)`: deletes a single file. Does not delete directories.
- `edit(path, old, new, {all=false})`: replaces exact text. `old` must be found, and unique
  unless `all` is set; otherwise an error lists the lines of every match. A CRLF file matches
  `\n` in `old` and keeps its CRLF.
- Files changed by `write`, `append`, `edit`, and `delete` are shown to the user as a diff at the
  end of the call; you only get a short confirmation.
- `grep(pattern, {path=".", glob=nil, ignore_case=false, literal=false, context=0, limit=100})`:
  searches file contents with a regex (or literal text when `literal=true`) and returns
  `path:line: text` per match (`path-line- text` for context lines). `path` can be a single path
  or a list of paths. It respects `.gitignore` and skips binary files and `.git`. `glob` is a file
  filter such as `*.rs`. A bad pattern raises an error.
- `sh(cmd, {timeout=nil, cwd=nil, env={}})`: `bash -c cmd`, stdout and stderr merged. Wrap shell
  commands that contain quotes, backslashes, or regexes in Lua long strings like `sh([=[...]=])`
  so Lua does not interpret shell escapes. Returns a table with `.out`, `.code` (`nil` when
  killed), `.ok` (`code == 0`), `.timed_out` and `:lines()`; `tostring(r)` is the output plus an
  exit note. Inherits the ordinary process environment plus any explicit `env` entries (Tuse does
  not inject `TUSE_*` variables). At its own timeout or the deadline of the call the process group
  is killed; Esc kills it too. While it runs, hooks and tasks go on.
- `sleep(seconds)`: waits without holding up the other coroutines.
- `spawn(fn, name)`: runs `fn` as a background task in the same VM, so it sees your globals and
  functions; returns its ID (`t1`, `t2`, ...). Run a server with
  `spawn(function() sh("npm run dev") end, "dev")`. A task has no deadline. Its `print` goes to
  `tasks/<id>.log`, followed by what it returned or its error. When it ends on its own, you get a
  message with its last line.
- `task_get(id, lines=50)` returns the task with the log tail in `.output`, `task_stop(id)` stops it
  (its coroutine gets a `cancelled` error at its next wait), `task_list()` lists them. A task has
  `.id`, `.name`, `.status` (`running`, `done`, `failed`, `stopped`), `.started` and `.ended`.
- `json.encode(value)`, `json.decode(text)` (`null` becomes `nil`) and `inspect(value)`.
- `show_markdown(name, text, pin)`: renders or updates a UI-only Markdown block by name.
  Unpinned blocks stay in chat order; pinned blocks stay together at the end of the chat after new
  messages. `show_markdown(name, "", false)` removes the block. Blocks survive `/reload` but are
  never sent to the model or saved across resume.
- `show_picker(options)` -> `option | nil`: shows a picker for a Lua list of strings (editor
  disabled, Enter selects, Esc returns `nil`).
- `show_question(text)` -> `user_input | nil`: renders a Markdown question panel and uses the
  editor for the answer (Enter returns the answer to Lua without sending to the model, Esc returns
  `nil`).
- Picker and question requests share one FIFO queue, pause the calling coroutine's timeout while
  waiting, and leave a UI-only chat entry when answered (cancelled prompts leave no entry).
- `set_title(text)` names the session (and emits `name_change`); `docs(topic)`: call `docs()` to
  list documentation topics, or `docs("<topic>")` to read one.

## Functions from the config

```lua
local t = require("tuse")

return {
  functions = {
    tail_tests = {
      args = "lines",
      description = "Run tests and return the last lines of output.",
      run = function(lines)
        local out = t.sh("make check"):lines()
        return table.concat(out, "\n", math.max(1, #out - (lines or 30) + 1))
      end,
    },
  },
}
```

Each `run` becomes a global of the VM under the function's name. `args` is only the text shown
to the agent between the parentheses. `show_agent = false` (default `true`) hides a helper from
the agent: it is left out of the system prompt and of `docs("functions")`, but it stays defined,
so other helpers, hooks and tasks can call it. Branch on `require("tuse").session().mode` to
choose which helpers to define.

`/reload` validates the config first, then runs the config file again in the same VM. On success,
retained helpers get their new code, helpers the config no longer defines disappear from Lua and
`docs("functions")` (unless the agent has rebound that global itself), and the agent's other
globals stay. A failed reload changes nothing: every existing helper keeps working.
