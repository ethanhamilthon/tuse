# Connect

How to manage API providers, pick discovered models and reasoning effort, describe model metadata, and read usage and cost.

## Providers

A provider is a named API endpoint plus credentials stored in `$TUSE_HOME/tuse.db` (default `~/.tuse/tuse.db`). Providers are managed inside a session through the shared provider service:

- **TUI:** `/providers list`, `/providers connect`, `/providers update [NAME]`, `/providers disconnect [NAME]`.
- **Lua:** `providers.list()`, `providers.connect(opts)`, `providers.update(name, patch)`, `providers.disconnect(name)` (callable from config callbacks, commands, hooks, and the agent without confirmation).
- **JSON-RPC:** `providers.list`, `providers.connect`, `providers.update`, `providers.disconnect` ([JSON mode](json.md#providers-and-plugins)).
- **CLI (read-only):** `tuse providers` prints `NAME<TAB>BASE_URL`, and `tuse providers --json` prints `[{ name, type, base }]`.

Provider fields and rules:

- `api` (returned as `type`): wire protocol — `openai-chat` (default, any OpenAI-compatible Chat Completions API), `openai-responses`, `anthropic-messages`, or `google-gemini`. To use two protocols on one host, save it under two names.
- `base`: endpoint base URL starting with `http://` or `https://`.
- `key`: literal API key or whole-value environment reference (`$ENV_VAR` or `${ENV_VAR}`). Environment references are stored as references and resolved at probe and request time. In the TUI connect and update wizards, key input is masked, cannot be copied, and is never written to chat history, `ui_result`, logs, or errors.
- `name`: non-empty identifier without `/` or whitespace.
- `headers` (optional via Lua/RPC): custom request headers (`Name -> value`, supporting `$ENV_VAR` references). Header names are case-insensitive; duplicates and overrides of built-in auth, protocol, or HTTP framing headers are rejected.

Before saving or updating, Tuse probes `GET <base>/models`. If the probe fails, nothing is modified and the error message redacts credentials and header values. Disconnecting the provider backing the currently selected model keeps the session's `model`, `provider`, and `effort` unchanged (no silent switch), marks `model_info.available = false`, prunes that provider's cached discovered models, allows any already in-flight request to finish, and fails subsequent requests cleanly until another model is selected or the provider is reconnected.

Keys and header values are never shown in `/providers list`, `/session`, `session.get()`, `tuse providers`, or JSON-RPC (`providers.list` and `tuse providers --json` expose only `name`, `type`, and `base`).

## Models

A model is named `<provider>/<model id>`, split at the first `/`. Everything after it is sent as the model id, so `openrouter/anthropic/claude-sonnet-4` works.

Both TUI `/models` and JSON-RPC `model.list` list **discovered models only**—IDs returned by connected providers' `/models` endpoints. Entries declared only in `config.models` do not appear in `/models` or `model.list`; when a discovered model matches an entry in `config.models`, Tuse annotates it with that entry's display name, context window, prices, and allowed efforts. Picking a model (and effort, unless `efforts = {}`) applies to the current session and saves the default for new sessions. Every session keeps its own model and effort, including when resumed. A selected model whose provider is disconnected is marked `unavailable`.

## Reasoning effort

The shared default effort list for all models and providers is `low`, `medium`, `high`, `xhigh`, and `max`. Tuse validates effort against each model’s allowed list:

- **Omitted `efforts`** (or a model not listed in `models`): uses the shared default list `low`, `medium`, `high`, `xhigh`, `max`.
- **Explicitly empty `efforts = {}`**: marks a non-reasoning model; `/models` skips the effort picker, `effort` is empty (`""`), and Tuse omits reasoning/thinking parameters completely on the wire.
- **Explicitly non-empty `efforts = { ... }`**: dense list of unique names from `low`, `medium`, `high`, `xhigh`, `max`, acting as the sole source of truth for that model.
- **Fallback effort:** when JSON-RPC `model.set` omits effort or a saved effort is invalid, Tuse picks `medium` if available, otherwise the first declared effort, or `""` when `efforts = {}`. Lua `session.set` keeps the current effort when valid for the new model; Lua `llm` defaults to `low` on another reasoning model. See [Lua](lua.md#utilities-llm-reload-docs-json).

Wire encoding per API when an effort is active (no reasoning fields are sent when `efforts = {}`):

| API | Wire field |
| --- | --- |
| `openai-chat` | Sends `"reasoning_effort": "<effort>"` verbatim. |
| `openai-responses` | Sends `"reasoning": {"effort": "<effort>", "summary": "auto"}` and `"include": ["reasoning.encrypted_content"]` verbatim. |
| `anthropic-messages` | Sends `"thinking": {"type": "adaptive"}` and `"output_config": {"effort": "<effort>"}` verbatim. |
| `google-gemini` (non-2.5) | Sends `"thinkingConfig": {"includeThoughts": true, "thinkingLevel": "<effort>"}` verbatim. |
| `google-gemini` (`gemini-2.5-*`) | Uses numeric `thinkingBudget`: `low` -> `1024`, `medium` -> `8192`, `high` -> `24576`. Unmapped levels (`xhigh`, `max`) return an explicit wire-encoding error rather than clamping or inventing budgets; restrict `models[].efforts = { "low", "medium", "high" }` for Gemini 2.5 models. |

Tuse does not enforce a hardcoded provider-API effort matrix; if an upstream endpoint rejects a verbatim effort string, that HTTP error is surfaced directly. Configure `models[].efforts` to match what your models accept.

## Model metadata

Context window, display name, efforts, and prices come from the top-level `models` map (metadata only—does not populate `/models` or `model.list`):

```lua
return {
  models = {
    ["openai/gpt-5"] = {
        display = "GPT-5",
        window = 400000,
        efforts = { "low", "medium", "high", "xhigh" },
        prices = { input = 1.25, output = 10, cache_read = 0.125, cache_write = 0 },
      },
    ["openai/gpt-4o"] = {
        efforts = {},
      },
  },
}
```

| Field | Default | Meaning |
| --- | --- | --- |
| `display` | `name` | Label in the status bar, `/models`, and turn summaries. |
| `window` | `0` (unknown) | Context window in tokens. |
| `efforts` | omitted (all 5) | Optional list of allowed efforts (`{}` disables reasoning parameters). |
| `prices` | unset | USD per 1M tokens: `input`, `output`, `cache_read`, `cache_write` (each defaults to `0`). |

`tuse models` prints this metadata and the saved default. Changes take effect on `/reload` for subsequent requests while previously stored session cost totals stay as recorded.

## Usage and cost

Each step records the provider's usage: `input` (all prompt tokens, including cache), `output`, `cache_read`, `cache_write`. The cost of a step is:

```text
(max(0, input - cache_read - cache_write) * input_price
 + cache_read * cache_read_price
 + cache_write * cache_write_price
 + output * output_price) / 1,000,000
```

`llm()` calls are counted the same way, and their cost is also tracked on its own. You can see the totals in the status bar, in `/session` (`$total ($main + $llm llm)`), in `session.get().usage`, and in JSON-RPC `session.state`. Cost stays unknown until some usage matches a model that has `prices`. Not every API reports cache writes: `openai-responses` and `google-gemini` report `0`.

## Requests, retries, errors

- Timeouts are 30 s to connect and 600 s to read. Redirects are never followed, so keys do not leak to other hosts.
- HTTP `429`, `5xx`, and network errors are retried up to 5 times, after 2, 5, 10, 20, and 30 s. The TUI flashes `Retry N/5 in Ds: <error>`. Other errors stop the step and show up as `HTTP <code>: <body>`, `network: …`, or `bad response: …`.
- You can switch models and APIs in the middle of a session. History is converted for each request, and provider-specific reasoning data is only sent back to the same provider, API, and model.
- Images (pasted, attached, or read by `fs.read()`) are sent to all four APIs.
