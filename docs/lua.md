# Lua

The agent's single `lua` tool, the persistent VM, every built-in function, background tasks, and hooks. The page ends with the exact prompt and tool description the model gets.

## The VM

Each session has one Lua 5.4 VM. It runs the agent's `lua` tool calls, your config, hooks, commands, `open` handlers, and background tasks, all as coroutines on one scheduler. Globals the agent creates last until the session closes. Resuming a session starts a fresh VM. Errors, timeouts, and cancels never kill the VM.

The VM has two environments:

- **Agent**: `lua` tool calls, the modules and files they load, and tasks they spawn. Built-ins are globals, together with the agent-only `functions` from the config. `require("tuse")` returns a read-only view of the same built-ins.
- **Trusted**: the config, its modules, hooks, commands, `open` handlers, and the bodies of configured `functions`. Built-ins are only reachable through `require("tuse")`, always as the originals. Agent-only functions are invisible here.

Each environment has its own `package.loaded`, so neither side can change the other's modules. A function keeps the environment it was defined in, including when it is passed across. Both sides get `string`, `table`, `math`, `utf8`, `io`, `os` (without `os.exit`), and `coroutine`. `debug` is not available.

## The `lua` tool

The model has exactly one tool, `lua`, which takes `{"code": "..."}`.

- Tuse first tries to run `return <code>`, so a lone expression returns its value. Otherwise it runs the code as statements.
- The result is everything printed with `print`/`io.write`, then each non-nil return value formatted with `inspect`. On error it is the message and a traceback. Output is never cut.
- Exit code: `0` on success, `1` on error or timeout, `null` when cancelled or handed off.
- Each call has a 120-second deadline. Waiting in `pick`, `ask`, or `reload` pauses the deadline. Tight loops are interrupted too.
- A file change by `write`, `append`, `edit`, or `delete` during a tool call adds a diff to the result, which the user sees. An image read with `read` is sent to the model on the next step. From hooks, tasks, and commands these functions still work, but they attach nothing.
- Unknown tools, bad JSON, or a missing `code` fail with code `1` and nothing runs.

## Functions

Errors are ordinary Lua errors (`pcall` catches them). Paths can be absolute, `~/…`, or relative to the session folder. Built-ins removed with `functions = { name = false }` are gone in the agent's environment.

### Files

- `read(path)` / `read(path, {lines=true, offset=1, limit=2000})`: Returns the full text by default. With `lines=true` it returns numbered lines, a page at a time, ending with `[lines a-b of n; continue with offset=…]`. For an image (`png`, `jpg`, `jpeg`, `gif`, `webp`, up to 5 MiB) it returns `[image: path, N KB]` and the model sees the image. Binary files return `[binary file, N bytes]`. Directories raise an error.
- `write(path, content)`: Creates or overwrites a file atomically, creating parent folders and keeping permissions. Returns `created …` or `overwrote … (a -> b lines)`.
- `append(path, text)`: Appends text, creating the file if needed.
- `delete(path)`: Deletes one file. It refuses directories.
- `edit(path, old, new, {all=false})`: Replaces exact text. `old` must occur exactly once unless `all=true`. If it is missing or appears more than once, it raises an error listing the matching lines. CRLF files keep their line endings.
- `grep(pattern, {path=".", glob=nil, ignore_case=false, literal=false, context=0, limit=100})`: Searches with a regex, or plain text when `literal=true`. It respects `.gitignore`, includes hidden files, and skips `.git` and binary files. Returns `path:line: text` lines, `[no matches]`, or a note when it stopped at `limit`. `path` can also be a list.

### Shell and tasks

- `sh(cmd, {timeout=nil, cwd=nil, env={}})`: Runs `bash -c cmd` with stdin closed, merging stdout and stderr. Returns `{out, code, ok, timed_out}` plus `:lines()`. `tostring(r)` adds `[exit code N]` or `[timed out]`. `code` is `nil` when the process was killed. `env` is added to the inherited environment. The command is killed (with its whole process group) on timeout, at the caller's deadline, or on cancel. Use long strings `sh([=[ … ]=])` for commands with quotes or backslashes.
- `spawn(fn, name)`: Runs `fn` as a background task with no deadline and returns its id (`t1`, `t2`, …).
- `task_get(id, lines=50)`: `{id, name, status, started, ended, output}`, where `output` is the end of the log.
- `task_stop(id)`: Stops a task.
- `task_list()`: Lists all tasks of the session.

### UI

- `show(name, text, pin)`: Shows or updates a Markdown block by name. `pin=true` keeps it at the bottom of the chat, and `show(name, "")` removes it. Blocks survive reload and resume and are never sent to the model.
- `pick(options)`: Shows a list of strings and returns the chosen one, or `nil` on Esc or an empty list.
- `ask(text)`: Shows a Markdown question and returns the typed answer, or `nil` on Esc. The answer is not sent to the model.

`pick` and `ask` share one queue. Answered prompts are saved in the chat history.

### Session and utilities

- `session()`: Returns a fresh snapshot (changing it does nothing) with `mode`, `configpath`, `cwd`, `home`, `id`, `name`, `model`, `provider`, `effort`, `ctx`, `ctx_size`, `usage` (`input`, `output`, `cache_read`, `cache_write`, `cost`, `llm_cost`), `model_info`, `steps`, `turns`, and `trace` (`{role, summary, text}` list). It never contains keys. During config evaluation only the first four fields are set.
- `llm(prompt, model, effort)`: Sends one prompt to a model, with no tools, history, or system prompt, and returns the text. `model` and `effort` default to the session's. It uses the same retries as turns, and its usage is counted as `llm_cost`.
- `reload()`: Reloads the config like `/reload` and raises an error if the config is invalid. Inside a hook, it runs after the hook returns.
- `set_title(text)`: Sets the session title (one line, up to 80 characters).
- `docs(topic)`: `docs()` lists the topics and `docs("lua")` reads one. These are the pages of this documentation.
- `json.encode(value)` / `json.decode(text)`: JSON, where `null` becomes `nil`.
- `inspect(value)`: Formats any value readably, the same formatting used for tool results.

`sh`, `pick`, `ask`, `llm`, and `reload` yield, so other jobs keep running while they wait, even inside your own coroutines. Where yielding is impossible (top-level config, or inside `string.gsub` callbacks), `sh` blocks and the others raise `interactive wait cannot run here`.

## Background tasks

```lua
local id = spawn(function()
  local r = sh("cargo test")
  print(r.out)
  if not r.ok then error("tests failed") end
  return "tests passed"
end, "tests")
```

- A task shares the VM and its globals. `print` writes to `~/.tuse/<session>/tasks/<id>.log`, and the return value or error is appended at the end.
- Status is `running`, `done`, `failed`, or `stopped`. `started` and `ended` are UTC timestamps, with `ended` empty while the task runs.
- When a task ends on its own, the agent gets a message, which starts a turn if the session is idle:
  ``<task>Task t1 (tests) finished. Last line: tests passed. Output: `task_get("t1").output`.</task>``
  The last line comes from the return value or the error, not from the log. Stopped tasks send no message.
- **Handoff.** If you steer or press Esc while a `lua` tool call runs, the whole call keeps running as a task named after its run, such as `lua-3`. Locals, a running `sh`, and the rest of the script all continue. The tool result becomes `[continued in background: tN]`. A call is handed off at most once.
- Stop tasks with `task_stop`, with `Ctrl+X` in `/tasks`, or with JSON-RPC `tasks.stop`. Esc and reload do not stop tasks, but closing or switching the session does.

## Hooks

Hooks are config functions that run on session events:

```lua
local t = require("tuse")
return {
  hooks = {
    notify = {
      events = { "agent_message" },
      run = function(e)
        if e.status == "finished" then t.sh([[osascript -e 'display notification "done"']]) end
      end,
    },
  },
}
```

| Event | When | Fields of `e` |
| --- | --- | --- |
| `start` | A session opened, or a reload committed. | `reason` (`launch`, `new`, `resume`, `reload`), `is_new`, `restored` |
| `close` | A session is closing, or a reload is about to replace the config. | `reason` (`exit`, `new`, `resume`, `reload`) |
| `user_message` | A turn takes a user message. | `text`, `is_first` |
| `agent_message` | A model step ended, or a turn was cancelled. | `text`, `status` (`working`, `finished`, `cancelled`), `tool_calls` |
| `model_change` | The model or effort changed. | `previous`, `new` (`{model, provider, effort}`) |
| `name_change` | The session title changed. | `previous`, `new` |
| `lua_start` | A tool call, command, task, or `open` handler started. | `id` (`lua-N`), `kind` (`tool`, `command`, `task`, `open`), `code`, `name` |
| `lua_end` | One of those ended. | `id`, `kind`, `status` (`success`, `error`, `cancelled`), `duration_ms`, `result`, `error` |

Call `t.session()` inside a hook to get session data. Each hook handles its events one at a time, in order, while different hooks run concurrently. Hooks have no deadline, and Esc cancels the running ones. An error shows up as the notice `hook <name>: <error>` and does not stop anything else. Every run is recorded in `/trace`. When a session closes, Tuse waits for all hooks to finish. Hook runs never fire `lua_start`/`lua_end`.

## What the agent sees

This is everything Tuse sends the model before the conversation, shown for the default config. Functions turned off with `functions = { name = false }` drop out of every line. Each string of `config.systems` follows the prompt, in order, and the compaction note is added once earlier records were dropped. Inside a session this section is rendered for that session's config.

### System prompt

```text
You are running inside Tuse, a coding agent.

# Tool
You have one tool, `lua`: it runs Lua 5.4 code in the session's persistent VM, so globals stay between calls (use `local` for scratch values). You get what the code prints plus the values it returns (a lone expression returns itself), or an error with a traceback. Nothing is cut, so keep big outputs short yourself. A call is stopped after 120 seconds; start a long job in the background with `spawn`.

The full Lua standard library is there (`string`, `table`, `math`, `io`, `os`, `utf8`, `coroutine`), plus `json.encode`/`json.decode` and `inspect(value)`. Lua patterns are not regexes: use `grep` or `sh` for those.

# Rules
- Prefer short, multi-step scripts over one big call. Create a file and extend it in small chunks instead of writing a 1000-line file in a single call.
- Prefer asynchronous work: start long jobs with `spawn` and keep working on anything that does not depend on them.
- If all that remains is waiting for a background task, briefly tell the user what is pending and end the turn. Do not poll or block waiting for it; task completion will wake the agent.

# Functions in `lua`
They are Lua globals, not tools: call them inside `lua` code. Errors are Lua errors (catch them with `pcall`). Prefer `read` for file contents, `grep` for search, and `write`/`edit`/`append`/`delete` for file changes so the user sees diffs. Use `sh` for running programs and processing their output, not as the default for file operations.
- `read(path)` / `read(path, {lines=true, offset=1, limit=2000})`: returns raw text by default, or an image (png, jpg, gif, webp) that you then see. Pass `{lines=true}` for numbered, paginated lines (ending with the next `offset`).
- `write(path, content)`: create a file or replace it completely; missing folders are created.
- `append(path, text)`: append text to a file (creating it if missing) and show the change as a diff.
- `delete(path)`: delete a single file and show its removal as a diff (does not delete directories).
- `edit(path, old, new, {all=false})`: replace exact text in a file. `old` must be unique unless `all` is set; read the file first. A wrong `old` raises an error.
- `grep(pattern, {path=".", glob=nil, ignore_case=false, literal=false, context=0, limit=100})`: search file contents with a regex (or plain text with `literal=true`); `path` can be a string or list of paths. Returns `path:line: text` per match and respects `.gitignore`.
- `sh(cmd, {timeout=nil, cwd=nil, env={}})`: runs `bash -c cmd` in the working directory; returns a table with `.out` (stdout and stderr merged), `.code` (`nil` when killed), `.ok`, `.timed_out` and `:lines()`. Use Lua long strings like `sh([=[grep -E "foo\\bar" f]=])` for commands with quotes, backslashes, or regexes so Lua does not interpret shell escapes. Returned as a value it shows its output and exit code. Shell state does not carry over between calls. A command is killed at its explicit timeout or the foreground Lua deadline, whichever comes first. Steering or cancelling the turn can move the whole Lua call to a background task, where the foreground deadline no longer applies.
- `spawn(fn, name)`: runs `fn` in the background as a task in this same VM (it sees your globals); returns its ID. A task has no deadline; its `print` goes to its log, and you get a message when it ends.
- `task_get(id, lines=50)`: return `{ id, name, status, started, ended, output }` with the log tail in `.output`.
- `task_stop(id)`: stop a running background task and return its task info.
- `task_list()`: list all tasks in the session with `.id`, `.name`, and `.status` (`running`, `done`, `failed`, `stopped`).
- `show(name, text, pin)`: render or update a UI-only Markdown block by name (`pin=true` keeps it at the end of the chat after new messages; `show(name, "", false)` removes it).
- `pick(options)`: show a list of strings to the user and return the selected string, or `nil` on Esc (pauses your timeout while waiting).
- `ask(text)`: show a Markdown question to the user and return their typed answer, or `nil` on Esc (pauses your timeout while waiting).
- `llm(prompt, model, effort)`: send one prompt (no tools, no history) to a model and return its text; `model` and `effort` default to the session's.
- `reload()`: reload the config like `/reload`; raises if it is invalid.
- `session()`: return a fresh snapshot of runtime session data (`mode`, `configpath`, `cwd`, `home`, `id`, `name`, `model`, `provider`, `effort`, `ctx`, `ctx_size`, `usage`, `model_info`, `steps`, `turns`, `trace`).
- `docs(topic)`: call `docs()` to list documentation topics, or `docs("<topic>")` to read one.
```

### Tool description

The one tool is `lua`. Its single required string argument `code` is described as "The Lua code to run.".

```text
Run Lua 5.4 code in the session's persistent VM (globals stay between calls). Returns what the code prints plus the values it returns (a lone expression returns itself), or an error with a traceback. Prefer read() for file contents, grep() for search, and write()/edit()/append()/delete() for file changes so the user sees diffs. Use sh(cmd) for running programs and processing their output, not as the default for file operations; use sh([=[…]=]) when a shell command has quotes, backslashes, or regexes.
```

### Compaction note

```text
Earlier parts of this conversation were compacted to save context: old tool calls and their outputs were removed and some later outputs were cut to their first line.
```
