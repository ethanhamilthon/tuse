# Tuse for the agent

You run inside Tuse. This page is what you need to work with it. Read it with `docs()` in the
`python` tool.

## The tool

You have one tool, `python(code, title, timeout)`: a persistent REPL. You get the output of
`print()` plus the value of the last expression (a `str` as it is, `None` nothing, a `sh` result as
its output and exit code), or a traceback. An error does not end the REPL; a timeout or a cancel
does not either. The REPL lives until Tuse exits and starts empty after `/resume`.

`title` is a few words about what the call does, in the user's language (`Running the tests`). The
user sees it in the working indicator while the call runs. While a function below works on a file
the indicator shows `reading PATH`, `writing PATH`, `editing PATH`, `searching PATTERN` or
`running CMD` instead; the last status stays until a new one comes.

A call is stopped after `settings.tool_timeout` seconds (pass `timeout` for more). The time the
user takes to answer an `ask` does not count. Output over `settings.max_output` bytes is cut in
the middle.

The REPL runs a Python subset ([Monty](https://github.com/pydantic/monty)): no `subprocess`, no
network, no pip packages, but `pathlib`, `open`, `os`, `json`, `re`, `datetime`, `hashlib`, `csv`,
`collections`, `itertools`, `functools`, `math`, `random`, `base64`, `tomllib` and `dataclasses`
work. The whole filesystem is readable and writable. A `.py` file in the config folder (next to
`main.yaml`) can be imported. Paths are absolute, `~/...` or relative to the working directory.

## Functions

Always defined in `python` and in hooks. Errors are Python exceptions.

- `read(path, offset=1, limit=2000)`: a text file with line numbers; long files come in pages
  (the end says which `offset` continues; a page never exceeds `max_output`). An image (png, jpg,
  gif, webp) is shown to you with the result. A binary file gives a short message, a directory
  raises `IsADirectoryError`. Images over `settings.max_image_bytes` raise `ValueError`.
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
- `ask(question, options=[])`: asks the user and **waits** for the answer. The user picks an option
  or writes their own answer (the last row of the list), so the result is any text. To ask without
  waiting, call it from a task: `task_run('print(ask("Deploy?", ["yes", "no"]))')`.
- `show(text, id=None)`: shows plain multiline text in the user's UI, without Markdown, and returns
  a block ID. `show(text, id=ID)` replaces that block, so a script can redraw its own interface in
  place. It raises `RuntimeError` in headless sessions.
- `task_run(code, name=None)`: starts a **background Python REPL** that runs `code` (a string). It
  is a new interpreter with the same functions, the `prelude` and the filesystem, but it does not
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
- `docs()`: this page and the config reference.

## Commands for the user

- `tuse [--config PATH] [--resume ID]` opens the TUI.
- `tuse -p [PROMPT|-]` runs one prompt headless: replies go to stdout, the session ID to stderr,
  and the exit code is 1 after an error. Background tasks are stopped at the end.
- `tuse connect` adds a provider.
- `tuse reset [--yes]` replaces the global config folder with the latest `main.yaml`. It deletes
  everything in that folder.
- `tuse update` updates the binary; `main.yaml` is not touched.

## User commands

When the user sends a message in the TUI during a turn, it stays dimmed in the input until
the agent takes it. Editing the waiting message withdraws it without stopping the turn;
Enter sends the edited draft.

`/new`, `/resume`, `/models`, `/theme`, `/tasks`, `/usage`, `/trace`, `/reload`, `/quit`.
Every list (`/resume`, `/models`, `/theme`, `/tasks`, `/trace`, an `ask`, and the `/`, `#` and `@`
pickers) has one look: a rule on top, a title with the position, the rows, all drawn over the
bottom of the chat. Typing filters (in an `ask` it is the answer). `/usage` and a task's output
open as text in the same frame (Esc closes it). `/trace` is a list, one row per item (`system`,
`user`, `agent`, `thinking`, `python`, `output`, `notice`, `error`); Enter opens the item in full
(with the diffs of the call), Esc goes back to the list.

The chat scrolls with the mouse wheel only; the arrows move in the input, and a click in the input
puts the cursor there. With the mouse captured the terminal's own text selection needs a modifier
(Shift, or Option in iTerm2 and Terminal.app).

A session keeps its own model and effort. `/models` changes them in the config and in the
current session; a new session takes them from the config; `/resume` and `/reload` do not
change the model of an old session.

## Environment

| Variable | Meaning |
| --- | --- |
| `TUSE_SESSION` | The current session ID (set in `sh`, tasks and `os.environ`). |
| `TUSE_GLOBAL_CONFIG` | Path of the global config file. Default `~/.config/tuse/main.yaml`. |
| `TUSE_HOME` | Where sessions live. Default `~/.tuse`. |
| `TUSE_RELEASE_URL` | Where `tuse reset` and `tuse update` download from. Default: the latest GitHub release. |
| `TUSE_CURRENT_CONFIG` | Path of the config file in use (`--config`, `./tuse.yaml` or the global one); not set without a file. Tuse does not read it. |
| `TUSE_CWD`, `TUSE_MODEL` | The working directory and the model of the session. |

## Config lookup

Tuse reads one config file:

1. `--config PATH`, if given.
2. `./tuse.yaml` in the working directory.
3. `$TUSE_GLOBAL_CONFIG`, else `~/.config/tuse/main.yaml`.

The system prompt (this tool and its functions, the environment, `AGENTS.md`) and the default
settings are built into the binary; the `systems` of the config come after them. Put your own
settings in `main.yaml`. `/reload` rebuilds the config after an edit.
