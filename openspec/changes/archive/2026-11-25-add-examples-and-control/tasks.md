# Tasks

## 1. Config
- [x] 1.1 Move `open` to `settings.open`; top-level `open` fails with `moved to settings.open`
- [x] 1.2 `UiSettings.hide_trace` with role validation
- [x] 1.3 `UiSettings.scope_models` with `provider/model` and `provider/*` validation
- [x] 1.4 `prompts` as string or `{args?, run}`; validate in `scripting/validate.rs`
- [x] 1.5 Update `tuse init` template and config tests

## 2. Stop and tasks
- [x] 2.1 `Session::stop_all()`: turn, running call (`[cancelled]`), tasks, hooks, commands, `open`, UI prompts
- [x] 2.2 Esc, `turn.cancel`, and new `/stop` call it; remove the Esc handoff (steering handoff per open question)
- [x] 2.3 Finished tasks queue their notice; the next user message takes them; no auto turn
- [x] 2.4 Capture function source in `spawn` into `TaskInfo.code`; expose in `task_get`, `tasks.list`, `tasks.get`

## 3. Trace
- [x] 3.1 `Record::Job` (task, command, `open`) and `Record::Title`
- [x] 3.2 `trace()` covers every `Record` variant with no `_` arm
- [x] 3.3 `/trace` filters `hide_trace` roles

## 4. TUI
- [x] 4.1 `/tasks` detail panel: meta, live elapsed, code, log tail
- [x] 4.2 `/config`: `Models` section and all new settings; test that fails on unlisted fields
- [x] 4.3 `/models` applies `scope_models`
- [x] 4.4 Arg picker after `#name` and on space; `#name arg` atom in accent; `run(arg)` builds the body on send
- [x] 4.5 `/stop` in the command picker
- [x] 4.6 Chat follows new content only at the bottom; scrolled-up view stays put (anchor by line offset from the top)

## 5. JSON-RPC
- [x] 5.1 `turn.submit` prompts accept `"name arg"`; validate
- [x] 5.2 `config.get` gets models, `settings.open`, new UI keys, prompt kinds and args

## 6. Examples and docs
- [x] 6.1 Write `examples/*.lua` from the design list
- [x] 6.2 `docs/examples.md`; add `examples` to `build.rs`, `docs.rs` test (8 topics), and `Makefile` `DOCS`
- [x] 6.3 Test: every example loads, every example has a link, every link exists
- [x] 6.4 Update `usage.md` (`/stop`, Esc, `/tasks`, `#` args), `config.md` (keys, `settings.open`, prompts), `lua.md` (tasks, handoff), `session.md` (cancel, trace roles, notices), `json.md`
- [x] 6.5 README: only add example links, keep the current uncommitted edits
- [x] 6.7 Switch this repo's `openspec.lua` to one `#openspec` prompt with args
- [x] 6.6 Changelog entry

## 7. Verify
- [x] 7.1 Production code still under 20000 lines
- [x] 7.2 `make check` and `make site` pass
