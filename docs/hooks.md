# Hooks

A hook is Python code started on an event; Tuse does not wait for it (except for `session_end`,
which gets up to `hook_timeout` before Tuse exits). Events: `session_start`, `session_end`,
`turn_start`, `turn_end`, `tool_start`, `tool_end`, `error`.

Every call runs in a **fresh interpreter** with the same access as the agent's `python` tool:
`read`, `write`, `edit`, `grep`, `sh`, `ask`, `show`, `task_*`, files, the whole standard library
of the sandbox, `import` of `.py` files from the config folder, and the `prelude`. Nothing is kept
between calls; keep state in files. A hook stops after `hook_timeout` seconds. An error shows as
the notice `hook NAME: ...`; `print()` output is dropped. Tool calls a hook makes do not fire
`tool_*` events. Hooks run concurrently, so two hooks of successive events can finish in any order.

Names defined in the hook (`None` when they do not apply, `cancelled` is `False`):

| Name | Value |
| --- | --- |
| `event`, `hook` | The event and the hook's name. |
| `session`, `cwd`, `model`, `config` | Session ID, working directory, `provider/model`, path of the config file. |
| `tool`, `input`, `title` | For `tool_start` and `tool_end`: `python`, its code and the `title` the agent gave it. |
| `output`, `code` | For `tool_end`: the result (cut to `max_output`) and `0`, `1` (error) or `None` (cancelled). |
| `error` | For `error`: the text. |
| `cancelled` | For `turn_end`: the turn was cancelled. |

```yaml
hooks:
  notify:
    events: [turn_end]
    run: |
      if not cancelled:
          sh(f"osascript -e 'display notification \"done\" with title \"{model}\"'")
  log_calls:
    events: [tool_end]
    run: |
      with open("/tmp/calls.log", "a") as f:
          f.write(title + "\n")
```

To run a script, call it: `run: sh("~/scripts/x.sh")`. Longer code goes into a `.py` file next to
the config and `run: import notify`.
