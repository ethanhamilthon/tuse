## Context

This design finalizes the `update-public-api-consistency` proposal with all user design decisions (`Q1`–`Q10` and `P1`–`P4`) resolved alongside all verified v0.13.1 audit findings (`H1`–`H6`, `M1`–`M10`, `L1`–`L6` including `L6.1`–`L6.4`).

Repository constraints from `tuse.lua`:
- Keep production Rust code under 20,000 lines (excluding tests).
- Zero comments in Rust source files; minimal targeted tests.
- Everything in English (UI, contracts, docs).
- Do not edit `README.md` without explicit user instruction.

### Audit Verification Nuances & Codebase Evidence

1. **Provider adapter wire encoding (`Q2` / `P3`, verified in `crates/api/src/`):**
   - `crates/api/src/client.rs:151-160` currently rejects requests pre-dispatch via `ApiType::supports_effort(effort)`. That hardcoded provider availability matrix is removed.
   - Adapter wire behavior under `P3` (verbatim pass-through, no renaming/clamping/invented mappings):
     - `openai_chat.rs:72-74`: sends `"reasoning_effort": effort` verbatim when `Some`, omits when `None`.
     - `openai_responses.rs:67-70`: sends `"reasoning": {"effort": effort, "summary": "auto"}` and `"include": ["reasoning.encrypted_content"]` verbatim when `Some`, omits when `None`.
     - `anthropic.rs:98-103`: sends `"thinking": {"type": "adaptive"}` and `"output_config": {"effort": effort}` verbatim when `Some` (and `!missing_thinking(req)`), omits when `None`.
     - `gemini.rs:112-124`: for non-2.5 models, sends `"thinkingConfig": {"includeThoughts": true, "thinkingLevel": effort}` verbatim when `Some`, omits when `None`. For `gemini-2.5-*`, there is no string effort field — only integer `thinkingBudget` (`"low" => 1024`, `"medium" => 8192`, `"high" => 24576`); the old `_ => 24576` catch-all silently clamped `"xhigh"` and `"max"` and MUST be replaced with an explicit wire-encoding error (no invented budget numbers, no silent clamping).
   - `crates/core/tests/api_types.rs:70, 124, 173-179` currently tests the old provider-allowlist rejection and must be updated to verify model-scoped `config.models[].efforts` enforcement and `gemini-2.5-*` non-clamping wire encoding.
2. **Tri-state `ModelDef.efforts` & legacy `"default"` (`Q2` / `P2`):**
   - Because Lua `{}` serializes to JSON `{}` (empty object), `ModelDef.efforts: Option<Vec<String>>` MUST use a custom optional-list deserializer (`opt_list`) that distinguishes omitted (`None` -> shared 5-level default list) from explicitly empty `efforts = {}` (`Some(vec![])` -> non-reasoning model sending no reasoning parameters).
   - `crates/core/src/storage/db.rs:115-120` (`Prefs::effort()`), `crates/rpc/src/methods.rs:163-166`, `crates/core/src/session/turn/run.rs:123`, and `crates/core/src/session/lua/commands.rs:145` currently special-case `"default"`. Under `P2`, `"default"` is retired everywhere; `Prefs` safely recovers preserved v3 `state.effort = "default"` via the `"medium"` / first-declared fallback rule (or `""` when `efforts = {}`).
3. **Mid-draft commands & existing TUI tests (`Q3` / `P4` / `H6` / `L4`):**
   - `crates/tui/src/ui/keys.rs:413-422` tests `"draft /models after"` opening `/models` mid-draft and restoring `"draft  after"`.
   - `crates/tui/src/kit/editor/mod.rs:291` (`command_before_token`) currently blocks mid-draft `CommandArg` pickers via `self.buf[0] != '/'`; `crates/tui/src/ui/commands.rs:178, 183` and `send.rs:80` call `self.editor.clear()`, wiping surrounding text and losing drafts on invalid args (`modal.rs:382` currently asserts clear on `/deploy nope`); `send.rs:66, 72` trims `"/new "` into `"/new"`, accidentally firing `/new` even after typing space disengaged the picker.
4. **Providers storage & JSON alignment (`Q6` / `M3`):**
   - `crates/core/src/storage/db.rs:198-216` (`Providers::apply`) merges saved SQLite profiles and Lua `config.providers` into `config.connect`. `crates/cli/tests/connect.rs:143-154` currently checks `tuse providers --json` for `"api"`; aligning `tuse providers --json` to `{ name, type, base }` (saved only) alongside `config.get.providers` (all active) updates that test and `docs/usage.md:39`.
5. **`L6.4` (`crates/rpc/src/events.rs:59`):** `Event::Switch(_)` is matched exhaustively on `tuse_core::Event`. Returning `Option<Value>` from `events::to_json` (`None` for `Event::Switch(_)`) prevents synthesizing an undocumented `{"type": "switch"}` payload, while `Session::cancel_turn(handoff)` and `State::cancel_pending` are dead code and deleted cleanly.
6. **`M5` (`crates/core/src/convo/wire.rs:27-35`):** Inline `data:` URLs in `turn.submit` are passed directly via `Record::User.images` and have no local file path, so omitting `<image id="..." path="..."/>` context tags for inline data URLs is intentionally retained while enforcing strict `data:image/<mime>,<data>` validation.
7. **`M6.2` (`crates/cli/src/args.rs:28`):** Guarding valued flags only against `-h`, `--help`, and known flags of the current command (rather than rejecting any `-`-prefixed token) allows `--key -secret` without breaking missing-value diagnostics like `tuse -c --resume id`.
8. **`M8.2` (`crates/core/src/session/meta.rs:120-128, 138-146`):** Both combined `set_session{ title, history }` and sequential `set_session{ history }` -> `set_session{ title }` during `tool_step` lose `Record::Meta` on `end_tool_step`; both are fixed by carrying deferred `meta` records into `pending_history`.
9. **`M10` bonus doc catches:** Also verified drift in `docs/connect.md:53` (omits `/models` scoping), `docs/config.md:129` (omits `bar` mark under `icons`), and `docs/session.md:11` (outdated pre-0.11 migration note).

## Intentional Design Boundaries & Audit Limitations

The following boundaries are intentional and MUST be preserved:

1. **`set_session` is Lua-only (`openspec/specs/tui/spec.md:76-90`):** JSON-RPC `session.set` and `config.set` stay `-32601` (method not found); history edits happen only through Lua `set_session` (via commands, hooks, or agent `lua` calls).
2. **Intentional v2 parity exception for `session.resume` (`P1` / `Q8`):** TUI `/resume` switches sessions in-process, whereas JSON-RPC v2 removes `session.resume` (`-32601`) and retains `session.list` for session discovery. JSON-RPC clients resume a saved session by launching a process with `tuse json -r <id>` or by invoking Lua `resume(id)` via a command/hook (which emits `session_switched`).
3. **`/models` scopes to `config.models`; JSON-RPC `model.list` ignores it (`openspec/specs/config/spec.md:237-250`, `docs/config.md:176`).**
4. **`Event::Status` emitted for JSON-RPC, dropped in TUI (`crates/tui/src/ui/events.rs:12-14`, tested in `crates/tui/src/ui/screen/status.rs:150-234`).**
5. **`Ctrl+A` / `Cmd+A` selects all draft text (`openspec/specs/tui/spec.md:392`).**
6. **Secret masking under `providers.*` (`crates/config/src/load.rs:43-44`):** Config validation errors under `providers.*` return generic `"invalid provider settings"` so secret values are never echoed. `H2`'s path-aware Lua converter MUST preserve this masking under `providers.*`.
7. **`ask()` and `pick()` titles render unlinked Markdown (`links = false`) (`crates/tui/src/ui/modal.rs:69`, `crates/tui/src/ui/screen/compose.rs:79`, `openspec/specs/tui/spec.md:467`).**
8. **Retained UI customization knobs (`Q9` / `L6.1`, `L6.2`):** Keep all four `ui.input` composer styles (`boxed`, `lines`, `fill`, `bar`) and all 33 `ui.theme` tokens.

**Audit Limitations:**
- Running `cargo test --workspace` inside the restricted sandbox blocked on loopback socket binds (`127.0.0.1:0`); `cargo test -p tuse-config --locked --quiet` passed (9/9). All findings and design choices were verified directly against source, specs, schemas, and tests.
- Live terminal emulator quirks (Kitty keyboard negotiation, OS clipboard backends) and external upstream API acceptance of verbatim `xhigh`/`max` effort strings were not live-tested (`P3` intentionally avoids claiming unverified upstream acceptance).

---

## Resolved Cross-Surface Design

### 1. Restored `TUSE_GLOBAL_CONFIG` (`H3` / `Q1`)
- **Entry config lookup precedence (`tuse_config::find`):**
  1. Explicit `-c PATH` / `--config PATH` (relative to `cwd`, `~/` expanded; missing/unreadable fails).
  2. `./tuse.lua` in the session folder (when it is a file).
  3. `$TUSE_GLOBAL_CONFIG` (when set and non-empty after trimming whitespace).
  4. `~/.config/tuse.lua` (when it is a file).
  5. `~/.config/tuse/init.lua` (when it is a file).
  6. Built-in defaults (`config_path` unset / `""`).
- **Path & directory resolution:** Resolve `$TUSE_GLOBAL_CONFIG` via `resolve_path(trimmed, cwd)` (expanding `~/` and resolving relative paths against `cwd`). If the resolved path is a directory, target `<resolved>/init.lua`; otherwise target `<resolved>` directly.
- **Empty & error semantics:**
  - Unset, empty (`""`), or whitespace-only (`"   "`) `TUSE_GLOBAL_CONFIG` is treated as unset and falls through to `~/.config/tuse.lua` / `~/.config/tuse/init.lua`.
  - When set and non-empty, if the target file is missing or unreadable, loading **fails fast** with an error naming `TUSE_GLOBAL_CONFIG` and the resolved path (never silently falling back to `~/.config/tuse.lua`).
- **`require("global")` behavior:** When `TUSE_GLOBAL_CONFIG` is set and non-empty, `module_candidates("global", base)` returns ONLY the `$TUSE_GLOBAL_CONFIG` target file (so `./tuse.lua` inherits the active distro, and a broken override fails cleanly). When unset/empty, `require("global")` searches `~/.config/tuse.lua` then `~/.config/tuse/init.lua`.
- **Docs:** Document in `docs/config.md`, `docs/usage.md`, and `tuse --help` (`crates/cli/src/main.rs:30-31`). (`README.md:72` already references `TUSE_GLOBAL_CONFIG` in the user's unstaged edit and stays untouched in this proposal.)

### 2. Model-Scoped Effort Availability, No-Reasoning Models, Verbatim Wire Encoding, & `llm()` (`H5` / `Q2` / `P2` / `P3`, `M7.2` / `Q7`)
- **Shared default effort list:** `DEFAULT_EFFORTS = ["low", "medium", "high", "xhigh", "max"]` for all models and providers. Legacy `"default"` is completely retired (rejected anywhere an effort name is validated).
- **Tri-state `config.models[].efforts`:**
  - **Omitted (`None`):** Uses `DEFAULT_EFFORTS` (`["low", "medium", "high", "xhigh", "max"]`).
  - **Explicitly empty (`efforts = {}` -> `Some(vec![])`):** Declares a non-reasoning model; **no reasoning parameter is sent on the wire** (`Request.effort = None`, session/saved `effort = ""`). MUST NOT be conflated with omitted `efforts`.
  - **Explicitly non-empty (`efforts = { ... }` -> `Some(vec![...])`):** Dense list of unique names from `DEFAULT_EFFORTS`; serves as the sole source of truth for that model. Unknown or duplicate entries (including `"default"`) fail validation with path `models[i].efforts[j]`.
  - Serialization: `config.get.models[]` omits `efforts` when `None` and serializes `"efforts": []` when explicitly empty (preserving the omitted vs empty distinction); `session.state.model_info.efforts` / `session().model_info.efforts` exposes the resolved effective list (`DEFAULT_EFFORTS`, `[]`, or declared list). `/config` and `tuse models` display declared `efforts` (showing `efforts=none` for `efforts = {}`).
- **Fallback effort rule (`P2`):**
  - Given resolved efforts `L = config.model_efforts(model)`:
    - If `L.is_empty()` (`efforts = {}`): no fallback effort (`""`, no reasoning parameter sent).
    - Else if `L.contains("medium")`: `"medium"`.
    - Else: `L[0]` (first declared effort in `L`).
  - Applied when `Prefs::effort()` is unset or holds a legacy `"default"` / disallowed effort from a preserved v3 `state` table or config reload, and when `model.set` omits `effort` for a non-empty model. When `model == ""` (cleared), omitted `effort` defaults to `"medium"`.
- **Single unified resolver across ALL entrypoints:**
  1. **TUI `/models`:** When the picked model has `efforts = {}`, skips the effort picker and selects the model immediately with `effort = ""` (status bar omits the effort segment when empty). Otherwise opens the effort picker listing only `L` (no `"unsupported"` items), highlighting the current effort if in `L` else the model's fallback effort.
  2. **JSON-RPC `model.set`:** For `model == ""`, clears the model (retaining `docs/json.md:79`). For non-empty `model`, if `efforts = {}`, accepts omitted or `""` effort (setting `effort = ""`) and rejects any non-empty effort with `-32000`; if `L` is non-empty, resolves omitted effort to the fallback (`"medium"` else `L[0]`) and rejects any effort not in `L` (including `"default"`) with `-32000`.
  3. **Lua `set_session{ model, effort }`:** Validates resulting `(model, effort)` atomically against `L`: for `efforts = {}`, accepts omitted/`""` effort (setting `effort = ""` when switching onto that model) and rejects any non-empty effort; for non-empty `L`, rejects any effort not in `L` (including `"default"`).
  4. **Lua `llm(prompt, model, effort)` (`Q7` + `P2`):**
     - Omitted/empty `model` (or `model == cur_model`) with omitted/empty `effort`: inherits session's current effort (validated against `L`: `""` valid iff `L` is empty).
     - Explicit `other_model != cur_model` with omitted/empty `effort`:
       - If `other_model` has `efforts = {}` (`L.is_empty()`): honors no-reasoning (`Request.effort = None`, no reasoning parameter sent).
       - Else if `L` includes `"low"`: uses `"low"` (not `"default"`, not inherited).
       - Else (`L` is non-empty and excludes `"low"`): raises an error naming `other_model` and its allowed efforts.
     - Explicit non-empty `effort`: validated against `L` (raises if `L` is empty or excludes `effort`).
  5. **Saved selections & turn dispatch:** Validate against `config.model_efforts(model)` instead of `ApiType::supported_efforts()`.
- **Verbatim wire encoding (`P3`):**
  - Remove `ApiType::supported_efforts()`, `ApiType::supports_effort()`, and the pre-dispatch check in `crates/api/src/client.rs:151-160`.
  - When `Request.effort` is `None` (`efforts = {}`), NO adapter sends reasoning/thinking parameters.
  - When `Request.effort` is `Some(e)`:
    - `openai-chat`: `"reasoning_effort": e` verbatim.
    - `openai-responses`: `"reasoning": {"effort": e, "summary": "auto"}` + `"include": ["reasoning.encrypted_content"]` verbatim.
    - `anthropic-messages`: `"thinking": {"type": "adaptive"}` + `"output_config": {"effort": e}` verbatim (when `!missing_thinking`).
    - `google-gemini` (non-2.5 models): `"thinkingConfig": {"includeThoughts": true, "thinkingLevel": e}` verbatim.
    - `google-gemini` (`gemini-2.5-*`, numeric `thinkingBudget` — no string effort field): encodes `"low" => 1024`, `"medium" => 8192`, `"high" => 24576`, and returns an explicit wire-encoding error for any other effort (`xhigh`, `max`) rather than inventing budgets or silently clamping to `24576`.
  - Upstream provider HTTP errors rejecting an effort level surface directly as normal provider errors; docs describe exact per-adapter wire encoding and upstream error handling without claiming unverified upstream support.

### 3. Mid-Draft Slash Commands, Open-Picker Execution, & Space Disengagement (`H6`, `L4` / `Q3` / `P4`)
- **No column-0-only restriction:** Both built-in and custom commands (including `CommandArg` pickers) work anywhere in a draft (start, middle, or end; single-line or multiline). Update `Editor::command_before_token()` (`crates/tui/src/kit/editor/mod.rs:289-297`) to inspect the token preceding the cursor at any buffer offset rather than requiring `self.buf[0] == '/'`.
- **Picker suppression for paths:** When the active `/...` token contains a second `/` (e.g., `/usr/bin`, `/etc/hosts`), `update_token_picker()` MUST NOT open (and closes) `Kind::Command` so path typing never flashes a command picker or hijacks `Enter`.
- **Exact `Enter` / picker / space state machine (`P4`):**
  1. **Open `Kind::Command` picker with a valid selected item (standalone or mid-draft):**
     - `Enter` (or `Tab`) accepts the selected command immediately:
       - Custom command with `args` -> replaces only the `/...` token with `/<name> `, opens `Kind::CommandArg(<name>)`, and preserves all surrounding draft text.
       - Zero-arg built-in or custom command without `args` -> removes only the `/...` token span, preserves all surrounding draft text (unless the command itself switches sessions, such as `/new`, `/resume`, or `/fork`), and **immediately executes** the command.
  2. **Open `Kind::CommandArg(<name>)` picker (standalone or mid-draft):**
     - `Enter` (or `Tab`) with a valid selected arg -> removes only the `/<name> <arg>` span, preserves all surrounding draft text, and runs `<name>(arg)`.
     - `Enter` with no matching arg (e.g., `fix /deploy nope` or `/deploy nope`) -> shows the validation error (`Unknown arg nope for /deploy`), does NOT run, does NOT send to the model, and **preserves the full draft** (updating `modal.rs:382`).
     - `Esc` closes `Kind::CommandArg` and leaves `/<name> ` in place without discarding surrounding draft text.
  3. **Typing a space after a zero-argument command (e.g., `/new `, `/models `, `/diff `):**
     - Typing ` ` advances past the command token. Because the command has no `args` picker, `Kind::Command` **disengages** (closes) and no arg picker opens.
     - Ordinary `Enter` MUST NOT trim whitespace into an executable command: `"/new "` (or `"/diff "`) MUST NOT execute `/new` (or `/diff`).
  4. **Ordinary `Enter` with NO open command picker (or empty `Kind::Command` picker falling through on first Enter):**
     - Check raw untrimmed `text = self.editor.text()`:
       - **Mid-draft prose & paths** (`!text.starts_with('/')`, e.g., `"explain /new works"`, `" /new"`, `"check /usr/bin/env and /etc/hosts"`, or `text.starts_with('/')` where the first whitespace-delimited token contains a second `/`, e.g., `"/usr/bin/env"`, `"/etc/hosts is broken"`): sent immediately as normal user message prose; no command executes and no unknown-command error fires.
       - **Standalone built-in command at column 0 (`/<builtin>`):**
         - If raw `text` has trailing whitespace or trailing words (`"/new "`, `"/new foo"`, `"/models bar"`): MUST NOT execute the built-in and MUST NOT send to the model — shows `/<name> takes no words` and **preserves the draft**.
         - Only exact raw `text == "/<builtin>"` (no leading or trailing whitespace, e.g. pasted or after dismissing picker with Esc) executes `/<builtin>`.
       - **Standalone custom command at column 0 (`/<custom>`):**
         - Zero-arg custom command (`args.is_empty()` and `params == Some(0)`): follows the exact same space rule — raw `"/diff "` or `"/diff x"` MUST NOT execute `/diff` or send to the model (shows `/diff takes no words`, preserves draft); exact `"/diff"` runs `diff()`.
         - Custom command with `args`: `"/deploy "` (after Esc in arg picker) runs `deploy(nil)` (preserving baseline scenario `Esc then Enter`); `"/deploy prod"` runs `deploy("prod")`; `"/deploy nope"` or `"/deploy a b"` shows error, preserves draft, and does not send.
         - Custom command with positional words (`"/mv a.txt b.txt"`): validates via `command_words` + `Command::check` before clearing the editor; valid words execute and clear; invalid words or unclosed quotes show error, preserve draft, and do not send.
       - **Standalone unknown single-segment command at column 0** (starts with `/` followed by a non-empty token containing no second `/` that matches neither built-in nor custom commands, e.g., `"/trace"`): fails immediately on the first `Enter` with `Unknown command /<token>`, preserves the draft, and does NOT send to the model.

### 4. Clean Breaking `protocolVersion: 2`, Unified Naming, & State/History Split (`M1` / `Q4`, `L3` / `Q8` / `P1`)
No compatibility bridges, dual keys, or method aliases.

- **Protocol version:** `PROTOCOL = 2` in `crates/rpc/src/methods.rs`. `initialize` defaults omitted `protocolVersion` to `2` and rejects `1` or any other version with `-32602`.
- **Canonical `snake_case` naming across Lua, storage, config, and JSON-RPC v2:**
  - `initialize` retains standard JSON-RPC handshake parameter/field `protocolVersion` (`2`).
  - All Tuse domain fields use one consistent `snake_case` schema:

| Concept | Canonical v2 Name | Retired Name(s) |
| --- | --- | --- |
| Session title | `title` | `name` (`session().name`, `session.snapshot.name`, `/session` `Name`) |
| Config file path | `config_path` | `configpath` |
| Current context tokens | `context` | `ctx` |
| Max context window | `context_window` | `ctx_size` |
| Cache-read tokens | `cache_read` | `cached` (`Record::Usage`, `session.stats`, `sessions.usage_cached`) |
| Cache-write tokens | `cache_write` | `cacheWrite` |
| Helper `llm()` cost | `llm_cost` | `llmCost` |
| Helper `llm()` requests | `llm_requests` | `llmRequests` |
| Elapsed turn milliseconds | `elapsed_ms` | `elapsedMs` (`session.info.turn`, `turn_end` event) |
| Queued input identifier | `input_id` | `inputId` (`turn.submit`, `turn.withdraw`, `user` event) |
| Provider entry (config/CLI JSON) | `{ name, type, base }` | `{ name, base_url, api }` |

- **Unified `SessionState` struct (shared by JSON-RPC `session.state` and Lua `session()`):**
  - Fields: `mode`, `config_path?`, `cwd`, `home`, `id?`, `started?`, `created?`, `title?`, `model?`, `provider?`, `effort?`, `busy?`, `turn?` (`{ elapsed_ms, status }`), `context?`, `context_window?`, `usage?` (`{ input, output, cache_read, cache_write, cost?, llm_cost? }`), `model_info?` (`{ name, display?, window?, efforts, prices? }`), `requests?`, `llm_requests?`, `steps?`, `turns?`, `calls?`.
  - Absent optional fields are omitted consistently in both JSON and Lua (`skip_serializing_if = "Option::is_none"`).
  - **JSON-RPC `session.state`** returns `SessionState` (lightweight — never serializes `history`).
  - **JSON-RPC `session.history`** returns the full structured record array (same fields as storage + display-only `summary`).
  - **JSON-RPC `session.list`** `{ cwd? }` is retained, returning started sessions in `cwd` newest first: `[{ id, cwd, title, model, effort, started, created, updated }]`.
  - **Lua `session()`** returns `SessionState` **plus** `history` (pre-session during config load: `{ mode, config_path?, cwd, home }`).
- **Retired JSON-RPC methods (all return `-32601` in `protocolVersion: 2`):**
  - `session.info`, `session.stats`, `session.snapshot`, `session.resume`, `model.get` (plus already-removed `session.set`, `config.set`, `session.trace`, `session.records`).
- **Client resume workflow (`P1`):**
  - JSON-RPC clients list started sessions via `session.list { cwd? }` and resume a chosen session by launching `tuse json -r <id>` (CLI `-r` opens the session and adopts its saved folder if it exists), or by triggering Lua `resume(id)` via a configured command/hook (which emits `session_switched`).
- **v2 handshake & switch payloads:**
  - `initialize` -> `{ protocolVersion: 2, tuse, session: <SessionState>, capabilities: { methods } }`.
  - `session.new` -> `{ session: <SessionState> }`.
  - `session.fork` -> `{ session: <SessionState>, draft }`.
  - `session_switched` event (from Lua `fork`/`resume`) -> `{ type: "session_switched", session: <SessionState>, draft }`.
  - `usage` event tells clients to refresh `session.state`.
- **Complete JSON-RPC v2 `METHODS` table (22 methods):**
  `initialize`, `turn.submit`, `turn.withdraw`, `turn.cancel`, `session.state`, `session.history`, `session.list`, `session.new`, `session.fork`, `model.set`, `model.list`, `config.get`, `config.reload`, `tasks.list`, `tasks.get`, `tasks.stop`, `commands.list`, `commands.run`, `ui.state`, `ui.reply`, `ui.cancel`, `shutdown`.

### 5. SQLite Schema v4 with Flat Usage Rows (`M2` / `Q5`)
- **Schema version:** `PRAGMA user_version = 4`.
- **Storage format:**
  - `Record::Usage` uses `#[serde(flatten)]` on `usage: Usage` (with `Usage.cache_read` replacing `Usage.cached`), storing flat JSON `{"role":"usage","model":...,"input":...,"output":...,"cache_read":...,"cache_write":...,"llm":...}` in `history.body`.
  - Delete both flatten/unflatten blocks in `crates/core/src/session/view/history.rs:154-158, 262-268`.
  - Rename `sessions.usage_cached` -> `sessions.usage_cache_read`.
  - **No migration and no dual-shape reader:** legacy nested `{"usage":{...}}` or `cached` rows are rejected by `deny_unknown_fields`.
- **Reset & failure semantics:**
  - Executed inside a single `TransactionBehavior::Immediate` transaction on DB open.
  - `user_version == 0`: drops legacy tables and creates the v4 schema, setting `user_version = 4`.
  - `1 <= user_version < 4`: preserves `providers` (adding `headers` column if `user_version == 1`) and `state`, drops and recreates `sessions` and `history` with the v4 schema, and sets `user_version = 4`. (Any legacy `"default"` effort remaining in preserved `state` safely falls back per `P2`.)
  - `user_version > 4`: MUST fail opening with a clear incompatible-schema-version error without modifying any table.
  - Any SQLite error inside the `IMMEDIATE` transaction rolls back atomically leaving the database at its prior version and fails startup cleanly.

### 6. Active Sanitized Providers & Raw Theme Tokens (`M3` / `Q6`)
- **`config.get`:**
  - Includes `"providers": [{ "name": "...", "type": "...", "base": "..." }]` for **all active providers** (saved SQLite + Lua config-only, from `config.connect`). Strictly ONLY `name`, `type`, and `base` — never `api_key`, `key`, `headers` (names or values), `source`, or extra metadata.
  - Serializes `ui.theme` by token key preserving raw token links (`"user": "fg"`, `"input": "user"`, `"icons": "accent"`, etc.) rather than resolving links away.
- **`/config` TUI panel:**
  - Adds a `Providers` section showing all active providers (`name`, `type`, `base`) in aligned columns, with no keys, headers, or extra metadata.
  - Displays declared `efforts` under `Models` (showing `efforts=none` for explicitly empty `efforts = {}`).
  - Continues rendering each `ui.theme` token with its resolved color swatch `■` beside its raw value/link.
- **`tuse providers --json`:** Aligned to emit `[{ "name", "type", "base" }]` for saved providers only (`tuse providers` and `tuse disconnect` stay saved-only per `providers/spec.md:16`).

### 7. Lua/RPC Task Parity & Core/TUI/CLI Settled Fixes (`H1`, `H2`, `H4`, `M4`–`M10`, `L1`, `L2`, `L5`, `L6.3`/`Q10`, `L6.4`)
- **Task `log` parity (`Q10` / `L6.3`):** Add `r.set("log", t.log.as_str())?` in `crates/core/src/session/lua/host.rs:300-310` so Lua `task_list`/`task_get`/`task_stop` return `{ id, name, status, started, ended, log, code }` (+ `output` on `task_get`), matching JSON-RPC `tasks.*`.
- **Task completion notices (`H1`):** Split `last_line(&str) -> Option<&str>` from `error_line(&str) -> String` in `crates/core/src/text.rs`; omit `Last line:` for succeeded tasks with empty return text; keep `"failed"` fallback only for failed tasks.
- **Config validation & reload (`H2`, `L2`, `L5`):** Path-aware Lua-to-JSON normalizer rejecting stray functions/userdata/threads everywhere (preserving `providers.*` masking and removing the `plugins` hack); `de::list` + dense-list check on `Prompt.args` and `Command.args`; element-level `HistoryFilter` deserializer eliminating duplicated `history.hide:` prefixes; track/restore trusted `package.preload` modules in `require.rs::forget`/`restore`.
- **Session state & `sh` cwd (`M7.1`, `M8`, `L6.4`):** Expand `~/` in `sh(..., { cwd })`; skip `session_updated` on empty `changed`; preserve `Record::Meta` written during `tool_step` with deferred `pending_history` (combined or sequential); validate `set_session{ history }` atomically against any pending `Switch::Fork(index)`; set `started = 1` when replacement history contains `Record::User`; delete dead `cancel_turn(handoff)`/`cancel_pending` plumbing and return `Option<Value>` in `events::to_json`.
- **TUI keys & theme (`M4`, `L1`):** `PageUp`/`PageDown` (without `Shift`) pages any open picker via `self.picker_rows()` with clamping (`0..n-1`); `Cmd+X` (`SUPER`) stops tasks in `/tasks` alongside `Ctrl+X`; `tasks_idle()` styles spinner with `icons` and task count with `accent`.
- **JSON-RPC validation & paths (`M5`):** Shared `is_valid_image_data_url` in `turn.submit` (`-32000` on invalid `data:` URLs); `tuse_config::resolve_path` in `convo/input.rs`; typed optional helpers (`opt_u64`, `opt_str`) in `proto.rs` returning `-32602` on non-null wrong types for `tasks.get.lines` (clamping `0` -> `1`) and `session.list.cwd`.
- **CLI & provider hint (`M6`):** Reject inline `=value` on plain flags; allow `-`-prefixed values on valued flags unless matching `-h`/`--help`/known flags; validate `tuse help [COMMAND]` and `tuse version`; suggest `--replace` in `connect.rs:43`.
- **Prompt, examples, docs (`H4`, `M9`, `M10`):** Fix `prompt.md` + verbatim `docs/lua.md`; fix `tmux`/`zellij`/`rex`/`orca` single-call title+status and `session().title`; update `compaction.lua` to `context`/`context_window` and remove `r.role == "tool"`; fix doc drift and add negative-coverage contract tests for removed identifiers.

## Baseline Spec Hygiene & Archive Merge Notes

Baseline files under `openspec/specs/` remain untouched during this proposal. When archiving:
1. Remove the corrupted `openspec/specs/config/spec.md:97-100` merge fragment via `## REMOVED Requirements` (`### Requirement: Trace filter setting\`` + `- TO: \`` + duplicate `### Requirement: History filter setting\``), preserving canonical `### Requirement: History filter setting` at lines 41–58.
2. Delete the stray `1` line under `## Requirements` (line 8) in `config`, `session`, `tui`, `docs`, and `examples`.
3. Apply `MODIFIED` updates replacing `tuse --json` (`session/spec.md:140`), `ctx`/`ctx_size` (`session/spec.md:166`), and `Both input styles`, `list_max_height`, `input_max_height`, `10  hook`, `pad_x` (`tui/spec.md:141, 238, 249, 306, 374`).

---

## Open Design Questions

**None.** All design questions (`Q1`–`Q10` and `P1`–`P4`) are resolved and encoded normatively in the delta specs and implementation tasks:

| ID | Topic | Final Resolution |
| --- | --- | --- |
| `Q1` (`H3`) | `TUSE_GLOBAL_CONFIG` | Restored with `-c > ./tuse.lua > $TUSE_GLOBAL_CONFIG > ~/.config/tuse.lua > ~/.config/tuse/init.lua`, `require("global")` override, empty=unset, directory->`init.lua`, fail-fast on missing path. |
| `Q2` / `P2` / `P3` (`H5`) | Effort availability, fallback, wire encoding | Shared default `low..max`; tri-state `config.models[].efforts` (`None` = default 5, `Some([])` = no reasoning params sent, `Some([...])` = allowlist); `"medium"` else `L[0]` fallback (none for empty); retired `"default"`; verbatim string wire pass-through with no clamping/invented `gemini-2.5-*` budgets; `model: ""` clearing kept. |
| `Q3` / `P4` (`H6`, `L4`) | Mid-draft commands, picker, & space behavior | Commands usable anywhere in draft; open picker `Enter` executes or transitions to arg picker preserving surrounding text; space after zero-arg command disengages picker & never trims into executable command; prose/paths preserved; standalone `/trace`, `/new `, `/new foo`, and invalid args never execute or send and preserve draft. |
| `Q4` (`M1`) | Schema & JSON-RPC v2 | Breaking `protocolVersion: 2`, unified `snake_case`, split `session.state` + `session.history`, keep `session.list`, retire `session.info`/`session.stats`/`session.snapshot`/`model.get`. |
| `Q5` (`M2`) | Flat `Record::Usage` storage | SQLite schema v4, `#[serde(flatten)]` + `cache_read`, no migration/dual-shape reader, atomic `IMMEDIATE` reset preserving `providers` + `state`, reject `user_version > 4`. |
| `Q6` (`M3`) | Providers & theme in `/config` / `config.get` | All active providers as `[{ name, type, base }]` (no secrets/headers/extra metadata); aligned `tuse providers --json`; raw `ui.theme` links in `config.get` + swatches in `/config`. |
| `Q7` (`M7.2`) | `llm(prompt, other_model)` omitted effort | Honors `efforts = {}` (sends no reasoning); otherwise `"low"` (raises if non-empty allowlist excludes `"low"`). |
| `Q8` / `P1` (`L3`) | JSON-RPC `session.resume` | Removed (`-32601`) in `protocolVersion: 2`; `session.list` retained; documented parity exception (resume via `tuse json -r <id>` or Lua `resume(id)`). |
| `Q9` (`L6.1`) | Composer styles | Keep all four (`boxed`, `lines`, `fill`, `bar`). |
| `Q10` (`L6.3`) | Lua task `log` parity | Add `log` to Lua `task_list`, `task_get`, `task_stop` tables. |

---

## Audit Traceability Matrix

Every finding ID and sub-point from the v0.13.1 audit is mapped below.

| ID | Summary | Verification | Disposition | Delta Spec(s) | Task(s) | Resolution Ref |
| --- | --- | --- | --- | --- | --- | --- |
| `H1` | Succeeded background task with empty return reports `Last line: failed.` | Confirmed (`host.rs:93-109`, `text.rs:11-18`, `job.rs:284-305`) | Resolved | `specs/session/spec.md` (`Task notices wake the agent`) | `1.1` | Settled |
| `H2` (`H2.1`) | Config validation strips unknown function/userdata/thread keys (`deny_unsupported_types(false)` + `plugins` hack) | Confirmed (`validate.rs:5-11, 265-269`) | Resolved | `specs/config/spec.md` (`Config layout`) | `1.2` | Settled (preserve `providers.*` masking) |
| `H2` (`H2.2`) | `Prompt.args` / `Command.args` reject empty `{}` and miss non-dense keys | Confirmed (`types.rs:79-88`, `validate.rs:83-114`) | Resolved | `specs/config/spec.md` (`Prompts with args`, `Custom commands with args and description`) | `1.3` | Settled |
| `H3` | `README.md:72` advertises removed `TUSE_GLOBAL_CONFIG` env var | Confirmed (`README.md:72` vs `modules.rs:32-39`, `CHANGELOG.md:177`) | Resolved | `specs/config/spec.md` (`Global config override via TUSE_GLOBAL_CONFIG`), `specs/docs/spec.md` | `1.4`, `4.3` | `Q1` closed (`README.md` untouched in proposal) |
| `H4` | `prompt.md` & `docs/lua.md` contradict cancel-no-handoff, omit `task_get.code`, typo `in order,.` | Confirmed (`prompt.md:10, 73, 75`, `docs/lua.md:129, 157, 159`) | Resolved | `specs/docs/spec.md` (`System prompt matches runtime semantics`) | `4.1` | Settled (+ synced with `Q4`, `Q7`, `Q10`, `P2`) |
| `H5` | `/models` and `model.set` accept & persist unsupported efforts | Confirmed (`models.rs:104-132`, `methods.rs:117-127`, `commands.rs:214-235`, `client.rs:151-160`, `gemini.rs:112-124`) | Resolved | `specs/config/spec.md` (`Model effort availability`, `Configured models limit the model list`), `specs/session/spec.md`, `specs/tui/spec.md`, `specs/providers/spec.md` | `1.5`, `1.6`, `2.1` | `Q2`, `P2`, `P3` closed |
| `H6` (`H6.1`) | Unknown slash command (e.g. `/trace`) falls through and sends to LLM | Confirmed (`send.rs:70-87`, `commands.rs:77` vs `tui/spec.md:61-63`) | Resolved | `specs/tui/spec.md` (`History command`, `Command descriptions and words`) | `2.2` | `Q3`, `P4` closed |
| `H6` (`H6.2`) | Built-in slash command with trailing text (e.g. `/new foo`) sends to LLM | Confirmed (`send.rs:66, 72`) | Resolved | `specs/tui/spec.md` (`Command descriptions and words`) | `2.2` | `Q3`, `P4` closed (reconciled with `/new ` space disengagement) |
| `H6` (`H6.3`) | Custom command arg validation clears editor before checking | Confirmed (`send.rs:80`, `commands.rs:183`, `modal.rs:382`) | Resolved | `specs/tui/spec.md` (`Command descriptions and words`) | `2.2` | `Q3`, `P4` closed (updates `modal.rs:382`) |
| `M1` | Cross-surface naming drift (`name`/`title`, `cached`/`cache_read`, `ctx`/`context`, casing) & 4 overlapping RPC getters | Confirmed (`snapshot.rs:23`, `methods.rs`, `record.rs`) | Resolved | `specs/session/spec.md`, `specs/tui/spec.md`, `specs/docs/spec.md`, `specs/examples/spec.md` | `1.11`, `2.5`, `4.1`–`4.4` | `Q4` closed (`protocolVersion: 2`, no bridges/aliases) |
| `M2` | `Record::Usage` nests `usage` in SQLite storage vs flat `session().history` | Confirmed (`record.rs:64-69`, `history.rs:154-158, 262-268`) | Resolved | `specs/session/spec.md` (`One name per record kind`, `History reset`) | `1.12` | `Q5` closed (schema v4, no migration/dual-shape reader) |
| `M3` | `providers` omitted from `/config` & `config.get`; `ui.theme` raw links vs resolved tokens | Confirmed (`types.rs:9-12`, `db.rs:198-216`, `config_view.rs:46-203`) | Resolved | `specs/config/spec.md` (`Config view is complete`, `Theme tokens`), `specs/cli/spec.md` (`Providers JSON matches config provider fields`) | `1.7`, `2.6`, `3.4` | `Q6` closed (`{ name, type, base }`; raw links in `config.get`) |
| `M4` (`M4.1`–`M4.2`) | `PageUp`/`PageDown` ignores untitled & modal pickers and wraps instead of clamping | Confirmed (`keys.rs:40-48`, `picker.rs:158-166`) | Resolved | `specs/tui/spec.md` (`Lists open below the input`) | `2.3` | Settled |
| `M4` (`M4.3`) | `Cmd+X` does not stop task in `/tasks` | Confirmed (`keys.rs:213` vs `docs/usage.md:101`) | Resolved | `specs/tui/spec.md` (`Task stop shortcut supports primary modifier`) | `2.3` | Settled |
| `M5` (`M5.1`–`M5.3`) | `turn.submit` unvalidated `data:` URLs & missing `~/` expansion; `tasks.get.lines` & `session.list.cwd` wrong types ignored | Confirmed (`methods.rs:81-92, 151-155, 195`, `convo/input.rs:46, 60, 70`) | Resolved | `specs/tui/spec.md` (`JSON-RPC v2 protocol, validation, and path expansion`) | `2.5` | Settled (inline `<image>` context tag intentionally omitted per `wire.rs`) |
| `M6` (`M6.1`–`M6.5`) | CLI `--flag=false`, `-`-prefixed values, `help`/`version` arg validation, `connect` missing `--replace` hint | Confirmed (`args.rs:23-36`, `main.rs:72, 91-120`, `connect.rs:43`) | Resolved | `specs/cli/spec.md`, `specs/providers/spec.md` | `3.1`–`3.3` | Settled (known-flag guard refinement for `M6.2`) |
| `M7` (`M7.1`) | `sh(..., { cwd = "~/..." })` does not expand `~/` | Confirmed (`crates/lua/src/api.rs:112-114`) | Resolved | `specs/session/spec.md` (`Shell working directory resolves paths consistently`) | `1.9` | Settled |
| `M7` (`M7.2`) | `llm(prompt, other_model)` inherits incompatible session effort | Confirmed (`host.rs:434-447`) | Resolved | `specs/session/spec.md` (`Helper llm calls use low effort when switching models`), `specs/docs/spec.md` | `1.10`, `4.1`, `4.3` | `Q7`, `P2` closed (`efforts = {}` sends no reasoning; else `low`) |
| `M8` (`M8.1`–`M8.4`) | `set_session` empty `session_updated`, erased `Record::Meta` in `tool_step`, stale `pending_switch`, missing `started = 1` | Confirmed (`meta.rs:30-37, 109-154`, `switch.rs:7-27`, `store.rs:136-178`) | Resolved | `specs/session/spec.md` (`Hook events`, `History replacement waits for tool calls`, `Session read and write from Lua`, `Started sessions`) | `1.13` | Settled (combined & sequential `tool_step` calls) |
| `M9` (`M9.1`–`M9.2`) | `tmux`/`zellij`/`rex`/`orca` overwrite title; `compaction.lua` checks removed `"tool"` | Confirmed (`examples/{tmux,zellij,rex,orca,compaction}.lua`) | Resolved | `specs/examples/spec.md` (`Examples folder`) | `4.2` | Settled (+ `Q4` `session().title`, `context`/`context_window`) |
| `M10` | Docs contradictions & baseline spec drift/corruption | Confirmed + 3 bonus doc catches (`connect.md:53`, `config.md:129`, `session.md:11`) | Resolved + archive cleanup | `specs/docs/spec.md`, `specs/config/spec.md`, `specs/session/spec.md`, `specs/tui/spec.md` | `4.3`, `4.4`, `5.3` | Settled; baseline files cleaned only on archive |
| `L1` | Idle task indicator uses `icons` instead of `accent` for task count | Confirmed (`status.rs:66-73, 139`) | Resolved | `specs/tui/spec.md` (`Idle status shows the project directory`) | `2.4` | Settled (spinner `icons`, count text `accent`) |
| `L2` | Duplicated `history.hide:` prefix in validation error | Confirmed (`names.rs:178-181`, `load.rs:51`) | Resolved | `specs/config/spec.md` (`History filter setting`) | `1.8` | Settled |
| `L3` | Diverging folder/`started` rules across Lua `resume(id)`, RPC `session.resume`, CLI `-r` | Confirmed (`launch.rs:26-44, 71-83, 119-126`, `rpc/src/lib.rs:151-154`) | Resolved | `specs/tui/spec.md` (`TUI and JSON-RPC parity`), `specs/docs/spec.md` | `2.5`, `4.3` | `Q8` / `P1` closed: remove RPC `session.resume`, retain `session.list`, document CLI `-r` & Lua `resume(id)` |
| `L4` | `/` command picker activates mid-sentence while `command_before_token` requires col 0 | Confirmed (`token_picker.rs:55`, `keys.rs:413-422`, `editor/mod.rs:291`) | Resolved | `specs/tui/spec.md` (`Command descriptions and words`) | `2.2` | `Q3`, `P4` closed |
| `L5` | File `require` caches by canonical path (intentional); `package.preload` survives `/reload` | Confirmed (`require.rs:363-391, 477-490`) | Resolved | `specs/config/spec.md` (`Reload clears preloaded config modules`) | `1.8` | Settled (`preload` cleared/restored; file cache retained) |
| `L6.1` | 4 composer styles (`boxed`, `lines`, `fill`, `bar`) | Confirmed (`compose.rs`, `input_view.rs`); not a bug | Retained | None (`M10` updates `"Both input styles"` -> `"Every input style"`) | None | `Q9` closed — retain all 4 |
| `L6.2` | 33 theme tokens (`ui.theme`) | Confirmed (`theme.rs:33-92`); not a bug | Retained | None | None | Optional tokens with linked defaults; removing breaks themes |
| `L6.3` | Task `log` field present in JSON-RPC `tasks.*`, omitted in Lua `task_*` | Confirmed (`host.rs:300-310` vs `tasks.rs:9-18`) | Resolved | `specs/session/spec.md` (`Lua task tables match JSON-RPC task fields`), `specs/tui/spec.md` (`Task details`), `specs/docs/spec.md` | `1.14`, `4.1` | `Q10` closed (add `log` to Lua task tables) |
| `L6.4` | Dead `cancel_turn(handoff)` / `cancel_pending` plumbing & `events::to_json(Event::Switch)` | Confirmed (`control.rs:55-70`, `tool.rs:162-165`); nuance on `events.rs:59` | Resolved (internal cleanup) | None (internal plumbing) | `1.15` | Settled; return `Option<Value>` in `events::to_json` |
