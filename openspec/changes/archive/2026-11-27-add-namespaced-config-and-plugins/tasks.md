## 1. Design Decisions (Resolve Before Applying Blocked Items)

- [x] 1.1 Resolve `Q1`–`Q7` in `design.md` (merge rules for remaining sections, entrypoint callable contract & `require` scope, selector edge cases, global path/`require("global")`, plugin naming/manifest extras, CLI `print`/`[repo]`/transport details, reload/`/config` projection, and `README.md` approval) and update delta specs accordingly.
- [x] 1.2 Coordinate merge order and overlapping edits with `openspec/changes/update-public-api-consistency/` (`validate.rs`, `require.rs`, `args.rs`, `main.rs`, `specs/config/spec.md`, `specs/cli/spec.md`).

## 2. Config Discovery, Namespaced Schema & Merging (`crates/config`, `crates/core`)

- [x] 2.1 Update config discovery and loading (`crates/config/src/load.rs`, `crates/config/src/modules.rs`, `crates/core/src/session/lua/config_lua.rs`) so `-c`/`--config PATH` behaves as local `tuse.lua`, primary config evaluates before `global` and `plugins`, `global = false` skips global execution, and `global = opts` (or `nil` when omitted / no local config) invokes the function returned by `~/.config/tuse/init.lua`.
- [x] 2.2 Implement namespaced config validation and merging (`crates/core/src/scripting/validate.rs`, `crates/config/src/types.rs`), rejecting legacy flat configs and non-function global configs, enforcing duplicate-key errors for `functions`, `commands`, and `prompts` across all merged contributions (including local re-exports of plugin functions), and committing merged Lua callbacks to `tuse.config` and `COMMIT_AGENT`.
- [x] 2.3 Implement `plugins` selector parsing and filtering (`omitted` and `{}` auto-load all installed plugins; `{ only = {} }` disables auto-loading; `only` allowlists; `without` excludes; `opts` keyed by plugin name).
- [x] 2.4 Implement plugin directory discovery under `~/.tuse/plugins` (counting only directories whose root contains `tuse-plugin.json`) and manifest parsing (`version` required, `entrypoint` defaulting to `init.lua`, rejecting path traversal outside plugin root).
- [x] 2.5 Implement `require("plugin:<name>")` (loads only that plugin and returns its raw entrypoint export without invoking functions) and `require("plugins")` (loads and merges all installed plugins into a flat table with `.functions`, `.commands`, `.prompts`, etc., independent of returned selector/opts, enforcing duplicate errors) in `crates/core/src/scripting/require.rs`.
- [x] 2.6 Update `/reload` (`config_lua::prepare`, `require::forget`/`restore`) to re-scan plugins and cleanly roll back global/plugin caches on failure, and update `/config` (`crates/tui/src/ui/screen/config_view.rs`) to project active `plugins`.

## 3. Official Plugin CLI & `tuse init` (`crates/cli`)

- [x] 3.1 Extend CLI parsing and dispatch (`crates/cli/src/args.rs`, `crates/cli/src/main.rs`) to support nested `tuse plugins <subcommand>` with optional positional arguments (`update [repo]`), update `USAGE` and help tests (replacing the legacy `spec("plugins").is_none()` assertion).
- [x] 3.2 Implement `tuse plugins add/list/delete/print/update [repo]` supporting public GitHub repositories and repository subdirectories, manifest verification (`tuse-plugin.json`), safe extraction into `~/.tuse/plugins`, and source metadata tracking.
- [x] 3.3 Update `tuse init` (`crates/cli/src/init.rs`) to generate a starter namespaced `./tuse.lua` without manual `require("global")` boilerplate, and verify `tuse models` loads namespaced configs and plugins.

## 4. Examples, Documentation & Tests

- [x] 4.1 Migrate repo root `tuse.lua` and all 18 `examples/*.lua` modules plus `docs/examples.md` and `crates/core/tests/examples.rs` to the namespaced config format.
- [x] 4.2 Update embedded documentation (`docs/config.md`, `docs/usage.md`, `docs/examples.md`, and `crates/core/src/prompt.md` / `docs/lua.md` if needed) to document namespaced config, `global`, `plugins` selectors, `require("plugin:<name>")`, `require("plugins")`, and `tuse plugins` subcommands while preserving the 8-topic build contract (`crates/core/build.rs`).
- [x] 4.3 Update existing config, validation, CLI, and RPC test fixtures (`crates/core/tests/`, `crates/cli/tests/`, `crates/rpc/tests/`) for the breaking namespaced schema, and add minimal targeted tests for primary-first evaluation, `global = false` / `global = opts`, `plugins = {}` vs `plugins = { only = {} }` / `only` / `without` / `opts`, duplicate errors (including `cl.fn` re-export), `require("plugin:<name>")`, `require("plugins")`, and `tuse plugins` CLI operations.
- [x] 4.4 Skipped by explicit user instruction (do not edit `README.md`).

## 5. Verification

- [x] 5.1 Run `cargo build` and `cargo test --workspace` (confirming docs coverage, `/config` coverage, examples loading, CLI help parity, and plugin/config tests pass).
- [x] 5.2 Verify production Rust code stays under 20,000 lines (excluding tests) and contains zero comments per `tuse.lua`.
