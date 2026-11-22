# Tuse - minimal AI coding agent

Rust + Crossterm, OpenAI Chat Completions API. User docs: `README.md` and `docs/`.

## Rules

- Separate crates with a clear split of responsibility, one crate per layer:
  - `crates/config` - config types, checks and theme tokens (built from the table `tuse.lua` returns);
  - `crates/api` - Chat Completions client: streaming, retries, usage;
  - `crates/lua` - the Lua 5.4 (mlua) runtime: one VM thread, the coroutine scheduler, `sh`, `sleep`, `spawn`;
  - `crates/core` - sessions and SQLite storage, agent loop, tools (`lua`, `read`, `write`, `edit`), compaction, hooks, tasks;
  - `crates/tui` - the terminal UI;
  - `crates/rpc` - `tuse json`: JSON-RPC 2.0 over stdio, a front end like `tui`;
  - `crates/cli` - the `tuse` binary: argument parsing and wiring.
  Lower layers never depend on higher ones (`tui`, `rpc` and `cli` sit on top).
- Minimum tests and comments: test only code that can really break, comment only what is not obvious.
- No file longer than 250 lines. Split a file before it grows past that.
- Total project size must not exceed 15,000 lines of code.
- Everything in English: UI text, comments, docs, commit messages.
- Colors come from the 10-token `Theme` and UI behavior/layout from `settings.ui`; no hardcoded colors in `tui` (symbols, labels and internal constants live in code).
- Run `make check` (fmt, clippy, tests) before committing; CI runs the same checks.
- Commit after every feature or fix. Releases: bump `version` in `Cargo.toml`, update `release/CHANGELOG.md`, tag `vX.Y.Z`.
