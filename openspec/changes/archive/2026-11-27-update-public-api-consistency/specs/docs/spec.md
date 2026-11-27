## ADDED Requirements

### Requirement: System prompt matches runtime semantics
The system prompt template in `crates/core/src/prompt.md` and its verbatim mirror at the end of `docs/lua.md` SHALL accurately describe runtime semantics: only steering (never turn cancellation) can hand a running `lua` call off to a background task; `task_get(id, lines=50)` and `task_list()` SHALL list `code` and `log` among returned fields; `session()` SHALL list the unified v2 `snake_case` fields (`title`, `config_path`, `context`, `context_window`, `busy`, `turn`, `requests`, `llm_requests`, `calls`, etc., not retired `name`/`configpath`/`ctx`/`ctx_size`); `llm(prompt, model, effort)` SHALL state that omitting `effort` when specifying another model sends no reasoning when that model declares `efforts = {}` and otherwise uses `low` (raising if that model's non-empty `efforts` excludes `low`); and prose typos such as `in order,.` SHALL be absent.

#### Scenario: Cancellation is not handoff
- **WHEN** the default system prompt and `docs("lua")` are rendered
- **THEN** `sh(...)` states that steering (not cancelling) can hand off a running Lua call to a background task, matching the `Stop everything` session contract

#### Scenario: Task, session, and llm contracts match runtime
- **WHEN** the default system prompt and `docs("lua")` are rendered
- **THEN** `task_get` lists `code` and `log`, `session()` lists `title`, `config_path`, `context`, and `context_window` (never `name`), `llm` documents the `low`/no-reasoning rule for another model, and `context-intro` does not contain `in order,.`

### Requirement: Docs match implemented contracts and reject stale identifiers
Documentation pages under `docs/` SHALL stay consistent with implemented v2 contracts and MUST NOT reference removed config keys, theme tokens, CLI flags, Lua fields, or retired JSON-RPC methods (`settings.open`, `` `surface` ``, `tuse --json`, `list_max_height`, `input_max_height`, `pad_x`, `session.snapshot`, `session.info`, `session.stats`, `session.resume`, `model.get`, `session().name`). Specifically:
- `docs/config.md` and `docs/usage.md` SHALL document `TUSE_GLOBAL_CONFIG` lookup precedence (`-c` > `./tuse.lua` > `$TUSE_GLOBAL_CONFIG` > `~/.config/tuse.lua` > `~/.config/tuse/init.lua`), `require("global")` override, empty/directory/missing-path error semantics, tri-state `config.models[].efforts` (omitted vs `efforts = {}` vs explicit list) and `"medium"`/first-declared fallback, `config.get.providers` (`{ name, type, base }`), raw `ui.theme` token links, and the `bar` mark under `icons`.
- `docs/connect.md` MUST NOT claim `/reload` recomputes session cost, SHALL note `/models` scoping when `models` is configured, and SHALL document the shared 5-level default effort list (`low`, `medium`, `high`, `xhigh`, `max`), `config.models[].efforts` (including `efforts = {}`), verbatim wire pass-through, exact `gemini-2.5-*` `thinkingBudget` handling, and upstream errors without claiming unverified upstream provider support.
- `docs/usage.md` SHALL reference `user_bg`, top-level `open`, started sessions for `/resume`, `Cmd+X` alongside `Ctrl+X` in `/tasks`, mid-draft slash commands (including open-picker execution, space after zero-arg commands disengaging the picker, prose/path preservation, and draft preservation on invalid args), and `{ name, type, base }` for `tuse providers --json`.
- `docs/session.md` SHALL document SQLite schema v4 flat `usage` rows (`cache_read`, no dual-shape reader) and atomic v4 reset/forward-version rejection semantics.
- `docs/json.md` SHALL document `protocolVersion: 2`, unified `snake_case` payloads, `session.state`, `session.history`, `session.list`, the client resume workflow via `tuse json -r <id>` (and Lua `resume(id)` emitting `session_switched`) with `session.resume` retired, retired getters, and `config.get.providers`; `docs/lua.md` SHALL document Lua `resume(id)` folder and `started` requirements.

#### Scenario: No stale claims or removed identifiers
- **WHEN** `cargo test` runs the docs contract suite
- **THEN** it fails if any page under `docs/` contains removed identifiers (`settings.open`, `` `surface` ``, `tuse --json`, `list_max_height`, `input_max_height`, `pad_x`, `session.snapshot`, `session.info`, `session.stats`, `session.resume`, `model.get`, `session().name`), and `docs/connect.md` does not claim `/reload` recomputes cost

#### Scenario: Restored env var, model efforts, wire encoding, and v2 JSON-RPC documented
- **WHEN** a reader inspects `docs("config")`, `docs("connect")`, `docs("usage")`, `docs("session")`, and `docs("json")`
- **THEN** `TUSE_GLOBAL_CONFIG`, tri-state `models[].efforts` (`efforts = {}`), verbatim wire pass-through and `gemini-2.5-*` `thinkingBudget` handling, schema v4 flat `usage` (`cache_read`), `protocolVersion: 2`, `session.state`, `session.history`, `session.list`, `tuse json -r <id>` resume workflow, mid-draft slash commands, and `config.get.providers` (`name`, `type`, `base`) are documented consistently with code
