# Providers & API Client

Canonical reference for Tuse's provider configuration, secret handling, model discovery HTTP requests, OpenAI-compatible Chat Completions wire contract (`POST <base_url>/chat/completions` and `GET <base_url>/models`), streaming, usage extraction, errors, and retries.

For model metadata (`models` in Lua config), selection UX, and session scoping, see [models.md](models.md); for cost calculation, display, and 50/25/25 context compaction, see [usage.md](usage.md); for Lua config keys, see [config.md](config.md).

## Storage, Snapshots, and Secrets

- Connected providers live in SQLite at `~/.tuse/tuse.db`, each storing `name`, `base_url`, and `api_key`.
- `api_key` may be an environment variable reference (`$VAR` or `${VAR}`, where `VAR` consists of ASCII letters, digits, and `_`) or a literal string.
- Tuse loads providers from SQLite into the session's config snapshot when a session opens and re-reads them on reload (`/reload`, `reload()`, `config.reload`), resolving `$VAR` / `${VAR}` against the current process environment each time.
- A running session checks provider availability and credentials against its loaded snapshot, not live SQLite queries. Adding or removing a provider via `tuse connect` or `tuse disconnect` in another terminal takes effect in an already-open session only after reload or reopening.
- If a referenced environment variable is unset at runtime, it resolves to an empty string and requests are sent with an empty bearer token (`Authorization: Bearer `) until the variable is set and reloaded or the provider is updated.
- Provider secrets are never exposed by `/config`, `config.get`, `/session`, `session.snapshot`, `tuse providers`, or `require("tuse").session()`.

## Managing Providers

Manage providers with the CLI:

```sh
tuse connect --base https://api.openai.com/v1 --key '$OPENAI_API_KEY' --name openai
tuse providers
tuse disconnect openai
```

### `tuse connect`

`tuse connect --base URL --key '$ENV' --name NAME`

- All three flags (`--base`, `--key`, `--name`) are required and must be non-blank.
- `--base` must start with `http://` or `https://`. Trailing `/` characters are stripped before building endpoint URLs.
- `--name` cannot contain `/` or whitespace, and must not already exist; to replace a provider, remove it with `tuse disconnect NAME` first.
- Quote `'$ENV'` in your shell so the variable reference is stored in SQLite rather than the expanded secret. `tuse connect` resolves `--key` immediately and fails before making any network request if the referenced variable is unset or empty.
- Before saving, Tuse sends a single non-retried `GET <base_url>/models` request (`Authorization: Bearer <resolved_key>`, 30 s connect / 600 s read timeout) and parses the response body as JSON, extracting string `id` values from `data[]`.
  - On HTTP, network, or JSON parse failure, nothing is saved and the error is printed as `can't fetch models from <base_url>: <error>`.
  - Any HTTP 200 response containing valid JSON succeeds and saves the provider, even if `data` is missing or yields `0` usable model IDs; this check does not prove that `POST <base_url>/chat/completions` works.
- On success, Tuse prints `Received <N> models. Saved \`<NAME>\`.`

### `tuse providers` and `tuse disconnect`

- `tuse providers` prints all saved providers in insertion order, one per line as `<NAME><tab><BASE_URL>` (keys are never printed).
- `tuse disconnect NAME` deletes the provider from SQLite and prints `Removed \`<NAME>\``, or fails with `no provider \`<NAME>\`` if it does not exist.

## Model Naming and Background Discovery

Models are addressed as `<provider>/<model_id>` (split on the first `/`, sending everything after the first `/` verbatim as `model`). For model metadata, selection, and session isolation, see [models.md](models.md).

- Opening `/models` or calling `model.list` (unless `cached: true`) returns immediately without waiting for network I/O and starts one background `GET <base_url>/models` request per connected provider in the session's loaded snapshot (single attempt, no retries).
- Each successful response reads string `data[].id` entries, caches `<provider>/<id>` in memory until the next reload, and emits a `models` event; a failure emits an `error` event (`<provider>: can't fetch models: <error>`).
- Reloading clears cached remote models and increments the session's config generation so stale in-flight fetch responses and errors from before the reload are silently discarded.
- Running a turn against a missing provider fails with `no \`connect\` entry named \`<provider>\` for model \`<model>\``; `llm()` fails with `llm: no provider \`<provider>\` for model \`<model>\``.

## Chat Completions Requests

Both agent turns and Lua `llm(prompt, model, effort)` use the session's shared HTTP client and the same endpoint:

- **Endpoint**: `POST <base_url>/chat/completions`
- **Headers**: `Authorization: Bearer <resolved_key>`, `Content-Type: application/json`
- **HTTP timeouts**: 30 seconds connect timeout, 600 seconds read timeout.
- **JSON payload**:
  - `model`: model ID after the first `/`.
  - `messages`: message array (see below).
  - `stream`: `true` (always requested).
  - `stream_options`: `{"include_usage": true}`.
  - `tools`: omitted when empty (such as in `llm()`); for agent turns, contains the single function tool `lua` with required string parameter `code`.
  - `reasoning_effort`: omitted when effort is `"default"`; otherwise set to the active effort (`"low"`, `"medium"`, `"high"`, `"xhigh"`, or `"max"`).

### Message Construction

- **Agent turns**:
  - Starts with `{"role": "system", "content": "<system prompt>"}`.
  - Conversation records follow compaction bounds (see [usage.md](usage.md)):
    - **Dropped prefix**: only user text, task notifications (`role: "user"`), and non-empty assistant text (`role: "assistant"`) are sent; hidden context, images, and tool calls/results are omitted.
    - **Pruned middle**: full user/assistant/tool messages are sent, except tool outputs are shortened to their first line (capped at 200 characters) plus `\n[tool output pruned]` and tool-returned images are omitted.
    - **Recent suffix**: full messages and images are sent.
  - User messages with hidden context (`#` prompts, `@` files/dirs, image tags) append the context after `\n\n`.
  - User messages with images send `content` as an array of `{"type": "text", "text": "..."}` followed by `{"type": "image_url", "image_url": {"url": "data:<mime>;base64,..."}}` parts.
  - Assistant messages include `content` and, when tool calls were made, `tool_calls` (`{id, type: "function", function: {name, arguments}}`).
  - Tool results use `role: "tool"`, `tool_call_id`, and `content`. Every assistant `tool_calls` entry is paired with a matching `role: "tool"` message (interrupted or crashed turns receive placeholder `[interrupted: no output]` tool results before the next request).
  - Images returned by `read()` during tool calls (outside pruned/dropped zones) are flushed immediately after their tool results—before the next user, assistant, or notification message, or at the end of the array—as a synthetic `role: "user"` message containing `[Images from the tool results above]`, `[Image: <input>]`, and `image_url` parts.
- **Lua `llm(prompt, model, effort)`**:
  - Sends `[{"role": "user", "content": "<prompt>"}]` with no system prompt, no session history, and no `tools`.

## Streaming and Non-Streaming Responses

Response parsing is selected by the HTTP `Content-Type` header:

- **SSE (`Content-Type` contains `event-stream`)**: reads line by line. Non-`data:` lines (blank lines, comments such as `: ping`, `event:` headers) are ignored. The stream ends normally on `data: [DONE]` or when the response reaches clean EOF. Every other `data:` payload is parsed as JSON.
- **Non-streaming fallback**: for any other `Content-Type`, reads the entire body as a single JSON object and processes it as one chunk (supporting proxies or servers that ignore `stream: true`).

For each parsed JSON object:

1. If a non-null top-level `error` field is present, parsing stops with an HTTP status error using `error.code` (defaulting to `500` if absent or non-numeric) and the serialized error object as the body.
2. If `usage` is a JSON object, it updates the reply's usage snapshot (later `usage` objects replace earlier ones).
3. From `choices[0]` (if present):
   - `finish_reason` is recorded when present as a string.
   - `delta` is read, falling back to `message` for non-streaming responses.
   - Both `reasoning_content` and `reasoning` string fields are checked in order; if both are present on the same chunk, both are appended to reply reasoning and emitted as `reasoning` deltas.
   - `content` appends to reply text and emits `text` deltas.
   - `tool_calls` elements accumulate by slot: `index` selects the slot when present; otherwise a non-empty `id` opens a new slot and an empty `id` appends to the last slot. `id` overwrites when non-empty; `function.name` and `function.arguments` concatenate across chunks.
   - As `function.arguments` streams for a named tool call, Tuse incrementally decodes the top-level JSON string value of `"code"` (supporting standard JSON escapes and `\uXXXX` including UTF-16 surrogate pairs; malformed escapes drop only the invalid escape sequence). For tool `lua`, the initial chunk once `name` is known and every non-empty decoded `code` slice emit a `tool_delta` event so TUI and `tuse json` clients can stream Lua source before execution begins.
4. When the stream finishes, tool call slots with an empty `name` are removed first; then any remaining call with an empty `id` is assigned `call_<index>` using its post-filter 0-based index.

### Cancellation Behavior

- HTTP cancellation is **cooperative**, not an immediate socket abort:
  - In SSE streams, cancellation is checked as each line arrives from the stream (an in-flight HTTP connect/POST or blocked line read waits until bytes arrive or the HTTP timeout expires).
  - During retry backoff sleeps, cancellation is checked every 100 ms.
  - The non-streaming JSON fallback has no mid-read cancellation check.
- On turn cancellation (Esc or `turn.cancel`), the session immediately invalidates the turn's generation and emits `turn_end` (`cancelled: true`), ignoring any subsequent output from the unwinding HTTP thread.
- For `llm()`, cancellation is bound to the calling Lua job: cancelling a turn cancels an `llm()` call inside an agent `lua` tool call, whereas cancelling a turn does **not** cancel an `llm()` call running inside a background task (which is cancelled only by `task_stop()` or session close).

## Usage Accounting

Each `usage` JSON object maps to four token counters (missing fields default to `0`):

| Tuse field | Chat Completions JSON field |
| --- | --- |
| `input` | `prompt_tokens` |
| `output` | `completion_tokens` |
| `cached` (cache read) | `prompt_tokens_details.cached_tokens` |
| `cache_write` | `max(prompt_tokens_details.cache_write_tokens, prompt_tokens_details.cached_creation_tokens)` |

- On an agent turn step, if the reply includes `usage`, Tuse updates the session context size (`ctx`) to `input + output` and appends a usage record; `usage` events are emitted at the end of every completed assistant step.
- On `llm()`, Tuse appends a separate usage record (`llm: true`, without changing `ctx`) and emits a `usage` event **only** when the reply includes a `usage` object.
- See [usage.md](usage.md) for pricing formulas, display, and compaction.

## Errors and Retries

Errors fall into four categories:

| Category | Formatted message | Retried? |
| --- | --- | --- |
| HTTP error status / inline JSON `error` | `HTTP <code>: <body on one line, max 300 chars + …>` | Yes for `429` and `>= 500`; No for other `4xx` |
| Network / I/O failure | `network: <message>` | Yes |
| Malformed JSON response | `bad response: <message>` | No |
| Cancellation | `cancelled` | No |

- **Retry policy**: configured by `settings.retries` (`0..=100`, default `5`). Backoff delays are fixed at `[2, 5, 10, 20, 30]` seconds. The effective retry limit is `min(settings.retries, 5)` (at most 5 retries / 6 total attempts; `0` disables retries). Only completion requests (`complete`) retry; `GET /models` requests never retry.
- **Agent turn retries**: before each retry sleep, Tuse emits `stream_reset` (discarding partial streamed text, reasoning, and pending tool call entries from the failed attempt) and a `flash` event formatted as `Retry <attempt>/<max> in <delay>s: <error>`.
- **`llm()` retries**: uses the same retry schedule and cancellation handling silently (no `stream_reset` or `flash` events).
- **Terminal failure**:
  - On an agent turn step, when retries are exhausted or a non-retryable error occurs, Tuse saves an `error` record, emits `error`, and ends the failed step. If no further input is queued, the turn finishes and emits `turn_end`; if another user message or task notification is already queued, the turn continues with the queued input.
  - A cancelled turn step stops without recording an API error.
  - In `llm()`, terminal failure or cancellation raises a Lua error prefixed with `llm: `.
