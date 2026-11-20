# Hooks

A hook is Python code started on an event; Tuse does not wait for it, except for `process_end`,
which gets up to `hook_timeout` before Tuse goes on. There are four events:

| Event | When | Variables |
| --- | --- | --- |
| `process_start` | A session opens in this process. | `reason`: `launch` (Tuse started, also with `--resume`), `new` (`/new`), `resume` (`/resume`), `reload` (`/reload`, on the new config). |
| `process_end` | The session closes. | `reason`: `exit`, `new`, `resume`, `reload` (on the old config, before it is replaced). |
| `user_message` | The agent takes a message the user typed (not task notifications). | `text`; `is_first`: the first message of the session (`False` after a resume). |
| `ask` | The main agent asked a question in the TUI or JSON client (not a background task or hook). Does not wait for the hook or fire on answer or dismissal. | `ask_id`, `question`, `options` (an empty list for free text). |
| `agent_message` | The agent wrote a message. Messages without text (only tool calls) are skipped, except on a cancel. | `text`; `status`: `working` (tool calls follow), `finished` (the turn is over) or `cancelled` (the user stopped the turn; `text` is what streamed so far, maybe empty). |

Every hook also gets `event`, `hook` (its name), `session` (ID), `cwd`, `model` (`provider/model`)
and `config` (path of the config file). Names an event does not set are `None`. A hook listening
to an unknown event shows a notice.

Each named hook has a persistent interpreter. Its events run in emission order, while different
hooks can run concurrently. Globals survive between events; event variables are refreshed for each
call. A timeout resets that interpreter; `/reload` runs `process_end` on the old hooks, then
`process_start` on the new ones, each in a fresh interpreter. Exceptions produce a
`hook NAME: ...` notice. Every run is kept in the session and listed in `/trace` as a `hook`
item (name, event, ok or the error, duration and the event's variables). Output from `print()` is discarded. Code under `run` is passed as is:
`$VAR` and `#{{cmd}}` are not expanded there (use `os.environ` and `sh`).

Hooks have the agent's Python functions plus `set_title(text)`, which updates and saves the session
title (up to 80 characters). This function is unavailable to the agent and background tasks.

```yaml
hooks:
  notify:
    events: [agent_message]
    run: |
      if status == "finished":
          sh(f"osascript -e 'display notification \"done\" with title \"{model}\"'")
  title:
    events: [user_message]
    run: |
      if is_first:
          set_title(text.splitlines()[0][:40])
```

To run a script, call it: `run: sh("~/scripts/x.sh")`. Longer code goes into a `.py` file next to
the config and call its entry point each time: `import notify; notify.on_event(event, text)`. A
module is imported only once per interpreter.

Hooks can call helpers defined under `functions` in the config.
