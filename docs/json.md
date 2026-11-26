# JSON mode

How to drive Tuse from another program with `tuse json`: JSON-RPC 2.0 over stdin and stdout, one JSON object per line. Sessions, locks, and history are shared with the TUI.

## Running it

```sh
tuse json [-c|--config PATH] [-r|--resume ID]
```

- Stdout carries only JSON-RPC frames, and diagnostics go to stderr. Lua that writes straight to `io.stdout` would break this, so don't.
- Blank lines are ignored. Requests are handled while turns, hooks, and tasks run in the background.
- A request without `id` is a notification: it runs, but gets no reply. Note that `"id": null` does get a reply.
- EOF on stdin, `shutdown`, or `SIGINT`/`SIGTERM`/`SIGHUP` closes the session and exits. Only `shutdown` runs `process_end`.

```text
> {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1}}
< {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"tuse":"0.10.3","session":{"id":"…","title":"","cwd":"/work","busy":false,"turn":null},"model":"openai/gpt-5","effort":"default","capabilities":{"methods":["initialize","…"]}}}
> {"jsonrpc":"2.0","id":2,"method":"turn.submit","params":{"text":"fix the test"}}
< {"jsonrpc":"2.0","id":2,"result":{"inputId":1,"queued":false}}
< {"jsonrpc":"2.0","method":"event","params":{"type":"text","text":"Let me"}}
< {"jsonrpc":"2.0","method":"event","params":{"type":"turn_end","elapsedMs":5120,"calls":2,"model":"openai/gpt-5","effort":"default","cancelled":false}}
```

## Errors

| Code | Meaning |
| --- | --- |
| `-32700` | Malformed JSON (`id` is `null`). |
| `-32600` | Not a valid request: `jsonrpc` isn't `"2.0"`, `method` isn't a string, or `params` isn't an object. |
| `-32601` | Unknown method. |
| `-32602` | Bad params, such as a wrong type, a bad `effort`, or an unsupported `protocolVersion`. |
| `-32000` | The session refused: a missing file or prompt, empty input, an unknown task or command, an invalid config, a locked session, or a stale UI id. |
| `-32002` | Called before `initialize`. |

## Handshake

Send `initialize` first. Only `shutdown` is accepted before it. `protocolVersion` is `1` (omitted means `1`). Any other version fails with `-32602`, and you can try again. Events start flowing after the `initialize` reply. Events and replies are independent streams, so match replies by `id`.

## Methods

`SessionInfo` is `{id, title, cwd, busy, turn: {elapsedMs, status} | null}`.

### Lifecycle

- `initialize` `{protocolVersion?, client?}` → `{protocolVersion, tuse, session: SessionInfo, model, effort, capabilities: {methods}}`.
- `shutdown` → `null`, then Tuse closes the session and exits.

### Turns

- `turn.submit` `{text, prompts?, images?, files?}` → `{inputId, queued}`.
  - `prompts` are names from config `prompts`, or `"name arg"` for a prompt with `args`. An unknown arg or a failing `run` fails with `-32000`.
  - `images` are `data:` URLs or file paths.
  - `files` are files or folders, attached like `@` in the TUI ([Usage](usage.md#typing-a-message)).
  - Unknown or missing references fail with `-32000`, and so does empty input.
  - `queued: true` means a turn was already running, so this is a steering message.
- `turn.withdraw` `{inputId}` → `true` if the queued message was removed before the agent read it.
- `turn.cancel` → `null`. Stops everything like Esc: the turn, the running `lua` call, tasks, hooks, and commands ([Session](session.md#a-turn)).

### Sessions

- `session.info` → `SessionInfo`.
- `session.snapshot` → the `session()` table, `history` included ([Lua](lua.md#session-and-utilities)).
- `session.history` → the structured records from `session().history` ([History](session.md#history-and-session-data)).
- `session.stats` → `{input, output, cached, cacheWrite, cost, llmCost, requests, llmRequests, context, turns, calls}`, where `cost` is `null` until prices are known.
- `session.list` `{cwd?}` → started sessions in that folder, newest first: `[{id, cwd, title, model, effort, started, created, updated}]`.
- `session.new` → `{session, model, effort}`.
- `session.resume` `{id}` → `{session, model, effort}`.
- `session.fork` `{index}` → `{session, model, effort, draft}`. `index` counts from 0 in the `session.history` list and must point at a `user` or `agent` record. The new session holds the history through an agent record, or up to a user record without it. `draft` is the text of that user record, and empty for an agent record.

For `session.new`, `session.resume`, and `session.fork`, events left over from the old session are sent before the reply, and events of the new session after it. If the switch fails, the current session stays.

### Models

- `model.get` → `{model, effort}`.
- `model.set` `{model, effort?, persist?}` → `null`.
  - `effort` defaults to `default`.
  - `persist` (default `true`) also saves the choice as the default for new sessions.
  - `model: ""` clears the model.
  - A model whose provider isn't connected fails with `-32000`.
- `model.list` `{cached?}` → sorted `"provider/model"` names. Unless `cached: true`, it also starts fetching from providers. Each provider that answers sends a `models` event, after which `model.list {cached: true}` returns the fuller list.

### Config

- `config.get` → the resolved config: `settings`, `theme`, `models`, `systems`, `prompts` (`{kind: "text" | "function", body, args}`), `functions` (`custom`, `override`, or `disabled`), `hooks` (`{events}`), `commands` (`{desc, args, params}` per name), `settings.open`, and `path` (`""` when the defaults are used). Secrets and Lua function bodies are left out.
- `config.reload` → `null`, or `-32000` with the reason. If the reload fails, the old config stays.

### Tasks and commands

- `tasks.list` → `[{id, name, status, started, ended, log, code}]`.
- `tasks.get` `{id, lines?}` → the same fields plus `output`, the last `lines` lines of the log (default 50).
- `tasks.stop` `{id}` → the task.
- `commands.list` → `[{name, desc, args, params}]` for the custom commands. `params` is the number of parameters `run` declares, or `null` when it takes `...`.
- `commands.run` `{name, argv?}` → `null`. Starts the command with the words in `argv`. Words the command does not accept fail with `-32000`.

### Interactive UI

`show`, `pick`, and `ask` become events, and the client answers them:

- `ui.state` → `{markdowns: [{name, text, pin}], active: UiRequest | null, entries: [{id, kind, prompt, value}]}`. Call it to resync at any time.
- An active picker has `id`, `kind: "picker"`, and `options`. A titled picker also has `title`, the original Markdown question; untitled pickers omit it in both `ui.state.active` and `ui_request`.
- `ui.reply` `{id, value}` → `null`. Answers the active request. For a picker, `value` must be one of the options.
- `ui.cancel` `{id}` → `null`. The Lua side gets `nil`. No `ui_clear` follows, so clear the prompt yourself.

## Events

Events come as `{"jsonrpc":"2.0","method":"event","params":{"type": …}}`:

| `type` | Fields | Meaning |
| --- | --- | --- |
| `turn_start` | | A turn began. |
| `user` | `inputId`, `text`, `images` | A message was taken into the turn. |
| `notification` | `text` | A task message was taken into the turn. |
| `text` | `text` | Assistant text delta. |
| `reasoning` | `text` | Reasoning delta. |
| `stream_reset` | | A retry: drop the deltas of the failed attempt. |
| `assistant` | `text` | Final text of a step. |
| `tool_delta` | `index`, `id`, `name`, `code` | Streamed tool code. |
| `tool_start` | `timeout`, `id`, `tool`, `input` | A tool call started. |
| `tool_end` | `id`, `output`, `code`, `diffs` | A tool call ended. |
| `status` | `text` | What the tool is doing right now, such as `reading path...`. |
| `notice` | `text` | Information, such as a hook error. |
| `flash` | `text` | Short banner, such as a retry countdown. |
| `error` | `text` | A turn, provider, command, or reload error. |
| `turn_end` | `elapsedMs`, `calls`, `model`, `effort`, `cancelled` | The turn ended. |
| `usage` | | Usage, model, or effort changed: read `session.stats` or `model.get`. |
| `tasks` | | Tasks changed: read `tasks.list`. |
| `models` | | A provider listed its models: read `model.list {cached: true}`. |
| `reloaded` | | The config was reloaded. |
| `history_replaced` | | Lua replaced the stored history; call `session.history` to refresh. |
| `ui_markdown` | `name`, `text`, `pin`, `removed` | A `show()` block changed. |
| `ui_request` | `id`, `kind`, plus `options` and optional `title` (`picker`) or `text` (`question`) | A prompt became active. |
| `ui_clear` | `id` | Tuse withdrew the active prompt. |
| `ui_result` | `id`, `kind`, `prompt`, `value` | A prompt was answered. |
