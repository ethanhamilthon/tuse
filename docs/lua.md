# Lua Runtime

Tuse embeds a Lua 5.4 VM per session. The agent's single tool (`lua`), config files, plugins, hooks, custom slash commands, local file openers, and background tasks all share this VM through a cooperative coroutine scheduler.

## VM Lifecycle

- **One VM per session**: created on a dedicated thread when a session opens, initialized with built-in host functions and the resolved config, and destroyed when the session closes (process exit, `/new`, `/resume`, `session.new`, or `session.resume`).
- **Persistence**: globals created by the agent persist across `lua(code)` tool calls for the lifetime of the session, and are shared with background tasks spawned in that VM. Resuming or switching sessions starts a fresh VM (agent globals are not persisted; config and plugins are re-evaluated).
- **Resilience**: Lua syntax errors, runtime errors, timeouts, and cancellations never terminate the VM.

## Execution Environments

The VM maintains two strictly isolated Lua environments:

### Agent Environment (`lua` tool)

- Covers `lua(code)` tool calls, modules `require`d from `lua(code)`, `load`/`loadfile`/`dofile` called from `lua(code)`, and background tasks `spawn`ed by `lua(code)`.
- Includes separate copies of the safe Lua 5.4 standard libraries (`string`, `table`, `math`, `utf8`, `io`, `os`, `coroutine`) plus isolated agent-scoped `package`, `require`, `load`, `loadfile`, and `dofile`. Shared metatables (`string`, `FILE*`, `sh`) are protected so agent mutations cannot affect trusted code or private runtime helpers. The `debug` library is unavailable (`nil`), and `os.exit` is set to `nil`.
- `package.path` and `package.cpath` are `""`; `require` uses Tuse's path-relative resolver ([config.md](config.md#modules-and-require)) against the agent's own `package.loaded` and `package.preload`, loading files into the agent environment.
- Live-gated built-in host functions (subject to overrides and `false` disables in `functions`) and live-gated configured custom functions are installed as globals; the agent's own persistent globals live in this table across calls.
- `require("tuse")` is reserved (always returning the live, genuinely read-only agent-scoped proxy regardless of `package.loaded.tuse`; also installed initially as `package.loaded.tuse`). Lookups dynamically read active built-in capabilities (overrides applied, disabled built-ins returning `nil`, custom non-built-in configured functions not exposed on `tuse`).

### Trusted Environment (Configs, Plugins, Trusted Modules, Callbacks)

- Covers config files, plugins, modules `require`d by trusted code, hooks, custom `commands`, `open` handlers, configured `functions` bodies themselves, and background tasks spawned via `require("tuse").spawn`.
- Has its own isolated `_G`/`_ENV`, `package`, `package.loaded`, `package.preload`, `require`, `load`, `loadfile`, `dofile`, and `tuse`. Agent code cannot inspect or mutate trusted `package.loaded`, cannot share module cache entries with trusted code, and cannot obtain the raw trusted `tuse` table.
- Built-in host function names are **not** globals; code imports the unmodified original built-ins via `require("tuse")`.
- Configured `functions` are **never** visible here — not as globals, not on `require("tuse")`, and not to other configured functions. Share helper code via `local` functions or `require`d helper modules.

### Callback Provenance and `spawn` vs `t.spawn`

- Every Lua function keeps the lexical `_ENV` where it was defined:
  - Passing an agent callback `cb` into a configured wrapper `wrap(cb)` runs `wrap` with trusted `_ENV` while `cb` retains agent `_ENV` (cannot see trusted `require("tuse")` or call disabled built-ins).
  - Returning a trusted closure from a configured wrapper retains its trusted `_ENV`.
- A task started via the agent's `spawn(fn, name)` runs in the agent job context; a task started via trusted `t.spawn(fn, name)` runs in the trusted job context. In both cases, `fn` retains its lexical `_ENV` (so `t.spawn(cb)` on an agent closure still runs `cb` in agent `_ENV`).

### Environment Variables (`os.getenv`)

- `os.getenv("THIS_PATH")` inspects the Lua call stack for the nearest file-loaded Lua frame and returns its path (resolved against `cwd` if relative), or `nil` for inline `lua` tool code.
- All other `os.getenv(name)` calls read the ordinary process environment unchanged. Tuse does not inject `TUSE_*` variables into Lua or `sh`.

## Coroutine Scheduler & Cooperative Yielding

Every executable Lua job runs as a Lua coroutine inside one scheduler on the VM thread (polling approximately every 5 ms while any job is active).

- **Yielding waits**: `sh()`, `sleep()`, `pick()`, `ask()`, `llm()`, and `reload()` yield the calling coroutine to the scheduler so other coroutines (background tasks, hooks, commands, tool calls) keep making progress concurrently.
- **User coroutines**: `coroutine.resume` and `coroutine.wrap` are wrapped so scheduler yields pass transparently through user-created coroutines; calling `sh()` or `sleep()` inside `coroutine.wrap(...)` works without blocking other jobs.
- **Non-yieldable contexts** (top-level config evaluation or inside a C callback such as `string.gsub`):
  - `sh()` and `sleep()` fall back to blocking the VM thread while still honoring timeouts and cancellation.
  - `pick()`, `ask()`, `llm()`, and `reload()` cannot block and raise `"interactive wait cannot run here"`.
- **Timeout pausing**: while a coroutine is yielded inside `pick()`, `ask()`, or `reload()`, its remaining deadline budget is paused and resumed once the wait finishes. `sh()`, `sleep()`, and `llm()` do not pause the deadline.
- **CPU loop interruption**: a VM instruction hook checks cancellation and deadlines every 1,000 instructions, so tight loops (`while true do end`) still terminate promptly on timeout or Esc / `turn.cancel`.
- **Wait cancellation**: cancelling or timing out a yielded job kills any running `sh()` child process group (`SIGKILL`), removes and clears a pending `pick()`/`ask()` request, signals cancellation to an in-flight `llm()` request (checked cooperatively between streamed SSE lines and retry sleeps, not an immediate socket abort—blocking reads and non-SSE JSON responses finish before noticing cancellation), and wakes the coroutine with `"cancelled"` or `"timed out"`.

## Job Kinds, Deadlines, and Cancellation

| Kind | Trigger | Deadline | Output sink | Error handling | Stopped by | Emits `lua_start` / `lua_end` |
| --- | --- | --- | --- | --- | --- | --- |
| `tool` | Model calls `lua(code)` with valid JSON and string `code` | 120 s (paused in `pick`/`ask`/`reload`) | Returned to model as tool output | Returned to model (`code = 1`, or `nil` if cancelled) | Esc / `turn.cancel`, session close | Yes (`code` set) |
| `task` | `spawn(fn, name?)` | None | Appended to task log file | Logged + agent notification | `task_stop`, `/tasks` Ctrl+X, `tasks.stop`, session close | Yes (`name` set), unless spawned in `lua_start`/`lua_end` hook |
| `command` | `/name` or `commands.run` | None | Buffered (not shown in chat) | Emits `error` event `/<name>: <error>` (unless cancelled) | Esc / `turn.cancel`, session close | Yes (`name` set) |
| `open` | Clicking a matching local path in TUI | None | Buffered (not shown in chat) | Emits `error` event `open.<ext>: <error>` (unless cancelled) | Esc / `turn.cancel`, session close | Yes (`name` = configured ext) |
| `hook` | Lifecycle event | None | Buffered (not shown in chat) | Emits `notice` event `hook <name>: <error>` + trace entry | Esc / `turn.cancel` (cancels active run in each lane; queued runs proceed); `close` hooks finish on close/reload | Never |

## The `lua` Tool Contract

The model is given a single tool named `lua` with one required string parameter `code`.

- **Unknown tools or bad arguments**: calling any tool name other than `lua` fails with exit code `1` and an error telling the model to call functions inside `lua`. Invalid JSON arguments or a missing/non-string `code` also fail with exit code `1`. None of these start a Lua job or emit `lua_start`/`lua_end`.
- **Expression convenience**: Tuse first tries to compile `return <code>` so a lone expression (such as `x + 1` or `{a = 1}`) returns its value directly; if that is not a valid expression, it compiles `<code>` as statements.
- **Output & return rendering**:
  - `print(...)` converts arguments with `tostring`, joins them with `\t`, appends `\n`, and writes to the job's output sink (or `stderr` if called outside any job, such as during config load).
  - `io.write(...)` converts arguments with `tostring`, concatenates them with no separator, and writes to the same sink.
  - Non-`nil` return values are formatted with `inspect(value)`, joined with `\t`, and appended after printed output (adding a separating newline if needed). If all return values are `nil`, nothing is appended.
  - On error, the error message and stack traceback are appended (or `[cancelled]` / `[timed out]` when stopped by cancellation/deadline).
  - Tool output is **never truncated** by the runtime.
- **Side-channel attachments (agent `tool` jobs only)**:
  - Every `write()`, `append()`, `edit()`, and `delete()` during an agent `lua` tool call appends a `(path, diff)` entry to the tool result's `diffs` list (displayed in TUI and included in `tool_end` and durable records; the Lua function returns only a confirmation string).
  - Every `read()` of an image during an agent `lua` tool call appends a base64 `data:<mime>;base64,...` URL to the tool result's `images` list so the model receives the image on the next step.
  - Calls to `read`/`write`/`append`/`edit`/`delete` from hooks, commands, tasks, or `open` handlers perform the filesystem operation normally, but do not attach chat diffs or model images.
- **Result exit code**: `0` on normal completion, `nil` (`null` in JSON) when cancelled, `1` on runtime/syntax/timeout/argument error.

## Custom Slash Commands (`commands`)

Configured as `commands = { name = function() ... end }`:

- Names appear in TUI `/` autocomplete and JSON-RPC `commands.list`.
- Invoked with `/name` in TUI or `commands.run { name = "name" }` in JSON-RPC. Commands take no arguments.
- Execute asynchronously as non-blocking Lua jobs with no deadline; multiple commands can run concurrently.
- May call `show`, `pick`, `ask`, `sh`, `llm`, `reload`, `spawn`, and all other host functions via `require("tuse")`.
- Output from `print()` is not shown in chat (though `lua_end` hooks receive it in `result`); use `show()` to display content in the UI.
- Cancelled by Esc / `turn.cancel` and session close; unaffected by `/reload` (though invoking the command afterward runs the updated function).

## Local File Openers (`open`)

Configured as `open = { ext = function(path) ... end }` (TUI-only):

- Triggered when clicking a linkified local file path in TUI (agent messages, `show` blocks, `/config`, `/session`, and `system`/`user`/`agent` entries in `/trace`).
- The clicked path is resolved against the session `cwd` and canonicalized. If the file does not exist, Tuse emits an `error` event (`cannot open <path>: file not found`).
- The file extension is matched case-insensitively against configured `open` keys.
- If a handler matches, it runs as a non-blocking Lua job with `(canonical_path)` and no deadline (cancelled by Esc / `turn.cancel` and session close). Errors emit `open.<ext>: <message>`.
- If no handler matches, Tuse falls back to the OS default opener (`open` on macOS, `xdg-open` on Linux, `cmd /C start` on Windows).
- Clicked URLs (`http://`, `https://`, `mailto:` under 2,000 characters) always use the OS opener and never invoke `open.<ext>`; unsupported URL schemes emit `cannot open URL \`...\``.
