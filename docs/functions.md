# Functions

Always defined in `python` and in hooks. Errors are Python exceptions.

- `read(path, offset=1, limit=2000)`: a text file with line numbers; long files come in pages
  (the end says which `offset` continues; a page never exceeds `max_output`). An image (png, jpg,
  gif, webp) is shown to you with the result. A binary file gives a short message, a directory
  lists its entries. Images over `settings.max_image_bytes` raise `ValueError`.
- `write(path, content)`: creates the file (and missing folders) or replaces it. The write is
  atomic and keeps the file's permissions.
- `edit(path, old, new, replace_all=False)`: replaces exact text. `old` must be found, and unique
  unless `replace_all` is set; otherwise `ValueError` (it lists the lines of every match). A CRLF
  file matches `\n` in `old` and keeps its CRLF.
- Files changed by `write` and `edit` are shown to the user as a diff at the end of the call; you
  only get a short confirmation.
- `grep(pattern, path=".", glob=None, ignore_case=False, context=0, limit=100)`: searches file
  contents with a regex and returns `path:line: text` per match (`path-line- text` for context
  lines). It respects `.gitignore` and skips binary files and `.git`. `glob` is a file filter such
  as `*.rs`. A bad pattern raises `ValueError`.
- `sh(cmd, timeout=None, cwd=None, env=None)`: `bash -c cmd`, stdout and stderr merged. Returns
  `.out`, `.code` (`None` when killed), `.ok` (`code == 0`), `.timed_out` and `.task`. `TUSE_SESSION`,
  `TUSE_CWD`, `TUSE_MODEL` and `TUSE_CURRENT_CONFIG` are set. **A command still running at its
  timeout (its own or the call's) is not killed**: it goes on as a background process task,
  `.task` is its ID and `.out` has what it printed so far (the rest is in `task_get(id).output`).
  Esc kills it, and so does a timeout inside a hook or a task.
Use `ask` for clarification, confirmation, or choices instead of asking in a text reply.

- `ask(question, options=[])`: asks the user and **waits** for the answer. The user picks an option
  or writes their own answer (the last row of the list), so the result is any text. To ask without
  waiting, call it from a task: `task_run('print(ask("Deploy?", ["yes", "no"]))')`.
Use `draw_ui` for diagrams, tables, dashboards and live progress in chat.

- `draw_ui(text, id=None)`: shows plain multiline text in the user's UI, without Markdown, and returns
  a block ID. `draw_ui(text, id=ID)` replaces that block, so a script can redraw its own interface in
  place. It raises `RuntimeError` in headless sessions.
- `task_run(code, name=None)`: starts a **background Python REPL** that runs `code` (a string). It
  is a new interpreter with the same functions and the filesystem, but it does not
  see your variables. Run a server with `task_run('sh("npm run dev")')`. `print()` and the value of
  the last expression go to the task log. The REPL stays alive when the code ends (`idle`); you get
  a message each time it goes idle, and when a process task exits. A task has no deadline.
- `task_add(id, code)`: queues one more snippet in the live REPL of a task. It runs after the
  current code, in the same interpreter (it sees the variables of the earlier code), in the order
  of the calls; an exception in one snippet does not stop the next. Its output goes to the same
  log after a `--- task_add #N` line. Returns its place in the queue. A process task has no REPL.
- `task_get(id, lines=50)` returns the task with the log tail in `.output`, `task_stop(id)` stops
  it (a REPL is cancelled and closed), `task_list()` lists them. A task has `.id`, `.name`, `.kind`
  (`repl` or `process`), `.code` (what it started with), `.status` (`running`, `idle`, `exited`,
  `stopped`), `.exit` (the exit code of a process), `.queue` and `.output`.
- `docs(topic=None)`: call `docs()` to list documentation topics, or `docs("<topic>")` to read one.

## Functions from the config

```yaml
functions:
  tail_tests:
    args: "lines=30"
    description: Run tests and return the last lines of output.
    run: return sh("make check").out.splitlines()[-lines:]
```

Helpers can call other helpers and import modules next to the config. `/reload`
redefines helpers in the agent's interpreter without clearing its variables.
A deleted helper remains callable in that interpreter until exit.
