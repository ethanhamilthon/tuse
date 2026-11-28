# Lua

The agent's single `lua` tool, the persistent VM, every built-in host group and function, background tasks, and hooks. The page ends with the exact prompt and tool description the model gets.

## The VM

Each session has one Lua 5.4 VM. It runs the agent's `lua` tool calls, your config, plugins, hooks, commands, `open` handlers, and background tasks, all as coroutines on one scheduler. Globals the agent creates last until the session closes. Resuming a session starts a fresh VM. Errors, timeouts, and cancels never kill the VM.

The VM has two environments:

- **Agent**: `lua` tool calls, the modules and files they load, and tasks they spawn. Built-in host groups and functions (`fs`, `tasks`, `session`, `ui`, `plugins`, `providers`, `sh`, `reload`, `llm`, `docs`, `json`) are globals, together with agent-only callbacks from config, exposed as `namespace.name`. `require("tuse")` returns a read-only view of the same built-ins.
- **Trusted**: the config, its modules, plugins, hooks, commands, `open` handlers, and the bodies of configured `functions`. Built-ins are only reachable through `require("tuse")`, always as the unmodified originals. Agent-only custom functions are invisible here.

Each environment has its own `package.loaded`, so neither side can change the other's modules. A function keeps the environment it was defined in, including when it is passed across. Both sides get `string`, `table`, `math`, `utf8`, `io`, `os` (without `os.exit`), and `coroutine`. `debug` is not available.

## The `lua` tool

The model has exactly one tool, `lua`, which takes `{"code": "..."}`.

- Tuse first tries to run `return <code>`, so a lone expression returns its value. Otherwise it runs the code as statements.
- The result is everything printed with `print`/`io.write`, then each non-nil return value formatted readably (tables sorted by key, strings quoted, shell outputs summarized). On error it is the message and a traceback. Output is never cut.
- Exit code: `0` on success, `1` on error or timeout, `null` when cancelled or handed off.
- Each call has a 120-second deadline. Waiting in `ui.pick`, `ui.ask`, or `reload` pauses the deadline. Tight loops are interrupted too.
- A file change by `fs.write`, `fs.append`, `fs.edit`, or `fs.delete` during a tool call adds a diff to the result, which the user sees. An image read with `fs.read` is sent to the model on the next step. From hooks, tasks, and commands these functions still work, but they attach nothing.
- Unknown tools, bad JSON, or a missing `code` fail with code `1` and nothing runs.

## Functions

Errors are ordinary Lua errors (`pcall` catches them). Paths can be absolute, `~/…`, or relative to the session folder (including `sh(..., { cwd = "~/..." })`).

All enabled host groups—including `session`, `ui`, `providers`, `plugins`, `reload`, and `llm`—are callable directly by both trusted callbacks and the agent without confirmation, even though the default agent prompt advertises only Lua and `fs`, `sh`, `tasks`, `docs`, and `json`.

### Files (`fs`)

- `fs.read(path)` / `fs.read(path, {lines=true, offset=1, limit=2000})`: Returns the full text by default. With `lines=true` it returns numbered lines, a page at a time, ending with `[lines a-b of n; continue with offset=…]`. For an image (`png`, `jpg`, `jpeg`, `gif`, `webp`, up to 5 MiB) it returns `[image: path, N KB]` and the model sees the image. Binary files return `[binary file, N bytes]`. Directories raise an error.
- `fs.write(path, content)`: Creates or overwrites a file atomically, creating parent folders and keeping permissions. Returns `created …` or `overwrote … (a -> b lines)`.
- `fs.append(path, text)`: Appends text, creating the file if needed.
- `fs.delete(path)`: Deletes one file. It refuses directories.
- `fs.edit(path, old, new, {all=false})`: Replaces exact text. `old` must occur exactly once unless `all=true`. If it is missing or appears more than once, it raises an error listing the matching lines. CRLF files keep their line endings.
- `fs.grep(pattern, {path=".", glob=nil, ignore_case=false, literal=false, context=0, limit=100})`: Searches with a regex, or plain text when `literal=true`. It respects `.gitignore`, includes hidden files, and skips `.git` and binary files. Returns `path:line: text` lines, `[no matches]`, or a note when it stopped at `limit`. `path` can also be a list.

### Shell and tasks (`sh`, `tasks`)

- `sh(cmd, {timeout=nil, cwd=nil, env={}})`: Runs `bash -c cmd` with stdin closed, merging stdout and stderr. `cwd` expands `~/…` and resolves relative paths against the session folder. Returns `{out, code, ok, timed_out}` plus `:lines()`. `tostring(r)` adds `[exit code N]` or `[timed out]`. `code` is `nil` when the process was killed. `env` is added to the inherited environment. The command is killed (with its whole process group) on timeout, at the caller's deadline, or on cancel. Use long strings `sh([=[ … ]=])` for commands with quotes or backslashes.
- `tasks.spawn(fn, name)`: Runs `fn` as a background task with no deadline and returns its id (`t1`, `t2`, …).
- `tasks.get(id, lines=50)`: `{id, name, status, started, ended, log, code, output}`, where `log` is the task log path, `code` is the source of the task's function, and `output` is the end of the log.
- `tasks.stop(id)`: Stops a task and returns `{id, name, status, started, ended, log, code}`.
- `tasks.list()`: Lists all tasks of the session, each with `{id, name, status, started, ended, log, code}`.

### UI (`ui`)

- `ui.show(name, text, pin)`: Shows or updates a Markdown block by name. `pin=true` keeps it at the bottom of the chat, and `ui.show(name, "")` removes it. Blocks survive reload and resume and are never sent to the model.
- `ui.pick(options, title)`: Shows a list of strings under an optional Markdown question and returns the chosen string, or `nil` on Esc or an empty list. For example, `ui.pick({"Retry", "Skip"}, "**Build failed.** What next?")`. A blank title is omitted.
- `ui.ask(text)`: Shows a Markdown question and returns the typed answer, or `nil` on Esc. The answer is not sent to the model.

`ui.pick` and `ui.ask` share one queue. Answered prompts are saved in the chat history with the question and answer. An untitled picker is recorded as `Picker`; cancelled prompts are not recorded.

### Session (`session`)

- `session.get()`: Returns a fresh snapshot (changing it does nothing) matching JSON-RPC `session.state` (`mode`, `config_path`, `cwd`, `home`, `id`, `started`, `created`, `title`, `model`, `provider`, `effort`, `busy`, `turn`, `context`, `context_window`, `usage` (`input`, `output`, `cache_read`, `cache_write`, `cost`, `llm_cost`), `model_info` (including resolved `efforts` and `available`), `requests`, `llm_requests`, `steps`, `turns`, `calls`) plus `history` (structured records; see [History](session.md#history-and-session-data)). It never contains credentials. During config evaluation only `mode`, `config_path`, `cwd`, and `home` are set.
- `session.set{title, model, effort, history}`: Pass any subset of these keys. The title is one line, up to 80 characters. Model and effort must match a saved provider and the model's resolved efforts (empty effort `""` for `efforts = {}`); when changing `model` with `effort` omitted, Tuse keeps the current effort if valid for the new model and otherwise chooses that model's fallback effort (`""` for `efforts = {}`, else `medium` or the first declared effort). History replaces the whole list; use `session.get().history` as a starting point. Each tool call must have exactly one result before the next user or agent record. Images must be data URLs, and a `system` record, if present, must come first. Replacing history with a `user` record marks the session started. Invalid input (including replacing history in a way that invalidates a pending `session.fork(position)`) raises without changing anything. A replacement requested during tool calls waits until the step's final result is recorded, preserving any `meta` records created in that step. A no-op `session.set` does not fire `session_updated`. Usage totals and `context` are unaffected.
- `session.fork(position)`: Opens a new session forked at the `user` or `agent` record at `position` of `session.get().history` (counting from 1), like `/fork`: the new history ends with an agent record, or just before a user record, whose text goes into the input. It raises on a position that is out of range or not a `user` or `agent` record, and when another switch is already pending. A switch requested during tool calls waits until the step's final result is recorded, after a pending `session.set` history, and then the turn ends in the old session. Queued messages stay with the old session. The old session closes like with `/new`, and the TUI and JSON-RPC (`session_switched` event) show the new one.
- `session.resume(id)`: Switches to another started session `id` belonging to the current folder, like `/resume`, with the same timing and errors as `session.fork` (rejecting the current session, unstarted sessions, or sessions from another folder).

### Providers (`providers`)

- `providers.list()`: Returns `[{ name, type, base }]` for saved providers in SQLite (never keys or headers).
- `providers.connect({ name, api?, base, key, headers?, replace? })`: Validates, probes `GET <base>/models`, saves the profile, and returns `{ name, type, base, models }`. Supports `$ENV_VAR` references in `key` and `headers`.
- `providers.update(name, { api?, base?, key?, headers? })`: Patches omitted-preserved fields, probes before commit, and returns `{ name, type, base, models }`.
- `providers.disconnect(name)`: Deletes the named profile and returns `{ name }`. If the disconnected provider backs the selected model, the selection stays marked unavailable and cached discovered models for that provider are pruned.

### Plugins (`plugins`)

- `plugins.list()`: Lists installed plugins under `$TUSE_HOME/plugins/` as `[{ name, version, entrypoint, path, source }]`.
- `plugins.get(target)`: Returns `{ name, version, entrypoint, path, source, manifest }` for an installed plugin without executing it.
- `plugins.load(name)` (trusted Lua only): Executes the installed plugin's entrypoint anew and returns its Lua export directly. Takes the installed name, not a repository source or path. Does not register a namespace.
- `plugins.add(source, opts?)`: Downloads, validates `tuse-plugin.json`, and installs a plugin (`owner/repo[/subdir][@ref]` or a GitHub repository or tree URL). Optional `opts` accepts `{ replace? }` (boolean, default `false`). Returns `{ name, version, entrypoint, path, source, replaced }`.
- `plugins.delete(target)`: Removes an installed plugin and returns `{ name }`.
- `plugins.update(target?)`: Updates one installed plugin or all installed plugins from their recorded sources. Returns `{ updated = [{ name, version, entrypoint, path, source }] }`.

Plugin mutations update files on disk and `plugins.list()` / `plugins.get()` immediately, including under `--no-plugins`. Running session contributions change only after an explicit successful `reload()` (or in a new session), never implicitly.

### Utilities (`llm`, `reload`, `docs`, `json`)

- `llm(prompt, model, effort)`: Sends one prompt to a model, with no tools, history, or system prompt, and returns the text. When `model` is omitted or matches the session's current model and `effort` is omitted, it inherits the session's model and effort. When switching to another model with `effort` omitted, it sends no reasoning parameter if that model declares `efforts = {}`, and otherwise uses `low` (raising if that model's declared `efforts` exclude `low`). Explicit efforts are validated against the target model's resolved efforts. It uses the same retries as turns, and its usage is counted as `llm_cost`.
- `reload()`: Reloads the entire configuration and eligible plugins, preserving launch exclusion flags. Takes no arguments. Raises on failure outside a hook; inside a hook it runs after the hook finishes and reports failures to the session.
- `docs`: Read-only documentation object. `docs.topics` returns the ordered list `{"readme", "usage", "connect", "config", "lua", "session", "json", "examples"}`, and each named field (`docs.readme`, `docs.usage`, `docs.connect`, `docs.config`, `docs.lua`, `docs.session`, `docs.json`, `docs.examples`) returns that page as text.
- `json.encode(value)` / `json.decode(text)`: JSON encoding and decoding, where `null` becomes `nil`.

`sh`, `ui.pick`, `ui.ask`, `llm`, `reload`, `providers.connect`, `providers.update`, `plugins.add`, and `plugins.update` yield, so other jobs keep running while they wait, even inside your own coroutines. Where yielding is impossible (top-level config, or inside `string.gsub` callbacks), `sh` blocks and interactive waits raise `interactive wait cannot run here`.

## Background tasks

```lua
local id = tasks.spawn(function()
  local r = sh("cargo test")
  print(r.out)
  if not r.ok then error("tests failed") end
  return "tests passed"
end, "tests")
```

- A task shares the VM and its globals. `print` writes to `$TUSE_HOME/<session>/tasks/<id>.log` (default root `~/.tuse`), and the return value or error is appended at the end.
- Status is `running`, `done`, `failed`, or `stopped`. `started` and `ended` are UTC timestamps, with `ended` empty while the task runs.
- When a task ends on its own, Tuse stores one `task` outcome. A running turn takes the notice at its next step (`steer`); an idle session starts a turn (`wake`). Only delivery adds a `task_message` history record and fires the corresponding hook:
  ``<task>Task t1 (tests) finished. Last line: tests passed. Output: `tasks.get("t1").output`.</task>``
  The last line comes from the return value or the error, not from the log; succeeded tasks with no return text omit `Last line:`. Stopped tasks have an outcome but send no message. Cancelling a turn before a queued notice is delivered keeps the outcome without emitting `task_message`.
- **Handoff.** If you steer while a `lua` tool call runs, the whole call keeps running as a task named after its run, such as `lua-3`. Locals, a running `sh`, and the rest of the script all continue. The tool result becomes `[continued in background: tN]`. A call is handed off at most once.
- Stop tasks with `tasks.stop`, with `Ctrl+X`/`Cmd+X` in `/tasks`, or with JSON-RPC `tasks.stop`. Esc, `/stop`, `turn.cancel`, and closing or switching the session stop all of them. Reload does not.

## Hooks

Hooks are per-event config functions under `hook`:

```lua
local t = require("tuse")
return {
  alerts = {
    hook = {
      agent_message = function(e)
        if e.status == "finished" then
          t.sh([[osascript -e 'display notification "done"']])
        end
      end,
    },
  },
}
```

Every `e` has `event`, the event name, plus the fields below.

| Event | When | Fields of `e` |
| --- | --- | --- |
| `config_load` | After a successful config load and system prompt rebuild. | None |
| `process_start` | Once on launch, including JSON mode. | None |
| `process_end` | Once on quit or RPC `shutdown`. | None |
| `session_created` | A new session is opened. | None |
| `session_resumed` | A saved session is opened. | None |
| `session_close` | Before switching sessions, not on quit. | None |
| `session_updated` | After a `session.set` call that changed at least one field. | `changed` (list of keys) |
| `user_message` | A user message was recorded. | None |
| `agent_message` | An agent reply was recorded. | `status` (`working`, `finished`, or `cancelled`) |
| `turn_error` | A turn failed. | `error` |
| `turn_stopped` | A turn was stopped. | None |
| `ui_prompt` | When the `ui.pick`/`ui.ask` queue goes from empty to nonempty or back; replayed after reload if a prompt is still open. Switching directly to another queued prompt does not fire this hook. | `active` (boolean) |
| `task_message` | A completed task notice is delivered to the agent. | `id`, `name`, `status`, `delivery` (`wake` or `steer`), `text` |

Launch runs `config_load`, `process_start`, and then `session_created` or `session_resumed`. Switching runs `session_close` in the old VM, then `config_load` and the new session event in the new VM. Call `t.session.get()` inside a hook to get session data. Hooks run in the background and nothing waits for them. Each listener handles its events one at a time, in order, while different listeners run concurrently. Hooks have no deadline, and Esc cancels the running ones. An error shows up as the notice `hook <name>: <error>` and does not stop anything else. Runs are not recorded in history. When a session closes, Tuse waits for all hooks to finish.

## What the agent sees

This is everything Tuse sends the model before the conversation, shown for the default config. A string `systems` replaces the entire system message; a named `systems` table lets `lua` replace the built-in section and appends other keys in sorted order. The prompt is stored as the first `system` history record and rebuilt on session creation, resume, and config load.

### System prompt

```text
You are running inside Tuse, a coding agent.

# Tool
You have one tool, `lua`: it runs Lua 5.4 code in the session's persistent VM, so globals stay between calls (use `local` for scratch values). You get what the code prints plus the values it returns (a lone expression returns itself), or an error with a traceback. Nothing is cut, so keep big outputs short yourself. A call is stopped after 120 seconds; start a long job in the background with `tasks.spawn`.

The full Lua standard library is there (`string`, `table`, `math`, `io`, `os`, `utf8`, `coroutine`), plus `json.encode`/`json.decode`. Lua patterns are not regexes: use `fs.grep` or `sh` for those.

# Rules
- Prefer short, multi-step scripts over one big call. Create a file and extend it in small chunks instead of writing a 1000-line file in a single call.
- Prefer asynchronous work: start long jobs with `tasks.spawn` and keep working on anything that does not depend on them.
- If all that remains is waiting for a background task, briefly tell the user what is pending and end the turn. Do not poll or block waiting for it; task completion will wake the agent.
- Read `docs.topics` and `docs.lua` when you need Tuse APIs or documentation; never guess from memory.

# Functions in `lua`
They are Lua globals, not tools: call them inside `lua` code. Errors are Lua errors (catch them with `pcall`). Prefer `fs.read` for file contents, `fs.grep` for search, and `fs.write`/`fs.edit`/`fs.append`/`fs.delete` for file changes so the user sees diffs. Use `sh` for running programs and processing their output, not as the default for file operations.
- `fs.read(path)` / `fs.read(path, {lines=true, offset=1, limit=2000})`: returns raw text by default, or an image (png, jpg, gif, webp) that you then see. Pass `{lines=true}` for numbered, paginated lines (ending with the next `offset`).
- `fs.write(path, content)`: create a file or replace it completely; missing folders are created.
- `fs.append(path, text)`: append text to a file (creating it if missing) and show the change as a diff.
- `fs.delete(path)`: delete a single file and show its removal as a diff (does not delete directories).
- `fs.edit(path, old, new, {all=false})`: replace exact text in a file. `old` must be unique unless `all` is set; read the file first. A wrong `old` raises an error.
- `fs.grep(pattern, {path=".", glob=nil, ignore_case=false, literal=false, context=0, limit=100})`: search file contents with a regex (or plain text with `literal=true`); `path` can be a string or list of paths. Returns `path:line: text` per match and respects `.gitignore`.
- `sh(cmd, {timeout=nil, cwd=nil, env={}})`: runs `bash -c cmd` in the working directory; returns a table with `.out` (stdout and stderr merged), `.code` (`nil` when killed), `.ok`, `.timed_out` and `:lines()`. Use Lua long strings like `sh([=[grep -E "foo\\bar" f]=])` for commands with quotes, backslashes, or regexes so Lua does not interpret shell escapes. Returned as a value it shows its output and exit code. Shell state does not carry over between calls. A command is killed at its explicit timeout or the foreground Lua deadline, whichever comes first. Steering the turn can move the whole Lua call to a background task, where the foreground deadline no longer applies.
- `tasks.spawn(fn, name)`: runs `fn` in the background as a task in this same VM (it sees your globals); returns its ID. A task has no deadline; its `print` goes to its log, and you get a message when it ends.
- `tasks.get(id, lines=50)`: return `{ id, name, status, started, ended, log, code, output }` with the log tail in `.output`.
- `tasks.stop(id)`: stop a running background task and return its task info (`{ id, name, status, started, ended, log, code }`).
- `tasks.list()`: list all tasks in the session with `{ id, name, status, started, ended, log, code }` (`status`: `running`, `done`, `failed`, `stopped`).
- `docs`: object with `docs.topics` (ordered topic names) and topic fields such as `docs.lua`, `docs.config`, `docs.usage`, and `docs.examples`.
```

### Tool description

The one tool is `lua`. Its single required string argument `code` is described as "The Lua code to run.".

```text
Run Lua 5.4 code in the session's persistent VM (globals stay between calls). Returns what the code prints plus the values it returns (a lone expression returns itself), or an error with a traceback. Prefer fs.read() for file contents, fs.grep() for search, and fs.write()/fs.edit()/fs.append()/fs.delete() for file changes so the user sees diffs. Use sh(cmd) for running programs and processing their output, not as the default for file operations; use sh([=[…]=]) when a shell command has quotes, backslashes, or regexes.
```
