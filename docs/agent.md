# Tuse for the agent

You run inside Tuse. This page is what you need to work with it. Read it with `docs()` in the
`python` tool.

## Tools

- `read(path, offset=1, limit=2000)`: a text file with line numbers; long files come in pages
  (the end says which `offset` continues). An image (png, jpg, gif, webp) is shown to you.
  A binary file or a directory gives a short message instead. Images over
  `settings.max_image_bytes` are refused.
- `write(path, content)`: creates the file (and missing folders) or replaces it. The write is
  atomic and keeps the file's permissions.
- `edit(path, old, new, replace_all=false)`: replaces exact text. `old` must be found, and unique
  unless `replace_all` is set; the error lists the lines of every match. A CRLF file matches `\n`
  in `old` and keeps its CRLF. The answer is a diff.
- `python(code, timeout)`: a persistent REPL. You get the output of `print()` plus the value of
  the last expression (a `str` as it is, `None` nothing, a `sh` result as its output and exit
  code), or a traceback. An error does not end the REPL; a timeout or a cancel does not either.
  The REPL lives until Tuse exits and starts empty after `/resume`.

Paths are absolute, `~/...` or relative to the working directory. The REPL runs a Python subset
([Monty](https://github.com/pydantic/monty)): no `subprocess`, no network, no pip packages, but
`pathlib`, `open`, `os`, `json`, `re`, `datetime`, `hashlib`, `csv`, `collections`, `itertools`,
`functools`, `math`, `random`, `base64`, `tomllib` and `dataclasses` work. The whole filesystem is
readable and writable. A `.py` file in the config folder (next to `main.yaml`) can be imported.

## Functions

Always defined in `python` and in hooks.

- `sh(cmd, timeout=None, cwd=None, env=None)`: `bash -c cmd`, stdout and stderr merged. Returns
  `.out`, `.code` (`None` when killed), `.ok` (`code == 0`) and `.timed_out`. `timeout` is in
  seconds and cannot be longer than what is left of the call's own timeout. `TUSE_SESSION`,
  `TUSE_CWD`, `TUSE_MODEL` and `TUSE_CURRENT_CONFIG` are set.
- `ask(question, options, wait=True)`: asks the user to pick an option and returns the answer.
  With `wait=False` it returns an ID and the answer comes later as a message.
- `show(text, id=None)`: shows plain multiline text in the user's UI, without Markdown, and returns
  a block ID. `show(text, id=ID)` replaces that block, so a script can redraw its own interface in
  place. It raises `RuntimeError` in headless sessions.
- `task_run(cmd, name=None)`: starts a background process (servers, watchers) and returns a task
  with `.id`, `.name`, `.cmd`, `.status`, `.code` and `.output`. You get a message when it exits.
  `task_get(id, lines=50)` fills `.output` with the log tail, `task_stop(id)` stops it,
  `task_list()` lists them.
- `docs()`: this page and the config reference.

## Commands for the user

- `tuse [--config PATH] [--resume ID]` opens the TUI.
- `tuse -p [PROMPT|-]` runs one prompt headless: replies go to stdout, the session ID to stderr,
  and the exit code is 1 after an error. Background tasks are stopped at the end.
- `tuse connect` adds a provider.
- `tuse reset [--yes]` replaces the global config folder with the latest `main.yaml` and
  `default.yaml`. It deletes everything in that folder.
- `tuse update` updates the binary and `default.yaml`; `main.yaml` is not touched.

## User commands

When the user sends a message in the TUI during a turn, it stays dimmed in the input until
the agent takes it. Editing the waiting message withdraws it without stopping the turn;
Enter sends the edited draft.

`/new`, `/resume`, `/models`, `/theme`, `/tasks`, `/usage`, `/trace`, `/reload`, `/quit`.
`/usage`, `/trace` and a task's output open in a panel over the chat (Esc closes it); they are
not part of the conversation. A panel has a rule on top. `/trace` is a list, one row per
item (`system`, `user`, `agent`, `thinking`, `python`, `read`, `write`, `edit`, `output`); Up and
Down move the selection, Enter opens the item in full, Esc goes back to the list and then closes
the panel.

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

The app knows nothing about `default.yaml`. The shipped `main.yaml` loads it with
`include: [default.yaml]`; it holds the system prompt and the default settings, and `tuse update`
replaces it. Put your own settings in `main.yaml`. `/reload` rebuilds the config after an edit.
