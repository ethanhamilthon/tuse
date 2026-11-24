# Hooks

A hook is a Lua function invoked on session lifecycle events. Tuse runs hooks asynchronously and does not wait for them, except for `close`, where Tuse waits until every active and queued hook finishes.

## Configuration

Declare hooks under the top-level `hooks` map:

```lua
local t = require("tuse")

return {
  hooks = {
    notify = t.session().mode == "tui" and {
      events = { "agent_message" },
      run = function(e)
        if e.status == "finished" then
          t.sh('osascript -e \'display notification "done"\'')
        end
      end,
    } or nil,
  },
}
```

- Each entry requires `events` (a non-empty list of known event names) and `run` (a Lua function). Unknown keys are rejected.
- Branch on `require("tuse").session().mode` (`"tui"` or `"json"`) and evaluate to `nil` to omit mode-specific hooks.

## Payloads

Hook payloads contain **only** `event` and event-specific fields. Absent or `null` fields are omitted (read as `nil` in Lua). Session metadata is never duplicated on `e`; call `require("tuse").session()` inside the hook for live session state (`id`, `name`, `cwd`, `model`, `provider`, `effort`, `home`, `configpath`, `usage`, `trace`, etc.).

| Event | When it fires | Payload fields |
| --- | --- | --- |
| `start` | After a session opens (`launch`, `new`, `resume`) or after reload commits a new config (`reload`). | `event`, `reason` (`"launch"`, `"new"`, `"resume"`, `"reload"`), `is_new` (`true` for fresh launch or `/new` / `session.new`; `false` for `--resume`, `/resume` / `session.resume`, reload), `restored` (`true` for `--resume` or `/resume` / `session.resume`; `false` otherwise). |
| `close` | When a session closes (`exit`, `new`, `resume`) or before reload replaces the config (`reload`). `session()` remains readable while `close` runs. | `event`, `reason` (`"exit"`, `"new"`, `"resume"`, `"reload"`). |
| `user_message` | When a turn step consumes a user-typed message (never for task notifications). | `event`, `text` (message text without hidden `#prompt`/`@file` context blocks; trimmed of leading/trailing whitespace in TUI, verbatim from `turn.submit` in JSON-RPC), `is_first` (`true` only if no prior user message exists in the session; `false` otherwise, including after resume). |
| `agent_message` | When a model step finishes with non-empty text and/or tool calls, or when a running turn is cancelled. (Not fired for an empty whitespace-only reply with no tool calls.) | `event`, `text` (assistant text, or accumulated streamed partial text on cancel; may be `""`), `status` (`"working"` when tool calls are present, `"finished"` for a text-only reply, `"cancelled"` on turn cancel), `tool_calls` (list of `{ id, name, arguments }` when non-empty, else `nil`). |
| `model_change` | After the session's `(model, effort)` changes (no-op if unchanged). `session()` already has the new values. | `event`, `previous` (`{ model?, provider?, effort }`), `new` (`{ model?, provider?, effort }`). `model` and `provider` are `nil` when unset. |
| `name_change` | After the session's title changes (initial auto-title attempted at turn start when unnamed from the first non-notification user message's first line, or `set_title`). No-op if unchanged. `session()` already has the new name. | `event`, `previous` (previous name or `nil`), `new` (new name). |
| `lua_start` | When a tracked Lua job starts. | `event`, `id` (sequential execution ID `"lua-1"`, `"lua-2"`, ...), `kind` (`"tool"`, `"command"`, `"task"`, `"open"`), `code` (source code string for `"tool"`, else `nil`), `name` (command name for `"command"`, task display name or task ID for `"task"`, configured extension key for `"open"`, `nil` for `"tool"`). |
| `lua_end` | When a tracked Lua job ends. | `event`, `id` (same ID as `lua_start`), `kind`, `status` (`"success"`, `"error"`, `"cancelled"`), `duration_ms` (elapsed milliseconds), `result` (captured output/return text on `"success"`; for `"task"`, only the rendered return value or `""` if none since `print`/`io.write` go to the task log; else `nil`), `error` (error text on `"error"`, else `nil`). |

## Tracked Jobs for `lua_start` and `lua_end`

Tracked Lua jobs are:

- **`tool`**: agent `lua(code)` tool calls with valid JSON arguments and a string `code` (unknown tools, invalid JSON arguments, and missing/non-string `code` fail before starting a Lua job and do not emit `lua_start`/`lua_end`).
- **`command`**: custom slash commands from `commands`.
- **`task`**: background tasks started via `spawn(fn, name?)`.
- **`open`**: local file opener callbacks from `open`.

Exclusions (to prevent infinite recursion):

- Hook runs themselves never emit `lua_start` or `lua_end`.
- Tasks spawned directly inside a `lua_start` or `lua_end` hook never emit `lua_start` or `lua_end`.
- Top-level config and plugin evaluation on startup or reload never emits `lua_start` or `lua_end`.

## Execution Semantics

- **Per-hook ordering, cross-hook concurrency**: each hook name has its own FIFO queue (lane). Events for one hook run strictly in emission order, one at a time. Different hooks run concurrently as coroutines in the session's VM and keep running while the agent waits on `sh` or the API.
- **State**: config upvalues survive across events until reload or session close. Hooks run in the file environment (import host functions via `require("tuse")`) and can call configured helpers.
- **No timeout**: hook runs have no execution deadline. Esc / `turn.cancel` cancels only the **currently running** execution in each hook lane; queued events in that lane are not cleared and still run afterward.
- **Errors**: an error in a hook does not abort other hooks or the session; it emits a `notice` event (`hook <name>: <first error line>`).
- **Trace persistence**: every finished hook run is stored as a durable `hook` record (`name`, `event`, `vars` as a newline-separated `key=value` string, `error`, `ms`) and appears in `/trace` and `session().trace`.
- **Close and reload synchronization**:
  - Before `close` fires (for exit, session switch, or reload), Tuse cancels any pending interactive UI requests (`pick`/`ask`) created by hooks and rejects new hook UI requests with `"interactive UI is unavailable during reload or close"`.
  - After queuing `close`, Tuse stops dispatching new hook events and waits for all active and queued hook runs to finish.
  - On reload: Tuse validates the new config first, drains old hooks with `close` (`reason = "reload"`), commits the new config, re-enables hooks and hook UI, and fires `start` (`reason = "reload"`) on the new hooks.
- **Calling `reload()` inside a hook**: returns `nil` immediately and schedules the reload right after that hook execution completes (preventing deadlock with `close`). If the deferred reload fails, Tuse emits an `error` event (`reload: ...`).
