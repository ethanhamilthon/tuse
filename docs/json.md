# JSON-RPC Mode (`tuse json`)

`tuse json [--config PATH] [--resume ID]` (`-c`, `-r`) serves a Tuse session over JSON-RPC 2.0 on stdin and stdout using NDJSON (one JSON object per newline).

- Tuse writes only JSON-RPC protocol frames to `stdout` and separates captured Lua `print`/`io.write` output and Tuse diagnostic logs (`stderr`); however, arbitrary Lua code performing direct `io.stdout` writes can still write to `stdout`.
- Blank or whitespace-only lines on `stdin` are ignored.
- Requests are processed while turns, hooks, commands, and tasks execute asynchronously in the background.
- Requests that **omit** the `id` key are JSON-RPC notifications: they execute normally and produce no response. An explicit `"id": null` is **not** a notification and produces a response with `"id": null`. Parse errors emit an error response with `"id": null`; invalid requests (`-32600`) echo the supplied `"id"` if present (otherwise `null`).
- Closing `stdin` (EOF), calling `shutdown`, or receiving `SIGINT`, `SIGTERM`, or `SIGHUP` closes the active session (clears pending UI requests, stops commands, cancels the active turn, stops all tasks, waits for `close` hooks), briefly drains final events (~100 ms), and exits.
- `tuse json` shares `~/.tuse/tuse.db` and per-session file locks (`~/.tuse/<id>/lock`) with the TUI; sessions can be resumed freely between TUI and JSON-RPC, and opening a session already locked by another process fails.

Illustrative NDJSON transcript (`>` client to stdin, `<` server to stdout):

```text
> {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"client":{"name":"ed"}}}
< {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"tuse":"0.9.2","session":{"id":"20261124T120000.000000000Z-a1b2c3d4e5f60708","title":"","cwd":"/work","busy":false,"turn":null},"model":"p/m","effort":"default","capabilities":{"methods":["initialize","turn.submit","..."]}}}
> {"jsonrpc":"2.0","id":2,"method":"turn.submit","params":{"text":"fix the test"}}
< {"jsonrpc":"2.0","id":2,"result":{"inputId":1,"queued":false}}
< {"jsonrpc":"2.0","method":"event","params":{"type":"text","text":"Let me"}}
< {"jsonrpc":"2.0","method":"event","params":{"type":"turn_end","elapsedMs":5120,"calls":2,"model":"p/m","effort":"default","cancelled":false}}
```

## TUI vs JSON-RPC Capabilities

| Capability | TUI | JSON-RPC |
| --- | --- | --- |
| Turns, steering, cancel | Enter, edit dimmed draft, Esc | `turn.submit`, `turn.withdraw`, `turn.cancel` |
| `#` prompts, `@` files, images | interactive pickers, Ctrl+V | `turn.submit` (`prompts`, `files`, `images`) |
| Sessions: new, resume, list | `/new`, `/resume` | `session.new`, `session.resume`, `session.list` |
| Model and effort | `/models` | `model.get`, `model.set`, `model.list` |
| Config inspection and reload | `/config`, `/reload` | `config.get`, `config.reload` |
| Background tasks | `/tasks` | `tasks.list`, `tasks.get`, `tasks.stop` |
| Custom commands | `/name` | `commands.list`, `commands.run` |
| `show`, `pick`, `ask` | rendered modals/blocks | `ui_*` events, `ui.state`, `ui.reply`, `ui.cancel` |
| Session data, history, trace, usage | chat, `/session`, `/trace` | `session.info`, `session.snapshot`, `session.records`, `session.trace`, `session.stats` |
| Clicked path openers (`open.<ext>`) | yes | TUI-only |
| Rendering, themes, selection, hotkeys | yes | client's job |

## Framing and Error Codes

Requests must be JSON objects with `"jsonrpc": "2.0"`, string `"method"`, optional `"id"`, and optional object `"params"` (defaults to `{}`).

Success responses have `{"jsonrpc":"2.0","id":...,"result":...}`. Error responses have `{"jsonrpc":"2.0","id":...,"error":{"code":...,"message":"..."}}`. Server notifications have `{"jsonrpc":"2.0","method":"event","params":{"type":"...",...}}`.

| Code | Meaning |
| --- | --- |
| `-32700` | Parse error (malformed JSON; `id` is `null`; server keeps running). |
| `-32600` | Invalid request (`jsonrpc` is not `"2.0"`, `method` is not a string, or `params` is not an object). Echoes supplied `id` when present, else `null`. |
| `-32601` | Unknown method. |
| `-32602` | Invalid params (missing/wrong-type param, unsupported `protocolVersion`, invalid `effort`, or invalid `config.set` `key`). |
| `-32000` | Session/domain error (missing prompt/file, empty input, unavailable model, unknown task/command, invalid reload, locked/missing session, invalid UI request ID or picker option). |
| `-32002` | Not ready: any method other than `initialize` or `shutdown` called before a successful `initialize`. |

## Handshake and Event Ordering

- The client must send `initialize` first (`shutdown` is also accepted before initialization).
- `protocolVersion` defaults to `1` when omitted or `null`. Any other value fails with `-32602` and leaves the connection uninitialized so the client can retry.
- Events emitted before the JSON-RPC server attaches its session subscriber are buffered up to a pre-subscription cap of 64 events; once subscribed, events queue in the server's channel and are delivered **only after** the `initialize` response is written.
- After initialization, ordinary `event` notifications are emitted asynchronously by a background pump and **can arrive before or after** the reply to a concurrent or triggering method call (for example, `usage` before `model.set` returns, or `reloaded` before `config.reload` returns). Clients must correlate responses by `id`.
- On `session.new` and `session.resume`, remaining events from the old session are drained before the switch response, and events from the new session begin arriving after the response.

## Methods

### Lifecycle

- **`initialize`**:
  - Params: `{ protocolVersion?: 1, client?: object }` (`client` is accepted and ignored).
  - Result: `{ protocolVersion: 1, tuse: string, session: SessionInfo, model: string, effort: string, capabilities: { methods: string[] } }`.
- **`shutdown`**:
  - Params: `{}`.
  - Result: `null`. Closes the active session and exits.

### Turns

- **`turn.submit`**:
  - Params: `{ text: string, prompts?: string[], images?: string[], files?: string[] }`.
    - `text` is required and preserved as given.
    - `prompts`: names from config `prompts` (deduplicated; unknown name fails with `-32000`).
    - `images`: `data:` URLs (attached directly) or file paths relative to `cwd` (read, base64-encoded, and tagged `<image id="N" path="..."/>`; missing file fails with `-32000`).
    - `files`: file or directory paths relative to `cwd` (deduplicated; images attach as images, directories as `<dir>`, text files up to 1 KiB as `<file>`, binary files as `<file ... binary="true"/>`; missing path fails with `-32000`). See [prompts.md](prompts.md).
    - Fails with `-32000` (`no input`) if `text` (trimmed), hidden context, and `images` are all empty.
  - Result: `{ inputId: number, queued: boolean }` (`queued` is `true` if a turn was already running).
- **`turn.withdraw`**:
  - Params: `{ inputId: number }`.
  - Result: `boolean` (`true` if the queued message was removed before the agent consumed it; `false` otherwise).
- **`turn.cancel`**:
  - Params: `{}`.
  - Result: `null`. Stops running custom commands, cancels running hooks, and cancels the active turn (clearing queued inputs, emitting `turn_end` with `cancelled: true`, and firing `agent_message` hook with `status = "cancelled"`). Does not stop background tasks.

### Sessions

`SessionInfo` is `{ id: string, title: string, cwd: string, busy: boolean, turn: { elapsedMs: number, status: string } | null }`.

- **`session.info`**:
  - Params: `{}`.
  - Result: `SessionInfo`.
- **`session.snapshot`**:
  - Params: `{}`.
  - Result: the full `session()` snapshot object ([functions.md](functions.md#session)) including `trace`. Optional absent fields are omitted.
- **`session.records`**:
  - Params: `{}`.
  - Result: the session's live in-memory record list in chronological order (including records buffered before a new session materializes in SQLite on the first user message, such as the initial `settings` record and `start` hook records). Canonical record schemas, tagged by `"t"`:
    - `"user"`: `{ t, text, context?, images?, at }`
    - `"assistant"`: `{ t, text, reasoning?, tool_calls?: [{ id, name, arguments }], replay?: { provider, api, model, items }, at }` (`replay` is omitted when absent/empty)
    - `"tool"`: `{ t, id, tool, input, output, code: int | null, images?, diffs?: [{ path, diff }], at }`
    - `"notification"`: `{ t, text }`
    - `"notice"`: `{ t, text }`
    - `"error"`: `{ t, text }`
    - `"compaction"`: `{ t, prune, drop }`
    - `"usage"`: `{ t, model, usage: { input, output, cached, cache_write }, llm?: true }`
    - `"settings"`: `{ t, model, effort }` (on resume, the latest recorded `settings` wins even when `model` is `""`)
    - `"hook"`: `{ t, name, event, vars: string, error: string | null, ms }` (`vars` is a newline-separated `key=value` formatted string excluding `event` and `null` fields)
    - `"turn_end"`: `{ t, elapsed_ms, calls, model, effort, cancelled }`
    - `"ui_markdown"`: `{ t, name, text, pin, removed }` (durable `show()` event in chronological order; backward-compatible additive v1 tag; excluded from model context and trace)
    - `"ui_result"`: `{ t, id, kind: "picker" | "question", prompt, value }` (durable answered `pick()`/`ask()` entry in chronological order; backward-compatible additive v1 tag; excluded from model context and trace)
- **`session.trace`**:
  - Params: `{}`.
  - Result: array of `{ role: string, summary: string, text: string }`.
- **`session.stats`**:
  - Params: `{}`.
  - Result: `{ input, output, cached, cacheWrite, cost: number | null, llmCost: number | null, llmRequests, requests, context, turns, calls }`:
    - `input`, `output`, `cached`, `cacheWrite`: cumulative token totals across main turns, `llm()`, and resumed history (`input` includes `cached` and `cacheWrite`).
    - `cost`, `llmCost`: cumulative USD totals (total and `llm()` subset), or `null` when no priced usage has been recorded.
    - `requests`: main-turn usage record count; `llmRequests`: `llm()` usage record count.
    - `context`: latest main-turn `input + output` (or estimated tokens immediately after compaction; on resume restores the last recorded main-turn usage `input + output`, not a post-compaction estimate).
    - `turns`: recorded `turn_end` count.
    - `calls`: cumulative tool calls from recorded `turn_end` entries plus the active turn's in-progress calls while `busy` is `true`.
- **`session.list`**:
  - Params: `{ cwd?: string }` (omitted, `null`, or non-string `cwd` falls back to the active session's `cwd`).
  - Result: array of `{ id, cwd, title, model, effort, created, updated }` for `cwd`, ordered by `updated` descending, excluding untitled sessions.
- **`session.new`**:
  - Params: `{}`.
  - Result: `{ session: SessionInfo, model: string, effort: string }`.
  - Prepares the new session (re-reading and validating config) before closing the current session (`reason = "new"`). If preparation fails, returns `-32000` and keeps the current session open.
- **`session.resume`**:
  - Params: `{ id: string }`.
  - Result: `{ session: SessionInfo, model: string, effort: string }`.
  - Fails with `-32000` if `id` is already open, does not exist, is locked by another process, or config fails to load (keeping the current session active). On success, closes the current session (`reason = "resume"`), opens the resumed session (inheriting its saved `cwd` if the directory exists, plus its saved model and effort; if the last assistant message with tool calls lacks tool results, appends placeholder interrupted tool records; replays persisted `ui_markdown` and `ui_result` records before `start` hooks fire while leaving pending prompts cleared), and switches the event stream. Read `session.records` for full chronological history (including interleaved `ui_markdown`/`ui_result` records) and `ui.state` for the effective restored UI blocks, answered entries, and any prompt queued by a `start` hook.

### Models

- **`model.get`**:
  - Params: `{}`.
  - Result: `{ model: string, effort: string }`.
- **`model.set`**:
  - Params: `{ model: string, effort?: string | null, persist?: boolean | null }`.
    - `effort`: omitted or `null` defaults to `"default"`; must be one of `default`, `low`, `medium`, `high`, `xhigh`, `max` (`-32602` otherwise).
    - `persist`: omitted or `null` defaults to `true` (`-32602` if non-boolean); when `true`, saves `model` and `effort` in SQLite as defaults for future new sessions.
    - `model`: `""` is accepted to clear the active model. Any non-empty `model` must match a provider in the session's **loaded provider snapshot** (not live SQLite or the remote catalog; external `tuse connect` / `tuse disconnect` changes take effect on reload/reopen), else `-32000`.
  - Result: `null`. Every successful call appends a `settings` record, updates session metadata, and emits `usage` (even if `(model, effort)` is unchanged); the `model_change` hook fires only when the pair actually changes.
- **`model.list`**:
  - Params: `{ cached?: boolean | null }` (omitted or `null` defaults to `false`; non-boolean fails with `-32602`).
  - Result: sorted, deduplicated `string[]` of `"provider/model"` names currently known (configured models whose provider exists in the loaded snapshot plus remote provider models fetched in this config generation).
  - Never waits for provider discovery: when `cached` is `false`, returns the current snapshot immediately **and** launches background provider model fetches. Each provider that responds updates the cache and emits a `models` event (failures emit `error` events). On `models`, call `model.list { "cached": true }` to read the refreshed list without triggering another fetch. In-flight fetches started before a `config.reload` are discarded.

### Config

- **`config.get`**:
  - Params: `{}`.
  - Result: structured resolved config object (no provider credentials, no Lua function bodies):
    - `settings`: `{ compact_at?: u64, max_image_bytes: int, retries: int, ui: { fold_strings, tool_line, diff_line, output_line, reasoning_line, scroll_step, pad_x, message_gap, list_max_height, input_max_height } }` (`compact_at` omitted when unset)
    - `theme`: `{ bg, surface, fg, dim, muted, border, accent, ok, err, info }` (strings)
    - `models`: `[{ name, display, window, prices: { input, output, cache_read, cache_write } | null, compact_at: u64 | null }]`
    - `systems`: `string[]`
    - `prompts`: `{ [name]: string }`
    - `functions`: `{ [name]: "custom" | "override" | "disabled" }`
    - `hooks`: `{ [name]: { events: string[] } }`
    - `commands`: `string[]` (sorted custom command names)
    - `open`: `string[]` (sorted file extensions without leading `.`)
    - `path`: `string` (`""` when running on defaults)
    - `plugins`: `[{ name, dir, ref?, commit? }]` (`ref` and `commit` omitted when absent)
- **`config.reload`**:
  - Params: `{}`.
  - Result: `null` (or `-32000` on plugin sync, load, or validation failure, reporting the failing plugin/path and leaving the previous config and callbacks active). Synchronizes enabled Git plugins (cloning missing checkouts and updating moving refs; see [plugins.md](plugins.md)), emits `reloaded` on success, and preserves live UI state, tasks, and active model.
- **`config.set`** *(deprecated; prefer `model.set`)*:
  - Params: `{ key: "model" | "effort", value: string }` (any other `key` fails with `-32602`).
  - Equivalent to `model.set` changing only that field with `persist: true` and the same validation rules.
  - Result: `null`.

### Tasks and Custom Commands

- **`tasks.list`**:
  - Params: `{}`.
  - Result: array of `{ id, name, status, started, ended, log }`.
- **`tasks.get`**:
  - Params: `{ id: string, lines?: number }` (omitted, `null`, negative, or non-integer `lines` defaults to `50`; `0` returns `output: ""`).
  - Result: `{ id, name, status, started, ended, log, output }`. Fails with `-32000` (`no task <id>`) if unknown.
- **`tasks.stop`**:
  - Params: `{ id: string }`.
  - Result: `{ id, name, status, started, ended, log }`. Fails with `-32000` if unknown.
- **`commands.list`**:
  - Params: `{}`.
  - Result: sorted `string[]` of configured custom command names.
- **`commands.run`**:
  - Params: `{ name: string }`.
  - Result: `null`. Starts `/name` asynchronously as a Lua job; fails with `-32000` (`unknown command /<name>`) if not configured.

### Interactive UI

- **`ui.state`**:
  - Params: `{}`.
  - Result: `{ markdowns: [{ name, text, pin }], active: UiRequest | null, entries: [{ id, kind, prompt, value }] }`. On resume, persisted `ui_markdown` and `ui_result` records replay before `start` hooks run (with `active = null` at the replay boundary and the UI request ID counter initialized above all restored `ui_result.id` values), so `ui.state` reflects surviving restored blocks, restored answered entries, and any blocks or active prompt created by `start` hooks.
- **`ui.reply`**:
  - Params: `{ id: number, value: string }`.
  - Result: `null`. Answers the active `ui_request`, persists a `ui_result` record, emits `ui_result`, and emits the next queued `ui_request` if any. Validates strictly against the current session incarnation's active queue (stale IDs from before close/switch are rejected). Fails with `-32000` if there is no active request, `id` does not match the active request, or (for a picker) `value` is not in `options`.
- **`ui.cancel`**:
  - Params: `{ id: number }`.
  - Result: `null`. Cancels the active `ui_request` (returns `nil` to Lua, leaves no `UiEntry`, emits **no** `ui_clear`, and emits the next queued `ui_request` if any). The client clears its own prompt when `ui.cancel` succeeds. Fails with `-32000` if there is no active request or `id` does not match.

`ui_request` makes a prompt active; `ui_clear { id }` means Tuse withdrew the active request on its own (job cancelled, session closed, or reload dropped a hook's prompt). Call `ui.state` at any time to resync.

## Events

Every session event arrives as a JSON-RPC notification with `"method": "event"` and `"params"` containing `"type"`. All four provider API types (`openai-chat`, `openai-responses`, `anthropic-messages`, `google-gemini`) emit the same streaming and lifecycle event types:

| `type` | Additional `params` fields | Meaning |
| --- | --- | --- |
| `notification` | `text` | Internal notification queued for the agent (e.g., background task completion). |
| `user` | `inputId`, `text`, `images` (count) | User message accepted into the turn. |
| `turn_start` | — | A turn began. |
| `text` | `text` | Streamed assistant text delta. |
| `reasoning` | `text` | Streamed model reasoning delta. |
| `stream_reset` | — | A retry started; discard uncommitted streamed deltas from the failed attempt. |
| `assistant` | `text` | Final assistant text for a step. |
| `tool_delta` | `index`, `id`, `name`, `code` | Streamed `lua` tool code chunk. |
| `tool_start` | `timeout` (`120`), `id`, `tool`, `input` | A tool call started executing. |
| `tool_end` | `id`, `output`, `code` (`int \| null`), `diffs` (`[{ path, diff }]`) | A tool call finished. |
| `status` | `text` | Transient activity label (first line of argument, up to 60 characters, e.g., `reading path...`, `running cmd...`; identical consecutive values are suppressed). |
| `notice` | `text` | Informational notice (e.g., compaction summary or `hook NAME: ...` error). |
| `flash` | `text` | Short transient banner (e.g., retry countdown). |
| `error` | `text` | Error message (turn/provider failure, unsupported provider capability, command error, model fetch error, deferred reload error). |
| `turn_end` | `elapsedMs`, `calls`, `model`, `effort`, `cancelled` | Turn completed or was cancelled. |
| `tasks` | — | Task list or status changed; re-read with `tasks.list` / `tasks.get`. |
| `usage` | — | Usage totals or model/effort changed; re-read with `session.stats` / `model.get`. |
| `models` | — | A provider finished fetching models; call `model.list { cached: true }`. |
| `reloaded` | — | A config reload committed (from `config.reload`, `/reload`, or Lua `reload()`). |
| `ui_markdown` | `name`, `text`, `pin`, `removed` | A `show()` block was created, updated, moved, or removed. |
| `ui_request` | `id`, `kind` (`"picker"` with `options: string[]`, or `"question"` with `text: string`) | An interactive Lua prompt became active. |
| `ui_clear` | `id` | Server withdrew the active prompt (not emitted after `ui.cancel`). |
| `ui_result` | `id`, `kind` (`"picker"` \| `"question"`), `prompt`, `value` | An interactive prompt was answered. |
