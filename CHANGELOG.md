# Changelog

## 0.1.0 - 2026-11-19

First release.

- Full-screen TUI: Markdown replies, `/` commands, `#` prompts, `@` file picker,
  turn timer and tool-call counter, Esc to stop a turn.
- Headless mode `tuse -p`, driven with `tuse steer`.
- One `bash` tool, run as `bash -c` with a timeout and output limit.
- Sessions in `~/.tuse/<id>/` with a lock and a socket; `/resume` and `--resume`.
- `tuse ask` (blocking or `--async`) and background `tuse task run/stop/get/list`.
- YAML config: `include`, `$VAR`, `#{{command}}`, hooks, `bins`, prompts, systems, a full theme.
- 50/25/25 context compaction, retries with backoff, `/usage` with token costs.
