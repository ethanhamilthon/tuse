# Examples

Ready Lua config modules for common setups: skills, MCP, themes, titles, notifications, multiplexers, and more. Copy a file next to your config and place it under a namespace:

```lua
return {
  tmux = require("tmux"),
  notify = require("notify"),
  compaction = require("compaction")(0.8, 20),
  colors = { ui = { theme = require("themes").gruvbox } },
}
```

The files live in the [`examples/`](https://github.com/ethanhamilthon/tuse/tree/main/examples) folder. Ask Tuse to adapt one for you.

## Skills

Loads every `skills/<name>/SKILL.md` as a `#<name>` prompt. Skills are just reusable prompts. [`skills.lua`](https://github.com/ethanhamilthon/tuse/blob/main/examples/skills.lua)

## MCP

Wraps a stdio MCP server as two agent functions, `mcp_tools()` and `mcp_call(name, args)`. [`mcp.lua`](https://github.com/ethanhamilthon/tuse/blob/main/examples/mcp.lua)

## Themes

Full `ui.theme` tables: light, gruvbox, solarized, and one that uses the terminal colors. [`themes.lua`](https://github.com/ethanhamilthon/tuse/blob/main/examples/themes.lua)

## Rename

Names the session with a small model after the first message, and adds `/rename`. [`rename.lua`](https://github.com/ethanhamilthon/tuse/blob/main/examples/rename.lua)

## Notifications

A desktop notification when a turn finishes, on macOS and Linux. [`notify.lua`](https://github.com/ethanhamilthon/tuse/blob/main/examples/notify.lua)

## tmux

Puts the session title and agent state together in the tmux window name. [`tmux.lua`](https://github.com/ethanhamilthon/tuse/blob/main/examples/tmux.lua)

## Zellij

The same for the Zellij pane name. [`zellij.lua`](https://github.com/ethanhamilthon/tuse/blob/main/examples/zellij.lua)

## cmux

Sets the cmux workspace status, sends a cmux notification when done, and renames the tab. [`cmux.lua`](https://github.com/ethanhamilthon/tuse/blob/main/examples/cmux.lua)

## Herdr

Reports `working`/`idle` on the Herdr pane and renames it after the session. [`herdr.lua`](https://github.com/ethanhamilthon/tuse/blob/main/examples/herdr.lua)

## Orca

Posts the session title and progress as an Orca worktree comment and marks the workspace unread when done. [`orca.lua`](https://github.com/ethanhamilthon/tuse/blob/main/examples/orca.lua)

## Rex

Renames the current Rex block (`$REX_BLOCK`) with the session title and agent state. [`rex.lua`](https://github.com/ethanhamilthon/tuse/blob/main/examples/rex.lua)

## Web search

A `websearch(query, n)` agent function backed by SearXNG, announced in `systems`. [`websearch.lua`](https://github.com/ethanhamilthon/tuse/blob/main/examples/websearch.lua)

## Sandbox

Wraps the agent's `sh` with a timeout and a deny list, and removes `delete`. [`sandbox.lua`](https://github.com/ethanhamilthon/tuse/blob/main/examples/sandbox.lua)

## Commands

`/diff`, `/test`, and `/commit` custom commands that use `show()` and `ask()`. [`commands.lua`](https://github.com/ethanhamilthon/tuse/blob/main/examples/commands.lua)

## Open handlers

`open` handlers: Markdown in a `show()` block, images and PDFs in the OS viewer. [`open.lua`](https://github.com/ethanhamilthon/tuse/blob/main/examples/open.lua)

## OpenSpec

One `#openspec` prompt with args (`explore`, `propose`, `apply`, `verify`, `archive`) instead of five prompts. [`openspec.lua`](https://github.com/ethanhamilthon/tuse/blob/main/examples/openspec.lua)

## Project config

A per-project namespaced `tuse.lua` with `history.hide` and a short `models` list. [`project.lua`](https://github.com/ethanhamilthon/tuse/blob/main/examples/project.lua)

## Compaction

`require("compaction")(0.8, 20)` summarizes older conversation records after a finished reply when `context` reaches 80% of `context_window`. It uses `llm` for the summary and `set_session` to replace the older history, retaining the system prompt and recent records without splitting tool calls from their results. [`compaction.lua`](https://github.com/ethanhamilthon/tuse/blob/main/examples/compaction.lua)
