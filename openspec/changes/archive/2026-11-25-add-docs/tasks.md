# Tasks

## 1. Prompt back into code
- [x] 1.1 Recreate the template from `git show HEAD:docs/system-prompt.md` as `crates/core/src/prompt.md`; drop the `tool-doc`, `tool-doc-functions`, `functions-doc-*` blocks and the docs-facing intro
- [x] 1.2 Point `prompt.rs` at it; remove `functions_doc`/`tool_doc`; add `agent_context(disabled) -> String` that renders the `What the agent sees` section
- [x] 1.3 Check that the default system prompt is byte-identical to the one before the move

## 2. Topics
- [x] 2.1 `build.rs`: fixed topic list (`readme` + six), summary taken from the first paragraph, fail on missing or extra pages
- [x] 2.2 `docs.rs`: new list and lookup; `lua` is rendered as the page head plus `agent_context(disabled)`; the disabled notice applies to all topics
- [x] 2.3 Update tests: `docs.rs` unit test, `contract.rs` (`lua` instead of `functions`), `agent_functions.rs`, `lua/helpers.rs`, tui/cli `commands` → `usage`, rpc `json`
- [x] 2.4 Unit test: the tail of `docs/lua.md` equals `agent_context(&[])`, and the expected text is printed when they differ
- [x] 2.5 `tuse help` keeps pointing at `docs/json.md` (the file still exists)

## 3. Pages
- [x] 3.1 `docs/usage.md`
- [x] 3.2 `docs/connect.md`
- [x] 3.3 `docs/config.md`
- [x] 3.4 `docs/lua.md` (with the generated tail)
- [x] 3.5 `docs/session.md`
- [x] 3.6 `docs/json.md`
- [x] 3.7 Check every page against the code (flags, keys, methods, defaults), not against the old docs

## 4. Site
- [x] 4.1 `book.toml` and a `make site` target (assemble into `target/book-src`, generate `SUMMARY.md`, run `mdbook build`)
- [x] 4.2 Rewrite `.github/workflows/pages.yml` around a pinned mdBook
- [x] 4.3 Changelog entry

## 5. Verify
- [x] 5.1 `make check` passes
- [x] 5.2 `make site` builds with a placeholder README
