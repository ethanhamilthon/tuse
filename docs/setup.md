# Setup Guide

This guide walks from a fresh Tuse install to a clean, maintainable personal and project configuration.

If you installed with `release/install.sh` interactively, it may already have created `~/.config/tuse/init.lua` with the bundled `tuse/default` plugin (see [install.md](install.md) and [plugins.md](plugins.md)). Extend that file instead of replacing it.

## 1. Connect a Provider Using Environment Variables

Providers and API keys are stored in `~/.tuse/tuse.db`, never in `tuse.lua`. Store an environment variable reference instead of a literal secret by single-quoting `'$VAR'`:

```sh
export OPENAI_API_KEY="sk-..."
tuse connect --base https://api.openai.com/v1 --key '$OPENAI_API_KEY' --name openai
```

- Tuse verifies the endpoint by fetching its models list before saving the provider.
- `$VAR` and `${VAR}` references expand from your process environment at startup and on `/reload`.
- Secrets never appear in `tuse.lua`, `/config`, JSON-RPC `config.get`, or `session()`.
- Manage saved providers with `tuse providers` and `tuse disconnect NAME`.

## 2. Configure Your Global Config

Defaults and the built-in system prompt live in the binary, so Tuse runs without any config file. When you want personal settings, model metadata, prompts, hooks, or plugins, create `~/.config/tuse/init.lua` (or `~/.config/tuse.lua`):

```lua
-- ~/.config/tuse/init.lua
return {
  settings = {
    retries = 5,
  },
  models = {
    {
      name = "openai/gpt-4.1",
      display = "GPT-4.1",
      window = 128000,
      prices = { input = 2, output = 8, cache_read = 0.5, cache_write = 0 },
    },
  },
}
```

Start `tuse` and run `/models` to pick your default model and reasoning effort. Tuse saves that pick in SQLite for future new sessions, while resumed sessions keep their own selection. Check your resolved models and default from the shell at any time with:

```sh
tuse models
```

Compaction runs automatically at 60% of the active model's `window` unless you set `settings.compact_at` or a per-model `compact_at` (`0` disables compaction; a per-model `compact_at` overrides `settings.compact_at` even when global `compact_at` is `0`).

## 3. Split Personal and Project Settings

Tuse loads `--config PATH`, else `./tuse.lua`, else your global config (`~/.config/tuse.lua` or `~/.config/tuse/init.lua`).

To add project-specific instructions, prompts, or overrides without duplicating your personal config, run `tuse init` in the repository root. It creates `./tuse.lua`:

```lua
-- ./tuse.lua
local ok, g = pcall(require, "global")
g = ok and g or {}

g.systems = g.systems or {}
table.insert(g.systems, "Always run `make check` before finishing a change.")

g.prompts = g.prompts or {}
g.prompts.review = "Review staged changes for correctness, missing tests, and edge cases."

g.settings = g.settings or {}
g.settings.retries = 3

return g
```

Best practices for project configs:

- **Trust boundary**: `tuse.lua` executes as Lua with your user permissions. Review any third-party repo's `tuse.lua` before launching Tuse there.
- **No secrets in project files**: keep API keys in shell environment variables and `tuse connect`, never hardcoded in `./tuse.lua`.
- **Safe table initialization**: guard optional tables (`g.prompts = g.prompts or {}`) so the project config still loads if a teammate has no global config yet.

## 4. Organize a Larger Global Config

Once your setup grows, split `~/.config/tuse/` into focused Lua modules loaded with `require` (resolved relative to the calling file):

```text
~/.config/tuse/
├── init.lua      -- entry point returning the merged table
├── models.lua    -- model metadata, windows, and prices
├── prompts.lua   -- reusable # prompts
├── hooks.lua     -- lifecycle hooks
└── helpers.lua   -- local Lua helpers shared by hooks, commands, and functions
```

```lua
-- ~/.config/tuse/init.lua
local t = require("tuse")
local helpers = require("./helpers")
local interactive = t.session().mode == "tui"

return {
  settings = {
    retries = 5,
    ui = {
      diff_line = 20,
      output_line = 4,
    },
  },
  theme = {
    accent = "#FFB86C",
  },
  models = require("./models"),
  prompts = require("./prompts"),
  hooks = require("./hooks"),
  plugins = {
    -- External Git plugin (add while running and run /reload, or git clone first)
    { "ethanhamilthon/tuse/plugins/welcome", ref = "v0.9.2" },
  },
  systems = {
    interactive and "Keep answers concise and actionable." or "Output plain text.",
    "You have `checks(lines)` in `lua`: runs project tests and returns the tail.",
  },
  functions = {
    -- Agent-only helpers (not exposed to hooks or commands; share logic via helpers.lua)
    checks = function(lines)
      return helpers.tail_cmd("make check", lines or 30)
    end,
  },
  open = {
    md = function(path)
      t.sh('open -a Typora "$P"', { env = { P = path } })
    end,
  },
}
```

### Adding External Git Plugins Cleanly

Startup is offline so launching Tuse stays fast and predictable; `/reload` clones and updates enabled Git plugins automatically. When adding a new external Git plugin:

- **In a running session**: add the entry to `plugins` and run `/reload` (Tuse clones the repository and checks out `ref` automatically).
- **Before launching Tuse (offline bootstrap)**: clone it once into `~/.tuse/plugins/...` first—for example, `git clone https://github.com/ethanhamilthon/tuse ~/.tuse/plugins/ethanhamilthon/tuse`—then launch `tuse`.

Use a full 40-character commit SHA as `ref` for an immutable pin (skipped on `/reload` once checked out), a release tag such as `v0.9.2` to follow a named release (checked on `/reload` in case it moved), or omit `ref` to track the default branch on `/reload`.

### Key Habits That Prevent Surprises

- **Use `require("tuse")` in config files and modules**: host functions are not globals outside the agent's `lua` tool.
- **Keep top-level code pure and fast**: top-level evaluation runs on both startup and `/reload` with pre-session `t.session()` (`id == nil`), and side effects run during reload cannot be rolled back if validation fails. Put runtime actions inside hooks, commands, `open`, or `functions`.
- **Branch on `t.session().mode`**: skip TUI-only hooks or desktop notifications when running under `tuse json`.
- **Share helper code via local modules**: configured `functions` are agent-only and undiscoverable by default; put code shared with hooks or commands in a module like `helpers.lua` and document any agent-callable custom functions in `systems` or `prompts`.

## 5. Verify Changes with `/config` and `/reload`

Edit your config files while Tuse is running, then apply and inspect without restarting the session:

1. Run **`/reload`** (or JSON-RPC `config.reload`).
   - Tuse re-reads all modified Lua modules, syncs enabled Git plugins, and strictly validates every key, type, range, and color before switching.
   - If anything is invalid, Tuse reports the exact file and key path (for example, `init.lua: models[1].prices.input: expected a finite non-negative number`) and keeps your previous working config, hooks, and helpers active.
   - A successful reload updates settings, theme, prompts, hooks, commands, `open` handlers, functions, and model metadata while keeping your session history, active model, and running background tasks intact.
2. Run **`/config`** (or JSON-RPC `config.get`).
   - Verify the loaded config file path, resolved settings, theme swatches, system prompts, `#` prompts, functions, hooks, custom `/` commands, `.ext` openers, and loaded plugins with their refs and commit SHAs.

See [config.md](config.md) for the complete key reference and [plugins.md](plugins.md) for plugin options.
