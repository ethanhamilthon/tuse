# docs

## Purpose
Documentation topics shared by the agent (`docs()`) and the site, and the agent context shown in them.

## Requirements

### Requirement: Documentation topics
Tuse SHALL ship exactly eight documentation topics, in this order: `readme`, `usage`, `connect`, `config`, `lua`, `session`, `json`, `examples`. `readme` SHALL be the content of `README.md` and every other topic the content of `docs/<topic>.md`, embedded at build time. The build MUST fail when a listed page is missing or when `docs/` holds any other `.md` file.

#### Scenario: Listing topics
- **WHEN** the agent calls `docs()`
- **THEN** it gets one `- <topic> — <summary>` line per topic in the fixed order, ending with `examples`, followed by the `Call docs("<topic>")` hint

#### Scenario: Reading examples
- **WHEN** the agent calls `docs("examples")`
- **THEN** it gets the content of `docs/examples.md`

#### Scenario: Stray page
- **WHEN** `docs/extra.md` exists or `docs/examples.md` is missing
- **THEN** `cargo build` fails

#### Scenario: Reading a topic
- **WHEN** the agent calls `docs(" Config.md ")`
- **THEN** it gets the content of `docs/config.md`

#### Scenario: Unknown or removed topic
- **WHEN** the agent calls `docs("functions")` or `docs("nope")`
- **THEN** it gets `Unknown docs topic "<name>".` followed by the topic list

#### Scenario: Reading the README
- **WHEN** the agent calls `docs("README.md")` or `docs("readme")`
- **THEN** it gets the content of `README.md`

### Requirement: Topics cover the public surface
The topics SHALL document every public surface: `usage` every CLI subcommand (including `tuse plugins add`, `list`, `delete`, `print`, and `update`) and every built-in slash command, `lua` every host function in `HOST_FUNCTIONS` and every hook event, `config` every `require("tuse")` export, the namespaced config layout, `global` function/options/opt-out semantics, `plugins` selection (`only`, `without`, `opts`), `require("plugin:<name>")` and `require("plugins")`, `tuse-plugin.json`, every namespace contribution key, every `ui` key, every `ui.theme` token, and the naming rules for prompts, commands, and `open` extensions, `json` every JSON-RPC method, and `session` every history role.

#### Scenario: Coverage tests
- **WHEN** `cargo test` runs
- **THEN** it fails if a CLI subcommand (including `tuse plugins`) or slash command is missing from `docs("usage")`, a host function or hook event from `docs("lua")`, a config/namespace key, `ui` key, or `ui.theme` token from `docs("config")`, a JSON-RPC method from `docs("json")`, or a history role from `docs("session")`

#### Scenario: Color aliases
- **WHEN** a color accepted by validation, such as `gray` or `dark_gray`, is looked up in `docs("config")`
- **THEN** it is listed there

### Requirement: Lua docs end with the agent's context
`docs/lua.md` SHALL end with a `## What the agent sees` section that holds, verbatim in `text` code blocks, the system prompt and the `lua` tool description that Tuse produces for the default config. The section MUST also say that disabled functions drop out, that `systems` strings are appended, and that the prompt is the first `system` history record, rebuilt on new, resume, and config load.

#### Scenario: Docs match the code
- **WHEN** the prompt template in the code or the tail of `docs/lua.md` changes so that they no longer match
- **THEN** `cargo test` fails and prints the expected section

#### Scenario: Session-specific rendering
- **WHEN** a session disables `sh` and the agent calls `docs("lua")`
- **THEN** the text starts with `Disabled in this session: sh.` and its `What the agent sees` section contains no `sh(` reference

#### Scenario: No compaction note
- **WHEN** the agent calls `docs("lua")`
- **THEN** the section does not mention compaction

### Requirement: System prompt template lives in code
The system prompt template SHALL be loaded from a file inside `crates/core/src`, not from `docs/`. The template and its placeholder rules SHALL be the source of the rendered prompt.

#### Scenario: Prompt rendered
- **WHEN** a session starts with the default config
- **THEN** its first system history record contains the rendered prompt

### Requirement: System prompt matches runtime semantics
The system prompt template in `crates/core/src/prompt.md` and its verbatim mirror at the end of `docs/lua.md` SHALL accurately describe runtime semantics: only steering (never turn cancellation) can hand a running `lua` call off to a background task; `task_get(id, lines=50)` and `task_list()` SHALL list `code` and `log` among returned fields; `session()` SHALL list the unified v2 `snake_case` fields (`title`, `config_path`, `context`, `context_window`, `busy`, `turn`, `requests`, `llm_requests`, `calls`, etc., not retired `name`/`configpath`/`ctx`/`ctx_size`); `llm(prompt, model, effort)` SHALL state that omitting `effort` when specifying another model sends no reasoning when that model declares `efforts = {}` and otherwise uses `low` (raising if that model's non-empty `efforts` excludes `low`); and prose typos such as `in order,.` SHALL be absent.

#### Scenario: Cancellation is not handoff
- **WHEN** the default system prompt and `docs("lua")` are rendered
- **THEN** `sh(...)` states that steering (not cancelling) can hand off a running Lua call to a background task, matching the `Stop everything` session contract

#### Scenario: Task, session, and llm contracts match runtime
- **WHEN** the default system prompt and `docs("lua")` are rendered
- **THEN** `task_get` lists `code` and `log`, `session()` lists `title`, `config_path`, `context`, and `context_window` (never `name`), `llm` documents the `low`/no-reasoning rule for another model, and `context-intro` does not contain `in order,.`

### Requirement: Prompt points the agent to the docs
When `docs` is enabled, the system prompt SHALL end its rules with three bullets: questions about Tuse are answered from `docs()` and not from memory; a change of agent behavior (prompt, functions, hooks, commands, models) is made by editing the Lua config as `docs("config")` and `docs("examples")` describe, then reloading; a change of the UI (theme, input style, limits) is made with `ui` in that config, with keys and slash commands in `docs("usage")`. When `docs` is disabled, the bullets MUST be absent and the prompt MUST NOT mention `docs`.

#### Scenario: Default config
- **WHEN** a session starts with the default config
- **THEN** the system prompt holds the three bullets and `docs/lua.md` shows them verbatim

#### Scenario: Docs disabled
- **WHEN** a session sets `functions = { docs = false }`
- **THEN** the system prompt has none of the three bullets and no `docs(` reference

### Requirement: Document Lua picker questions
The Lua reference and agent system prompt SHALL describe `pick(options, title)` with an optional Markdown title. The Lua reference SHALL describe how the question and selected answer appear in history. The JSON-RPC reference SHALL document the optional `title` string in active picker requests.

#### Scenario: Find the picker title contract
- **WHEN** an agent or a JSON-RPC client author looks up picker questions
- **THEN** the Lua reference and agent prompt show how to supply a Markdown title, the Lua reference explains history behavior, and the JSON-RPC reference explains when the raw Markdown field appears in `ui_request` and `ui.state.active`

### Requirement: Document session switching from Lua
`docs("lua")` SHALL describe `fork(index)` and `resume(id)`: the 1-based index into `session().history`, the allowed roles, the draft for a `user` record, the wait for running tool calls, and the errors. `docs("json")` SHALL describe the `session_switched` event, and the agent's system prompt SHALL list both functions with the other host functions.

#### Scenario: Coverage
- **WHEN** `cargo test` runs
- **THEN** it fails if `fork` or `resume` is missing from `docs("lua")` or `session_switched` from `docs("json")`

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
