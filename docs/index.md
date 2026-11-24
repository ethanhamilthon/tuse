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
- [Providers & API Client](providers.md) — Provider credentials, four API adapters (`openai-chat`, `openai-responses`, `anthropic-messages`, `google-gemini`), streaming, reasoning replay, retries, and errors.
- [Providers & Models](models.md) — Model naming, background discovery, Lua metadata, reasoning effort, and selection scoping.
- [Usage, Cost & Context Compaction](usage.md) — Token accounting, USD cost formulas, display, and 50/25/25 compaction.

## Lua Runtime & Extensibility

- [Lua Runtime](lua.md) — Persistent Lua 5.4 VM, coroutine scheduler, module loading, and execution limits.
- [The `lua` Tool](tool.md) — Single agent tool contract, output handling, diffs, and error behavior.
- [Functions](functions.md) — Built-in host functions (`session()`, file operations, `grep`, `sh`, `spawn`, UI, `llm`, `reload`, `docs`).
- [Background Tasks](tasks.md) — Asynchronous tasks (`spawn`, `task_get`, `task_stop`, `task_list`), logs, and completion notifications.
- [Hooks](hooks.md) — Lifecycle event hooks (`start`, `close`, `user_message`, `agent_message`, `model_change`, `name_change`, `lua_start`, `lua_end`).

## Configuration & Customization

- [Setup Guide](setup.md) — Step-by-step setup: connecting providers, global and project configs, modular layout, and verifying with `/reload` and `/config`.
- [Config](config.md) — Lua config modules, `require` resolution, schema keys, strict validation, and `/reload`.
- [Environment & Config Lookup](env.md) — Environment variables read by Tuse and config file search order.
- [Theme & UI Settings](theme.md) — The 10-token color theme and `settings.ui` behavior and layout options.
- [Plugins](plugins.md) — Git, local, and bundled (`tuse/default`) plugins, options, merge rules, and reload sync.
- [Plugin Authoring Guide](plugin-authoring.md) — Writing, testing, and publishing Tuse plugins.
- [Subscription Proxy (`subs`)](subs.md) — Optional `tuse/default` CLIProxyAPI wrapper (`/subs`), loopback proxy lifecycle, and provider wiring.

