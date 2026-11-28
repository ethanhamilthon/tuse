# Tuse

![Tuse](docs/demo.gif)

Tuse is a fast, minimal terminal coding agent built in Rust and shaped with Lua. Most coding agents are heavily opinionated, while building your own takes weeks and tens of thousands of lines of code. Tuse gives you a thin Rust core with one persistent Lua 5.4 tool so you can start working immediately with zero config, then customize every part of the workflow in Lua or with external plugins.

- **Two interfaces:** a full-screen TUI and [JSON-RPC v2](https://ethanhamilthon.github.io/tuse/json.html) for your own front end, sharing sessions, locks, and history.
- **Bring your own provider:** OpenAI Chat Completions, OpenAI Responses, Anthropic Messages, Google Gemini, or any compatible gateway—managed interactively inside Tuse.
- **Lua configuration:** organize your global config into namespaces and extend it with separately installed plugins, then reload without restarting.

## Quickstart

Install on macOS or Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/ethanhamilthon/tuse/main/release/install.sh | sh
tuse
```

The installer verifies the release checksum and installs only the `tuse` binary.

Inside Tuse:

1. Run `/providers connect`, pick one of the four API types (`openai-chat`, `openai-responses`, `anthropic-messages`, `google-gemini`), enter your base URL, enter your API key (or `$ENV_VAR`) in the masked field, and give the profile a name.
2. Run `/models` to select a discovered model and reasoning effort.
3. Send your first message.

## Configuration: make Tuse yours

Tuse runs fine with no config at all. To customize it, create `~/.config/tuse/init.lua` (or point `TUSE_GLOBAL_CONFIG` at a different file). Settings such as `ui` and `systems` go at the top level. Prompts, functions, hooks, and commands live under a namespace. Tuse does not automatically execute a project's `tuse.lua`.

```lua
local t = require("tuse")

return {
  systems = { rules = "This is a Rust workspace. Run `cargo test` before finishing." },
  ui = { input = "boxed", picker = "bottom", theme = { accent = "#FFB86C" } },
  work = {
    prompts = { review = "Review the current diff for bugs and missing tests." },
    functions = {
      status = function()
        return t.sh("git status --short").out
      end,
    },
    hook = {
      agent_message = function(e)
        if e.status == "finished" then
          t.sh("printf '\\a'")
        end
      end,
    },
    commands = {
      diff = function()
        t.ui.show("diff", "```diff\n" .. t.sh("git diff --stat; git diff | head -200").out .. "\n```")
      end,
    },
  },
}
```

Edit your config and run `/reload` (or ask Tuse to edit it and call `reload()`) to apply changes live.

## System prompt and reusable prompts

`systems` supports two forms:

- **Replace everything** with a single string:

```lua
return {
  systems = "You are Tuse, a coding agent.",
}
```

Use full replacement with care: replacing the built-in section removes Tuse's instructions on using Lua, `fs.*`, `sh`, `tasks.*`, and `docs`.

- **Extend or override selectively** with a named table:

```lua
return {
  systems = { chat_rules = "Keep replies direct and concise." },
}
```

Named entries append in sorted key order after Tuse's built-in prompt. Setting the reserved `lua` key inside a named `systems` table replaces only the built-in section while keeping all other named sections.

For on-demand instructions, select `#work:review` in the composer. Prompts are literal strings and take no arguments.

## Functions, hooks, commands, and management

- **Functions (`functions`):** expose Lua callbacks in the agent VM under their namespace, such as `work.status()`.
- **Hooks (`hook`):** subscribe directly by event name (`hook = { user_message = fn, agent_message = fn }`). Hooks run concurrently in the background without blocking turns.
- **Commands (`commands`):** register zero-argument callbacks such as `/work:diff`, which can use `ui.show`, `ui.pick`, and `ui.ask`.
- **Built-in management:** manage API profiles with `/providers list|connect|update|disconnect` and installed plugins with `/plugins list|get|add|delete|update` (or programmatically via Lua `providers.*` / `plugins.*` and JSON-RPC). Plugin changes update installed files immediately and take effect in the running session after an explicit `/reload`.

## UI and themes

Customize layout, line limits, overlay placement (`ui.picker = "bottom"` or `"top"`), input style (`boxed`, `lines`, `fill`, `bar`), spacing, and full color themes under `ui`:

```lua
return {
  ui = {
    input = "bar",
    picker = "top",
    lines = { output = 3, diff = 20, strings = 2 },
    space = { padding = 1, steps = 0 },
    theme = { accent = "#FFB86C", code = { keyword = "accent" } },
  },
}
```

## Examples and plugins

See [Examples](https://ethanhamilthon.github.io/tuse/examples.html) for copyable Lua snippets. Plugins are separately distributed packages with a `tuse-plugin.json` manifest, installed into `$TUSE_HOME/plugins/` via `/plugins add <source>`. No plugins are bundled. Only load config modules or plugins you trust.

## Best practices

- Keep your global `~/.config/tuse/init.lua` thin; explicitly load only trusted project files.
- Install only plugins you trust and need—every extra plugin adds surface area.
- Split reusable pieces into sibling `.lua` modules with `require(...)`.
- Group related prompts under one namespace, such as `#work:review` and `#work:test`.
- Review your prompts, custom functions, and plugins carefully: they shape model output directly.

## Beta status, security, and updates

Tuse is currently in **Beta**. Patch releases preserve compatibility, while minor releases may change config or scripting contracts.

- **Interactive updates and rollback:** run `tuse update` to compare your installed version with the latest release, optionally inspect all intervening changelog entries, verify the release SHA-256 checksum, back up the current binary to `$TUSE_HOME/backups/<version>-tuse-bin`, and replace the executable. If a release breaks your workflow, run `tuse rollback` to restore the binary from the last successful update without touching configs, plugins, or `tuse.db`.
- **Security:** Tuse has no built-in permission prompts or OS sandbox. The agent runs Lua and shell commands with your user permissions. If you want isolation, run Tuse inside a container, VM, or dedicated machine,.

## Benchmark

An earlier 50-case benchmark on Tuse 0.11.5, included for reference (not a measurement of 0.15.0). Tuse's single-tool Lua runtime keeps context compact, typically using 2–3x fewer input tokens at comparable pass rates:

| Agent | Full passes | Time* | Input* | Output* |
| :-- | --: | --: | --: | --: |
| **Tuse 0.11.5** | **49/50** | 57.3 s | 54.7k | 11.4k |
| **OpenCode** | **49/50** | 66.2 s | 126.5k | 10.9k |
| Claude Code | 47/50 | 69.0 s | 153.8k | 13.7k |
| Pi | 46/50 | 55.3 s | **52.6k** | 11.2k |
| Codex | 46/50 | **54.4 s** | 154.0k | **9.7k** |


## Docs and contributing

Read the [documentation](https://ethanhamilthon.github.io/tuse/) or let the agent inspect `docs.topics` and `docs.lua`. See the [changelog](https://github.com/ethanhamilthon/tuse/blob/main/release/CHANGELOG.md) for release history. Discussions, issues, and pull requests are welcome. Tuse is [MIT licensed](https://github.com/ethanhamilthon/tuse/blob/main/LICENSE).
