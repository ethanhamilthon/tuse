# Planned: Persist `show`, `pick`, and `ask` in UI History (TODO #8)

## Goal

Persist `show()` Markdown blocks and completed `pick()` / `ask()` prompt results in each session's durable history so resuming a session (in TUI or JSON-RPC) restores them in their original chat positions while keeping them out of the model's conversation context.

## Target Behavior

### What Is Persisted

1. **`show(name, text, pin)`**:
   - Every `show()` call records a durable UI markdown event with `{ name, text, pin, removed }`, where `removed` is `true` iff `text == ""` and `pin == false`.
   - Recorded at the exact point in the session's ordered record stream when `show()` executes.
2. **Completed `pick(options)` and `ask(text)`**:
   - Only when the user submits an answer (`Enter` in TUI, `ui.reply` in JSON-RPC), record a durable UI result event with `{ id, kind, prompt, value }`:
     - For `pick`: `kind = "picker"`, `prompt = "Picker"`, `value = <selected option>`.
     - For `ask`: `kind = "question"`, `prompt = <markdown text>`, `value = <typed answer>`.
3. **What is NOT persisted**:
   - Cancelled prompts (`Esc` in TUI, `ui.cancel` in JSON-RPC, job cancellation, hook reload cancellation): create **no** persisted record and no UI history entry.
   - Pending (unanswered) prompts still in the queue when a session closes: cleared on close, never persisted, and never restored on resume.

### Persistence Format, Protocol v1 Compatibility, and Materialization

- **Decision:** Store persisted UI history in the session's durable SQLite record stream using two additive record tags (`"t"`):
  - `{"t": "ui_markdown", "name": string, "text": string, "pin": bool, "removed": bool}`
  - `{"t": "ui_result", "id": u64, "kind": "picker" | "question", "prompt": string, "value": string}`
- **Protocol v1 compatibility**:
  - **Decision:** Adding `"ui_markdown"` and `"ui_result"` to `session.records` is a backward-compatible additive extension within `protocolVersion = 1`. Existing v1 clients already filter `session.records` by `"t"` (skipping non-chat tags such as `"compaction"`, `"usage"`, `"settings"`, `"hook"`, and `"notification"`), so clients that ignore unknown `"t"` values keep working, while clients that handle `"ui_markdown"` and `"ui_result"` reconstruct exact interleaved UI history.
- **Lazy materialization and UI-only sessions on quit**:
  - **Decision:** Keep SQLite session materialization and session lock acquisition gated on the first `user` chat message. UI history records emitted before the first user message (for example, `welcome`'s `show()` on `start`, or a custom command running `show`/`pick`/`ask` before chatting) buffer in the session's pending records and flush to SQLite in their original order when the first user message arrives.
  - Consequently, if a user opens a fresh session, interacts only with `show`/`pick`/`ask` (no user chat message is ever sent), and quits, **no session is saved in SQLite** (it does not appear in `/resume` or `session.list`, and no `lock` file is created; note that `~/.tuse/<id>/tasks/` or `images/` may still exist on disk if a task was spawned or an image pasted).

### Replay Boundary and Restoration on Resume (TUI and JSON-RPC)

When opening a resumed session (`--resume`, `/resume`, `session.resume`), persisted `ui_markdown` and `ui_result` records are replayed in record order **before** the `start` (`reason = "resume"`, `restored = true`) hook fires:

- **`show` replay semantics** (identical to live `show()`):
  - Creating an unpinned block (`pin = false`, `removed = false`) anchors it at that record's position in the transcript.
  - Updating an existing unpinned block with the same `name` (`pin = false`) replaces its `text` **in place** at its original anchor position without moving it.
  - Changing `pin` (`false -> true` or `true -> false`) removes the block from its old section and inserts it into the new section at the record position of the pin change (or at the end of the pinned list when `pin = true`).
  - Updating an existing pinned block (`pin = true`) replaces its `text` in place in the pinned list.
  - A removal (`removed = true`) deletes the named block from whichever section it occupied so it does not appear on resume.
  - Subsequent `show(name, ...)` calls after resume continue updating, moving, pinning/unpinning, or removing restored blocks by `name` (and persist those mutations as new records).
- **`pick` / `ask` replay semantics and prompt ID scoping**:
  - Each `ui_result` record restores its chat entry at that exact record position.
  - **Decision (incarnation-scoped prompt IDs):** Pending prompts are never restored across resume, so immediately after replay (before `start` hooks run), the pending prompt queue is empty (`active = null`). Initialize the resumed session's UI request ID counter to `max(restored ui_result.id)` (so the next live prompt receives `max + 1`, never colliding with any restored `ui_result.id`). Because pending prompts are scoped to a single live session incarnation and cleared on close/switch, `ui.reply`/`ui.cancel` validate strictly against the current incarnation's active queue (any stale reply from before close or switch is rejected as not active, even if an earlier unpersisted cancelled prompt used a lower ID).
- **Interaction with `start` hooks on resume**:
  - Because `start` (`reason = "resume"`) runs after replay completes, a `start` hook can call `show()`, `pick()`, or `ask()` before a JSON-RPC client calls `ui.state`. Thus `active` is guaranteed `null` at the replay boundary before `start` hooks run, and by the time `ui.state` is queried it reflects restored history plus any blocks or prompts created by `start` hooks.
- **TUI rendering**: restored unpinned `show` blocks and `ui_result` entries render in chat at their original positions interleaved with user/assistant/tool/notice/error/turn_end entries; restored pinned `show` blocks render together at the end of the chat after all messages.
- **JSON-RPC exposure**:
  - `session.records` includes `ui_markdown` and `ui_result` records in chronological order alongside conversation records so clients reconstructing chat history can interleave them at their original positions.
  - `ui.state` returns the reconstructed effective state (`markdowns` with surviving non-removed blocks `{ name, text, pin }`, `entries` with restored `{ id, kind, prompt, value }`, and `active` as `null` unless a `start` hook queued a prompt).
- **Reload after resume**: `/reload` and `config.reload` continue to preserve both restored and newly added UI blocks and entries.

### Model Context, Compaction, and Trace Isolation

- `ui_markdown` and `ui_result` records are **never** included in messages sent to the model API (both full context and compacted/conversation-only context), never counted toward context token estimation, and never affect tool-call/tool-result pairing or compaction bounds.
- **Decision:** Keep `ui_markdown` and `ui_result` records out of `/trace` and `session().trace` (matching current live behavior where UI-only blocks and prompt entries are excluded from trace).

### Backward Compatibility

- Existing sessions saved before this change contain no `ui_markdown` or `ui_result` records; resuming them produces empty restored `markdowns` and `entries` (before any `start` hook runs) with identical conversation rendering to before.

## Error Reporting

- Persistence uses the same record append path as all other session records; no new error codes are introduced.
- Unknown or malformed stored record rows continue to be ignored on load without aborting resume.

## Docs-First Handoff Gate

Per TODO #3, keep baseline docs describing current behavior until ready to implement this spec. Before handing off to an implementation agent, update and review these baseline doc sections first:

- `docs/functions.md`: update `show`, `pick`, and `ask` to document UI history persistence across resume, exclusion from model context, and non-persistence of cancelled/pending prompts.
- `docs/json.md`: add `"ui_markdown"` and `"ui_result"` to `session.records` and update `session.resume` and `ui.state` (including `start`-hook ordering).
- `docs/sessions.md`: document the new durable UI history record tags and their exclusion from model context and compaction.

## Acceptance Tests

1. **Unpinned `show` position preserved**: interleave turn 1, `show("a", "first", false)`, turn 2, `show("b", "second", false)`; close and resume in TUI and JSON-RPC; verify `"a"` appears between turn 1 and turn 2 and `"b"` appears after turn 2.
2. **Named `show` update keeps position**: call `show("a", "v1", false)`, run a turn, call `show("a", "v2", false)`; close and resume; verify only one `"a"` block exists with `"v2"` at its original position before the turn.
3. **Named `show` removal persists**: call `show("tmp", "hi", false)`, then `show("tmp", "", false)`; close and resume; verify `"tmp"` is absent. Also remove a block created before resume, close, and resume again; verify it stays gone.
4. **Pinning and unpinning persist**: create pinned block `"p"`, add messages, resume; verify `"p"` renders after all messages. Unpin `"p"`, add more messages, resume; verify `"p"` stays inline at its unpin position. Pin an existing unpinned block, resume; verify it renders at the end.
5. **Answered `pick` and `ask` persist in position**: answer a `pick` and an `ask` between turns; close and resume in TUI and JSON-RPC; verify both entries appear in original order in TUI chat, `session.records`, and `ui.state.entries`.
6. **Cancelled and pending prompts not persisted**: cancel one `pick` and one `ask`, leave another prompt pending when closing; resume (with no `start` hook prompting); verify zero records/entries exist for cancelled or pending prompts and `ui.state.active` is `null`.
7. **`start` hook interaction on resume**: configure a `start` hook that calls `show()` and `ask()` on `reason == "resume"`; verify persisted history restores before `start` runs and `ui.state` reflects both restored history and the hook's new block/prompt.
8. **Model context exclusion**: create `show`, answered `pick`, and answered `ask`, resume, and submit another turn; verify the API request payload (before and after compaction) contains none of the UI block or prompt text.
9. **Old sessions & v1 client compatibility**: resume an old session with no UI records (loads cleanly); also resume a session containing `ui_markdown` and `ui_result` records and verify a v1 JSON-RPC client ignoring unknown `"t"` tags still reconstructs the conversation cleanly while a UI-aware client sees both records in `session.records` and `ui.state`.
10. **Cross-frontend resume**: create UI history in TUI and resume in JSON-RPC, and create UI history in JSON-RPC and resume in TUI; verify identical restored order and pinning.
11. **Incarnation-scoped prompt IDs**: answer `ask` (ID 1), cancel a second `ask` (ID 2), close, resume, and verify: (a) a stale `ui.reply` for ID 2 is rejected as not active, and (b) a new `ask` after resume receives an ID strictly greater than all restored `ui_result.id` values (ID 2) without colliding with restored entry ID 1.
12. **UI-only session on quit**: open a fresh session, run `show()`, answer a `pick()`, and answer an `ask()` without ever sending a user chat message; quit; verify no session row exists in SQLite (`session.list` / `/resume` does not include it) and no `lock` file was created.
