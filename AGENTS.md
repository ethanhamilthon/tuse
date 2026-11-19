# Tuse - minimal AI coding agent

Rust + Crossterm, OpenAI Chat Completions API. User docs: `README.md` and `docs/`.

## Rules

- Separate crates with a clear split of responsibility, one crate per layer:
  - `crates/config` - YAML loading, includes, `$VAR` and `#{{cmd}}` expansion, theme tokens;
  - `crates/api` - Chat Completions client: streaming, retries, usage;
  - `crates/core` - sessions, agent loop, bash tool, compaction, hooks, tasks, bins;
  - `crates/socket` - session socket protocol, server and client;
  - `crates/tui` - the terminal UI;
  - `crates/cli` - the `tuse` binary: argument parsing and wiring.
  Lower layers never depend on higher ones (`tui` and `cli` sit on top).
- Minimum tests and comments: test only code that can really break, comment only what is not obvious.
- No file longer than 250 lines. Split a file before it grows past that.
- Everything in English: UI text, comments, docs, commit messages.
- Every UI decision is a theme token (colors, spacing, limits, symbols); no hardcoded styles in `tui`.
- Run `make check` (fmt, clippy, tests) before committing; CI runs the same checks.
- Commit after every feature or fix. Releases: bump `version` in `Cargo.toml`, update `CHANGELOG.md`, tag `vX.Y.Z`.
