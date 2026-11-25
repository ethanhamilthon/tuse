# Add examples and control

## Why
Users ask how to do skills, MCP, themes, multiplexers and so on, and the answer is "write some Lua", with no real code to copy. The TUI also hides things: `/tasks` shows only a log, `/config` skips models, `/trace` misses some records and can't be filtered, and `/models` lists every model of every provider. Esc hands a running call off to the background instead of stopping it, and finished tasks start turns on their own. Prompts are plain strings, so a family like OpenSpec fills the `#` picker with five entries.

## What Changes
- New `examples/` folder with ready Lua configs, and a new docs topic `examples` (`docs/examples.md`) that links each one by GitHub URL. It is in `docs()` and on the site. README gets only links to the examples where it mentions those things, on top of the current uncommitted README edits.
- `/tasks` Enter opens a detail panel: id, name, status, started, ended, how long it has been running, the task's function source, then the log tail.
- `/config` shows `models` and every other resolved key; a test makes sure no config field is missing.
- `settings.ui.hide_trace = {"hook", "usage", ...}` hides trace roles in `/trace`.
- The trace covers everything: every record kind, task, command, and `open` runs, title changes.
- `settings.ui.scope_models = {...}` limits what `/models` lists.
- **BREAKING** Top-level `open` moves to `settings.open`.
- A `prompts` entry may be a table `{args?, run}`: `#openspec propose` picks an arg from `args` and `run(arg)` builds the body. Prompt and arg are drawn in the accent color.
- **BREAKING** Esc (and `turn.cancel`) stops everything: the turn, the running `lua` call (no handoff), background tasks, hooks, and commands. Steering still hands the running call off.
- **BREAKING** A finished task no longer starts a turn. Its notice waits and goes with the next user message.
- The chat follows new content only when the user is at the bottom; a scrolled-up view no longer jumps.
- New built-in `/stop` command that does the same as Esc, also when idle.

## Impact
- Specs: `docs`, `docs-site` (modified); `examples`, `config`, `tui`, `session`, `prompts` (new).
- Code: `crates/config` (settings, prompts, validation), `crates/core` (trace, tasks, cancel, notifications, docs build), `crates/lua` (function source), `crates/tui` (tasks, config, trace, models, `#` args, `/stop`), `crates/rpc` (prompts args, `config.get`, `tasks.*`).
- Repo: `examples/`, `docs/examples.md`, docs pages, `Makefile`, `README.md`.
- Production code is at ~15.7k lines, so there is room; the arg picker and full trace are the biggest pieces.
