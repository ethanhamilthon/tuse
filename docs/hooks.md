# Hooks

A hook is a Lua function called on an event; Tuse does not wait for it, except for `close`,
which Tuse waits for until every running hook finishes.

Hook payloads contain **only** `event` and event-specific fields. Call `require("tuse").session()`
inside the hook for session metadata (`id`, `name`, `cwd`, `model`, `provider`, `effort`, `home`,
`configpath`, `usage`, `trace`, etc.).

| Event | When | Payload fields |
| --- | --- | --- |
| `start` | A session opens in this process, or after `/reload` commits a new config. | `event`, `reason` (`"launch"`, `"new"`, `"resume"`, `"reload"`), `is_new` (`true` for fresh launch or `/new`; `false` for `--resume`, `/resume`, `/reload`), `restored` (`true` for `--resume` or `/resume`; `false` otherwise). |
| `close` | The session closes, or before `/reload` replaces the config. The session remains readable via `tuse.session()` while `close` runs. | `event`, `reason` (`"exit"`, `"new"`, `"resume"`, `"reload"`). |
| `user_message` | The agent takes a message the user typed (not task notifications). | `event`, `text`, `is_first` (`true` for the first user message of the session, `false` otherwise including after resume). |
| `agent_message` | The agent produced an assistant message (text and/or tool calls) or was cancelled mid-step. | `event`, `text` (may be `""` when only tool calls were emitted or cancelled early), `status` (`"working"`, `"finished"`, `"cancelled"`), `tool_calls` (list of `{id, name, arguments}` when present, else `nil`). |
| `model_change` | After the session's model, provider, or effort changes. | `event`, `previous` (`{model, provider, effort}`), `new` (`{model, provider, effort}`). |
| `name_change` | After the session's name changes (initial auto-title or `set_title`). | `event`, `previous` (previous name or `nil`), `new` (new name). |
| `lua_start` | A tracked Lua job starts. | `event`, `id` (execution ID, e.g. `"lua-1"`), `kind` (`"tool"`, `"command"`, `"task"`, `"open"`), `code` (source code for `"tool"`, else `nil`), `name` (command/task/extension name for `"command"`/`"task"`/`"open"`, else `nil`). |
| `lua_end` | A tracked Lua job ends. | `event`, `id` (same execution ID as `lua_start`), `kind`, `status` (`"success"`, `"error"`, `"cancelled"`), `duration_ms` (elapsed milliseconds), `result` (output/return text on `"success"`, else `nil`), `error` (error text on `"error"`, else `nil`). |

## Which Lua jobs emit `lua_start` and `lua_end`

Tracked jobs are:
- **`tool`**: agent `lua(code)` tool calls;
- **`command`**: custom slash commands from `commands = {...}`;
- **`task`**: background tasks started with `spawn(fn, name)`;
- **`open`**: local file opener callbacks from `open = {...}`.

Hook runs themselves never trigger `lua_start` or `lua_end`, and any task spawned from inside a
`lua_start` or `lua_end` hook is also excluded so hooks cannot recursively trigger themselves.
Top-level config evaluation on load or `/reload` does not emit `lua_start`/`lua_end`.

```lua
local t = require("tuse")

return {
  hooks = {
    notify = t.session().mode == "tui" and {
      events = { "agent_message" },
      run = function(e)
        if e.status == "finished" then t.sh("say done") end
      end,
    } or nil,
  },
}
```

Hooks run as coroutines in the session's VM and can call configured helpers. Their config upvalues
survive between events. Events for one hook run in emission order; different hooks run
concurrently and keep running while the agent waits on `sh`. A hook run has no time limit; Esc
stops running hooks. An error emits a `hook NAME: ...` notice. `/reload` validates the new config
first, then runs `close` on the old hooks, commits the new config, and runs `start` (`reason =
"reload"`) on the new hooks. Every hook run is recorded in `/trace` and `tuse.session().trace`.
