# Usage, Cost & Context Compaction

Tuse tracks token usage and USD cost across main turns and `llm()` calls, and automatically compacts older tool output when a session's context grows past its compaction threshold.

## Usage accounting

### Token extraction

When an API response includes a `usage` object, Tuse records:

- `input` (`prompt_tokens`) — total input tokens, including cached reads and cache writes.
- `output` (`completion_tokens`) — completion tokens generated.
- `cache_read` (`prompt_tokens_details.cached_tokens`) — cached prompt tokens read.
- `cache_write` — `max(prompt_tokens_details.cache_write_tokens, prompt_tokens_details.cached_creation_tokens)`.

Main turn steps persist a main `usage` record (`llm = false`) and update the session's live context size to `input + output`. Lua `llm()` calls persist a `usage` record marked `llm = true` (and emit `usage`) only when the reply supplies usage, counting toward cumulative totals and spend without changing the session's `context`.

### Cost calculation

For each `usage` record whose `model` matches a configured entry with `prices`:

```text
plain_input = max(0, input - (cache_read + cache_write))
cost_usd = (plain_input * prices.input
          + cache_read  * prices.cache_read
          + cache_write * prices.cache_write
          + output      * prices.output) / 1,000,000
```

- `session().usage.cost` is the cumulative total across main turns and `llm()` calls; `session().usage.llm_cost` is the `llm()` portion alone.
- Cost is `nil` / `(unknown)` (and omitted from the TUI model bar) until at least one usage record matches a model with configured `prices`.
- Updating `models[].prices` and running `/reload` recomputes cumulative costs across all records in the session immediately.

### Token formatting and display

Token counts format compactly:

- `< 1,000`: exact integer (`999`)
- `1,000`–`999,999`: `k` with up to 1 decimal, dropping `.0` (`5.2k`, `200k`)
- `>= 1,000,000`: `m` with up to 1 decimal, dropping `.0` (`1m`, `1.5m`)

Where usage appears:

- **TUI bottom status bar:** `<display> · <effort> [· <used>/<window>] [· $<cost>]`. The context segment is omitted completely when both `<used>` and `<window>` are `0`; when `<window>` is `0` and `<used> > 0`, it shows `<used>/?`. Cost renders to 2 decimal places when priced (omitted otherwise); `<used>` is colored `accent`.
- **`/session` panel:** `Context` (`<used> / <window> (<pct>%), compacts at <threshold>`), `Turns` and `steps`, `Usage` (`in`, `cache read`, `cache write`, `out`), and `Cost` (`$<total>` to 4 decimal places plus `($<main> + $<llm> llm)`).
- **Lua / JSON-RPC:** `session().ctx`, `session().ctx_size`, `session().usage`, `session().steps`, `session().turns`, and `session.stats` / `session.snapshot`.

## Context compaction (50/25/25)

Compaction reduces what is sent to the model on future steps without altering or deleting any saved records in SQLite or `/trace`.

### Compaction threshold

After tool execution finishes at the end of a tool-using step, Tuse resolves the compaction threshold (`compact_at`):

1. Active model's `models[].compact_at` if set. This takes precedence over `settings.compact_at` in **both** directions: a positive per-model threshold overrides a global `settings.compact_at = 0`, and a per-model `compact_at = 0` disables compaction for that model even if `settings.compact_at` is positive.
2. Otherwise `settings.compact_at` if set (`0` disables compaction globally unless overridden per model).
3. Otherwise `floor(model.window * 6 / 10)` (60% of the configured `window`), or `0` (disabled) when `window` is unknown (`0`).

If `threshold > 0` and the latest `context >= threshold`, compaction runs.

### Token estimation and three-zone algorithm

Tuse walks backward from the newest record, estimating tokens per message as:

- **Non-multipart messages** (string `content`, or assistant messages with `tool_calls`): `floor(serialized_message_json_bytes / 4)`.
- **Multipart messages** (`content` is a JSON array, such as user messages with attached images): sum across `content` parts—each `image_url` part counts as `1,000` tokens, and every other part counts as `floor(serialized_part_json_bytes / 4)`.

Using these estimates, Tuse computes monotonic record index bounds `(drop, prune)`:

- **Fresh zone (newest ~25% of `threshold`, `index >= prune`):** Sent in full—user text with hidden context and images, assistant text and tool calls, full tool outputs, and images returned by `read()`.
- **Pruned middle zone (from ~25% to ~50% of `threshold`, `drop <= index < prune`):** Keeps user messages, hidden context, assistant messages, and tool-call structure, omits images from tool results, and shortens each tool result output to its first line (up to 200 characters) plus `\n[tool output pruned]` whenever the output is multiline or longer than 200 characters.
- **Dropped old zone (older than ~50% of `threshold`, `index < drop`):** Anchored at a `user` or `notification` record boundary so assistant `tool_calls` and their matching `tool` results are never split. Keeps only plain conversation text: user `text` (without hidden `context` or `images`), `notification` text, and non-empty assistant `text`. All tool calls and tool results in this zone are omitted.

Bounds only advance forward (`max` with previous bounds). If neither `drop` nor `prune` advances, nothing changes.

### Effects of compaction and resume

When bounds advance:

1. Appends a `compaction` record (`prune`, `drop`) to the session log (restored automatically on resume).
2. Subsequent API requests append a note to the system prompt when `drop > 0`: *"Earlier parts of this conversation were compacted to save context: old tool calls and their outputs were removed and some later outputs were cut to their first line."*
3. Updates the live session's `context` counter in memory to the estimated token count of the compacted message list.
4. Appends and emits a notice: `Context compacted at <threshold> tokens; now about <estimated>.`

On **resume**, Tuse restores the `compaction` bounds from `compaction` records, while `context` is restored from the last non-`llm` `usage` record (`input + output`), **not** from the temporary post-compaction estimate.
