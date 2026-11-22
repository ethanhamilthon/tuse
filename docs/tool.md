# The `python` tool

You have one tool, `python(code, title)`: a persistent REPL. You get the output of
`print()` plus the value of the last expression (a `str` as it is, `None` nothing, a `sh` result as
its output and exit code), or a traceback. An error does not end the REPL; a timeout or a cancel
does not either. The REPL lives until Tuse exits and starts empty after `/resume`.

`title` is a few words about what the call does, in the user's language (`Running the tests`). The
user sees it in the working indicator; it stays until the next call sets a new `title`.

A call is stopped after `settings.tool_timeout` seconds (the agent cannot change it; a long job goes to `task_run`). Output over `settings.max_output` bytes is cut in
the middle.

The REPL runs a Python subset ([Monty](https://github.com/pydantic/monty)): no `subprocess`, no
network, no pip packages, but `pathlib`, `open`, `os`, `json`, `re`, `datetime`, `hashlib`, `csv`,
`collections`, `itertools`, `functools`, `math`, `random`, `base64`, `tomllib` and `dataclasses`
work. The whole filesystem is readable and writable. A `.py` file in the config folder (next to
`main.yaml`) can be imported. Paths are absolute, `~/...` or relative to the working directory.

Tuse also bundles `glob.glob` and `iglob`, POSIX `shlex.split`, `quote` and `join`, and
`shutil.which` and `copyfile`. These are subsets, not the entire CPython modules.

Monty is not CPython: dynamic `__import__` and `compile` are unavailable, some standard
library modules are missing, `open` cannot open `/dev/null` or `/dev/tty`, and `re.sub`
does not accept a callable replacement. Monty 1.6 also lacks `re.subn`, `Path.home()` and
ordering for `Path` objects: use `Path(os.environ["HOME"])` and `sorted(paths, key=str)`. Use `sh` when a command needs native OS facilities.

The functions (`read`, `grep`, `sh`...) live inside `python`; they are not tools. When a
model still calls one as a tool of its own, Tuse runs it as `python` code `name(**args)`, and the
chat and `/trace` show that code.
