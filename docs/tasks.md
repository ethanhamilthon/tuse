# Background Tasks

Background tasks run Lua functions asynchronously in the session's VM without blocking agent turns, hooks, or other tasks.

## Starting a Task

Call `spawn(fn, name?)` (global in the `lua` tool, or `require("tuse").spawn` in config/hooks/commands):

- Requires a live session.
- Assigns sequential session-local IDs: `t1`, `t2`, `t3`, ...
- `name` defaults to the task's ID when omitted or `""`.
- Creates the task log file at `~/.tuse/<session-id>/tasks/<id>.log` immediately (creating `~/.tuse/<session-id>/tasks/` on disk even if the session has not yet been materialized in SQLite by a first user message).
- Starts `fn` as a coroutine in the session VM with **no deadline**. Because it shares the VM, a task spawned from the `lua` tool can read and update the agent's globals.
- Emits a `tasks` session event and fires `lua_start` (`kind = "task"`, `name = display_name`) unless spawned from inside a `lua_start` or `lua_end` hook.
- Returns the task ID string immediately.

```lua
local id = spawn(function()
  local r = sh("npm test")
  print(r.out)
  if not r.ok then error(tostring(r)) end
  return "tests passed"
end, "tests")
```

Note that `sh()` captures stdout/stderr into `r.out` rather than streaming it to the task log; call `print(r.out)` to record shell output in the log file and return a short summary value for the completion notification.

## Handing Off a Running Lua Call

Submitting a steering message or cancelling the turn while an agent `lua` tool call is running hands off the **entire running Lua call** as one background task. This does not apply to hooks, custom commands, open handlers, or calls that have not started. Task completions are not steering inputs and do not cause handoff.

- The same coroutine continues in the same VM with its existing locals, globals, and pending wait. A running `sh()` process is neither killed nor restarted; it and subsequent `sh()` calls return their usual results to the script. The tool's 120-second deadline no longer applies; an explicit `sh()` timeout still counts from the start of that command. The rest of the script, including file changes, can still run after Esc.
- Handoff is recognized at a scheduler yield or instruction-hook check. It does not make blocking, non-yieldable code cooperative: for example, a `sh()` inside a C callback can still hold the VM until it returns. On Esc, an already-open TUI picker or `ask()` is dismissed first, without cancelling the turn or handing off the call.
- The task uses the normal task ID, log, lifecycle events, and stop controls; its display name is the original Lua execution ID (for example, `lua-3`). It inherits buffered `print()` / `io.write()` output. Diffs and images collected before handoff stay with the immediate tool result; later file operations do not attach diffs or images to chat. Shell output is still captured as `sh().out`, not automatically copied to the log.
- The foreground tool finishes with `[continued in background: tN]` (`code = null`); the foreground `lua_end` hook reports `cancelled` with that marker as its error. The handoff emits `lua_start` for the task; its eventual return value or error goes to the task log and completion notification, not to a second tool result.
- A handoff happens at most once. If the tool finishes before it can be handed off, there is no task. Once the handoff occurs, withdrawing a queued steering message or cancelling another turn does not undo it. Use `/tasks` or `task_stop()` to stop the resulting task.

## Task Fields and States

| Field | Type | Present in | Meaning |
| --- | --- | --- | --- |
| `id` | string | Lua & JSON-RPC | Task ID (`"t1"`, `"t2"`, ...). |
| `name` | string | Lua & JSON-RPC | Display name (defaults to `id`). |
| `status` | string | Lua & JSON-RPC | `"running"`, `"done"`, `"failed"`, or `"stopped"`. |
| `started` | string | Lua & JSON-RPC | Start timestamp in UTC (`YYYY-MM-DDTHH:MM:SSZ`). |
| `ended` | string | Lua & JSON-RPC | End timestamp in UTC, or `""` while running (including after `task_stop` marks a task `"stopped"` until its coroutine actually exits). |
| `log` | string | JSON-RPC only | Full filesystem path to `<id>.log`. |
| `output` | string | `task_get` & `tasks.get` only | Tail of the task log. |

State transitions:

- `running` -> `done`: `fn` returns normally.
- `running` -> `failed`: `fn` raises an unhandled Lua error.
- `running` -> `stopped`: stopped via `task_stop`, `/tasks` Ctrl+X, JSON-RPC `tasks.stop`, or session close.

## Logs and Output

- Inside a task, `print(...)` and `io.write(...)` append directly to `~/.tuse/<session-id>/tasks/<id>.log` (uncapped).
- When the task ends, any non-`nil` return value (formatted via `inspect`) or error with stack traceback (or `[cancelled]` when stopped) is appended as a final line to the log file.
- Reading logs reads up to the last 256 KiB of the log file and returns the trailing `lines`:
  - `task_get(id, lines?)`: defaults to `50` lines (clamped `>= 1`).
  - JSON-RPC `tasks.get { id, lines? }`: defaults to `50` lines (`0` returns `""`; non-integer/negative values default to `50`).
  - TUI `/tasks` (Enter): displays the last `200` lines in a scrollable panel.

## Completion Notifications

When a task ends on its own (`done` or `failed`), Tuse:

1. Fires `lua_end` (`kind = "task"`, `status = "success"` or `"error"`, with `result` holding the rendered return value on `"success"`—or `""` if the task returned nothing—and `error` holding the error/traceback on `"error"`) unless excluded by the `lua_start`/`lua_end` anti-recursion rule.
2. Emits a `tasks` session event.
3. Submits a notification message to the session:

```text
<task>Task t1 (tests) finished. Last line: tests passed. Output: `task_get("t1").output`.</task>
```

- `Last line` is extracted from the task's **final return value or error text**, not from lines written to the log via `print()` / `io.write()`. If a task returns normally with no return value (`nil`), its return text is empty and `Last line` falls back to `"failed"` (producing `finished. Last line: failed.`). For `failed` tasks, `Last line` is the last non-empty line before `stack traceback:` (or `"failed"` if none).
- If the session is idle, the notification starts a new agent turn; if a turn is running, it is queued for the next step.
- In model API requests, notifications are sent with role `"user"`, but the event and stored record are still named `notification`, hidden from the TUI chat transcript, and do not trigger `user_message` hooks or session auto-titling.
- Tasks ended via `stopped` emit `tasks` and `lua_end` (`status = "cancelled"`) but send **no** completion notification to the agent.

## Stopping and Lifecycle Rules

- **Stopping a task** (`task_stop(id)`, JSON-RPC `tasks.stop`, TUI `/tasks` Ctrl+X): marks the task `"stopped"`, sets its cancel flag, emits a `tasks` event, and returns the updated task record (`ended` remains `""` until the coroutine exits and finalizes). The task coroutine receives `"cancelled"` at its next wait or instruction-hook check (killing any running `sh()` process group and cancelling any pending `pick`/`ask`/`llm`). Calling stop on an already-ended task succeeds without changing its status. Unknown IDs fail with `"no task <id>"`.

For full signatures of `spawn`, `task_get`, `task_stop`, and `task_list`, see [functions.md](functions.md).
- **Esc / `turn.cancel`**: does **not** stop background tasks. It hands off a running agent `lua` call and cancels the foreground turn, hooks, commands, and open handlers.
- **Reload (`/reload`, `config.reload`, `reload()`)**: does **not** stop running tasks; they continue running in the same VM.
- **Session close or switch (`/new`, `/resume`, `shutdown`)**: stops all running tasks. Tasks are not persisted or restored when resuming a session.

## TUI and JSON-RPC

- **TUI `/tasks`**: opens a picker listing all tasks in the session (`<id> <name>` and `<status>`). If no tasks exist, flashes `No tasks. The agent starts them with \`spawn()\`.`. Press Enter to view the last 200 lines of the selected task's log, or Ctrl+X to stop it. The list refreshes live when `tasks` events arrive.
- **JSON-RPC**: `tasks.list`, `tasks.get`, and `tasks.stop`, plus `tasks` event notifications ([json.md](json.md)).
