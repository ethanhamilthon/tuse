# Providers & Models

Tuse separates **provider credentials** (stored in SQLite via the CLI; see [providers.md](providers.md)) from **model metadata** (declared in Lua config) and **model selection** (scoped per session, with a global default for new sessions in SQLite).

## Model naming

Models are always identified as `<provider>/<model_id>`, split on the **first** `/`:

- Provider prefix = substring before the first `/`.
- API `model` sent to the provider = substring after the first `/` (so nested IDs such as `openrouter/anthropic/claude-sonnet-4` work cleanly).
- Configured `models[].name` values must have non-empty provider and model parts and no whitespace.

## Availability and discovery

A model is considered **available** if and only if its provider prefix exists in the **session's loaded provider snapshot** (loaded from SQLite at startup and refreshed on `/reload`, `reload()`, or `config.reload`). Availability does **not** query SQLite live and does **not** verify that the remote catalog listed the model ID; removing a provider with `tuse disconnect` takes effect in an existing session only after reload or reopen.

The available model list (shown in `/models` and returned by JSON-RPC `model.list`) is the sorted, deduplicated union of:

1. Configured `models[].name` entries whose provider is in the session's loaded provider snapshot.
2. Remote model IDs previously fetched via each loaded provider's model-listing endpoint (`GET <base_url>/models`, paginated for `anthropic-messages` and `google-gemini`; see [providers.md](providers.md)), prefixed with `<provider>/` and cached in memory until reload.

Background discovery:

- Opening `/models` in the TUI or calling JSON-RPC `model.list` (without `cached: true`) **returns immediately** with the currently known list (configured + already-cached remote models) and starts background fetches in parallel (one thread per loaded provider) without waiting for them.
- Each provider that responds updates the session cache and emits a `models` event so the picker or JSON-RPC client can refresh live (`model.list {"cached": true}`).
- Reload clears the cached remote model list and discards any stale in-flight fetch responses or errors from before the reload.
- Fetch failures emit `error` (`<provider>: can't fetch models: <err>`) without blocking other providers.

## Model metadata (`models` in Lua config)

Context windows, display names, per-model compaction thresholds, and token pricing are declared in the `models` list of your Lua config (or plugins):

```lua
return {
  models = {
    {
      name = "openai/gpt-5",
      display = "GPT-5",
      window = 200000,
      compact_at = 120000,
      prices = { input = 2.0, output = 10.0, cache_read = 0.2, cache_write = 2.5 },
    },
  },
}
```

| Field | Required | Default | Meaning |
| --- | --- | --- | --- |
| `name` | yes | — | Full `provider/model` identifier. |
| `display` | no | `""` | Friendly label shown in the status bar, turn summary, and `/models` picker (falls back to `name`). |
| `window` | no | `0` | Context window in tokens (`0` means unknown). |
| `compact_at` | no | unset | Per-model compaction threshold in tokens; overrides `settings.compact_at` in both directions (a positive value overrides global `0`, and `0` disables compaction for this model even if global `settings.compact_at` is positive). |
| `prices` | no | unset | Table of finite non-negative USD rates per 1,000,000 tokens: `input`, `output`, `cache_read`, `cache_write` (each defaults to `0`). |

- **Precedence:** Plugin `models` lists come first, then your config's `models`. When multiple entries share the same `name`, the **last** entry wins.
- **Inspecting metadata:** Run `tuse models [-c PATH]` from the CLI or `/session` inside the TUI.
- **On reload:** Updated metadata takes effect immediately (display name, `ctx_size`, `compact_at`, and recalculated historical session costs) and the cached remote model list is cleared.

## Reasoning effort

Valid effort levels are: `"default"`, `"low"`, `"medium"`, `"high"`, `"xhigh"`, `"max"`.

- `"default"` omits effort/thinking parameters from API requests across all four API types.
- Non-default effort levels are mapped per provider API type ([providers.md](providers.md#capability-matrix)):
  - `openai-chat`: `low`, `medium`, `high`, `xhigh`, `max`
  - `openai-responses`: `low`, `medium`, `high`, `xhigh` (`max` unsupported)
  - `anthropic-messages`: `low`, `medium`, `high`, `max` (`xhigh` unsupported)
  - `google-gemini`: `low`, `medium`, `high` (`xhigh`, `max` unsupported)
- The full 6-value effort set stays available in `/models` (annotating unsupported levels when the provider's API type is known) and in `model.set`; unsupported combinations fail fast before any HTTP request on agent turns and `llm()`.
- Invalid effort strings outside the 6-value set are rejected immediately by `model.set` and `llm()`.

## Selection, persistence, and scoping

- **Global defaults:** Stored in SQLite `state` (`model` defaults to `""`, `effort` defaults to `"default"`). New sessions inherit these defaults and append an initial `settings` record.
- **Changing model/effort:**
  - TUI `/models` prompts for model, then effort (pre-selecting the session's current values), updates the active session, and persists both as the new global defaults.
  - JSON-RPC `model.set {model, effort?, persist?}` defaults `effort` to `"default"` and `persist` to `true`. Passing `model: ""` is accepted and clears the session's active model (and the global default if `persist` is `true`).
  - Every successful selection appends a `settings` record, updates session metadata, and emits `usage`—even when `model` and `effort` did not change. Only the `model_change` hook is gated on an actual change.
- **Session isolation:** Existing and resumed sessions keep their own recorded model and effort (the latest `settings` record wins on resume, even if `model` is `""`). Changing the global default in another session, running `/reload`, or resuming an older session never overwrites a session's own selection.
- **Unavailable or unset current model:** The `/models` picker always includes the session's current model even when its provider is not in the loaded provider snapshot (marked `unavailable` when non-empty) and refuses re-selecting an unavailable model (`Model \`<name>\` is unavailable`). Submitting a turn with an unavailable or empty model fails with `no \`connect\` entry named \`<provider>\` for model \`<model>\``.
