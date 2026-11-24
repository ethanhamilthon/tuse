# Sessions & Persistence

Tuse persists sessions, conversation history, providers, and global defaults in SQLite under `~/.tuse/`, with per-session directories for locks, background task logs, and pasted images. TUI and `tuse json` share the exact same store and lock format, so any session can be resumed from either front end.

## On-disk layout

- **`~/.tuse/tuse.db`** — SQLite database (WAL journal mode, 5-second busy timeout) containing:
  - **Sessions table:** one row per persisted session keyed by `id`, storing JSON metadata (`id`, `cwd`, `title`, `model`, `effort`, `created`, `updated`), indexed by `cwd`.
  - **Records table:** append-only ordered log `(session, seq, body)` of durable session records (JSON).
  - **Providers table:** connected API providers (`name`, `base_url`, `api_key`, `api`). Older databases automatically migrate missing `api` columns to `'openai-chat'`.
  - **State table:** global key-value defaults (`model`, `effort`).
- **`~/.tuse/<id>/`** — per-session directory holding:
  - `lock` — OS advisory file lock created when the session materializes in SQLite (or immediately on resume) and held while open.
  - `tasks/<task-id>.log` — log files created immediately when background tasks are started with `spawn()`.
  - `images/image-<N>.png` — PNG files written when images are pasted from the clipboard in the TUI.

## Session IDs and timestamps

- **Session ID:** UTC timestamp with nanosecond precision followed by `-` and 16 random lowercase hex characters, e.g. `20261119T004924.125476000Z-7136dd19e3fc184d`.
- **Metadata timestamps (`created`, `updated`):** UTC ISO-8601 format `YYYY-MM-DDTHH:MM:SSZ`.

## Lazy DB materialization and locking

- **New sessions:** Opening a fresh session does not acquire `~/.tuse/<id>/lock` or write rows to SQLite until the first `user` record is appended. Earlier records (such as the initial `settings` record, `start` hook records, or `ui_markdown`/`ui_result` records from pre-chat `show()`/`pick()`/`ask()` calls) and metadata are buffered in memory and flushed together in original order once the first `user` record arrives. Exiting before sending a user message leaves no rows in SQLite (and no lock file), even if the user interacted with `show`/`pick`/`ask`, though `~/.tuse/<id>/` itself can already exist on disk if a background task log or pasted clipboard PNG was created earlier.
- **Live record list:** `session.records` in `tuse json` (and internal session queries) returns the live in-memory record list, which includes records buffered before DB materialization.
- **Resumed sessions:** Opening an existing session with `--resume`, `/resume`, or `session.resume` immediately acquires `~/.tuse/<id>/lock`.
- **Single-writer exclusion:** If another Tuse process (TUI or `tuse json`) already holds the lock for `<id>`, opening it fails with `session <id> is open in another tuse process`. Switching to the already-open session fails with `session <id> is already open`.

## Durable records

Every persisted entry in a session's `records` log is a JSON object tagged by `"t"`. See [json.md](json.md) for JSON-RPC payload schemas.

| `"t"` | Fields | Purpose |
| --- | --- | --- |
| `user` | `text`, optional `context`, optional `images` (data URLs), `at` | User message and hidden context/images. |
| `assistant` | `text`, optional `reasoning`, optional `tool_calls`, optional `replay` (`{provider, api, model, items}`), `at` | Model response step and optional provider-scoped reasoning replay items. |
| `tool` | `id`, `tool`, `input`, `output`, `code`, optional `images`, optional `diffs` (`{path, diff}`), `at` | Tool execution result. |
| `notification` | `text` | Task notification fed to the model as a user message; hidden from TUI chat. |
| `notice` | `text` | Informational message shown in chat (for example compaction notices). |
| `error` | `text` | Error message shown in chat. |
| `compaction` | `prune`, `drop` | Context compaction record indices. |
| `usage` | `model`, `usage`, optional `llm: true` | Token usage for a main turn step or `llm()` call. |
| `settings` | `model`, `effort` | Model and effort active from this point in the session. |
| `hook` | `name`, `event`, `vars` (formatted `key=value` lines string), `error`, `ms` | Hook execution log (visible in `/trace`). |
| `turn_end` | `elapsed_ms`, `calls`, `model`, `effort`, `cancelled` | End-of-turn summary. |
| `ui_markdown` | `name`, `text`, `pin`, `removed` | Durable `show()` UI Markdown event; restored in chat order on resume, excluded from model context, compaction, and `/trace`. |
| `ui_result` | `id`, `kind` (`"picker"` \| `"question"`), `prompt`, `value` | Durable answered `pick()`/`ask()` result; restored in chat order on resume, excluded from model context, compaction, and `/trace`. |

## Session titles and listing

- **Auto-title:** Attempted once at the start of each turn if the session is still unnamed: Tuse inspects the queued inputs at turn start, finds the first non-notification user message, takes its first line (trimmed, `\r`/`\n` replaced with spaces, capped at 80 characters), saves it as the title if non-empty, and emits the `name_change` hook. (If a turn starts solely from a background task notification, or if the first line is empty, the session stays unnamed until a later turn starts with a non-empty user message or `set_title` is called.)
- **Manual title:** Lua `set_title(text)` applies the same normalization (rejecting empty strings), persists updated metadata, and emits `name_change` if the title changed.
- **Listing (`/resume` and `session.list`):** Queries sessions matching the target `cwd` ordered by `updated DESC`, including only sessions with a non-empty `title`. In the TUI `/resume` picker, the currently active session is excluded.

## Resuming and crash recovery

When resuming session `<id>`:

1. Loads session metadata from SQLite (fails with `no session <id>` if missing).
2. If the saved working directory still exists as a directory on disk, switches the session working directory to it; otherwise uses the caller's working directory.
3. Acquires `~/.tuse/<id>/lock` and loads/validates the Lua config in a fresh Lua VM.
4. Reads all durable `records` in sequence order:
   - **Interrupted tool-call recovery:** Inspects the **last `assistant` record that has non-empty `tool_calls`**; for any call ID lacking a subsequent `tool` record (for example after a crash or `SIGKILL`), appends and persists synthetic `tool` records with `output = "[interrupted: no output]"` and `code = null` so future API requests remain valid.
   - Replays `compaction` records to restore compaction bounds.
   - Replays `settings` records in order to restore model and effort—falling back to saved metadata and then global SQLite defaults only if no `settings` record exists. The latest `settings` record wins unconditionally, even if its `model` is explicitly `""`.
   - Replays non-`llm` `usage` records to restore the latest reported `context` token count (`input + output`). Note: resume restores this last API-reported count, **not** a post-compaction estimate if compaction happened after the last usage record.
   - Recomputes cumulative usage, cost, turn count, and step count using the current config's `models` metadata.
   - Replays `ui_markdown` and `ui_result` records in order **before** `start` hooks run, restoring surviving `show()` blocks (with original positions and pin state), answered `pick()`/`ask()` entries, and initializing the UI request ID counter to `max(restored ui_result.id)` with an empty pending prompt queue.
5. Fires `start` hooks with `reason = "launch"` (CLI `--resume`) or `"resume"` (`/resume` or `session.resume`), `is_new = false`, `restored = true`.

**What is not restored across resume:** Lua VM globals, running background tasks, and cancelled or pending (unanswered) `pick`/`ask` prompts.

## Switching and closing sessions

- **Atomic switch (`/new`, `/resume`, `session.new`, `session.resume`):** Tuse prepares and validates the next session (checking existence, lock, and Lua config) **before** closing the current session. If preparation fails, the current session stays open and unaffected.
- **Close order:** Closing a session (on exit, switch, or signal) runs in order:
  1. Closes the interactive UI queue (pending `pick`/`ask` calls receive an error and active prompts are cleared).
  2. Cancels running custom slash commands and `open` handlers.
  3. Cancels any active agent turn.
  4. Stops all running background tasks.
  5. Cancels hook UI waits, fires `close` hooks (`reason`: `"exit"`, `"new"`, or `"resume"`), disables further hooks, and waits until all active hook runs finish.
  6. Releases the session lock.

## Session snapshot and `/trace`

- **`require("tuse").session()`** returns a **fresh snapshot** table (mutating it in Lua has no effect on Tuse's live state) with `mode`, `configpath`, `cwd`, `home`, `id`, `name`, `model`, `provider`, `effort`, `ctx`, `ctx_size`, `usage`, `model_info`, `steps`, `turns`, and `trace` (never exposing provider API keys). During top-level config and plugin evaluation (both on startup and `/reload`), only `mode`, `configpath`, `cwd`, and `home` are populated; all session-only fields are `nil`.
- **`/session`** displays every snapshot field except `trace`.
- **`/trace`** (and `session().trace` / `session.trace`) builds chronological `{role, summary, text}` entries starting with index `0` (`system`, the current resolved system prompt including compaction note when active), followed by entries derived from durable records: `user` (text + hidden context + image count), `thinking` (non-empty reasoning), `agent` (non-empty assistant text), `lua` / `tool` (call input), `output` (`exit <code>`, output, image count, full diffs), `notification`, `notice`, `error`, and `hook` (`<name> <event> <ok|error> (<ms> ms)` plus `vars`). Each `summary` is the first non-empty trimmed line with control characters replaced by spaces (for empty tool output, `exit <code>`; for hooks, `<name> <event> <ok|error>`).
