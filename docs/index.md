# Tuse Documentation

Source of truth for Tuse's observable behavior and public contracts. All baseline pages below describe how Tuse works today; upcoming specifications live separately under [Planned Changes](#planned-changes).

## Getting Started & Interfaces

- [Installation & Updates](install.md) — Supported platforms, shell installer, building from source, and `tuse update`.
- [CLI](cli.md) — Executable subcommands, argument parsing, output formats, and exit behavior.
- [Terminal UI (TUI)](tui.md) — Full-screen terminal interface, keybindings, mouse, pickers, panels, links, and steering.
- [Commands](commands.md) — Quick reference for CLI subcommands and TUI slash commands.
- [JSON-RPC Mode (`tuse json`)](json.md) — JSON-RPC 2.0 over stdio: handshake, methods, events, and interactive UI requests.

## Core Runtime & Providers

- [Sessions & Persistence](sessions.md) — SQLite store (`~/.tuse/tuse.db`), session locks, resume, and TUI/JSON parity.
- [Agent Loop & Tool Lifecycle](agent-loop.md) — Turn execution, streaming, tool calls, cancellation, and recovery.
- [Prompts, Files & Images](prompts.md) — `#` prompts, `@` files/directories, pasted images, and hidden context blocks.
- [Providers & API Client](providers.md) — Provider credentials, Chat Completions wire contract, streaming, retries, and errors.
- [Providers & Models](models.md) — Model naming, background discovery, Lua metadata, reasoning effort, and selection scoping.
- [Usage, Cost & Context Compaction](usage.md) — Token accounting, USD cost formulas, display, and 50/25/25 compaction.

## Lua Runtime & Extensibility

- [Lua Runtime](lua.md) — Persistent Lua 5.4 VM, coroutine scheduler, module loading, and execution limits.
- [The `lua` Tool](tool.md) — Single agent tool contract, output handling, diffs, and error behavior.
- [Functions](functions.md) — Built-in host functions (`session()`, file operations, `grep`, `sh`, `spawn`, UI, `llm`, `reload`, `docs`).
- [Background Tasks](tasks.md) — Asynchronous tasks (`spawn`, `task_get`, `task_stop`, `task_list`), logs, and completion notifications.
- [Hooks](hooks.md) — Lifecycle event hooks (`start`, `close`, `user_message`, `agent_message`, `model_change`, `name_change`, `lua_start`, `lua_end`).

## Configuration & Customization

- [Config](config.md) — Lua config modules, `require` resolution, schema keys, strict validation, and `/reload`.
- [Environment & Config Lookup](env.md) — Environment variables read by Tuse and config file search order.
- [Theme & UI Settings](theme.md) — The 10-token color theme and `settings.ui` behavior and layout options.
- [Plugins](plugins.md) — Git and local plugins, options, merge rules, and bundled plugins.

## Planned Changes

Specifications for planned (not yet implemented) features and changes. Baseline pages above reflect only current behavior until each spec is implemented.

- [01 — Navigate `/trace` Entries with Left and Right](planned/01-trace-arrow-navigation.md)
- [02 — Support Four API Types](planned/02-api-types.md)
- [04 — Publish Docs on GitHub Pages](planned/04-github-pages-docs.md)
- [05 — Simplify the TUI Working Indicator](planned/05-working-indicator.md)
- [07 — Bundled Default Plugin: `theme` and `subs`](planned/07-default-plugin-theme-subs.md)
- [10 — Setup Guide](planned/10-setup-guide.md)
- [10 — Plugin Authoring Guide](planned/10-plugin-authoring-guide.md)
- [11 — First-Run Setup in the Installer](planned/11-installer-first-run.md)
- [12 — Hide Entire Diff Block When `diff_line = 0`](planned/12-diff-line-zero.md)
- [13 — Rewrite `README.md`](planned/13-readme-rewrite.md)
- [14 — Tuse Landing Page on GitHub Pages](planned/14-landing-page.md)
- [15 — Use Accent Colors for Interactive Links](planned/15-link-accent-colors.md)
