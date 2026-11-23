# The `lua` tool

You have one tool, `lua(code)`: Lua 5.4 code run in the session's VM. Globals
stay between calls (use `local` for scratch values). You get the output of `print()` and
`io.write()`, then the values the code returns (a lone expression such as `x + 1` returns
itself; a table is shown with `inspect`; a `sh` result as its output and exit code), or the
error with its traceback. An error does not end the VM; a timeout or a cancel does not either.
The VM lives until Tuse exits and starts empty after `/resume`.

A call is stopped after 120 seconds; start a long job in the background with `spawn`. Nothing is cut: not the output of a call, not `sh().out`, not a task log, so keep
big outputs short yourself (`out:sub(1, 2000)`, `grep` with a `limit`).

The whole standard library is there: `string`, `table`, `math`, `utf8`, `io`, `os` (but not
`os.exit`), `coroutine`, `require` and `dofile`. Tuse adds `json.encode`, `json.decode` and
`inspect(value)`. Lua patterns are not regular expressions; use `grep` or `sh` for those.
`require` works as in configs: `require("tuse")` gives the host functions, `require("name")`
loads `name.lua` or `name/init.lua` relative to the calling file (or the working directory for inline calls),
and `~/path` or a `.lua` path loads that file (see `docs("config")`). Modules must import host
functions even when the agent loads them.
Paths are absolute, `~/...` or relative to the working directory.

Everything runs in one VM on its own thread: the agent's calls, hooks, custom slash commands, and
background tasks are coroutines of a scheduler. `sh`, `sleep`, `pick`, `ask`, and
other waits yield, so the other coroutines keep running meanwhile (interactive UI waits also pause
the calling coroutine's timeout until replied or cancelled). Prefer `read`, `grep`, `write`,
`edit`, `append`, and `delete` for file operations, and wrap complex shell commands in `sh([=[…]=])`.
A loop that never waits still ends at its deadline or on Esc. `sh` also yields from inside a
coroutine of your own (`coroutine.wrap`). Where Lua cannot yield (inside a C callback such as a
`string.gsub` function) `sh` blocks the VM until it ends.

The functions (`read`, `grep`, `sh`...) live inside `lua`; they are not tools. When a model calls
one as a tool of its own, the call fails with a hint to use `lua`.
