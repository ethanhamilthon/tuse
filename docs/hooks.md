# Hooks

A hook is a Lua function called on an event; Tuse does not wait for it, except for
`process_end`, which gets up to `hook_timeout` before Tuse goes on. There are four events:

| Event | When | Fields |
| --- | --- | --- |
| `process_start` | A session opens in this process. | `reason`: `launch` (Tuse started, also with `--resume`), `new` (`/new`), `resume` (`/resume`), `reload` (`/reload`, on the new config). |
| `process_end` | The session closes. | `reason`: `exit`, `new`, `resume`, `reload` (on the old config, before it is replaced). |
| `user_message` | The agent takes a message the user typed (not task notifications). | `text`; `is_first`: the first message of the session (`false` after a resume). |
| `agent_message` | The agent wrote a message. Messages without text (only tool calls) are skipped, except on a cancel. | `text`; `status`: `working` (tool calls follow), `finished` (the turn is over) or `cancelled` (the user stopped the turn; `text` is what streamed so far, maybe empty); `tasks`: this session's running background tasks, a list of `{id, name}`; `children`: running descendant sessions at any depth (see `/childrens`; running = holds its session lock), a flat list of `{id, name, cwd, elapsed}` with `elapsed` in seconds. Both are empty lists when nothing runs, for every `status`. |

The function gets one table `e` with the event's fields and also `event`, `hook` (its name),
`session` (ID), `cwd`, `model` (`provider/model`) and `config` (path of the config file). Fields
an event does not set are `nil`. A hook listening to an unknown event shows a notice.

```lua
return {
  hooks = {
    notify = {
      events = {"agent_message"},
      modes = {"tui"},
      run = function(e)
        if e.status == "finished" then sh("say done") end
      end,
    },
  },
}
```

Hooks run as coroutines in the session's VM, so they share its globals and helpers (and the
upvalues of the config file, which survive between events). The events of one hook run in
emission order; different hooks can run concurrently, and they keep running while the agent
waits on `sh`. Each run has `hook_timeout` seconds (default 30); Esc stops the runs going on.
An error produces a `hook NAME: ...` notice. `/reload` runs `process_end` on the old hooks, then
`process_start` on the new ones. Every run is listed in `/trace` with its fields and time.
