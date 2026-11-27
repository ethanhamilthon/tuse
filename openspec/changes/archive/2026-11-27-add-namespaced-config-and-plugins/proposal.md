## Why

Tuse currently loads a single flat Lua config table (`{ ui, systems, prompts, functions, hooks, commands, ... }`) where project configs must manually `require("global")` and mutate tables to compose reusable modules. This makes sharing and combining independent extensions brittle, provides no standard installation path for third-party Lua plugins, and forces ad-hoc merging in user config files.

Replacing the flat top-level config with a namespaced contribution model, automatic global and plugin composition with opt-outs, explicit plugin `require` primitives, and an official `tuse plugins` CLI provides clean, composable configuration without silent name shadowing.

## What Changes

- **Namespaced config format (breaking, no backward compatibility):**
  - Primary configs (`./tuse.lua` and `-c`/`--config PATH`) and the global config (`~/.config/tuse/init.lua`) return tables of named namespaces (e.g., `{ herdr = { functions = ..., prompts = ... }, rename = { functions = ... } }`), replacing the flat top-level `{ commands, systems, ... }` schema.
  - Duplicate keys in `functions`, `commands`, or `prompts` across merged contributions fail validation with an error (not first-wins), including when a local namespace re-exports a function (such as `cl.fn` from `require("plugin:cliproxyapi")`) whose plugin is also auto-loaded.
- **Primary-first evaluation and `global` config function:**
  - `-c`/`--config PATH` behaves identically to local `./tuse.lua`.
  - The primary config is evaluated before auto-loading `global` or `plugins`, so opt-outs skip their execution entirely.
  - Global `~/.config/tuse/init.lua` returns a function `function(opts) ... end` that receives `opts` from `global = opts` in the local config, or `nil` when omitted or when no local config exists. Setting `global = false` opts out and skips executing global config.
- **Plugin discovery, auto-loading selectors, and explicit `require`:**
  - Installed plugins live under `~/.tuse/plugins`; a directory counts as a plugin only if its root contains `tuse-plugin.json` (`version` required, `entrypoint` optional defaulting to `init.lua`).
  - Omitted `plugins` and `plugins = {}` both auto-load all installed plugins by default. `plugins = { only = {} }` disables auto-loading while leaving explicit `require` working.
  - `plugins` accepts `{ only = <list>, without = <list>, opts = { <name> = <opts> } }` where `only` allowlists, `without` excludes, and `opts` supplies per-plugin options.
  - `require("plugin:<name>")` loads only that plugin and returns its raw entrypoint export (including functions without invoking them).
  - `require("plugins")` loads and merges all installed plugin exports into a flat config table (`.functions`, `.commands`, `.prompts`, etc.) independent of the returned `plugins` selector and `opts`, enforcing the duplicate error rule.
- **Official `tuse plugins` CLI:**
  - Add `tuse plugins add/list/delete/print/update [repo]` supporting public GitHub repositories and repository subdirectories, installing into `~/.tuse/plugins`.
  - Update `tuse init`, `examples/*.lua`, and embedded docs (`docs/config.md`, `docs/usage.md`, `docs/examples.md`) to match the namespaced format and plugin CLI.
- **Open items explicitly tracked for review in `design.md` (`Q1`–`Q7`):**
  - Merge semantics for the remaining sections (`ui`, `history`, `open`, `providers`, `models`, `systems`, `hooks`) and `functions` disable/override interactions; plugin entrypoint callable contract and `require` scope; selector edge cases; global path / `require("global")` retention; plugin naming, manifest extras, and CLI details (`[repo]` syntax, transport, metadata store, `print` output, `update` detection); reload and `/config` projection; and optional `README.md` update.

## Impact

- **Specs:** `config` (modified/added), `plugins` (new), `cli` (new in baseline; overlaps active change), `examples` (modified), `docs` (modified); `providers` potentially impacted pending `Q1`.
- **Implementation (upon apply):** `crates/config`, `crates/core`, `crates/cli`, `crates/tui`, `crates/rpc` tests, `examples/`, `docs/`, `tuse.lua` (repo root). `README.md` stays untouched unless explicitly requested per `tuse.lua`.
- **Breaking changes:** Intentional breaking change with no backward compatibility for legacy flat configs or non-function global `init.lua`.
- **Dependencies & overlaps with active `openspec/changes/update-public-api-consistency/`:**
  - Overlaps `specs/config/spec.md` (`Config layout`, validation in `crates/core/src/scripting/validate.rs`, reload cache handling in `crates/core/src/scripting/require.rs`) and `specs/cli/spec.md` (CLI parsing in `crates/cli/src/args.rs` and `crates/cli/src/main.rs`).
  - Coordinates with `update-public-api-consistency` `Q1` (`TUSE_GLOBAL_CONFIG` vs `-c`) and `Q6` (`providers` in `/config` and `config.get`).
  - Existing unstaged worktree edits (`README.md`, `tuse.lua`) and `openspec/changes/update-public-api-consistency/` are preserved untouched.
- **Code-size constraint:** Production Rust code is currently ~18,990 lines against the 20,000-line ceiling in `tuse.lua` (~1,010 lines headroom), requiring a compact implementation reusing `crates/cli`'s existing `ureq`/`flate2`/`tar` stack where appropriate.
