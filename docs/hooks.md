# Hooks

A hook is Python code started on an event; Tuse does not wait for it (except for `session_end`,
which gets up to `hook_timeout` before Tuse exits). Events: `session_start`, `session_end`,
`turn_start`, `turn_end`, `tool_start`, `tool_end`, `error`.

Each named hook has a persistent interpreter. Its events run in emission order,
while different hooks can run concurrently. Globals survive between events;
event variables are refreshed for each call. A timeout resets that interpreter,
and `/reload` discards queued events and starts fresh interpreters. Exceptions
produce a `hook NAME: ...` notice. Output from `print()` is discarded.

Hooks have the agent's Python functions plus `set_title(text)`, which updates
and saves the session title (up to 80 characters). This function is unavailable
to the agent and background tasks.

Names defined in the hook (`None` when they do not apply, `cancelled` is `False`):

| Name | Value |
| --- | --- |
| `event`, `hook` | The event and the hook's name. |
| `session`, `cwd`, `model`, `config` | Session ID, working directory, `provider/model`, path of the config file. |
| `tool`, `input`, `title` | For `turn_start`: `input` is the prompt and `title` is the session title. For `tool_start` and `tool_end`: `python`, its code and the `title` the agent gave it. |
| `output`, `code` | For `tool_end`: the result (cut to `max_output`) and `0`, `1` (error) or `None` (cancelled). |
| `error` | For `error` and failed `turn_end`: the text. |
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
the config and call its entry point each time: `import notify; notify.on_event(event, input)`. A module is imported only once per interpreter.

Hooks can call helpers defined under `functions` in the config.
