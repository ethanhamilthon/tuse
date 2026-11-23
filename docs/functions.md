# Functions

Always defined in `lua`, in hooks and in tasks. Errors are Lua errors with a plain message:
catch them with `pcall`. Options are passed as a table, like `read(path, {offset = 40})`.

- `read(path, {offset=1, limit=2000})`: a text file with line numbers; long files come in pages
  (the end says which `offset` continues). An image (png, jpg, gif, webp) is shown to you with the
  result. A binary file gives a short message. Images over `settings.max_image_bytes` raise an error.
- `write(path, content)`: creates the file (and missing folders) or replaces it. The write is
  atomic and keeps the file's permissions.
- `edit(path, old, new, {all=false})`: replaces exact text. `old` must be found, and unique
  unless `all` is set; otherwise an error lists the lines of every match. A CRLF file matches
  `\n` in `old` and keeps its CRLF.
- Files changed by `write` and `edit` are shown to the user as a diff at the end of the call; you
  only get a short confirmation.
- `grep(pattern, {path=".", glob=nil, ignore_case=false, context=0, limit=100})`: searches file
  contents with a regex and returns `path:line: text` per match (`path-line- text` for context
  lines). It respects `.gitignore` and skips binary files and `.git`. `glob` is a file filter such
  as `*.rs`. A bad pattern raises an error.
- `sh(cmd, {timeout=nil, cwd=nil, env={}})`: `bash -c cmd`, stdout and stderr merged. Returns a
  table with `.out`, `.code` (`nil` when killed), `.ok` (`code == 0`), `.timed_out` and
  `:lines()`; `tostring(r)` is the output plus an exit note. `TUSE_SESSION`, `TUSE_CWD`,
  `TUSE_MODEL`, `TUSE_MODE` and `TUSE_CURRENT_CONFIG` are set. At its own timeout or the deadline of the call
  the process group is killed; Esc kills it too. While it runs, hooks and tasks go on.
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
- `set_title(text)` names the session; `docs(topic)`: call `docs()` to list documentation topics,
  or `docs("<topic>")` to read one.

## Functions from the config

```lua
return {
  functions = {
    tail_tests = {
      args = "lines",
      description = "Run tests and return the last lines of output.",
      run = function(lines)
        local out = sh("make check"):lines()
        return table.concat(out, "\n", math.max(1, #out - (lines or 30) + 1))
      end,
    },
  },
}
```

Each `run` becomes a global of the VM under the function's name. `args` is only the text shown
to the agent between the parentheses. `show_agent = false` (default `true`) hides a helper from
the agent: it is left out of the system prompt and of `docs("functions")`, but it stays defined,
so other helpers, hooks and tasks can call it. Use `TUSE_MODE` to choose which helpers to define.

`/reload` runs the config file again in the same VM: helpers are redefined and the agent's
globals stay. A deleted helper remains defined until exit.
