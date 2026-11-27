# Session

The TUI and [JSON mode](json.md) use the same session database and history.

## Storage

- `~/.tuse/tuse.db` is SQLite (schema version `4`). It holds session metadata, history, usage totals, providers, and the default model and effort. The database is the source of truth for each reader, including the next model request.
- `~/.tuse/<id>/lock` is held while a session is open. A second process cannot open the same session at the same time.
- `~/.tuse/<id>/tasks/<task>.log` holds background task logs. `~/.tuse/<id>/images/` holds images pasted into the TUI.

Tuse saves a new session immediately, even if you never send a message. The first recorded user message—or replacing history via `set_session` with a `user` record—marks it as started. Only started sessions appear in `/resume` and `session.list`.

Schema initialization runs atomically inside an `IMMEDIATE` transaction: databases with version `1`–`3` preserve `providers` and `state` while recreating `sessions` and `history` at version `4` (no pre-v4 history migration or dual-shape reader is kept). Opening a database with a future schema version (`> 4`) fails with an incompatible-schema error without modifying any table.

## A turn

A turn starts when you send a message while the session is idle. A task notification that arrives while idle also starts a turn. Each step:

1. Record pending user input and task notifications. Run `user_message` hooks for the user messages.
2. Read the stored system prompt and conversation and send them with the `lua` tool definition. Tuse does not prune or summarize history on its own.
3. Stream the reply. Text, reasoning, and tool code appear live; retry messages are saved as history.
4. Save the `agent` record and its usage in one transaction, then publish the reply and run `agent_message` hooks.
5. If the reply called tools, record each result before the next step. Otherwise end the turn, unless more input is queued.

The first line of the first message becomes the session title (up to 80 characters) if no title has been set through Lua.

**Steering.** Input sent during a turn is queued until the next step. A running `lua` call can move to a background task so the new input can be handled right away. Input can be withdrawn until it is recorded.

**Stop.** Esc, `/stop`, and `turn.cancel` stop the turn and its queued input, foreground Lua, background tasks, hooks, commands, and open `pick`/`ask` prompts. Tuse saves the partial streamed reply with `cancelled = true` and runs `turn_stopped`.

## Resume

`tuse -r ID`, `tuse json -r ID`, TUI `/resume`, and Lua `resume(id)` reopen a session with its saved model, effort, context usage, and UI pins. For CLI `-r ID`, the saved folder is used if it still exists; TUI `/resume` and Lua `resume(id)` switch to started sessions in the current folder. The config is loaded in a fresh VM; globals, running tasks, and unanswered prompts are not restored. Tool calls left without a result after a crash get `[interrupted: no output]`.

`/new` and `/resume` prepare the next session before leaving the current one. If configuration or locking fails, you stay in the current session. The old VM runs `session_close`; the new one runs `config_load` and then `session_created` or `session_resumed`. On quit, Tuse runs `process_end` instead of `session_close`.

## History and session data

`/history` shows the stored history, one entry per record. Agent replies with only tool calls are left out of `/history` and `/fork`; their `lua` entries show the code. `history.hide` filters this list only. `session().history` and JSON-RPC `session.history` both return all structured records. Each has a `role`, a display-only `summary`, and the exact same flat fields as stored in the database:

| Role | Fields |
| --- | --- |
| `system`, `notification` | `text` |
| `message` | `level` (`flash`, `notice`, `error`), `text` |
| `user` | `text`, `context` (hidden attachments), `images` (data URLs) |
| `agent` | `text`, `reasoning`, `tool_calls` (`{id, name, arguments}`), `replay`, `cancelled` |
| `lua` | `id`, `code`, `output`, `exit`, `images`, `diffs` (`{path, diff}`) |
| `usage` | `model`, `input`, `output`, `cache_read`, `cache_write`, `llm` |
| `meta` | `title`, `model`, `effort` (only what changed) |
| `turn_end` | `elapsed_ms`, `calls`, `model`, `effort`, `cancelled` |
| `task`, `command`, `open` | `name`, `code`, `status`, `ms`, `error` |
| `show` | `name`, `text`, `pin`; empty `text` without `pin` removes the block |
| `pick`, `ask` | `id`, `prompt`, `value` |

Hook runs are not recorded; a failing hook leaves a `notice` message.

`replay` holds opaque provider data (`{provider, api, model, items}`). It is sent back only to the matching provider, API, and model. Editing an agent's text or tool calls drops its replay.

Tuse writes the system prompt as the first `system` record when creating or resuming a session and after every successful config reload. Lua can [replace the history](lua.md#session-and-utilities), including the prompt, until the next rebuild. Neither frontend has a direct history editor.

The `/session` panel, `session()`, and JSON-RPC `session.state` show `title`, `model`, `effort`, `context` (the last model reply's input plus output tokens), `context_window`, `requests`, `llm_requests`, `steps`, `turns`, `calls`, and cumulative usage and cost. Usage totals are saved separately and do not change when history or configured prices change.
