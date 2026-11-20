# JSON mode

`tuse json [-c PATH] [-r ID]` serves one session over JSON-RPC 2.0 on stdin and stdout, one
message per line (NDJSON), for editors and other front ends. stdout carries only the protocol; logs
go to stderr. Closing stdin, `shutdown`, SIGINT, SIGTERM or SIGHUP close the session (tasks stop,
`process_end` hooks run) and exit. One process is one session: start another for another session.
`ask` and `draw_ui` work, and the prompt does not say "headless".

```
> {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"client":{"name":"ed"}}}
< {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"tuse":"0.5.4","session":{...},"model":"p/m","effort":"default","capabilities":{...}}}
> {"jsonrpc":"2.0","id":2,"method":"turn.submit","params":{"text":"fix the test"}}
< {"jsonrpc":"2.0","id":2,"result":{"inputId":1,"queued":false}}
< {"jsonrpc":"2.0","method":"event","params":{"type":"text","text":"Let me"}}
< {"jsonrpc":"2.0","method":"event","params":{"type":"ask","id":"a1","question":"Run it?","options":["yes","no"]}}
> {"jsonrpc":"2.0","id":3,"method":"ask.answer","params":{"id":"a1","answer":"yes"}}
< {"jsonrpc":"2.0","method":"event","params":{"type":"turn_end","elapsedMs":5120,"calls":2,...}}
```

Requests are served while a turn runs. Methods before `initialize` fail with `-32002`.

| Method | Params → result |
| --- | --- |
| `initialize` | `{protocolVersion, client}` → version, session, model, effort, capabilities |
| `turn.submit` | `{text, images?: [path or data URL], files?: [path]}` → `{inputId, queued}` |
| `turn.withdraw` | `{inputId}` → `true` when the waiting message was taken back |
| `turn.cancel` | stops the turn |
| `ask.answer` | `{id, answer}`; `answer: null` dismisses the ask |
| `session.info` / `.records` / `.trace` / `.stats` | the session, its history, `/trace` items, `/usage` totals |
| `session.list` | `{cwd?}` → saved sessions, for `--resume` |
| `model.get` / `model.set` / `model.list` | `{model, effort}`; the list fetches provider models (then a `models` event) |
| `config.get` / `config.reload` / `config.set` | the YAML; reload; `{key, value, scope: global or settings}` |
| `tasks.list` / `tasks.get` / `tasks.stop` | `{id, lines?}` |
| `shutdown` | closes the session and exits |

Every session event arrives as an `event` notification with `type`: `user`, `turn_start`, `text`
and `reasoning` (deltas), `stream_reset`, `assistant`, `tool_delta`, `tool_start`, `tool_end`
(with `diffs`), `status`, `notice`, `flash`, `error`, `turn_end`, `ask`, `ask_done`, `tasks`,
`print` (`draw_ui`), `usage`, `models`, `notification`. After `--resume`, read
`session.records` to show the history.

Errors use the JSON-RPC codes: `-32700` bad JSON (the process keeps running), `-32600` not a
request, `-32601` unknown method, `-32602` bad params, `-32000` an error from the session (such as
`no ask a9`).
