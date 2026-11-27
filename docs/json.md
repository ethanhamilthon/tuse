# JSON mode

How to drive Tuse from another program with `tuse json`: JSON-RPC 2.0 over stdin and stdout, one JSON object per line, speaking `protocolVersion: 2`. Sessions, locks, and history are shared with the TUI.

## Running it

```sh
tuse json [-c|--config PATH] [-r|--resume ID]
```

- Stdout carries only JSON-RPC frames, and diagnostics go to stderr. Lua that writes straight to `io.stdout` would break this, so don't.
- Blank lines are ignored. Requests are handled while turns, hooks, and tasks run in the background.
- A request without `id` is a notification: it runs, but gets no reply. Note that `"id": null` does get a reply.
- EOF on stdin, `shutdown`, or `SIGINT`/`SIGTERM`/`SIGHUP` closes the session and exits. Only `shutdown` runs `process_end`.

```text
> {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":2}}
< {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":2,"tuse":"0.14.0","session":{"mode":"json","cwd":"/work","home":"/home/me/.tuse","id":"…","started":false,"model":"openai/gpt-5","provider":"openai","effort":"medium","busy":false,"context":0,"usage":{"input":0,"output":0,"cache_read":0,"cache_write":0},"requests":0,"llm_requests":0,"steps":0,"turns":0,"calls":0},"capabilities":{"methods":["initialize","…"]}}}
> {"jsonrpc":"2.0","id":2,"method":"turn.submit","params":{"text":"fix the test"}}
< {"jsonrpc":"2.0","id":2,"result":{"input_id":1,"queued":false}}
< {"jsonrpc":"2.0","method":"event","params":{"type":"text","text":"Let me"}}
< {"jsonrpc":"2.0","method":"event","params":{"type":"turn_end","elapsed_ms":5120,"calls":2,"model":"openai/gpt-5","effort":"medium","cancelled":false}}
```

## Errors

| Code | Meaning |
| --- | --- |
| `-32700` | Malformed JSON (`id` is `null`). |
| `-32600` | Not a valid request: `jsonrpc` isn't `"2.0"`, `method` isn't a string, or `params` isn't an object. |
| `-32601` | Unknown or retired method. |
| `-32602` | Bad params, such as a wrong type on required or optional fields, or an unsupported `protocolVersion`. |
| `-32000` | The session refused: a missing file or prompt, invalid image data URL, empty input, disallowed effort, an unknown task or command, an invalid config, a locked session, or a stale UI id. |
| `-32002` | Called before `initialize`. |

## Handshake

Send `initialize` first. Only `shutdown` is accepted before it. `protocolVersion` is `2` (omitted means `2`). Any other version (including `1`) fails with `-32602`, and you can try again. Events start flowing after the `initialize` reply. Events and replies are independent streams, so match replies by `id`.

## Methods

All Tuse domain fields use `snake_case`. `SessionState` is `{mode, config_path?, cwd, home, id, started, created, title?, model?, provider?, effort, busy, turn?: {elapsed_ms, status}, context, context_window?, usage: {input, output, cache_read, cache_write, cost?, llm_cost?}, model_info?: {name, display?, window?, efforts, prices?}, requests, llm_requests, steps, turns, calls}` (lightweight—never includes `history`).

### Lifecycle

- `initialize` `{protocolVersion?, client?}` → `{protocolVersion: 2, tuse, session: SessionState, capabilities: {methods}}`.
- `shutdown` → `null`, then Tuse closes the session and exits.

### Turns

- `turn.submit` `{text, prompts?, images?, files?}` → `{input_id, queued}`.
  - `prompts` are names from config `prompts`, or `"name arg"` for a prompt with `args`. An unknown arg or a failing `run` fails with `-32000`.
  - `images` are valid `data:image/<mime>,<data>` URLs or file paths (`~/` expands). Invalid `data:` URLs fail with `-32000`.
  - `files` are files or folders (`~/` expands), attached like `@` in the TUI ([Usage](usage.md#typing-a-message)).
  - Unknown or missing references fail with `-32000`, and so does empty input.
  - `queued: true` means a turn was already running, so this is a steering message.
- `turn.withdraw` `{input_id}` → `true` if the queued message was removed before the agent read it.
- `turn.cancel` → `null`. Stops everything like Esc: the turn, the running `lua` call, tasks, hooks, and commands ([Session](session.md#a-turn)).

### Sessions

- `session.state` → `SessionState` (lightweight state without `history`).
- `session.history` → the structured records from `session().history` ([History](session.md#history-and-session-data)).
- `session.list` `{cwd?}` → started sessions in that folder, newest first: `[{id, cwd, title, model, effort, started, created, updated}]`. A non-string non-null `cwd` fails with `-32602`.
- `session.new` → `{session: SessionState}`.
- `session.fork` `{index}` → `{session: SessionState, draft}`. `index` counts from 0 in the `session.history` list and must point at a `user` or `agent` record. The new session holds the history through an agent record, or up to a user record without it. `draft` is the text of that user record, and empty for an agent record.

External clients discover saved session IDs via `session.list` and resume a chosen session by launching `tuse json -r <id>` (or via a configured Lua command/hook calling `resume(id)`, which emits `session_switched`).

For `session.new` and `session.fork`, events left over from the old session are sent before the reply, and events of the new session after it. If the switch fails, the current session stays.

### Models

- `model.set` `{model, effort?, persist?}` → `null`.
  - `model: ""` clears the selected model.
  - For a non-empty model, effort is validated against the shared model effort resolver (`config.models[].efforts`, else `low`, `medium`, `high`, `xhigh`, `max`).
  - When `effort` is omitted: resolves to `""` if the model declares `efforts = {}`, else `"medium"` if available, else the model's first declared effort.
  - Disallowed efforts (or any non-empty effort on an `efforts = {}` model) fail with `-32000`.
  - `persist` (default `true`) also saves the choice as the default for new sessions.
  - A model whose provider isn't connected fails with `-32000`.
- `model.list` `{cached?}` → sorted `"provider/model"` names. Unless `cached: true`, it also starts fetching from providers. Each provider that answers sends a `models` event, after which `model.list {cached: true}` returns the fuller list.

### Config

- `config.get` → the resolved config: `providers` (`[{name, type, base}]` for all active saved and Lua providers — never keys or headers), `plugins` (`[{name, version}]`), `ui` (with raw `ui.theme` token links preserved), `history`, `open`, `models` (omitting `efforts` when unset and serializing `[]` when explicitly empty), `systems`, `prompts` (`{kind: "text" | "function", body, args}`), `functions` (`custom`, `override`, or `disabled`), `hooks` (`{events}`), `commands` (`{desc, args, params}` per name), and `path` (`""` when defaults are used). Secrets and Lua function bodies are never exposed.
- `config.reload` `{ns?}` → `null`, or `-32000` with the reason. Omit `ns` (or pass `null`) for a full reload. Pass an active namespace name, e.g. `{"ns":"work"}`, to evaluate the config once but apply only that namespace's local and global contributions. Adding or removing namespaces requires a full reload. An empty or non-string `ns` (other than `null`) returns `-32602`. If reload fails, the old config stays.

### Tasks and commands

- `tasks.list` → `[{id, name, status, started, ended, log, code}]`.
- `tasks.get` `{id, lines?}` → the same fields plus `output`, the last `lines` lines of the log (default 50; `0` clamps to `1`; wrong types fail with `-32602`).
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
| `user` | `input_id`, `text`, `images` | A message was taken into the turn. |
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
| `turn_end` | `elapsed_ms`, `calls`, `model`, `effort`, `cancelled` | The turn ended. |
| `usage` | | Usage, model, or effort changed: read `session.state`. |
| `tasks` | | Tasks changed: read `tasks.list`. |
| `models` | | A provider listed its models: read `model.list {cached: true}`. |
| `reloaded` | | The config was reloaded. |
| `history_replaced` | | Lua replaced the stored history; call `session.history` to refresh. |
| `session_switched` | `session`, `draft` | Lua called `fork` or `resume` and the switch happened. It follows the last event of the old session and comes before the first event of the new one. `draft` is the text of the user record a fork stopped before, or empty. |
| `ui_markdown` | `name`, `text`, `pin`, `removed` | A `show()` block changed. |
| `ui_request` | `id`, `kind`, plus `options` and optional `title` (`picker`) or `text` (`question`) | A prompt became active. |
| `ui_clear` | `id` | Tuse withdrew the active prompt. |
| `ui_result` | `id`, `kind`, `prompt`, `value` | A prompt was answered. |
