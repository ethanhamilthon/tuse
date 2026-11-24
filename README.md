# Tuse

A minimal AI coding agent for the terminal written in Rust + Crossterm, centered on a single `lua` tool in a persistent Lua 5.4 VM, with both a full-screen TUI and a JSON-RPC 2.0 stdio mode.

[Website](https://ethanhamilthon.github.io/tuse/) · [Documentation](https://ethanhamilthon.github.io/tuse/docs/) ([local index](docs/index.md)) · [Releases](https://github.com/ethanhamilthon/tuse/releases) ([changelog](release/CHANGELOG.md))

## Why Tuse

- **One tool: `lua`** — runs in one persistent Lua 5.4 VM per session with `read`, `write`, `edit`, `append`, `delete`, `grep`, `sh`, `sleep`, and `spawn` background coroutines, live colored diffs, and no output cap.
- **TUI and `tuse json`** — full-screen terminal UI (Markdown, `/` commands, `#` prompts, `@` files, image paste) plus a JSON-RPC 2.0 stdio server (`tuse json`) sharing SQLite sessions (`~/.tuse/tuse.db`).
- **Plain Lua config & plugins** — configure hooks, custom slash commands, helpers, file openers, a 10-token theme, and Git or local plugins with live `/reload`.
- **Lean context** — small built-in system prompt, 50/25/25 context compaction, and automatic retries.
- **Single binary** — built-in defaults require no config file to start, and `tuse update` replaces the binary without touching your config.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/ethanhamilthon/tuse/main/release/install.sh | sh
```

Interactive installs can optionally set up `~/.config/tuse/init.lua` without overwriting an existing config; non-interactive, `CI`, `TUSE_NO_SETUP=1`, and `--no-setup` runs install only the binary.

Update in place with `tuse update`, or build from source with `cargo install --locked --git https://github.com/ethanhamilthon/tuse tuse` or `make install`. See [docs/install.md](docs/install.md) and [docs/env.md](docs/env.md) for version pinning (`TUSE_VERSION`) and custom install directories (`TUSE_BIN_DIR`).

## Quick Start

1. Connect an OpenAI-compatible provider (quote `'$ENV'` to store an environment variable reference instead of the raw key):

   ```sh
   tuse connect --base https://api.openai.com/v1 --key '$OPENAI_API_KEY' --name openai
   ```

2. Start `tuse`, pick a model and reasoning effort with `/models`, and begin chatting.

Resume a saved session with `tuse --resume ID` (the session ID prints to stderr on exit), or run `tuse json` for editor and tool integration.

## Documentation

Browse the published guides and references at [ethanhamilthon.github.io/tuse/docs/](https://ethanhamilthon.github.io/tuse/docs/) or locally starting from [docs/index.md](docs/index.md) — covering CLI flags, TUI keybindings, JSON-RPC, Lua host functions, hooks, configuration, themes, and plugins.

## Development & License

Built with Rust (toolchain pinned in `rust-toolchain.toml`); prebuilt releases target macOS 11+ and Linux (glibc 2.35+). See [AGENTS.md](AGENTS.md) for architecture rules and run `make check` before committing.

Licensed under the [MIT License](LICENSE).
