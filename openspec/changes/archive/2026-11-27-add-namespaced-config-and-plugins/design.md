## Context

Current Tuse (v0.13.1) resolves a single config entry file (`-c` → `./tuse.lua` → `~/.config/tuse.lua` → `~/.config/tuse/init.lua` → defaults), evaluates it into a flat Lua table of 10 top-level keys (`ui`, `history`, `open`, `providers`, `models`, `systems`, `prompts`, `functions`, `hooks`, `commands`), and rejects `plugins` as an unknown key (`crates/core/src/scripting/validate.rs:5-11`, `crates/cli/src/main.rs:202`).

This change replaces the flat schema with namespaced configs, automatic `global` and `plugins` composition, explicit plugin `require` primitives, and `tuse plugins` CLI management.

### Repository Constraints & Security Considerations

- **Production Rust LOC budget (`tuse.lua:22`):** Must stay under **20,000** lines excluding tests. Baseline is currently **~18,990 lines** (~1,010 lines headroom, shared with any settled work from `update-public-api-consistency`).
  - Keep namespace validation/merging in a tight pass in `crates/core/src/scripting/` and reuse existing `crates/cli` dependencies (`ureq`, `flate2`, `tar`) instead of adding heavy abstractions or new crates.
  - Zero comments in Rust files; minimal targeted tests; English only in code, UI, contracts, and docs; do not modify `README.md` unless explicitly requested.
- **Trusted plugin execution model:** Installed plugins execute inside the trusted Lua environment with full user permissions (same trust boundary as `tuse.lua` and `init.lua`).
  - Only install plugins from explicit user `tuse plugins add` commands; never auto-download at session startup.
  - Validate archive extraction paths and `tuse-plugin.json` `entrypoint` paths strictly within the plugin root directory (reject `..`, absolute paths, and symlink escapes).
  - Preserve secret masking under `providers.*` (`crates/config/src/load.rs:43-44`) so keys/headers never echo in validation errors, `/config`, or `config.get`.
  - Keep `~/.tuse/plugins` isolated from session directories (session IDs use `<timestamp>-<hex>` format in `crates/core/src/id.rs`).

### Overlaps & Coordination with Active Changes

An active unapplied change exists at `openspec/changes/update-public-api-consistency/` (plus unstaged worktree edits to `README.md` and `tuse.lua`):
1. **`specs/config/spec.md` & `crates/core/src/scripting/validate.rs`:** Both changes modify `Config layout` and rewrite Lua-to-JSON validation (replacing `deny_unsupported_types(false)` and removing the legacy `plugins: unknown key` pre-check). If `update-public-api-consistency` lands first, this change builds on its path-aware converter; if this change lands first, path-aware rejection of unsupported Lua values must be built directly into the namespaced validator.
2. **`crates/core/src/scripting/require.rs`:** `update-public-api-consistency` adds trusted `package.preload` reload clearing; this change adds `plugin:<name>` and `plugins` module resolution and plugin reload invalidation.
3. **`specs/cli/spec.md`, `crates/cli/src/args.rs`, `crates/cli/src/main.rs`:** `update-public-api-consistency` introduces `specs/cli/spec.md` with strict boolean flags, hyphenated values, and `tuse help [COMMAND]`. `tuse plugins <subcommand>` requires subcommand dispatch and optional positional args (`update [repo]`), which should compose cleanly with those flag/help rules.
4. **Shared open questions:** `update-public-api-consistency` `Q1` (`TUSE_GLOBAL_CONFIG` vs `-c`) affects global config path lookup (`Q4` below), and `Q6` (`/config` and `config.get` provider display) interacts with config introspection (`Q7` below).

---

## Confirmed Architecture

### 1. Config Loading & Evaluation Order

1. **Primary config discovery:** `-c`/`--config PATH` behaves identically to local `./tuse.lua` (both act as the primary local config). If neither `-c` nor `./tuse.lua` is present, there is no primary local config.
2. **Primary-first execution:** When a primary config exists, evaluate it first to obtain its returned table before touching `global` or auto-loading `plugins`.
   - `global = false` prevents loading or executing the global config file.
   - `global = <opts>` loads `~/.config/tuse/init.lua` and calls its returned function with `<opts>`.
   - Omitted `global` (or no primary config) loads `~/.config/tuse/init.lua` (if present) and calls its returned function with `nil`.
3. **Global config contract:** `~/.config/tuse/init.lua` MUST return a Lua function `function(opts) ... return { <namespaces> } end`. Returning a non-function (such as a legacy flat table) fails validation (no backward compatibility).
4. **Namespaced shape:** Every config return table maps namespace names to contribution tables containing config sections (`ui`, `history`, `open`, `providers`, `models`, `systems`, `prompts`, `functions`, `hooks`, `commands`), alongside the reserved top-level control keys `global` and `plugins`. Flat top-level config tables are rejected.

### 2. Plugin Discovery, Auto-Loading & Selectors

- **Root manifest rule:** Under `~/.tuse/plugins/`, only immediate plugin directories whose root contains `tuse-plugin.json` are recognized as installed plugins; directories without a root `tuse-plugin.json` are ignored.
- **Manifest minimum schema:** `tuse-plugin.json` requires `version` (string) and supports optional `entrypoint` (string, default `"init.lua"`).
- **Selector semantics (`plugins` key):**
  - **Omitted `plugins`** → auto-load all installed plugins (with no per-plugin opts).
  - **`plugins = {}`** → auto-load all installed plugins (same default behavior as omitted).
  - **`plugins = { only = {} }`** → disable auto-loading (zero plugins auto-loaded; explicit `require("plugin:<name>")` and `require("plugins")` still work).
  - **`plugins = { only = { ... }, without = { ... }, opts = { <name> = <opts> } }`**:
    - `only`: allowlist of plugin names to auto-load.
    - `without`: list of plugin names to exclude from auto-loading.
    - `opts`: table keyed by plugin name passing options to matching plugins.

### 3. Explicit `require` Primitives

- **`require("plugin:<name>")`:** Loads only the named installed plugin's entrypoint and returns its **raw export as-is** (including returning a Lua function without invoking it).
- **`require("plugins")`:** Loads all installed plugins, merges their exports into a flat config table exposing `.functions`, `.commands`, `.prompts`, etc., **independent of** whatever `plugins` selector or `opts` the primary config later returns, and applies the same duplicate error rule.

### 4. Merge & Duplicate Error Rule

- Merging combines contributions into the resolved `Config` and Lua runtime registry (`tuse.config` + `COMMIT_AGENT`) so downstream runtime lookups (`registry.rs`, `commands.rs`, `open.rs`, `lanes.rs`) continue operating on unified tables.
- **Confirmed duplicate rule:** Duplicate keys in `functions`, `commands`, or `prompts` across any merged contributions (including local namespaces, global namespaces, and auto-loaded plugins) MUST fail validation with an error identifying the duplicate key and conflicting sources. In particular, re-exporting a plugin function inside a local namespace (e.g., `local cl = require("plugin:cliproxyapi"); return { my = { functions = { fn = cl.fn } } }`) while `cliproxyapi` is also auto-loaded fails as a duplicate key error unless auto-loading excludes that plugin (e.g., `plugins = { without = { "cliproxyapi" } }` or `plugins = { only = {} }`).

### 5. Official Plugin CLI

- Add `tuse plugins` with five subcommands:
  - `tuse plugins add <repo>` — install a plugin from a public GitHub repository or repository subdirectory into `~/.tuse/plugins`.
  - `tuse plugins list` — list installed plugins.
  - `tuse plugins delete <target>` — remove an installed plugin from `~/.tuse/plugins`.
  - `tuse plugins print <target>` — print plugin information (exact format open in `Q6`).
  - `tuse plugins update [repo]` — update one or all installed plugins.

---

## Resolved Decisions (`Q1`–`Q7`)

All open design questions are resolved and coordinated with `update-public-api-consistency`:

- **Q1 (Remaining section merge rules):**
  - `open`, `providers`, `hooks`, `functions` (including `false` vs function override), `commands`, `prompts`, and `models[].name`: duplicate keys across merged contributions fail validation with an error naming the duplicate key and sources.
  - `ui`: deep-merged by leaf field with layer priority `local > global > plugins`; conflicting leaves within the same layer (e.g., two local namespaces or two plugins setting the same leaf) fail validation.
  - `systems` and `history.hide`: concatenated in deterministic order `global` -> `plugins` (sorted by plugin name) -> `local` (sorted by namespace name).
- **Q2 (Plugin entrypoint & `require` scope):**
  - A plugin entrypoint may return either a flat contribution table `{ functions, ... }` or `function(opts) -> table`.
  - `require("plugins")` invokes function entrypoints with `nil` and merges all installed plugins into a flat config table independent of the primary config's `plugins` selector/opts, enforcing duplicate errors.
  - `require("plugin:<name>")` and `require("plugins")` are exposed in `trusted_env` only (never `agent_env`).
- **Q3 (`plugins` selector edge cases):**
  - Simultaneous `only` and `without` fail validation.
  - Unknown (uninstalled) plugin names in `only`, `without`, or `opts` fail validation.
  - `plugins = false` is rejected (use `plugins = { only = {} }`).
  - Local `plugins` overrides global `plugins`.
  - Installed plugins still auto-load when neither local nor global config file exists.
- **Q4 (Global config target & `require("global")`):**
  - Auto-loaded global target follows `$TUSE_GLOBAL_CONFIG` (if set/non-empty, expanding `~/` and directory -> `<dir>/init.lua`, fail-fast if missing) > `~/.config/tuse.lua` > `~/.config/tuse/init.lua`.
  - `require("global")` remains available and returns the raw global function from that same target.
- **Q5 (Plugin naming & manifest):**
  - `<name>` comes from `tuse-plugin.json` `name` if present, else repo/subdir basename, validated to safe path/identifier characters (non-empty, no `/`, `\`, `:`, whitespace, control chars, or `.`/`..`).
  - `tuse-plugin.json` requires non-empty `version` string, optional `entrypoint` (default `"init.lua"`, validated strictly inside plugin root), optional `name`, and allows extra metadata fields.
  - Name collisions on `tuse plugins add` fail unless `--replace` is passed.
- **Q6 (`tuse plugins` CLI transport, metadata, and `print`):**
  - Accepts `owner/repo[/subdir][@ref]` and `https://github.com/owner/repo[/tree/<ref>/<subdir>]`.
  - Downloads via HTTPS tarball (`ureq` + `flate2` + `tar`, with `TUSE_PLUGINS_URL` test base URL override).
  - Source metadata is stored in `~/.tuse/plugins.json`.
  - `tuse plugins list [--json]` lists installed plugins; `tuse plugins print <target> [--json]` prints plugin name, version, entrypoint, path, source, and manifest details (JSON with `--json`); `tuse plugins delete <target>` removes the plugin directory and metadata; `tuse plugins update [target]` updates one or all installed plugins from recorded source metadata.
- **Q7 (Reload, `/config` / `config.get`, `config_path`, `README.md`):**
  - `/reload` re-scans `~/.tuse/plugins/`, clears trusted plugin/global/preload caches, and rolls back on failure.
  - `config.get` and `/config` include `plugins: [{ name, version }]` for active loaded plugins alongside the merged config fields (`providers`, `models`, etc.); `session().config_path` stays the primary loaded config path (or global path when no primary config exists).
  - `README.md` remains untouched per project rules.
