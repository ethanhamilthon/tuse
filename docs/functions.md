# Functions

Built-in host functions are globals in the agent's `lua` tool (and are also available via `require("tuse")`, subject to any overrides or `false` disables configured in `functions`). In config files, plugins, trusted `require`d modules, hooks, custom commands, `open` handlers, and configured `functions` bodies, built-in host functions are not globals—import the unmodified originals with `require("tuse")`.

Errors are plain Lua errors (catchable with `pcall`). Optional settings are passed as a table (for example, `read(path, { lines = true, offset = 40 })`). File paths accept absolute paths, `~/...` (home directory), or paths relative to the session working directory.

Prefer `read` for file contents, `grep` for search, and `write`/`edit`/`append`/`delete` for file changes so diffs are recorded and shown to the user. Use `sh` for running programs and processing their output.

## `session()`

`session()` (or `require("tuse").session()`) is the single source of runtime session data. Every call returns a **fresh, detached snapshot** table (mutating it has no effect on Tuse) and never exposes provider credentials or API keys.

| Field | Type | Meaning |
| --- | --- | --- |
| `mode` | string | Front end: `"tui"` or `"json"`. Always set. |
| `configpath` | string \| nil | Path of the loaded config file, or `nil` when running on built-in defaults. |
| `cwd` | string | Session working directory. Always set. |
| `home` | string | Tuse state directory (`~/.tuse`). Always set. |
| `id` | string \| nil | Session ID (`nil` before a session exists). |
| `name` | string \| nil | Session title/name, or `nil` when unnamed. |
| `model` | string \| nil | Active model (`"provider/model"`), or `nil` when unset. |
| `provider` | string \| nil | Active provider prefix (before `/`), or `nil` when unset. |
| `effort` | string \| nil | Active reasoning effort (`"default"`, `"low"`, `"medium"`, `"high"`, `"xhigh"`, `"max"`). |
| `ctx` | integer \| nil | Latest request context size in tokens (`input + output` of the last main-turn usage record, or estimated tokens immediately after compaction; `0` initially). |
| `ctx_size` | integer \| nil | Context window in tokens from configured model metadata (`nil` when unknown or `0`). |
| `usage` | table \| nil | Cumulative session totals (including resumed history and `llm()` calls): `input` (prompt tokens, including cached/cache-write), `output` (completion tokens), `cache_read` (cached prompt tokens read), `cache_write` (prompt tokens written to cache), `cost` (total USD = main + `llm()`, or `nil` if unpriced), `llm_cost` (`llm()` USD subset, or `nil` if unpriced). |
| `model_info` | table \| nil | Configured metadata for the active model (`name`, optional `display`, optional `window`, optional `compact_at`, optional `prices = { input, output, cache_read, cache_write }` in USD per 1M tokens), or `nil` if not configured. |
| `steps` | integer \| nil | Completed assistant steps in the session (including resumed history). |
| `turns` | integer \| nil | Completed or cancelled user turns in the session (including resumed history). |
| `trace` | list \| nil | Full chronological session trace (`{ role, summary, text }`), including resumed history, with no UI truncation. |

**Pre-session semantics (startup and `/reload`):** During top-level config and plugin evaluation, `session()` returns `{ mode, configpath, cwd, home }` with `id` and all session-only fields set to `nil`. Inside hooks (including `start` and `close`), commands, helpers, `open` handlers, tasks, and `lua` tool calls, `session()` returns the full live session snapshot.

## File Functions

- `read(path)` / `read(path, { lines = false, offset = 1, limit = 2000 })`:
  - Works in live sessions and during top-level config load (outside a session uses `cwd` and default `max_image_bytes`).
  - Raises an error on directories (`<path> is a directory; list it with sh("ls")`).
  - **Images** (`.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, case-insensitive): raises an error if larger than `settings.max_image_bytes`. Otherwise returns `"[image: <path>, <KB> KB]"` and, when called inside an agent `lua` tool call, attaches the base64 image so the model sees it on the next step (images read from hooks, commands, tasks, or `open` handlers are not sent to the model).
  - **Binary files** (NUL byte in the first 8,192 bytes): returns `"[binary file, <bytes> bytes]"`.
  - **Default mode** (`lines = false` or omitted): returns full raw text (lossy UTF-8) with no line numbers, no pagination footer, and no truncation (`""` for empty files).
  - **Paginated mode** (`lines = true`): `offset` and `limit` clamp to `>= 1`. Returns `<line_no>\t<text>` per line (lines over 2,000 characters cut with `… [line cut]`). Empty file returns `"[empty file]"`; `offset` past EOF returns `"[offset <offset> is past the end of the file (<n> lines)]"`; more lines remaining appends `"[lines <offset>-<last> of <n>; continue with offset=<last+1>]"`.
- `write(path, content)`:
  - Creates missing parent folders and writes atomically (temp file + sync + rename, preserving existing permissions). Rejects directories.
  - When called inside an agent `lua` tool call, records a diff for the user (up to 200 lines for a new file, or up to 200 lines per changed side—old and new—plus 2 context lines before and after for an overwrite). Returns `"created <path> (<n> lines)"` or `"overwrote <path> (<old> -> <new> lines)"`.
- `append(path, text)`:
  - Atomically creates or appends to a file (creating missing parent folders and preserving permissions). Rejects directories. Existing file bytes are decoded as lossy UTF-8 before appending `text` (invalid UTF-8 sequences in existing files become replacement characters).
  - When called inside an agent `lua` tool call, records a diff (up to 200 lines for a new file, or up to 200 lines per changed side plus 2 context lines). Returns `"created <path> (<n> lines)"` or `"appended <path> (<old> -> <new> lines)"`.
- `delete(path)`:
  - Deletes a single file (or symlink). Rejects directories (`delete() only removes a single file`).
  - When called inside an agent `lua` tool call, records a removal diff (up to 200 lines, or `- [binary file, <bytes> bytes]`). Returns `"deleted <path> (<n> lines)"` (or byte count for binary files).
- `edit(path, old, new, { all = false })`:
  - Replaces exact UTF-8 text atomically, preserving file permissions.
  - Raises an error if `old` is empty, `old == new`, the file is not valid UTF-8, `old` is not found, or `old` matches multiple times when `all = false` (error lists up to 10 matching line numbers).
  - If the file uses CRLF (`\r\n`) and `old` contains no `\r`, `\n` in `old` and `new` automatically match and preserve `\r\n`.
  - When called inside an agent `lua` tool call, records up to 5 diff hunks (2 context lines before and after each). Returns `"edited <path> (<n> replacement[s])"`.
- `grep(pattern, { path = ".", glob = nil, ignore_case = false, literal = false, context = 0, limit = 100 })`:
  - Searches file contents with a regex (or escaped literal text when `literal = true`). `path` accepts a single path string or a list of path strings.
  - Respects `.gitignore`, includes hidden files, and skips `.git` and binary files (NUL byte in the first 8,192 bytes). `glob` filters file paths (e.g., `"*.rs"`). `limit` clamps to `>= 1`.
  - Formats matches as `<path>:<line>: <text>` and context lines as `<path>:<line>- <text>`, separating non-contiguous context blocks in the same file with `--`. Lines over 300 characters are clipped with `…`.
  - Returns `"[no matches]"` when nothing matches; appends `"[stopped at <limit> matches; narrow the pattern or raise limit]"` when truncated.
  - Raises an error on an invalid regex, invalid glob, invalid `path` type, missing target path, or job cancellation/timeout.

## Shell, Timing, and Background Tasks

- `sh(cmd, { timeout = nil, cwd = nil, env = {} })`:
  - Runs `bash -c cmd` with stdin closed, stdout and stderr merged, in its own session/process group (no controlling terminal).
  - Wrap shell commands containing quotes, backslashes, or regexes in Lua long strings (`sh([=[...]=])`) so Lua does not interpret shell escapes.
  - `timeout` (seconds, clamped `>= 0`) takes the earlier of itself and the caller's remaining deadline. `cwd` resolves relative to the session `cwd`. `env` merges string pairs onto the inherited process environment (no `TUSE_*` variables are injected).
  - Returns a table with `.out` (string, uncapped), `.code` (integer exit code, or `nil` when killed), `.ok` (`code == 0`), `.timed_out` (boolean), and `:lines()` (1-based list of lines, stripping `\r?\n` and trailing empty element). `tostring(r)` returns `.out` plus `[timed out]`, `[killed by signal]`, or `[exit code N]` when non-zero/killed.
  - Yields cooperatively in coroutines (blocking in non-yieldable contexts). Kills the entire process group (`SIGKILL`) on timeout, cancellation, or drop.
- `sleep(seconds)`:
  - Waits for `seconds` (number; `nil` or negative behaves as `0`), yielding without blocking other coroutines (blocking in non-yieldable contexts). Raises `"cancelled"` or `"timed out"` if stopped early.
- `spawn(fn, name)`:
  - Starts `fn` as a background task coroutine in the same VM with no deadline; returns its task ID (`"t1"`, `"t2"`, ...). `name` defaults to the task ID. See [tasks.md](tasks.md).
- `task_get(id, lines = 50)`:
  - Returns `{ id, name, status, started, ended, output }` where `output` is the last `lines` (clamped `>= 1`) of the task's log file. Raises `"no task <id>"` if unknown.
- `task_stop(id)`:
  - Stops task `id` (sets status `"stopped"` and cancels its coroutine at the next wait/check) and returns `{ id, name, status, started, ended }`. Idempotent on ended tasks; raises `"no task <id>"` if unknown.
- `task_list()`:
  - Returns a list of `{ id, name, status, started, ended }` for all tasks in the session.

## Interactive UI

- `show(name, text, pin = false)`:
  - Renders or updates a UI-only Markdown block identified by `name`.
  - Unpinned (`pin = false`) blocks stay in chat order and update in place by `name`. Pinned (`pin = true`) blocks render together at the end of chat after new messages. Changing `pin` moves the block to the new section at the current position.
  - `show(name, "", false)` (or `show(name, "")`) removes the block.
  - Blocks survive `/reload` and are persisted in the session's UI history (`ui_markdown` records) so resuming in TUI or JSON-RPC restores them in their original chat positions and pin states (replayed before `start` hooks run). Blocks are never sent to the model or included in `/trace`.
- `pick(options)` -> `string | nil`:
  - Displays a picker for a Lua list of strings (raises if any element is not a string; an empty list returns `nil` immediately without prompting).
  - In TUI, input editing is disabled; Enter returns the selected option, Esc returns `nil`.
  - Answered pickers append and persist a UI-only chat entry (`ui_result` record with `kind = "picker"`, `prompt = "Picker"`, `value`) restored in position across resume (excluded from model context and `/trace`). Cancelled and unanswered (pending-at-close) pickers are not persisted and leave no entry.
- `ask(text)` -> `string | nil`:
  - Displays `text` as a Markdown question panel and uses the editor for the answer (restoring any existing draft on close).
  - Enter returns the typed string to Lua without sending it to the model; Esc returns `nil`.
  - Answered questions append and persist a UI-only chat entry (`ui_result` record with `kind = "question"`, `prompt = text`, `value`) restored in position across resume (excluded from model context and `/trace`). Cancelled and unanswered (pending-at-close) questions are not persisted and leave no entry.

`pick` and `ask` share a single FIFO queue, yield cooperatively, and **pause the calling coroutine's timeout** while waiting. They raise `"interactive wait cannot run here"` in non-yieldable contexts and `"interactive UI is unavailable during reload or close"` when called from a hook during reload or session close.

## Control, Model, Docs, and Utilities

- `reload()`:
  - Performs the same config reload as `/reload` and JSON-RPC `config.reload` ([config.md](config.md#reloading-reload-configreload-reload)). In TUI, Lua-triggered `reload()` restyles the palette on the `reloaded` event but does not rebuild already-cached chat entries until resized or `/reload` is run.
  - Outside hooks: yields (pausing caller timeout) until reload finishes, returns `nil` on success, and raises a Lua error if the config is invalid (keeping the previous config active).
  - Inside hooks: returns `nil` immediately and schedules the reload right after that hook run finishes (since reload waits for `close` hooks); any failure emits an `error` event (`reload: ...`).
- `llm(prompt, model, effort)` -> `string`:
  - Sends a single stateless user-message request (no tools, no system prompt, no conversation history) through configured providers using `settings.retries` (capped at 5 retries).
  - `model` (`"provider/model"`) and `effort` default to the session's active values when `nil` or `""`. Raises an error on invalid `effort`, unset `model`, unknown provider, or request failure.
  - Yields without blocking other coroutines (does **not** pause caller timeout). Cancellation follows the calling job: Esc / `turn.cancel` cancels `llm()` inside a `lua` tool call, hook, or custom command, but does **not** cancel `llm()` inside a background task (use `task_stop`). HTTP cancellation is cooperative (checked between SSE chunks and retry sleeps, not an immediate socket abort).
  - If the provider reply includes usage data, records usage in the session and emits a `usage` event (priced using configured `models` prices; included in `session().usage.cost` and separated out in `session().usage.llm_cost`).
- `set_title(text)`:
  - Trims whitespace, replaces newlines with spaces, and truncates to 80 characters. Raises `"empty title"` if empty.
  - Saves the new session title and emits `name_change` (no-op if unchanged).
- `docs(topic)`:
  - Call `docs()` (or pass whitespace) to list the built-in embedded topics (`README.md`, `tool`, `functions`, `json`, `commands`, `env`, `config`, `prompts`, `plugins`, `hooks`, `theme`). Only these embedded topics are callable via `docs()`—other files under `docs/` (such as `lua.md` or `tasks.md`) are not embedded topics.
  - Call `docs("<topic>")` (case-insensitive, optional `.md` suffix) to read a page. When built-in functions are disabled via `functions = { name = false }`, `docs("functions")` and `docs("tool")` omit those disabled built-ins, and other embedded topics prepend a `Disabled in this session: ...` notice. Configured custom functions are never appended to `docs("functions")`.
  - Unknown topics return an error message followed by the topic list (does not raise a Lua error).
- `json.encode(value)` / `json.decode(text)`:
  - Encodes a Lua value to compact JSON or decodes JSON to a Lua value (JSON `null` becomes `nil`). Raises a Lua error on invalid JSON or unsupported Lua types.
- `inspect(value)`:
  - Formats any Lua value into a readable string: root strings (depth `0`) return raw while nested strings are quoted, tables list sequence items before sorted key-value pairs, `__tostring` metamethods are respected, cycles render as `<cycle>`, and tables at depth beyond `6` (root is depth `0`) elide as `{...}`.

## Functions from Config

`functions` maps a Lua identifier to either a plain Lua function or `false` (`nil` is equivalent to omitting the key):

```lua
local t = require("tuse")

return {
  systems = {
    "You have `websearch(query)` in `lua`: searches the web and returns the body.",
  },
  functions = {
    -- Custom agent-only function (undiscoverable unless described in systems/prompts)
    websearch = function(query)
      return t.sh('curl -fsSL "$URL"', { env = { URL = "https://example.com/?q=" .. query } }).out
    end,

    -- Override a built-in for the agent while delegating to the unmodified original
    sh = function(cmd, opts)
      opts = opts or {}
      local env = { CI = "1" }
      for k, v in pairs(opts.env or {}) do env[k] = v end
      return t.sh(cmd, { timeout = opts.timeout or 30, cwd = opts.cwd, env = env })
    end,

    -- Disable a built-in completely for the agent
    delete = false,
  },
}
```

Configured `functions` overrides and disables are capability-scoping and ergonomic controls for cooperative agents, not a hostile-code security sandbox (the agent still runs standard Lua 5.4 libraries such as `os.execute` and `io.open`).

- **Key rules**: must be a valid ASCII Lua identifier (`[_A-Za-z][_A-Za-z0-9]*`) and not a Lua keyword or Lua base/standard-library global (`_G`, `_VERSION`, `assert`, `collectgarbage`, `dofile`, `error`, `getmetatable`, `ipairs`, `load`, `loadfile`, `next`, `pairs`, `pcall`, `print`, `rawequal`, `rawget`, `rawlen`, `rawset`, `require`, `select`, `setmetatable`, `tonumber`, `tostring`, `type`, `warn`, `xpcall`, `coroutine`, `io`, `math`, `os`, `package`, `string`, `table`, `utf8`). All built-in host names (`sh`, `sleep`, `spawn`, `read`, `write`, `append`, `delete`, `edit`, `grep`, `docs`, `set_title`, `session`, `task_get`, `task_list`, `task_stop`, `show`, `pick`, `ask`, `reload`, `llm`, `json`, `inspect`) are allowed.
- **Value rules**:
  - Must be a Lua `function` or boolean `false`. Passing any other value (including the removed `{ run, description, args, show_agent }` table form) fails validation.
  - `json` is a module table (`{ encode, decode }`), not a function: `json = false` disables `json` for the agent, while `json = function(...) ... end` fails validation.
  - `false` is valid only on a built-in host name or a function name defined by an enabled plugin in the merged config; setting `false` on an unknown name fails validation.
- **Agent-only and undiscoverable by default**:
  - Configured functions are exposed **only** in the agent's `lua` environment. They are never visible in top-level config, plugins, trusted `require`d modules, hooks, custom `commands`, `open` handlers, other configured `functions`, or tasks spawned via `require("tuse").spawn`. Share code across trusted callbacks using `local` functions or `require`d helper modules.
  - Configured functions are **never** listed automatically in the system prompt or `docs("functions")`. Describe any custom agent function explicitly in `systems` or `prompts` if you want the agent to discover it.
- **Overriding built-ins**:
  - Setting any function-valued built-in in `functions` replaces both the global `<name>` and `require("tuse").<name>` in the agent environment.
  - Use `local t = require("tuse")` in trusted config/plugin code to call the unmodified original built-in without recursing (`t.sh(...)`, `t.write(...)`, etc.). Job-scoped side effects (diffs, model images, 120-second deadline, timeout pausing, Esc cancellation) remain bound to the active agent tool call.
  - Overrides never affect trusted code (config, plugins, hooks, commands, `open` handlers, or trusted tasks).
- **Disabling built-ins (`false`) and live-gated handles**:
  - Disabling a built-in sets both the global and `require("tuse").<name>` to `nil` in the agent environment (including `load(...)`, agent-`require`d `.lua` files, and agent-`spawn`ed tasks).
  - Every built-in function, `json.encode`/`json.decode`, and configured custom function exposed to the agent is wrapped in a live-dispatch proxy. Saved aliases captured before reload (`saved_sh = sh`, `saved_enc = json.encode`, `saved_tuse = require("tuse")`, `saved_fn = plugin_fn`) immediately observe reloads: disabled entries read `nil` on `saved_tuse` and raise `"<name> is disabled"` (or `"<name> is unavailable"` if a custom function was removed) when called; newly installed overrides take effect immediately on saved aliases too.
  - Disabling `inspect = false`, `json = false`, `sh = false`, etc. never breaks Tuse's private runtime helpers (such as return-value rendering with private `inspect`).
  - Disabled built-ins are automatically omitted from the system prompt, the `lua` tool description, `docs("functions")`, and `docs("tool")`, and other embedded `docs(topic)` pages prepend a `Disabled in this session: ...` notice. Overridden (not disabled) built-ins remain documented.
- **Mode filtering**: branch on `require("tuse").session().mode` (`"tui"` or `"json"`) when building the `functions` table.
- **Reload behavior**: on successful `/reload`, new/updated functions and overrides activate immediately (including through saved aliases), removed custom functions are unbound from the agent global environment unless the agent rebound that global to a different value (and saved handles raise `"<name> is unavailable"`), removed overrides or `false` disables restore original built-ins everywhere (globals, `require("tuse")`, saved aliases, prompt, tool description, and `docs()`), and agent scratch globals stay intact. On failed reload, nothing changes.
