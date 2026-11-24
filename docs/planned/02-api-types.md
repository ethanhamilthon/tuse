# Planned: Support Four API Types (TODO #2)

Status: **Planned** (not yet implemented). Baseline behavior is documented in [../providers.md](../providers.md).

## Goal

Extend Tuse's provider layer from OpenAI-compatible Chat Completions alone to four API types:

1. `openai-chat` — OpenAI-compatible Chat Completions (existing behavior, default)
2. `openai-responses` — OpenAI Responses API
3. `anthropic-messages` — Anthropic Messages API
4. `google-gemini` — Native Google Gemini REST API

Each API type lives behind a separate protocol adapter while preserving identical session history, streaming UX, `lua` tool lifecycle, compaction, `llm()`, and TUI / `tuse json` event contracts wherever the upstream API supports them.

## Provider Selection & Backward Compatibility

### API Type Selection

- Add an optional `--api TYPE` flag to `tuse connect`:
  ```sh
  tuse connect --base https://api.openai.com/v1 --key '$OPENAI_API_KEY' --name openai
  tuse connect --base https://api.openai.com/v1 --key '$OPENAI_API_KEY' --name openai-resp --api openai-responses
  tuse connect --base https://api.anthropic.com/v1 --key '$ANTHROPIC_API_KEY' --name anthropic --api anthropic-messages
  tuse connect --base https://generativelanguage.googleapis.com/v1beta --key '$GEMINI_API_KEY' --name gemini --api google-gemini
  ```
- Valid values for `TYPE`: `openai-chat`, `openai-responses`, `anthropic-messages`, `google-gemini`.
  - **Decision:** Use explicit kebab-case `<vendor>-<protocol>` identifiers (`openai-chat`, `openai-responses`, `anthropic-messages`, `google-gemini`) rather than short aliases (`chat`, `responses`, `anthropic`, `gemini`) so there is one unambiguous name across CLI, SQLite, and docs.
  - **Decision:** Keep `--base` required on `tuse connect` for all four API types (no hidden default hostnames) to stay consistent with Tuse's explicit-configuration model.
- When `--api` is omitted, it defaults to `openai-chat`.
- An invalid `--api` value fails immediately before any network call with an error listing the four supported values.
- API type is a **provider-level** setting. To use two protocols against the same host (for example, Chat Completions and Responses), connect two named providers (e.g., `openai` and `openai-resp`).
- `tuse connect` verifies the connection using the selected adapter's model-listing endpoint before saving to SQLite.
- `tuse providers` preserves its exact two-column default output `<NAME><tab><BASE_URL>` for backward compatibility with existing scripts and parsers, and adds an opt-in structured flag:
  - **Decision:** Keep `tuse providers` output strictly `<NAME><tab><BASE_URL>` (never appending a 3rd column by default), and add `tuse providers --json` returning a JSON array `[{"name": "...", "base_url": "...", "api": "..."}]` (never exposing `api_key`).

### Existing Provider Records (Backward Compatibility)

- Add column `api TEXT NOT NULL DEFAULT 'openai-chat'` to the `providers` table in `~/.tuse/tuse.db`.
- On opening an existing database without the `api` column, run an automatic schema migration (`ALTER TABLE providers ADD COLUMN api TEXT NOT NULL DEFAULT 'openai-chat'`).
- Rows with a missing or empty `api` value are treated as `openai-chat`.
- Existing `provider/model_id` identifiers, `models` config entries, saved default model/effort state, session history records, and `tuse disconnect NAME` remain unchanged.
- Users can switch freely between models backed by different API types within the same session (and across `/resume`) because stored session history is translated per request by the active provider's adapter.

## Capability Matrix

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

## Adapter Boundaries

The turn loop and `llm()` build a **protocol-agnostic request** rather than pre-formatted Chat Completions JSON:

- **Shared input to every adapter**:
  - `model`: raw model ID (provider prefix stripped)
  - `system`: optional system prompt string (empty/omitted for `llm()`)
  - `messages`: structured conversation items obeying compaction bounds — user text + base64 images, assistant text + tool calls (`id`, `name`, `arguments`) + optional provider-scoped replay metadata (see below), tool results (`id`, `name`, `output`, optional base64 images)
  - `tools`: structured tool definitions (`name`, `description`, JSON Schema `parameters`)
  - `effort`: optional effort level (omitted when `"default"`)
- **Per-adapter responsibilities**:
  1. Validate request capabilities against the adapter (fail fast on unsupported capabilities).
  2. Encode HTTP URL, headers, auth, and JSON request body (including replaying matching provider/model replay metadata when present).
  3. Parse SSE stream (and non-streaming JSON fallback) into common stream deltas (`text`, `reasoning`, `tool_delta`), reply `text`/`reasoning`/`tool_calls`, ordered replay metadata, normalized usage counters, `finish` reason, and normalized errors.
  4. Fetch and normalize raw model IDs for `tuse connect`, `/models`, and `model.list`.
- **Shared across all adapters (no duplication)**:
  - HTTP client timeouts (30 s connect, 600 s read).
  - Retry schedule (`[2, 5, 10, 20, 30]` s capped at `min(settings.retries, 5)`), retryability rules, cooperative cancellation polling, and turn `stream_reset` + `flash` notifications.
  - Incremental JSON `"code"` string extractor for adapters that stream partial JSON arguments (`openai-chat`, `openai-responses`, `anthropic-messages`).
  - Identical TUI rendering, SQLite persistence, compaction, hooks, and `tuse json` event sequence (`text`, `reasoning`, `tool_delta`, `stream_reset`, `assistant`, `tool_start`, `tool_end`, `usage`, `models`, `flash`, `error`, `turn_end`).

## Reasoning Replay Metadata, Compaction & Cross-Provider Fallback

Modern reasoning APIs require unmodified reasoning artifacts to be echoed back in exact order across multi-step tool loops (and across `/resume`):

- **Persisted replay metadata**:
  - **Decision:** Add an optional `replay` field on persisted `assistant` records (omitted when absent/empty so pre-existing records and v1 JSON-RPC readers stay backward-compatible):
    ```json
    {
      "api": "anthropic-messages | openai-responses | google-gemini",
      "model": "<raw model id>",
      "items": [ ... ]
    }
    ```
  - `items` preserves the exact ordered wire items/blocks/parts returned by that provider for that assistant step (unmodified signatures, encrypted reasoning blobs, `redacted_thinking`, and their exact interleaving with text and tool-call items).
- **Same-API & same-model replay**:
  - When building a request for an assistant record whose `replay.api` and `replay.model` match the active request's API type and raw model ID (and the record is outside the dropped compaction zone), the adapter replays `replay.items` verbatim instead of reconstructing plain `text` + `tool_calls`.
- **Cross-provider / cross-model fallback (and legacy records without `replay`)**:
  - **Decision:** Never send `replay.items` from a different `api` or `model` (foreign signatures/encrypted blobs cause HTTP 400 validation failures). Fall back cleanly to protocol-agnostic `text` + `tool_calls`:
    - `openai-chat`, `openai-responses`, `google-gemini`: serialize plain `text` + tool calls (omitting foreign reasoning signatures/encrypted items).
    - `anthropic-messages`: prior completed turns (before the latest user message) serialize as plain `text` + `tool_use` without foreign thinking blocks. If an **unfinished** tool loop (an assistant tool-call step after the latest user message, e.g., after a crash/resume or mid-loop switch onto Anthropic) lacks valid Anthropic `replay` thinking blocks for the active model, **Decision:** omit `thinking` on that recovery request rather than triggering an unrecoverable HTTP 400 `Expected thinking or redacted_thinking, but found tool_use`.
- **Compaction interaction**:
  - **Dropped prefix** (`index < drop`): drops all tool calls, tool results, AND `replay` metadata (keeping only plain conversation text, matching today's behavior; `drop` always aligns on a `user`/`notification` boundary so an active tool loop is never split).
  - **Pruned middle** (`drop <= index < prune`) and **Recent suffix** (`index >= prune`): retain `replay` metadata alongside retained tool calls so multi-turn verification never breaks after pruning. Token estimation counts human-readable message text/images (ignoring opaque encrypted/signature base64 noise in `replay`, or capping opaque blob estimates) so opaque signatures do not cause runaway premature compaction.

## Per-Adapter Protocol Contracts

### 1. `openai-chat`

Unchanged from [../providers.md](../providers.md).

### 2. `openai-responses`

- **Endpoints**: `POST <base_url>/responses`, `GET <base_url>/models` (single page, `data[].id`).
- **Headers**: `Authorization: Bearer <key>`, `Content-Type: application/json`.
- **Request**:
  - `model`, `stream: true`, `store: false` (**Decision:** send `store: false` so local SQLite history and 50/25/25 compaction stay the single source of truth).
  - **Encrypted reasoning replay (`store: false`)**: **Decision:** whenever `effort` is not `"default"` (and on models supporting reasoning), include `"include": ["reasoning.encrypted_content"]` in the request so OpenAI returns stateless encrypted reasoning items (`{ id, type: "reasoning", summary, encrypted_content }`) that can be replayed verbatim on subsequent tool-loop turns and after `/resume`. If an upstream Responses gateway omits `encrypted_content` on a reasoning item, do not store or replay an empty/unencrypted reasoning stub.
  - `instructions`: system prompt when non-empty.
  - `input`: ordered array of items:
    - User message -> `{"role": "user", "content": [...]}` using `{"type": "input_text", "text": ...}` and `{"type": "input_image", "image_url": "data:..."}`.
    - Assistant step -> when matching `replay` metadata exists, replay its ordered `reasoning` (with original `id` and `encrypted_content`), `message`, and `function_call` items verbatim; otherwise emit `{"role": "assistant", "content": [{"type": "output_text", "text": ...}]}` (if text non-empty) followed by one `{"type": "function_call", "call_id": id, "name": name, "arguments": arguments}` item per call.
    - Tool results -> one `{"type": "function_call_output", "call_id": id, "output": output}` item per result; if tool images exist, flush a follow-up `role: "user"` item with `[Images from the tool results above]` and `input_image` parts.
  - `tools`: flat function definitions `[{"type": "function", "name": ..., "description": ..., "parameters": ...}]` (omitted when empty).
  - `reasoning`: omitted when effort is `"default"`; otherwise `{"effort": "<effort>", "summary": "auto"}` for `low`, `medium`, `high`, `xhigh`.
- **Streaming SSE (event correlation & argument precedence)**:
  - `response.output_text.delta` -> `text` delta.
  - `response.reasoning_summary_text.delta` and `response.reasoning_text.delta` -> `reasoning` delta.
  - Tool-call slot correlation:
    - `response.output_item.added` (`item.type == "function_call"`): creates/selects slot keyed by `output_index` and `item.id`; captures `call_id` (falling back to `item.id`) and `name`. If `item.arguments` is already non-empty, appends it and feeds the incremental `"code"` extractor.
    - `response.function_call_arguments.delta`: matches slot by `item_id` (falling back to `output_index`), appends `delta` to `arguments`, and feeds the incremental `"code"` extractor -> `tool_delta`.
    - **Final-argument precedence** (`response.function_call_arguments.done` and `response.output_item.done` with `item.type == "function_call"`): backfills any missing `call_id` or `name`; if `done` carries a non-empty `arguments` string, it is authoritative for the final stored tool-call `arguments` (if no deltas arrived earlier, feed it once to emit `tool_delta`; if deltas were already streamed, do not re-emit duplicate `tool_delta` characters).
  - Replay capture: `response.output_item.done` appends each completed item (`reasoning` with `encrypted_content`, `message`, `function_call`) in `output_index` order into `replay.items`.
  - `response.completed` / `response.incomplete` -> capture `finish` (`response.status`) and `response.usage`.
  - `response.failed` / top-level `error` -> inspect `response.error.code` (or `error.code`): map rate-limit and server errors (`rate_limit_exceeded`, `server_error`, `service_unavailable`, or numeric `429` / `>= 500`) to retryable HTTP status errors; map client/validation errors (`invalid_request_error`, `context_length_exceeded`, `invalid_api_key`, etc.) to non-retryable HTTP status errors (`400`/`401`).
- **Non-SSE JSON fallback**:
  - Parse top-level Response JSON object: fail on non-null `error` or `status == "failed"` using the same retryability mapping above; record `finish = status` and `usage`; walk `output[]` in order:
    - `type == "reasoning"` -> emit `summary[].text` as `reasoning` deltas and save item (when `encrypted_content` is present) into `replay.items`.
    - `type == "message"` -> emit `content[].text` (where `type == "output_text"`) as `text` deltas and save item into `replay.items`.
    - `type == "function_call"` -> record `{ id: call_id || id, name, arguments }`, feed `arguments` once to `"code"` extractor -> `tool_delta` for `lua`, and save item into `replay.items`.
- **Usage**: `input <- input_tokens`, `output <- output_tokens`, `cached <- input_tokens_details.cached_tokens`, `cache_write <- 0`.

### 3. `anthropic-messages`

- **Endpoints**:
  - `POST <base_url>/messages`
  - `GET <base_url>/models?limit=100`: follows `&after_id=<last_id>` while `has_more == true`, `last_id` is non-empty, and `last_id` has not been seen, capped at **10 pages** (up to 1,000 models); reads `data[].id`.
- **Headers**: `x-api-key: <key>`, `anthropic-version: 2023-06-01`, `Content-Type: application/json`.
  - **Decision:** Send `x-api-key` and `anthropic-version: 2023-06-01` (standard Anthropic Messages contract).
- **Request**:
  - `model`, `stream: true`, `max_tokens`: **Decision:** default `max_tokens = 16384` (Anthropic requires `max_tokens` on every request; if a per-model max output setting is added later, it overrides this default).
  - `system`: when non-empty, sent as top-level `system` text block array with `cache_control: {"type": "ephemeral"}`.
    - **Decision:** Attach ephemeral `cache_control` breakpoints on the system prompt and the latest user/tool-result turn so Anthropic prompt caching works out of the box without extra config keys.
  - `messages`: strictly alternating `user` and `assistant` roles; consecutive items of the same role (for example, a notification followed by a user message) merge into one message's `content` block array:
    - User text -> `{"type": "text", "text": ...}`; user images -> `{"type": "image", "source": {"type": "base64", "media_type": mime, "data": b64}}`.
    - Assistant step -> when matching `replay` metadata exists, replay its ordered `content` blocks verbatim (`thinking` with unmodified `thinking` + `signature`, `redacted_thinking` with unmodified `data`, `text`, and `tool_use` in original order); otherwise emit non-empty `text` blocks and `{"type": "tool_use", "id": id, "name": name, "input": <parsed JSON object>}` (malformed historical arguments fall back to `{}`).
    - Tool results -> `role: "user"` blocks `{"type": "tool_result", "tool_use_id": id, "content": [...]}` carrying the text output and any tool-returned base64 image blocks directly inside `tool_result.content`.
  - `tools`: `[{"name": ..., "description": ..., "input_schema": ...}]` (omitted when empty).
  - Effort / thinking:
    - **Decision:** When effort is `low`, `medium`, `high`, or `max`, send `thinking: {"type": "adaptive"}` and `output_config: {"effort": "<effort>"}` (reject `xhigh` as unsupported), subject to the unfinished-loop recovery rule above when prior tool calls lack valid Anthropic thinking blocks. When effort is `"default"`, omit both.
- **Streaming SSE**:
  - `message_start` -> initialize `usage` from `message.usage`.
  - `content_block_start`: track block by `index`:
    - `text` -> start text block
    - `thinking` -> start thinking block (accumulating both `thinking` text and `signature`)
    - `redacted_thinking` -> record opaque `data` directly for replay (no UI delta)
    - `tool_use` -> allocate tool call slot (`id`, `name`)
  - `content_block_delta`:
    - `text_delta` -> append text + emit `text` delta
    - `thinking_delta` -> append reasoning text + emit `reasoning` delta
    - `signature_delta` -> append `signature` to the active `thinking` block for verbatim replay (no UI delta)
    - `input_json_delta` (`partial_json`) -> append tool arguments + feed incremental `"code"` extractor -> `tool_delta`
  - `content_block_stop`: finalize block at `index` into ordered `replay.items` (`thinking` with full `thinking` + `signature`, `redacted_thinking` with `data`, `text`, or `tool_use` with parsed `input`).
  - `message_delta` -> update `stop_reason` and final `usage` (`output_tokens` and any updated input/cache counters).
  - `ping` -> ignored; `message_stop` -> finish stream.
  - `error` event -> map error type (`rate_limit_error` -> retryable HTTP 429, `overloaded_error` -> retryable HTTP 529, `api_error` -> retryable HTTP 500, `invalid_request_error`/`authentication_error`/`permission_error` -> non-retryable HTTP 400/401/403).
- **Non-SSE JSON fallback**:
  - Parse top-level Message JSON object: fail on `type == "error"` or non-null `error` using the error mapping above; record `finish = stop_reason` and normalized `usage`; iterate `content[]` in order:
    - `type == "thinking"` -> emit `thinking` as `reasoning` delta and preserve `{ type: "thinking", thinking, signature }` in `replay.items`.
    - `type == "redacted_thinking"` -> preserve `{ type: "redacted_thinking", data }` in `replay.items` (no UI delta).
    - `type == "text"` -> emit `text` delta and preserve block in `replay.items`.
    - `type == "tool_use"` -> serialize `input` object to JSON string `arguments`, record `{ id, name, arguments }`, emit `tool_delta` once for `lua`, and preserve block in `replay.items`.
- **Usage normalization**:
  - Anthropic reports `input_tokens` **excluding** cached tokens, whereas Tuse's `input` counter is **total** prompt tokens (used for `ctx = input + output` and `plain = input - cached - cache_write`).
  - Normalize as:
    - `cached = cache_read_input_tokens`
    - `cache_write = cache_creation_input_tokens`
    - `input = input_tokens + cached + cache_write`
    - `output = output_tokens`

### 4. `google-gemini`

- **Endpoints**:
  - Streaming generation: `POST <base_url>/models/<model_id>:streamGenerateContent?alt=sse`
  - Model listing: `GET <base_url>/models?pageSize=100` following `&pageToken=<nextPageToken>` while `nextPageToken` is non-empty and unseen, capped at **10 pages** (up to 1,000 models); reads `models[].name`, strips leading `models/` prefix, and skips entries whose `supportedGenerationMethods` is present and does not include `generateContent`.
- **Headers**: `x-goog-api-key: <key>`, `Content-Type: application/json` (never pass the API key as a URL query parameter so secrets cannot leak in error messages).
- **Request**:
  - `systemInstruction`: `{"parts": [{"text": system}]}` when non-empty.
  - `contents`: array of `{"role": "user" | "model", "parts": [...]}` with consecutive same-role turns merged:
    - User text -> `{"text": ...}`; user images -> `{"inlineData": {"mimeType": mime, "data": b64}}`.
    - Assistant step (`role: "model"`) -> when matching `replay` metadata exists, replay its ordered raw `parts` array verbatim (preserving every `thought`, `text`, `functionCall`, and part-level `thoughtSignature` byte-for-byte in received order); otherwise emit `{"text": ...}` (when non-empty) and `{"functionCall": {"id": id, "name": name, "args": <parsed JSON object>}}` parts.
    - Tool results -> `role: "user"` parts `{"functionResponse": {"id": id, "name": name, "response": {"output": output}}}` plus `inlineData` parts for any tool-returned images.
  - `tools`: `[{"functionDeclarations": [{"name": ..., "description": ..., "parameters": ...}]}]` (omitted when empty).
  - Effort / thinking:
    - When effort is `"default"`, omit `thinkingConfig` completely (so non-thinking models, Gemini 2.5, and Gemini 3+ all run with their native defaults).
    - **Decision:** When effort is `low`, `medium`, or `high`, select the model-appropriate thinking parameter (rejecting `xhigh` and `max` fail-fast as unsupported):
      - For Gemini 2.5 models (`model_id` starting with `gemini-2.5-`): send `generationConfig: {"thinkingConfig": {"includeThoughts": true, "thinkingBudget": <N>}}` with `low = 1024`, `medium = 8192`, `high = 24576`.
      - For Gemini 3+ and other Gemini models: send `generationConfig: {"thinkingConfig": {"includeThoughts": true, "thinkingLevel": "<effort>"}}`.
      - If a specific Gemini model does not support thinking or rejects the field, the upstream HTTP 400 surfaces cleanly as a non-retryable error (and setting effort to `"default"` omits `thinkingConfig`).
- **Streaming SSE (and non-streaming JSON fallback)**:
  - Each SSE `data:` JSON chunk (or each object if a non-streaming response returns a single JSON object or JSON array of response chunks) inspects `candidates[0].content.parts[]`:
    - Preserve every received part object (including any `thoughtSignature` field on thought, text, or `functionCall` parts) into ordered `replay.items`, merging contiguous plain/thought text chunks while never dropping or moving a `thoughtSignature` across parts.
    - Part with `thought == true` and `text` -> `reasoning` delta.
    - Part with `text` (not `thought`) -> `text` delta.
    - Part with `functionCall` (`{id?, name, args}`) -> serialize `args` to JSON, assign `call_<index>` if `id` is absent, extract `"code"` for tool `lua`, and emit `tool_delta` once.
  - `candidates[0].finishReason` -> `finish`.
  - Top-level `error` object -> HTTP status error using `error.code` (default `500`).
- **Usage**:
  - `input <- usageMetadata.promptTokenCount` (already includes cached tokens)
  - `cached <- usageMetadata.cachedContentTokenCount`
  - `cache_write <- 0`
  - `output <- usageMetadata.candidatesTokenCount + usageMetadata.thoughtsTokenCount` (so total output includes thinking tokens, matching OpenAI and Anthropic).

## Unsupported Capability Reporting

Unsupported capabilities are handled in two clear ways:

1. **Known adapter incompatibility (fail fast, non-retryable)**:
   - Triggered before sending the HTTP request when the request asks for a capability the provider's API type cannot represent:
     - Unsupported effort level (e.g., `max` on `openai-responses`, `xhigh` on `anthropic-messages`, `xhigh`/`max` on `google-gemini`).
     - Image attachment on an agent turn that is not a valid base64 `data:<mime>;base64,<data>` URL when an adapter requires inline base64 (`anthropic-messages`, `google-gemini`).
   - Error format: `provider \`<name>\` (<api_type>) does not support <capability> \`<value>\` (<hint>)` — for example, `provider \`gemini\` (google-gemini) does not support effort \`max\` (supported: default, low, medium, high)`.
   - Never retried. In an agent turn, saves an `error` record and emits `error` (ending the turn with `turn_end` when the queue is empty) in both TUI and `tuse json`. In `llm()`, raises a Lua error `llm: ...`.
   - **Decision:** Keep the full 6-value effort set (`default`, `low`, `medium`, `high`, `xhigh`, `max`) in Tuse's config/picker and enforce adapter support at request time (optionally annotating unsupported efforts in `/models` when the provider's API type is known, while still enforcing on turn and `llm()` for resumed sessions and direct RPC calls).
2. **Graceful protocol differences (no error)**:
   - Missing `cache_write` reporting (`openai-responses`, `google-gemini`) reports `0`.
   - Non-incremental tool call arguments (`google-gemini`) emit a single `tool_delta` with the complete `code` before `tool_start`.
   - Upstream model-specific rejections (e.g., a text-only model rejecting images, or a non-thinking model rejecting `thinkingConfig`) surface verbatim as non-retryable `HTTP 4xx: ...` errors.

## Baseline Docs to Update Before Implementation (Handoff Gate)

Keep current baseline docs describing today's Chat Completions behavior until work begins on #2. Before handing off implementation to a coding agent (per TODO #3), update and review:

- [../providers.md](../providers.md): document all four API types, `--api`, `tuse providers --json`, capability matrix, replay metadata/compaction/fallback rules, per-adapter request/streaming/fallback/usage/error contracts, and unsupported-capability reporting.
- [../models.md](../models.md): update provider overview and effort-level notes to reflect per-API support.
- [../cli.md](../cli.md) and [../commands.md](../commands.md): document `--api` on `tuse connect` and `--json` on `tuse providers` while keeping two-column default output documented.
- [../sessions.md](../sessions.md) and [../json.md](../json.md): document optional `replay` metadata on `assistant` records, identical streaming events across all four API types, and unsupported-capability `error` events.

## Acceptance Tests

For **each** of the four adapters (`openai-chat`, `openai-responses`, `anthropic-messages`, `google-gemini`):

1. **Request encoding**: verifies HTTP method, path, auth/version headers (confirming Gemini never puts the API key in the query string), system prompt placement, user text + base64 image encoding, assistant tool call + tool result + tool image encoding, adjacent same-role merging (Anthropic and Gemini), tool schema, effort mapping (including Gemini 2.5 `thinkingBudget` vs Gemini 3+ `thinkingLevel` and `"default"` omitting `thinkingConfig`), Responses `include: ["reasoning.encrypted_content"]`, and `llm()` tool-free single-prompt request.
2. **Multi-step reasoning tool loops & resume**: verifies a **second tool step** (turn 1 reasoning + tool call -> tool result -> turn 2 request) AND **session close + `/resume` + next turn** with reasoning enabled:
   - `anthropic-messages` replays unmodified `thinking` (with `signature`) and `redacted_thinking` blocks in exact order before `tool_use`.
   - `google-gemini` replays ordered `parts` preserving part-level `thoughtSignature` (both when attached to a thought/text part and when attached to a `functionCall` part).
   - `openai-responses` replays ordered `reasoning` (`id` + `encrypted_content`) and `function_call` items with `store: false`.
   - Compaction retains `replay` in pruned/recent zones and drops it in the dropped prefix; cross-provider/cross-model switching never sends foreign signatures/encrypted items and recovers cleanly.
3. **Streaming & non-SSE fallback**: verifies SSE parsing of text deltas, reasoning/thinking deltas, tool calls (OpenAI Chat/Responses/Anthropic incremental chunking + `tool_delta` code streaming; Responses correlation across `output_index`/`item_id`/`call_id` and final-argument precedence on `done` without duplicate `tool_delta`; Gemini complete `functionCall`), non-SSE JSON fallback schemas for **all four** adapters (including Responses and Anthropic), and cooperative cancellation.
4. **Usage & cost**: verifies token extraction and normalization (specifically Anthropic `input = input_tokens + cache_read + cache_write` and Gemini `output = candidates + thoughts`), session `ctx` update, `llm()` usage separation, and price calculation.
5. **Errors & retries**: verifies retry on HTTP 429, HTTP 5xx, transport drop, and retryable inline SSE errors (Anthropic `overloaded_error`/`rate_limit_error`, Responses retryable `response.failed` such as `rate_limit_exceeded`/`server_error`) with `stream_reset` + `flash`; verifies no retry on HTTP 400/401/403, parse errors, or non-retryable inline SSE errors (Anthropic `invalid_request_error`, Responses non-retryable `response.failed` such as `invalid_request_error`/`context_length_exceeded`).
6. **Model discovery & pagination**: verifies `GET /models` parsing and pagination up to the 10-page cap (and repeated-cursor loop termination) for Anthropic (`limit=100`, `has_more`/`last_id`) and Gemini (`pageSize=100`, `nextPageToken`, stripping `models/` and filtering `generateContent`), and single-page `data[].id` for OpenAI Chat/Responses.
7. **Unsupported capabilities**: verifies fast non-retryable error (zero HTTP requests made) for unsupported effort values on agent turns (TUI and JSON mode) and `llm()`, and for invalid non-base64 image attachments on agent turns (TUI and JSON mode).
8. **Persistence & CLI compatibility**: verifies automatic SQLite migration of pre-existing `providers` rows to `openai-chat`, `tuse connect` default vs explicit `--api`, rejection of invalid `--api`, unchanged two-column `tuse providers` default output, `tuse providers --json` structured output (without `api_key`), and mid-session + post-resume switching between providers using different API types in both TUI and `tuse json`.
