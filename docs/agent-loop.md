# Agent Loop & Tool Lifecycle

A turn begins when the user (or a completed background task) submits input to an idle session and ends when a step finishes without tool calls (or fails) and no further queued inputs remain, or when cancellation stops it. For Chat Completions wire framing, streaming details, and retry rules, see [providers.md](providers.md); for Lua execution and built-in functions, see [tool.md](tool.md) and [functions.md](functions.md).

## Turn submission and steering queue

- **Idle session:** Submitting input marks the session busy, starts a fresh generation and cancellation flag, attempts session auto-titling if unnamed ([sessions.md](sessions.md)), emits `turn_start`, and starts the turn loop on a background worker thread.
- **Busy session (steering):** Submitting input while a turn is already running appends it to the session's FIFO input queue and returns `(inputId, queued = true)`.
- **Withdrawing queued input:** Before the turn loop drains a queued item at the start of the next step, the client can withdraw it by `inputId` (in the TUI, editing the dimmed draft withdraws it automatically; in JSON-RPC, `turn.withdraw`). Withdrawing does not interrupt the running step.

## Step execution loop

Each iteration of the turn loop runs one model request and any tool calls it emits:

1. **Drain queued inputs:** All currently queued items are taken in order:
   - User inputs emit `user`, persist a `user` record, and trigger `user_message` hooks (`is_first = true` only for the first `user` record in the session).
   - Task completion notifications emit `notification` and persist a `notification` record (they do not trigger `user_message` hooks and are not displayed in the TUI chat).
2. **Assemble prompt and messages:**
   - **System prompt:** Built-in system prompt (omitting any built-in host functions disabled with `false`) + each string in `config.systems` + a compaction explanation note if earlier records were dropped. Configured `functions` are not listed automatically.
   - **Message history:** Built from durable `records` using the active compaction bounds ([usage.md](usage.md), [providers.md](providers.md)).
   - **Tools:** Always advertises a single function tool: `lua(code)`.
3. **Resolve provider and call API:**
   - Splits active `model` on the first `/` and looks up the provider in the session's loaded provider snapshot. If missing, records and emits an `error` (`no \`connect\` entry named \`<provider>\` for model \`<model>\``) and treats the step as done.
   - Sends the Chat Completions request via the HTTP client ([providers.md](providers.md)): streams SSE until `data: [DONE]` or stream EOF (or parses a single JSON fallback body), emitting `reasoning` deltas (from `reasoning_content` and/or `reasoning` — both emit if both appear), `text` deltas, and live `tool_delta` code chunks. When the response ends, tool calls with empty names are dropped and remaining calls without IDs receive synthetic IDs `call_0`, `call_1`, etc.
   - Retryable failures (HTTP `429`, HTTP `5xx`, network/transport errors) retry up to `min(settings.retries, 5)` times over `[2, 5, 10, 20, 30]` seconds, emitting `stream_reset` and `flash` (`Retry A/M in Ds: <error>`) before each delay.
   - Non-retryable or exhausted errors record and emit an `error` and treat the step as done (see step 5: if more inputs are already queued, the turn continues to a new step instead of ending).
4. **Record assistant step:**
   - If the response includes `usage`, updates session `context = input + output` and persists a main `usage` record (`llm = false`).
   - Persists an `assistant` record with accumulated text, reasoning, and tool calls.
   - If text is non-blank or tool calls are present, fires `agent_message` hooks with `status = "working"` (when tool calls exist) or `"finished"` (when none).
   - Emits `assistant` and `usage`.
5. **Branch on tool calls (and failed steps):**
   - **No tool calls (or step failed with an error):** Checks the input queue. If new inputs arrived while the step ran, loops into another step to process them; otherwise marks the session idle, persists a `turn_end` record (`cancelled = false`), updates session metadata, and emits `turn_end`.
   - **One or more tool calls:** Runs each tool call sequentially in order, checks compaction after all calls finish ([usage.md](usage.md)), and loops to the next step.

## Tool call lifecycle

For each tool call emitted in a step:

1. Parses `arguments` as JSON (extracting `code` as display input when present, otherwise raw `arguments`) and emits `tool_start` (`timeout = 120`, `id`, `tool`, `input`).
2. Dispatches execution:
   - Invalid JSON arguments, any tool name other than `lua`, or missing/non-string `code` immediately returns an error result (`code = 1`) **without** starting a Lua job or emitting `lua_start`/`lua_end` hooks.
   - Valid `lua` calls assign a sequential execution ID (`lua-1`, `lua-2`, ...), fire `lua_start` (`kind = "tool"`), run the code as a coroutine in the session's persistent Lua VM with a 120-second deadline and the turn's cancel flag, and fire `lua_end` (`kind = "tool"`, status `"success"` / `"error"` / `"cancelled"`, duration in ms).
3. While running, built-in host functions emit transient `status` updates (`<verb> <first line, max 60 chars>...`, suppressing consecutive identical strings). File mutations (`write`, `append`, `edit`, `delete`) accumulate diffs and `read` on image files accumulates image data URLs on the active tool job.
4. If the turn was cancelled mid-call, the stale result is discarded (cancellation already recorded placeholder tool results). Otherwise Tuse increments the turn's call counter, persists a `tool` record (output, exit code `0` / `1` / `null`, captured images, captured diffs), and emits `tool_end`.

## Cancellation

Cancelling (`Esc` in TUI when no overlay is open, or `turn.cancel` in JSON-RPC):

1. Stops running custom slash commands and `open` handlers.
2. Cancels currently running hook invocations (queued follow-up events in a hook's lane are not cleared).
3. If a turn is busy: sets the turn's cancel flag, increments generation (so any late API or tool result is discarded), marks the session idle, clears pending queued inputs, appends synthetic `[interrupted: no output]` tool records for any unfinished tool calls, persists and emits `turn_end` with `cancelled = true`, saves metadata, and fires `agent_message` hooks with any partial streamed text and `status = "cancelled"`.

Cancellation is **cooperative** for in-flight HTTP requests: SSE streaming checks the cancel flag before reading each line, and retry delays poll every 100 ms, so an ongoing socket connect/read or a non-SSE JSON body read is not immediately aborted at the OS socket level (the session still becomes idle right away and ignores the stale response when it returns).

## Background task notifications and `llm()`

- **Task notifications:** When a background task started with `spawn()` finishes or fails on its own (not stopped via `task_stop`), Tuse submits a `notification` input:
  `<notification>Task tN (name) finished|failed. Last line: <last>. Output: \`task_get("tN").output\`.</notification>`
  `<last>` is derived from the task's **return value or error message** (not lines written via `print()`/`io.write()` to the task log); a task that returns `nil` has no return text, which yields `Last line: failed.` even when it finished cleanly ([tasks.md](tasks.md)). If the session is idle, the notification starts a new turn; if busy, it is queued for the next step.
- **One-shot `llm(prompt, model?, effort?)`:** Sends a standalone user-message request (no tools, no conversation history) through the same provider and retry client ([providers.md](providers.md)). `model` and `effort` default to the session's active values. It yields cooperatively, cancels when its own calling coroutine is cancelled or times out (so cancelling a turn cancels `llm()` inside tool calls, hooks, or commands, but **not** inside background tasks), and records a `usage` record (`llm = true`) and emits `usage` **only if** the API reply includes a `usage` object.
