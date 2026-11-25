# Connect

How to connect API providers, pick models and reasoning effort, describe model metadata, and read usage and cost.

## Providers

A provider is a named API endpoint plus a key. Providers are stored in `~/.tuse/tuse.db`, not in Lua config:

```sh
tuse connect --base https://api.openai.com/v1 --key '$OPENAI_API_KEY' --name openai
tuse connect --base https://api.openai.com/v1 --key '$OPENAI_API_KEY' --name openai-resp --api openai-responses
tuse connect --base https://api.anthropic.com/v1 --key '$ANTHROPIC_API_KEY' --name anthropic --api anthropic-messages
tuse connect --base https://generativelanguage.googleapis.com/v1beta --key '$GEMINI_API_KEY' --name gemini --api google-gemini
tuse providers
tuse disconnect openai
```

- `--base` must start with `http://` or `https://`. You always pass it, because there are no built-in hosts.
- `--name` can't contain `/` or whitespace and must be new. Use `--replace` to update an existing one. The old entry is only replaced if the new one checks out.
- `--key '$VAR'` (or `'${VAR}'`) stores the reference, not the secret. It is read from the environment every time a session starts or reloads. Keep the single quotes so your shell does not expand it. `--key-stdin` reads a literal key from stdin so it never shows up in argv. If you pass a literal `--key`, it is stored as given.
- `--api` picks the wire protocol: `openai-chat` (the default, any OpenAI-compatible Chat Completions API), `openai-responses`, `anthropic-messages`, or `google-gemini`. To use two protocols on one host, connect it twice under different names.

Before saving, `tuse connect` lists models from `GET <base>/models`. If that fails, nothing is saved and it prints `can't fetch models from <base>: <error>`. If it works, it prints ``Received <N> models. Saved `<NAME>`.`` This only checks that model listing works, not generation.

A running session uses the providers it loaded at start. After `tuse connect` or `tuse disconnect` in another terminal, run `/reload` to pick up the change. Keys never appear in `/config`, `/session`, `session()`, `tuse providers`, or JSON-RPC.

## Models

A model is named `<provider>/<model id>`, split at the first `/`. Everything after it is sent as the model id, so `openrouter/anthropic/claude-sonnet-4` works.

`/models` (or JSON-RPC `model.list`) lists your configured `models` plus every model id the connected providers return. The list appears right away and fills in as providers answer. Picking a model then an effort applies to the current session and also saves the default for new sessions. Every session keeps its own model and effort, including when resumed. A model whose provider is not connected is marked `unavailable`. A turn on it fails with ``no `connect` entry named `<provider>` for model `<model>` ``.

## Reasoning effort

Effort is one of `default`, `low`, `medium`, `high`, `xhigh`, `max`. `default` sends no reasoning settings at all. Not every API takes every level:

| API | Levels besides `default` |
| --- | --- |
| `openai-chat` | `low`, `medium`, `high`, `xhigh`, `max` (`reasoning_effort`) |
| `openai-responses` | `low`, `medium`, `high`, `xhigh` (`reasoning.effort`, with encrypted reasoning kept between steps) |
| `anthropic-messages` | `low`, `medium`, `high`, `max` (adaptive thinking with `output_config.effort`) |
| `google-gemini` | `low`, `medium`, `high` (`thinkingBudget` on Gemini 2.5, `thinkingLevel` otherwise) |

An unsupported level fails before any request is sent, with ``provider `<name>` (<api>) does not support effort `<value>` ``.

## Model metadata

Context window, display name, compaction threshold, and prices come from the `models` list in your config:

```lua
return {
  models = {
    {
      name = "openai/gpt-5",
      display = "GPT-5",
      window = 400000,
      compact_at = 200000,
      prices = { input = 1.25, output = 10, cache_read = 0.125, cache_write = 0 },
    },
  },
}
```

| Field | Default | Meaning |
| --- | --- | --- |
| `name` | required | `provider/model`. If two entries share a name, the last one wins. |
| `display` | `name` | Label in the status bar, `/models`, and turn summaries. |
| `window` | `0` (unknown) | Context window in tokens. |
| `compact_at` | unset | Compaction threshold for this model. It wins over `settings.compact_at`, and `0` turns compaction off. |
| `prices` | unset | USD per 1M tokens: `input`, `output`, `cache_read`, `cache_write` (each defaults to `0`). |

`tuse models` prints this metadata. Changes take effect on `/reload`, which also recomputes the session's cost.

## Usage and cost

Each step records the provider's usage: `input` (all prompt tokens, including cache), `output`, `cache_read`, `cache_write`. The cost of a step is:

```text
(max(0, input - cache_read - cache_write) * input_price
 + cache_read * cache_read_price
 + cache_write * cache_write_price
 + output * output_price) / 1,000,000
```

`llm()` calls are counted the same way, and their cost is also tracked on its own. You can see the totals in the status bar, in `/session` (`$total ($main + $llm llm)`), in `session().usage`, and in JSON-RPC `session.stats`. Cost stays unknown until some usage matches a model that has `prices`. Not every API reports cache writes: `openai-responses` and `google-gemini` report `0`.

## Requests, retries, errors

- Timeouts are 30 s to connect and 600 s to read. Redirects are never followed, so keys don't leak to other hosts.
- HTTP `429`, `5xx`, and network errors are retried up to 5 times, after 2, 5, 10, 20, and 30 s. The TUI flashes `Retry N/5 in Ds: <error>`. Other errors stop the step and show up as `HTTP <code>: <body>`, `network: …`, or `bad response: …`.
- You can switch models and APIs in the middle of a session. History is converted for each request, and provider-specific reasoning data is only sent back to the same provider, API, and model.
- Images (pasted, attached, or read by `read()`) are sent to all four APIs.
