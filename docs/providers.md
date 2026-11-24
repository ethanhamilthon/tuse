# Providers & API Client

Canonical reference for Tuse's provider configuration, secret handling, four API protocol adapters (`openai-chat`, `openai-responses`, `anthropic-messages`, `google-gemini`), model discovery, streaming, reasoning replay, usage accounting, errors, and retries.

For model metadata (`models` in Lua config), selection UX, and session scoping, see [models.md](models.md); for cost calculation, display, and 50/25/25 context compaction, see [usage.md](usage.md); for Lua config keys, see [config.md](config.md).

## Storage, Snapshots, and Secrets

- Connected providers live in SQLite at `~/.tuse/tuse.db`, each storing `name`, `base_url`, `api_key`, and `api` (default `'openai-chat'`).
- On opening an older database lacking the `api` column, Tuse automatically migrates `providers` with `ALTER TABLE providers ADD COLUMN api TEXT NOT NULL DEFAULT 'openai-chat'`. Missing or empty `api` values are treated as `openai-chat`.
- `api_key` may be an environment variable reference (`$VAR` or `${VAR}`, where `VAR` consists of ASCII letters, digits, and `_`) or a literal string.
- Tuse loads providers from SQLite into the session's config snapshot when a session opens and re-reads them on reload (`/reload`, `reload()`, `config.reload`), resolving `$VAR` / `${VAR}` against the current process environment each time.
- A running session checks provider availability and credentials against its loaded snapshot, not live SQLite queries. Adding or removing a provider via `tuse connect` or `tuse disconnect` in another terminal takes effect in an already-open session only after reload or reopening.
- If a referenced environment variable is unset at runtime, it resolves to an empty string and requests are sent with an empty credential header until the variable is set and reloaded or the provider is updated.
- Provider secrets are never exposed by `/config`, `config.get`, `/session`, `session.snapshot`, `tuse providers`, `tuse providers --json`, or `require("tuse").session()`.

## Managing Providers

Manage providers with the CLI:

```sh
tuse connect --base https://api.openai.com/v1 --key '$OPENAI_API_KEY' --name openai
tuse connect --base https://api.openai.com/v1 --key '$OPENAI_API_KEY' --name openai-resp --api openai-responses
tuse connect --base https://api.anthropic.com/v1 --key '$ANTHROPIC_API_KEY' --name anthropic --api anthropic-messages
tuse connect --base https://generativelanguage.googleapis.com/v1beta --key '$GEMINI_API_KEY' --name gemini --api google-gemini
tuse providers
tuse providers --json
tuse disconnect openai
```

### `tuse connect`

`tuse connect --base URL --key '$ENV' --name NAME [--api TYPE]`

- `--base`, `--key`, and `--name` are required and must be non-blank for all four API types (no hidden default hostnames).
- `--api` is optional and defaults to `openai-chat`. Valid values are:
  - `openai-chat` — OpenAI-compatible Chat Completions
  - `openai-responses` — OpenAI Responses API
  - `anthropic-messages` — Anthropic Messages API
  - `google-gemini` — Native Google Gemini REST API
- An invalid `--api` value fails immediately before any network call with an error listing all four supported values.
- API type is a **provider-level** setting. To use two protocols against the same host (for example, Chat Completions and Responses), connect two named providers.
- `--base` must start with `http://` or `https://`. Trailing `/` characters are stripped before building endpoint URLs.
- `--name` cannot contain `/` or whitespace, and must not already exist; to replace a provider, remove it with `tuse disconnect NAME` first.
- Quote `'$ENV'` in your shell so the variable reference is stored in SQLite rather than the expanded secret. `tuse connect` resolves `--key` immediately and fails before making any network request if the referenced variable is unset or empty.
- Before saving, Tuse verifies the connection using the selected adapter's model-listing endpoint (single non-retried flow, 30 s connect / 600 s read timeout).
  - On HTTP, network, or JSON parse failure, nothing is saved and the error is printed as `can't fetch models from <base_url>: <error>`.
  - Any HTTP 200 response containing valid JSON succeeds and saves the provider, even if it yields `0` usable model IDs; this check verifies model listing rather than generation.
- On success, Tuse prints `Received <N> models. Saved \`<NAME>\`.`

### `tuse providers` and `tuse disconnect`

- `tuse providers` prints all saved providers in insertion order, one per line as `<NAME><tab><BASE_URL>` (strictly two columns for backward compatibility; keys are never printed).
- `tuse providers --json` prints a JSON array `[{"name": "...", "base_url": "...", "api": "..."}]` (never exposing `api_key`).
- `tuse disconnect NAME` deletes the provider from SQLite and prints `Removed \`<NAME>\``, or fails with `no provider \`<NAME>\`` if it does not exist.

## Capability Matrix

Users can switch freely between models backed by different API types within the same session and across `/resume`, because stored session history is translated per request by the active provider's adapter.

| Capability | `openai-chat` | `openai-responses` | `anthropic-messages` | `google-gemini` |
| --- | --- | --- | --- | --- |
| Streaming text (`text` deltas) | Yes | Yes | Yes | Yes |
| Streaming reasoning (`reasoning` deltas) | Yes (`reasoning_content` / `reasoning`) | Yes (reasoning summary / reasoning deltas) | Yes (`thinking_delta`) | Yes (`thought: true` text parts) |
| Multi-step reasoning + tool loop replay | Plain text + `tool_calls` | Ordered `reasoning` (`encrypted_content`) + `function_call` items | Unmodified `thinking` / `redacted_thinking` + `signature` + `tool_use` blocks | Ordered `parts` preserving part-level `thoughtSignature` |
| Tool calling (`lua`) | Yes | Yes | Yes | Yes |
| Incremental `tool_delta` code streaming | Yes (chunked JSON args) | Yes (chunked JSON args) | Yes (`input_json_delta`) | Single shot per call (emitted once when complete `functionCall` arrives) |
| Vision / images (user & tool `read()`) | Yes (`image_url` data URLs) | Yes (`input_image` data URLs) | Yes (base64 `image` blocks, including inside `tool_result`) | Yes (`inlineData` base64 parts) |
| Reasoning effort levels | `default`, `low`, `medium`, `high`, `xhigh`, `max` | `default`, `low`, `medium`, `high`, `xhigh` (`max` unsupported) | `default`, `low`, `medium`, `high`, `max` (`xhigh` unsupported) | `default`, `low`, `medium`, `high` (`xhigh`, `max` unsupported) |
| Prompt cache read tokens (`cached`) | Yes | Yes | Yes | Yes |
| Prompt cache write tokens (`cache_write`) | Yes | No (reports `0`) | Yes | No (reports `0`) |
| Model listing (`GET /models`) | Yes (`data[].id`, single page) | Yes (`data[].id`, single page) | Yes (`data[].id`, paginated up to 10 pages × 100) | Yes (`models[].name`, `models/` stripped, paginated up to 10 pages × 100) |
| Non-streaming JSON fallback | Yes | Yes | Yes | Yes |
| Single-shot `llm()` | Yes | Yes | Yes | Yes |

## Model Naming and Background Discovery

Models are addressed as `<provider>/<model_id>` (split on the first `/`, sending everything after the first `/` verbatim as `model`). For model metadata, selection, and session isolation, see [models.md](models.md).

- Opening `/models` or calling `model.list` (unless `cached: true`) returns immediately without waiting for network I/O and starts one background model-listing request per connected provider in the session's loaded snapshot (no retries).
- Each successful response caches `<provider>/<id>` in memory until the next reload and emits a `models` event; a failure emits an `error` event (`<provider>: can't fetch models: <error>`).
- Reloading clears cached remote models and increments the session's config generation so stale in-flight fetch responses and errors from before the reload are silently discarded.
- Running a turn against a missing provider fails with `no \`connect\` entry named \`<provider>\` for model \`<model>\``; `llm()` fails with `llm: no provider \`<provider>\` for model \`<model>\``.

## Shared Request Building, Compaction & Reasoning Replay

Both agent turns and Lua `llm(prompt, model, effort)` build a protocol-agnostic request (`provider`, `model`, `system`, `messages`, `tools`, `effort`) and hand it to the provider's adapter. All adapters share the same HTTP timeouts (30 s connect, 600 s read), retry schedule, cooperative cancellation, and TUI / `tuse json` event stream (`text`, `reasoning`, `tool_delta`, `stream_reset`, `assistant`, `tool_start`, `tool_end`, `usage`, `models`, `flash`, `error`, `turn_end`).

### Conversation Bounds and Message Construction

- **Agent turns**:
  - `system`: built-in system prompt plus configured `systems` (and compaction note when `drop > 0`).
  - Conversation records follow 50/25/25 compaction bounds (see [usage.md](usage.md)):
    - **Dropped prefix** (`index < drop`): only user text, task notifications, and non-empty assistant text are sent; hidden context, images, tool calls/results, and `replay` metadata are omitted (`drop` always aligns on a `user`/`notification` boundary so an active tool loop is never split).
    - **Pruned middle** (`drop <= index < prune`): full user/assistant/tool messages and `replay` metadata are retained, except tool outputs are shortened to their first line (capped at 200 characters) plus `\n[tool output pruned]` and tool-returned images are omitted.
    - **Recent suffix** (`index >= prune`): full messages, `replay` metadata, and images are sent.
  - User messages with hidden context (`#` prompts, `@` files/dirs, image tags) append the context after `\n\n`.
  - Every assistant tool call is paired with a matching tool result (interrupted or crashed turns receive placeholder `[interrupted: no output]` tool results before the next request).
  - Token estimation for compaction counts human-readable text, tool calls/results, and images while ignoring opaque encrypted/signature blobs inside `replay` so opaque signatures do not cause premature compaction.
- **Lua `llm(prompt, model, effort)`**:
  - Sends a single user message containing `prompt` with empty `system`, no session history, and no `tools`.

### Reasoning Replay Metadata and Cross-Provider Fallback

Modern reasoning APIs require unmodified reasoning artifacts to be echoed back in exact order across multi-step tool loops and across `/resume`:

- Persisted `assistant` records carry an optional `replay` object (omitted when absent/empty):
  ```json
  {
    "api": "anthropic-messages | openai-responses | google-gemini",
    "model": "<raw model id>",
    "items": [ ... ]
  }
  ```
- **Same-API and same-model replay**: when an assistant step's `replay.api` and `replay.model` match the active request's API type and raw model ID (and the record is outside the dropped prefix), the adapter replays `replay.items` verbatim instead of reconstructing plain text + tool calls.
- **Cross-provider / cross-model fallback**: foreign `replay.items` are never sent to a different `api` or `model`. Adapters fall back cleanly to plain `text` + tool calls. On `anthropic-messages`, prior completed turns serialize as plain `text` + `tool_use`; if an **unfinished** tool loop (an assistant step with tool calls after the latest user message, such as after a crash/resume or mid-loop provider switch) lacks valid Anthropic `thinking` / `redacted_thinking` replay blocks for the active model, Tuse omits `thinking` and `output_config` on that recovery request rather than failing with HTTP 400.

## Per-Adapter Protocol Contracts

### 1. `openai-chat`

- **Endpoints**: `POST <base_url>/chat/completions`, `GET <base_url>/models` (single page, `data[].id`).
- **Headers**: `Authorization: Bearer <resolved_key>`, `Content-Type: application/json`.
- **Request**:
  - `model`, `stream: true`, `stream_options: {"include_usage": true}`.
  - `messages`: starts with `{"role": "system", "content": system}` when `system` is non-empty; user messages with images use `{"type": "text", ...}` and `{"type": "image_url", "image_url": {"url": ...}}`; assistant messages include `content` and optional `tool_calls`; tool results use `role: "tool"`, `tool_call_id`, `content`, and any tool-returned images are flushed after their tool results as a `role: "user"` message containing `[Images from the tool results above]`, `[Image: <input>]`, and `image_url` parts.
  - `tools`: omitted when empty; otherwise `[{"type": "function", "function": {"name", "description", "parameters"}}]`.
  - `reasoning_effort`: omitted when effort is `"default"`; otherwise set to `"low"`, `"medium"`, `"high"`, `"xhigh"`, or `"max"`.
- **Streaming SSE & non-streaming fallback**:
  - SSE (`Content-Type` contains `event-stream`): reads `data:` lines until `data: [DONE]` or EOF; other `Content-Type` parses the body as a single JSON object.
  - Non-null top-level `error` fails with `HTTP <error.code or 500>`.
  - `usage` updates reply usage; `choices[0]` provides `finish_reason`, `delta` (falling back to `message`), `reasoning_content` and `reasoning` deltas, `content` deltas, and `tool_calls` accumulated by slot (`index`, else non-empty `id` opens a slot, else last slot) with incremental `"code"` JSON extraction emitting `tool_delta` for `lua`.
  - On finish, drops calls with empty `name` and assigns `call_<index>` for empty `id`.
- **Usage**: `input <- prompt_tokens`, `output <- completion_tokens`, `cached <- prompt_tokens_details.cached_tokens`, `cache_write <- max(prompt_tokens_details.cache_write_tokens, prompt_tokens_details.cached_creation_tokens)`.

### 2. `openai-responses`

- **Endpoints**: `POST <base_url>/responses`, `GET <base_url>/models` (single page, `data[].id`).
- **Headers**: `Authorization: Bearer <resolved_key>`, `Content-Type: application/json`.
- **Request**:
  - `model`, `stream: true`, `store: false` (keeping local SQLite history and compaction authoritative).
  - `instructions`: system prompt when non-empty.
  - `input`: ordered array of items:
    - User message -> `{"role": "user", "content": [...]}` with `{"type": "input_text", "text": ...}` and `{"type": "input_image", "image_url": "data:..."}`.
    - Assistant step -> verbatim matching `replay.items` when available; otherwise `{"role": "assistant", "content": [{"type": "output_text", "text": ...}]}` (when text is non-empty) followed by `{"type": "function_call", "call_id": id, "name": name, "arguments": arguments}` per tool call.
    - Tool results -> `{"type": "function_call_output", "call_id": id, "output": output}` per result; if tool images exist, flushes a follow-up `role: "user"` item with `[Images from the tool results above]` and `input_image` parts.
  - `tools`: omitted when empty; otherwise `[{"type": "function", "name", "description", "parameters"}]`.
  - `reasoning` and `include`: omitted when effort is `"default"`; for `low`, `medium`, `high`, `xhigh`, sends `reasoning: {"effort": "<effort>", "summary": "auto"}` and `include: ["reasoning.encrypted_content"]`. Rejects `max` fail-fast.
- **Streaming SSE**:
  - `response.output_text.delta` -> `text` delta.
  - `response.reasoning_summary_text.delta` and `response.reasoning_text.delta` -> `reasoning` delta.
  - Tool-call slot correlation:
    - `response.output_item.added` (`item.type == "function_call"`): creates/selects slot by `output_index` and `item.id`, captures `call_id` (falling back to `item.id`) and `name`, and feeds any initial non-empty `item.arguments` to the incremental `"code"` extractor.
    - `response.function_call_arguments.delta`: matches slot by `item_id` (falling back to `output_index`), appends `delta`, and streams `"code"` via `tool_delta`.
    - `response.function_call_arguments.done` and `response.output_item.done` (`function_call`): backfills missing `call_id`/`name`; a non-empty `arguments` string on `done` is authoritative for the stored call (feeding the `"code"` extractor once only if no arguments were streamed earlier, avoiding duplicate `tool_delta` characters).
  - Replay capture: `response.output_item.done` records completed `reasoning` (only when `encrypted_content` is present and non-empty), `message`, and `function_call` items in `output_index` order into `replay.items`.
  - `response.completed` / `response.incomplete` -> records `finish` (`response.status`) and `response.usage`.
  - `response.failed` / top-level `error` -> maps `rate_limit_exceeded` (429), `server_error` (500), `service_unavailable` (503), and numeric `429` / `>= 500` to retryable status errors; maps client/validation codes (`invalid_request_error`, `context_length_exceeded`, `invalid_api_key`, etc.) to non-retryable `400`/`401`.
- **Non-SSE JSON fallback**:
  - Fails on non-null `error` or `status == "failed"` with the same retryability mapping; records `finish = status` and `usage`; walks `output[]` in order, emitting `reasoning` (`summary[].text`), `message` (`output_text`), and `function_call` (single-shot `tool_delta`) while capturing `replay.items` (including `reasoning` only when `encrypted_content` is present).
- **Usage**: `input <- input_tokens`, `output <- output_tokens`, `cached <- input_tokens_details.cached_tokens`, `cache_write <- 0`.

### 3. `anthropic-messages`

- **Endpoints**:
  - `POST <base_url>/messages`
  - `GET <base_url>/models?limit=100`: follows `&after_id=<last_id>` while `has_more == true`, `last_id` is non-empty, and `last_id` has not been seen, capped at 10 pages (up to 1,000 models); reads `data[].id`.
- **Headers**: `x-api-key: <resolved_key>`, `anthropic-version: 2023-06-01`, `Content-Type: application/json`.
- **Request**:
  - `model`, `stream: true`, `max_tokens: 16384`.
  - `system`: when non-empty, sent as `[{"type": "text", "text": system, "cache_control": {"type": "ephemeral"}}]`.
  - `messages`: strictly alternating `user` and `assistant` roles with consecutive same-role items merged into one `content` block array, attaching `cache_control: {"type": "ephemeral"}` to the last block of the latest `user` message:
    - User text -> `{"type": "text", "text": ...}`; user images -> `{"type": "image", "source": {"type": "base64", "media_type": mime, "data": b64}}`.
    - Assistant step -> verbatim matching `replay.items` (`thinking`, `redacted_thinking`, `text`, `tool_use`) when available; otherwise non-empty `text` blocks and `{"type": "tool_use", "id", "name", "input": <parsed JSON object, fallback {}>}`.
    - Tool results -> `role: "user"` blocks `{"type": "tool_result", "tool_use_id": id, "content": [...]}` carrying the text output and any tool-returned base64 image blocks inside `tool_result.content`.
  - `tools`: omitted when empty; otherwise `[{"name", "description", "input_schema"}]`.
  - Effort / thinking: omitted when effort is `"default"` (or during unfinished-loop recovery without valid Anthropic thinking replay); for `low`, `medium`, `high`, `max`, sends `thinking: {"type": "adaptive"}` and `output_config: {"effort": "<effort>"}`. Rejects `xhigh` fail-fast.
- **Streaming SSE**:
  - `message_start` -> initializes usage counters from `message.usage`.
  - `content_block_start` -> tracks `text`, `thinking`, `redacted_thinking` (no UI delta), and `tool_use` blocks by `index`.
  - `content_block_delta` -> `text_delta` (`text`), `thinking_delta` (`reasoning`), `signature_delta` (appends `signature` to active `thinking` block, no UI delta), `input_json_delta` (`partial_json` -> tool arguments + incremental `"code"` extractor -> `tool_delta`).
  - `content_block_stop` -> finalizes block at `index` into ordered `replay.items`.
  - `message_delta` -> updates `stop_reason` and final usage fields without clobbering earlier input/cache counters.
  - `ping` ignored; `message_stop` ends stream.
  - `error` event -> maps `rate_limit_error` (429), `overloaded_error` (529), `api_error` (500) to retryable status errors; `invalid_request_error` (400), `authentication_error` (401), `permission_error` (403) to non-retryable status errors.
- **Non-SSE JSON fallback**:
  - Fails on `type == "error"` or non-null `error`; records `finish = stop_reason` and normalized usage; walks `content[]` in order emitting `thinking`, `text`, and `tool_use` (single-shot `tool_delta`) while preserving `thinking`, `redacted_thinking`, `text`, and `tool_use` in `replay.items`.
- **Usage normalization**:
  - `cached <- cache_read_input_tokens`
  - `cache_write <- cache_creation_input_tokens`
  - `input <- input_tokens + cached + cache_write` (total prompt tokens, matching Tuse's `ctx = input + output` and `plain = input - cached - cache_write`)
  - `output <- output_tokens`

### 4. `google-gemini`

- **Endpoints**:
  - `POST <base_url>/models/<model_id>:streamGenerateContent?alt=sse`
  - `GET <base_url>/models?pageSize=100`: follows `&pageToken=<nextPageToken>` while `nextPageToken` is non-empty and unseen, capped at 10 pages (up to 1,000 models); strips leading `models/` from `models[].name` and skips entries whose `supportedGenerationMethods` is present and excludes `generateContent`.
- **Headers**: `x-goog-api-key: <resolved_key>`, `Content-Type: application/json` (the API key is never placed in URL query parameters).
- **Request**:
  - `systemInstruction`: `{"parts": [{"text": system}]}` when non-empty.
  - `contents`: array of `{"role": "user" | "model", "parts": [...]}` with consecutive same-role turns merged:
    - User text -> `{"text": ...}`; user images -> `{"inlineData": {"mimeType": mime, "data": b64}}`.
    - Assistant step (`role: "model"`) -> verbatim matching `replay.items` parts (preserving `thought`, `text`, `functionCall`, and part-level `thoughtSignature`) when available; otherwise `{"text": ...}` and `{"functionCall": {"id", "name", "args": <parsed JSON object, fallback {}>}}`.
    - Tool results -> `role: "user"` parts `{"functionResponse": {"id", "name", "response": {"output": output}}}` plus `inlineData` parts for tool-returned images.
  - `tools`: omitted when empty; otherwise `[{"functionDeclarations": [{"name", "description", "parameters"}]}]`.
  - Effort / thinking:
    - `"default"` omits `thinkingConfig`.
    - `low`, `medium`, `high`: for `gemini-2.5-*` models sends `generationConfig.thinkingConfig = {"includeThoughts": true, "thinkingBudget": 1024 | 8192 | 24576}`; for Gemini 3+ and other Gemini models sends `generationConfig.thinkingConfig = {"includeThoughts": true, "thinkingLevel": "<effort>"}`.
    - Rejects `xhigh` and `max` fail-fast.
- **Streaming SSE & non-streaming JSON fallback**:
  - Processes each SSE `data:` JSON chunk (or, for non-SSE responses, a single JSON object or JSON array of chunks):
    - Preserves received part objects into ordered `replay.items`, merging contiguous plain/thought text chunks only when neither part carries `thoughtSignature` (never dropping or moving a `thoughtSignature` across parts).
    - `thought == true` text -> `reasoning` delta; plain text -> `text` delta.
    - `functionCall` (`{id?, name, args}`) -> serializes `args` to JSON, assigns `call_<index>` when `id` is absent, and emits `tool_delta` once for `lua`.
  - `candidates[0].finishReason` -> `finish`.
  - Non-null top-level `error` -> `HTTP <error.code or 500>`.
- **Usage**:
  - `input <- usageMetadata.promptTokenCount`
  - `cached <- usageMetadata.cachedContentTokenCount`
  - `cache_write <- 0`
  - `output <- usageMetadata.candidatesTokenCount + usageMetadata.thoughtsTokenCount`

## Unsupported Capability Reporting

Unsupported capabilities are handled in two ways:

1. **Known adapter incompatibility (fail fast, non-retryable, zero HTTP requests)**:
   - Unsupported effort level for the provider's API type (for example `max` on `openai-responses`, `xhigh` on `anthropic-messages`, or `xhigh`/`max` on `google-gemini`).
   - Non-base64 image attachment (anything not matching `data:<mime>;base64,<data>`) on an adapter requiring inline base64 (`anthropic-messages`, `google-gemini`).
   - Error format: `provider \`<name>\` (<api_type>) does not support <capability> \`<value>\` (<hint>)`.
   - Never retried. On an agent turn, saves an `error` record and emits `error` (followed by `turn_end` when the queue is empty) in both TUI and `tuse json`. In `llm()`, raises a Lua error `llm: ...`.
2. **Graceful protocol differences (no error)**:
   - Missing `cache_write` reporting (`openai-responses`, `google-gemini`) reports `0`.
   - Non-incremental tool arguments (`google-gemini`) emit a single `tool_delta` with the full `code` before `tool_start`.
   - Upstream model-specific rejections (such as a non-thinking model rejecting `thinkingConfig`) surface verbatim as non-retryable `HTTP 4xx: ...` errors.

## Cancellation Behavior

- HTTP cancellation is **cooperative**, not an immediate socket abort:
  - In SSE streams, cancellation is checked as each line arrives from the stream.
  - During retry backoff sleeps, cancellation is checked every 100 ms.
  - The non-streaming JSON fallback has no mid-read cancellation check.
- On turn cancellation (Esc or `turn.cancel`), the session immediately invalidates the turn's generation and emits `turn_end` (`cancelled: true`), ignoring any subsequent output from the unwinding HTTP thread.
- For `llm()`, cancellation is bound to the calling Lua job: cancelling a turn cancels an `llm()` call inside an agent `lua` tool call, whereas cancelling a turn does **not** cancel an `llm()` call running inside a background task (which is cancelled only by `task_stop()` or session close).

## Usage Accounting

- On an agent turn step, if the reply includes `usage`, Tuse updates the session context size (`ctx`) to `input + output` and appends a usage record; `usage` events are emitted at the end of every completed assistant step.
- On `llm()`, Tuse appends a separate usage record (`llm: true`, without changing `ctx`) and emits a `usage` event **only** when the reply includes a `usage` object.
- See [usage.md](usage.md) for pricing formulas, display, and compaction.

## Errors and Retries

Errors fall into five categories:

| Category | Formatted message | Retried? |
| --- | --- | --- |
| HTTP error status / inline stream `error` | `HTTP <code>: <body on one line, max 300 chars + …>` | Yes for `429` and `>= 500`; No for other `4xx` |
| Network / I/O failure | `network: <message>` | Yes |
| Malformed JSON response | `bad response: <message>` | No |
| Unsupported adapter capability | `provider \`<name>\` (<api>) does not support ...` | No |
| Cancellation | `cancelled` | No |

- **Retry policy**: configured by `settings.retries` (`0..=100`, default `5`). Backoff delays are fixed at `[2, 5, 10, 20, 30]` seconds. The effective retry limit is `min(settings.retries, 5)` (at most 5 retries / 6 total attempts; `0` disables retries). Only completion requests (`complete`) retry; model-listing requests never retry.
- **Agent turn retries**: before each retry sleep, Tuse emits `stream_reset` (discarding partial streamed text, reasoning, and pending tool call entries from the failed attempt) and a `flash` event formatted as `Retry <attempt>/<max> in <delay>s: <error>`.
- **`llm()` retries**: uses the same retry schedule and cancellation handling silently (no `stream_reset` or `flash` events).
- **Terminal failure**:
  - On an agent turn step, when retries are exhausted or a non-retryable error occurs, Tuse saves an `error` record, emits `error`, and ends the failed step. If no further input is queued, the turn finishes and emits `turn_end`; if another user message or task notification is already queued, the turn continues with the queued input.
  - A cancelled turn step stops without recording an API error.
  - In `llm()`, terminal failure or cancellation raises a Lua error prefixed with `llm: `.
