# Session

How a session runs and is stored: turns, steps, steering and cancel, compaction, resume, and the trace. The same sessions work in the TUI and in [JSON mode](json.md).

## Storage

Everything lives under `~/.tuse/`:

- `tuse.db` is SQLite and holds sessions (id, folder, title, model, effort, timestamps), each session's append-only record log, providers, and the default model and effort.
- `<id>/lock` is held while the session is open. Opening a session that another Tuse process holds fails with `session <id> is open in another tuse process`.
- `<id>/tasks/<task>.log` holds background task logs.
- `<id>/images/` holds images pasted into the TUI.

A new session is written to disk only when its first message arrives. If you quit before typing anything, nothing is saved. Session ids look like `20261125T101500.123456789Z-7136dd19e3fc184d`.

## A turn

A turn starts when you send a message while the session is idle. A task that finishes while the session is idle does not start one: its message is sent with your next message. It runs in steps:

1. Take all queued input: your messages (which fire `user_message` hooks) and task notifications.
2. Build the request: the system prompt ([Lua](lua.md#what-the-agent-sees)), then your `systems`, the history, and the single `lua` tool.
3. Stream the reply: text, reasoning, and tool code all appear live. Retries happen here ([Connect](connect.md#requests-retries-errors)).
4. Save the assistant step and its usage, and fire `agent_message`.
5. If the step called tools, run them one by one and start the next step. If not, the turn ends, unless more input arrived in the meantime.

The first line of your first message becomes the session title (up to 80 characters), unless `set_title` was used.

**Steering.** A message sent during a turn is queued and read at the next step. If a `lua` call is running at that moment, it is handed off to a background task so the agent can answer right away ([Lua](lua.md#background-tasks)). A queued message can be withdrawn until it is read.

**Cancel** (`Esc`, `/stop`, or `turn.cancel`) stops everything right away: the turn and its queued input, the running `lua` call (its result is `[cancelled]`), background tasks, hooks, commands, `open` handlers, and open `pick`/`ask` prompts. Other unfinished calls get `[interrupted: no output]`. The turn ends with `cancelled`, and `agent_message` fires with what was streamed so far.

## Compaction

Long sessions are compacted so they fit the model. The threshold is the model's `compact_at`, else `settings.compact_at`, else 60% of `window` ([Config](config.md#settingscompact_at)). After a step that called tools, if the context is over the threshold, the history is split into three zones (by estimated tokens, newest first):

- **Newest ~25%**: sent in full.
- **Next ~25%**: tool outputs are cut to their first line plus `[tool output pruned]`, and images from tools are dropped.
- **Older**: only the plain conversation text is kept (your messages, task notifications, and agent text). Tool calls and their results are left out.

Nothing is deleted: the database and `/trace` keep every record. The boundaries only move forward. When the oldest zone is in use, the system prompt gets the compaction note, and the chat shows `Context compacted at N tokens; now about M.`

## Resume

`tuse -r ID`, `/resume`, or `session.resume` reopen a saved session:

- The saved folder is used if it still exists, and the config is loaded fresh in a new VM.
- The model and effort come from the session's last selection, not from the global default.
- Tool calls that never got a result (after a crash, say) get `[interrupted: no output]`.
- Usage, cost, compaction state, `show()` blocks, and answered `pick`/`ask` prompts are restored. Lua globals, running tasks, and unanswered prompts are not.
- `start` hooks run with `restored = true`.

`/new` and `/resume` prepare the next session first. If its config or lock fails, you stay in the current one. Closing a session closes UI prompts, cancels commands and the turn, stops tasks, waits for `close` hooks, and releases the lock.

## Records

The session log is a list of JSON records tagged by `t`:

| `t` | Fields |
| --- | --- |
| `user` | `text`, `context?` (hidden attachments), `images?`, `at` |
| `assistant` | `text`, `reasoning?`, `tool_calls?` (`{id, name, arguments}`), `replay?`, `at` |
| `tool` | `id`, `tool`, `input`, `output`, `code`, `images?`, `diffs?` (`{path, diff}`), `at` |
| `notification` | `text`, a task message sent to the model as a user message |
| `notice`, `error` | `text` |
| `compaction` | `prune`, `drop` |
| `usage` | `model`, `usage` (`input`, `output`, `cached`, `cache_write`), `llm?` |
| `settings` | `model`, `effort` |
| `hook` | `name`, `event`, `vars`, `error`, `ms` |
| `turn_end` | `elapsed_ms`, `calls`, `model`, `effort`, `cancelled` |
| `ui_markdown` | `name`, `text`, `pin`, `removed` |
| `ui_result` | `id`, `kind` (`picker`, `question`), `prompt`, `value` |
| `job` | `kind` (`task`, `command`, `open`), `name`, `code?`, `status`, `ms`, `error?` |
| `title` | `new` |

`replay` holds provider reasoning data (`{provider, api, model, items}`). It is sent back only to the same provider, API, and model.

## Trace and session data

`/trace`, `session().trace`, and JSON-RPC `session.trace` list `{role, summary, text}` entries. Entry 0 is `system`, the current full system prompt. After it, every record has an entry with one of these roles: `user`, `thinking`, `agent`, `lua`, `output`, `notification`, `notice`, `error`, `hook`, `task`, `command`, `open` (with status, duration, error, and code), `compaction`, `usage`, `model`, `turn`, `title`, `show`, `pick`, and `ask`. `settings.ui.hide_trace` only hides roles in `/trace`.

`/session` and `session()` show the live state: model and effort, `ctx` (the last request's `input + output`) and `ctx_size`, the `steps` and `turns` counts, and cumulative `usage` and cost, including resumed history.
