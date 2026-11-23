# JSON mode

`tuse json [--config PATH] [--resume ID]` serves a Tuse session over JSON-RPC 2.0 on stdin and stdout, one
message per line (NDJSON), with feature parity with the TUI. stdout carries only the protocol; logs
go to stderr. Closing stdin, `shutdown`, SIGINT, SIGTERM or SIGHUP close the active session (tasks
stop, pending UI requests clear, `close` hooks run) and exit. `session.new` and `session.resume`
switch sessions within the same process and share the same SQLite store and session lock as the
TUI (sessions can be resumed freely between TUI and JSON-RPC).

```
> {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"client":{"name":"ed"}}}
< {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"tuse":"0.9.0","session":{...},"model":"p/m","effort":"default","capabilities":{...}}}
> {"jsonrpc":"2.0","id":2,"method":"turn.submit","params":{"text":"fix the test"}}
< {"jsonrpc":"2.0","id":2,"result":{"inputId":1,"queued":false}}
< {"jsonrpc":"2.0","method":"event","params":{"type":"text","text":"Let me"}}
< {"jsonrpc":"2.0","method":"event","params":{"type":"turn_end","elapsedMs":5120,"calls":2,...}}
```

Requests are served while a turn runs. Methods before `initialize` fail with `-32002`.
In `turn.submit`, `text` stays as given; `prompts`, `files`, and image paths are attached as hidden
context blocks, just like `#` prompts, `@` files, and pasted images in the TUI
([prompts.md](prompts.md)). A missing prompt or file fails the request.

| Method | TUI equivalent | Params -> result |
| --- | --- | --- |
| `initialize` | launch | `{protocolVersion, client}` -> version, session, model, effort, capabilities |
| `turn.submit` | Enter | `{text, prompts?: [name], images?: [path or data URL], files?: [path]}` -> `{inputId, queued}` |
| `turn.withdraw` | edit dimmed draft | `{inputId}` -> `true` when the waiting message was taken back |
| `turn.cancel` | Esc | stops the active turn, hooks, and custom commands |
| `session.info` / `.snapshot` / `.records` / `.trace` / `.stats` | chat, `/session`, `/trace` | session info, full `tuse.session()` snapshot, durable history, trace items, usage totals |
| `session.list` | `/resume` list | `{cwd?}` -> saved sessions for resuming |
| `session.new` / `session.resume` | `/new`, `/resume` | `{}` / `{id}` -> `{session, model, effort}` (switches session in-process) |
| `model.get` / `model.set` / `model.list` | `/models` | `{model, effort?, persist?}` (`persist` defaults to `true`, validating & saving both in one call); `model.list` fetches provider models |
| `config.get` / `config.reload` / `config.set` | `/config`, `/reload` | structured resolved config JSON (with `commands`, `open`, `plugins`, no secrets/Lua bodies); reload (validates before applying, keeps live UI state); `{key, value}` saves `model` or `effort` |
| `tasks.list` / `tasks.get` / `tasks.stop` | `/tasks` | `{id, lines?}` |
| `commands.list` / `commands.run` | `/` custom commands | list configured custom command names; `{name}` runs `/name` as a Lua job |
| `ui.state` / `ui.reply` / `ui.cancel` | `show_markdown`, `show_picker`, `show_question` | snapshot of `{markdowns, active, entries}`; `{id, value}` answers active prompt; `{id}` cancels it (returns `nil`, leaves no entry) |
| `shutdown` | `/quit` | closes the session and exits |

Every session event arrives as an `event` notification with `type`: `user`, `turn_start`, `text`
and `reasoning` (deltas), `stream_reset`, `assistant`, `tool_delta`, `tool_start`, `tool_end`
(with `diffs`), `status`, `notice`, `flash`, `error`, `turn_end`, `tasks`, `usage`, `models`,
`notification`, `ui_markdown` (`{name, text, pin, removed}`), `ui_request` (`{id, kind, options}` or
`{id, kind, text}`), `ui_clear` (`{id}`), and `ui_result` (`{id, kind, prompt, value}`). After
resuming a session (`--resume` or `session.resume`), read `session.records` for durable history;
transient `show_markdown` blocks, interactive requests, and their UI results are not restored across
resume.

Errors use the JSON-RPC codes: `-32700` bad JSON (the process keeps running), `-32600` not a
request, `-32601` unknown method, `-32602` bad params, `-32000` an error from the session (such as
`no task t9`).
