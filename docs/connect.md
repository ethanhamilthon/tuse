# Connect

How to connect API providers, pick models and reasoning effort, describe model metadata, and read usage and cost.

## Providers

A provider is a named API endpoint plus a key. `tuse connect` saves a provider in `~/.tuse/tuse.db`:

```sh
tuse connect --base https://api.openai.com/v1 --key '$OPENAI_API_KEY' --name openai
tuse connect --base https://api.openai.com/v1 --key '$OPENAI_API_KEY' --name openai-resp --api openai-responses
tuse connect --base https://api.anthropic.com/v1 --key '$ANTHROPIC_API_KEY' --name anthropic --api anthropic-messages
tuse connect --base https://generativelanguage.googleapis.com/v1beta --key '$GEMINI_API_KEY' --name gemini --api google-gemini
tuse providers
tuse disconnect openai
```

- `--base` must start with `http://` or `https://`. You always pass it, because there are no built-in hosts.
- `--name` can't contain `/` or whitespace and must be new. Pass `--replace` to update an existing provider in place, or `tuse disconnect <name>` to remove it. The old entry is only replaced if the new one checks out.
- `--key '$VAR'` (or `'${VAR}'`) stores the reference, not the secret. It is read from the environment every time a session starts or reloads. Keep the single quotes so your shell does not expand it. `--key-stdin` reads a literal key from stdin so it never shows up in argv. If you pass a literal `--key`, it is stored as given.
- `--header 'Name: value'` adds a header to model discovery and generation. Repeat it for multiple headers. Quote `--header 'X-Gateway-Token: $TOKEN'` to store an environment reference instead of a literal value. Header names are case-insensitive; duplicates and overrides of built-in auth, protocol, or HTTP framing headers are rejected.
- `--api` picks the wire protocol: `openai-chat` (the default, any OpenAI-compatible Chat Completions API), `openai-responses`, `anthropic-messages`, or `google-gemini`. To use two protocols on one host, connect it twice under different names.

Before saving, `tuse connect` lists models from `GET <base>/models`. If that fails, nothing is saved and it prints `can't fetch models from <base>: <error>`. If it works, it prints ``Received <N> models. Saved `<NAME>`.`` This only checks that model listing works, not generation.

A running session uses the providers it loaded at start. After `tuse connect` or `tuse disconnect` in another terminal, run `/reload` to pick up the change. Keys and header values are never shown in `/config`, `/session`, `session()`, `tuse providers`, or JSON-RPC (`config.get.providers` and `tuse providers --json` expose only `name`, `type`, and `base`).

### Providers in Lua config

Use `providers` inside a namespace for a project or personal connection that should not be saved in the database:

```lua
return {
  corp = {
    providers = {
      gateway = {
        base = "https://gateway.example/v1",
        key = "$GATEWAY_API_KEY",
        api = "openai-chat",
        headers = { ["X-Tenant"] = "my-team", ["X-Gateway-Token"] = "$GATEWAY_TOKEN" },
      },
    },
  },
}
```

The table key is the provider name. `base` and `key` are required; `api` defaults to `openai-chat`, and `headers` is optional. Whole-value environment references in keys and headers are resolved on startup and reload. Missing or empty required values fail validation. A name already saved by `tuse connect` cannot also be used in Lua. `tuse providers` and `tuse disconnect` only operate on saved profiles.

Config loading does not wait for a network probe. Model discovery can still query `/models`; if the gateway does not implement it, declare a `gateway/<model-id>` entry in `models`.

## Models

A model is named `<provider>/<model id>`, split at the first `/`. Everything after it is sent as the model id, so `openrouter/anthropic/claude-sonnet-4` works.

When `models` is configured in Lua, `/models` lists only those configured models; when `models` is empty or omitted, `/models` lists every model id returned by connected providers. JSON-RPC `model.list` ignores that scoping and returns all configured plus discovered models. Picking a model (and effort, unless `efforts = {}`) applies to the current session and saves the default for new sessions. Every session keeps its own model and effort, including when resumed. A model whose provider is not connected is marked `unavailable`. A turn on it fails with ``no `connect` entry named `<provider>` for model `<model>` ``.

## Reasoning effort

The shared default effort list for all models and providers is `low`, `medium`, `high`, `xhigh`, and `max`. Every selection entrypoint (`/models`, JSON-RPC `model.set`, Lua `set_session`, Lua `llm`, saved defaults, and turn validation) uses the same model-scoped effort resolver:

- **Omitted `efforts`** (or a model not listed in `models`): uses the shared default list `low`, `medium`, `high`, `xhigh`, `max`.
- **Explicitly empty `efforts = {}`**: marks a non-reasoning model; `/models` skips the effort picker, `effort` is empty (`""`), and Tuse omits reasoning/thinking parameters completely on the wire.
- **Explicitly non-empty `efforts = { ... }`**: dense list of unique names from `low`, `medium`, `high`, `xhigh`, `max`, acting as the sole source of truth for that model.
- **Fallback effort:** when effort is unset or omitted (or recovering an invalid saved effort), Tuse picks `medium` if available for that model, otherwise the first declared effort in `efforts`, or `""` (no reasoning parameter) when `efforts = {}`.

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

Context window, display name, efforts, and prices come from `models` inside any namespace:

```lua
return {
  models_cfg = {
    models = {
      {
        name = "openai/gpt-5",
        display = "GPT-5",
        window = 400000,
        efforts = { "low", "medium", "high", "xhigh" },
        prices = { input = 1.25, output = 10, cache_read = 0.125, cache_write = 0 },
      },
      {
        name = "openai/gpt-4o",
        efforts = {},
      },
    },
  },
}
```

| Field | Default | Meaning |
| --- | --- | --- |
| `name` | required | `provider/model`. Duplicate model names fail validation. |
| `display` | `name` | Label in the status bar, `/models`, and turn summaries. |
| `window` | `0` (unknown) | Context window in tokens. |
| `efforts` | omitted (all 5) | Optional list of allowed efforts (`{}` disables reasoning parameters). |
| `prices` | unset | USD per 1M tokens: `input`, `output`, `cache_read`, `cache_write` (each defaults to `0`). |

`tuse models` prints this metadata. Changes take effect on `/reload` for subsequent requests (previously recorded session cost totals stay as stored).

## Usage and cost

Each step records the provider's usage: `input` (all prompt tokens, including cache), `output`, `cache_read`, `cache_write`. The cost of a step is:

```text
(max(0, input - cache_read - cache_write) * input_price
 + cache_read * cache_read_price
 + cache_write * cache_write_price
 + output * output_price) / 1,000,000
```

`llm()` calls are counted the same way, and their cost is also tracked on its own. You can see the totals in the status bar, in `/session` (`$total ($main + $llm llm)`), in `session().usage`, and in JSON-RPC `session.state`. Cost stays unknown until some usage matches a model that has `prices`. Not every API reports cache writes: `openai-responses` and `google-gemini` report `0`.

## Requests, retries, errors

- Timeouts are 30 s to connect and 600 s to read. Redirects are never followed, so keys don't leak to other hosts.
- HTTP `429`, `5xx`, and network errors are retried up to 5 times, after 2, 5, 10, 20, and 30 s. The TUI flashes `Retry N/5 in Ds: <error>`. Other errors stop the step and show up as `HTTP <code>: <body>`, `network: …`, or `bad response: …`.
- You can switch models and APIs in the middle of a session. History is converted for each request, and provider-specific reasoning data is only sent back to the same provider, API, and model.
- Images (pasted, attached, or read by `read()`) are sent to all four APIs.
