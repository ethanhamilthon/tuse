# Plugin Authoring Guide

A Tuse plugin is a Lua module returning a config table or `function(opts)` that returns one. Tuse validates every plugin strictly and merges plugins underneath the user's config.

## 1. Small Working Example

Create a directory `tuse-git-summary/` with two files:

```lua
-- tuse-git-summary/git.lua
local t = require("tuse")

local M = {}

function M.status(cwd)
  local r = t.sh("git --no-optional-locks status -sb", {
    cwd = cwd,
    env = { LC_ALL = "C" },
    timeout = 3,
  })
  if not r.ok then
    return nil, r.out ~= "" and r.out or "git status failed"
  end
  return r.out
end

return M
```

````lua
-- tuse-git-summary/init.lua
local t = require("tuse")
local git = require("./git")

return function(opts)
  local block = opts.block or "git-summary"
  local pin = opts.pin == true
  local show_on_start = opts.on_start ~= false

  local function render()
    local s = t.session()
    local out, err = git.status(s.cwd)
    local body = out and ("```text\n" .. out .. "```") or ("_" .. err .. "_")
    t.show(block, body, pin)
  end

  local cfg = {
    commands = {
      git_summary = render,
    },
  }

  if show_on_start and t.session().mode == "tui" then
    cfg.hooks = {
      git_summary_start = {
        events = { "start" },
        run = function(e)
          if e.is_new then render() end
        end,
      },
    }
  end

  return cfg
end
````

Users add it to their `plugins` list in `~/.config/tuse/init.lua` and run `/reload` to install it—or clone it beforehand into `~/.tuse/plugins/you/tuse-git-summary` if bootstrapping offline before starting Tuse:

```lua
plugins = {
  -- ...existing plugins...
  { "you/tuse-git-summary", ref = "v0.1.0", opts = { pin = false } }, -- example release tag
}
```

## 2. Plugin Layout

Tuse looks for `init.lua` first, then `tuse.lua`, inside the plugin directory:

- **Single-plugin repository**: place `init.lua` at the repository root so users load `"owner/repo"`.
- **Multi-plugin repository (monorepo)**: place each plugin in a subfolder such as `plugins/my-plugin/init.lua` so users load `"owner/repo/plugins/my-plugin"` (or `{ "https://...", path = "plugins/my-plugin" }`).
- **Sibling modules**: import local files with `require("./helper")` or `require("helper")`; paths resolve relative to the calling file and cache by canonical path, so your `util.lua` never collides with another plugin's `util.lua`.
- **Static assets**: use `os.getenv("THIS_PATH")` to locate sibling non-Lua files relative to your plugin file.

## 3. Options and Defaults

- If your plugin returns a `function(opts)`, Tuse always passes a table (`{}` when the user omits `opts`).
- Apply explicit defaults for every option; check boolean defaults carefully (`opts.on_start ~= false` when defaulting to `true`, `opts.pin == true` when defaulting to `false`).
- Validate bad option types early with a clear `error("my-plugin: opts.timeout must be a positive number", 0)`.
- Never mutate `opts` directly; copy or normalize into local variables.

## 4. Config Merge Rules and Collision Safety

Plugins load and validate in list order, then the user config merges on top:

| Config section | Merge behavior | Authoring guideline |
| --- | --- | --- |
| `systems`, `models` | Concatenated (plugins first, then user); last model with the same `name` wins. | Keep `systems` short and opt-in; avoid bloating every turn's prompt. |
| `prompts`, `functions`, `hooks`, `commands`, `open` | Merged by key. **Two plugins defining the same key is an error.** User config silently overrides any plugin key. | Namespace keys with your plugin name (e.g., `git_summary`, `myplug_check`) so two plugins do not collide. |
| `settings`, `theme` | Deep-merged table-by-table; later plugins override earlier ones; user wins over all plugins. | Only set `theme` or `settings` if that is the plugin's primary purpose. |
| `plugins` | Forbidden inside plugins. | Document peer/optional dependencies in your README instead. |

Additional rules:

- Returning unknown top-level or nested keys fails strict validation with `plugin <name>: <path>: <message>`.
- Configured `functions` are **agent-only and undiscoverable by default**: they are not visible to hooks, commands, or other configured functions (share code via `local` functions or `require("./...")`), and they are not added to the system prompt automatically. If your plugin provides an agent function, either add a concise opt-in `systems` note or document how users can reference it in their own prompts.
- Do not override or disable built-in host functions (`sh`, `read`, etc.) unless that is the explicit, documented purpose of the plugin.

## 5. Safe Side Effects

Top-level plugin evaluation runs during both startup and `/reload`, under pre-session `session()` semantics (`t.session().id == nil`):

- **Keep top-level load pure and fast**: do not make network requests, spawn daemons, write files, or run slow shell commands at top level (note that during `/reload` the session already exists, so top-level side effects cannot be rolled back if validation later fails).
- **Defer session work to callbacks**: functions requiring a live session (`show`, `pick`, `ask`, `spawn`, `llm`, `reload`, `set_title`, `write`, `edit`) belong inside hooks, commands, `open` handlers, or `functions`.
- **Guard TUI vs JSON mode**: check `t.session().mode == "tui"` before registering TUI-only visuals or OS notifications.
- **Bound shell commands and quote safely**: pass `{ timeout = N }` to `t.sh(...)` and pass untrusted strings via `env = { P = path }` rather than interpolating into shell syntax.
- **Clean up on `close`**: if your plugin starts external background processes or temporary resources, register a `close` hook to stop them cleanly. Never call `pick()` or `ask()` inside `close` hooks (interactive UI is unavailable during reload and close).
- **Avoid `lua_start`/`lua_end` feedback loops**: tasks spawned inside `lua_start`/`lua_end` hooks are excluded from re-triggering those hooks, but keep observers lightweight and non-blocking.

## 6. Local Testing

Develop against a local working copy using `dir` (no Git push required):

```lua
-- test-tuse.lua
return {
  plugins = {
    { "git-summary", dir = "~/code/tuse-git-summary", opts = { pin = true } },
  },
}
```

Test loop:

1. Start Tuse with `tuse --config test-tuse.lua`.
2. Edit files in `~/code/tuse-git-summary` and run `/reload` — `dir` plugins re-read straight from disk without Git or network calls.
3. Inspect the merged result in `/config` and verify hook timing and errors in `/trace`.
4. Test edge cases:
   - Missing `opts` (`{ "git-summary", dir = "..." }`).
   - Both front ends: TUI (`tuse -c test-tuse.lua`) and JSON-RPC (`tuse json -c test-tuse.lua`).
   - Reloading twice in a row (ensures `close` cleans up cleanly and `start` with `reason = "reload"` behaves properly).
   - Resuming a session (`--resume`, `/resume`) where `start` fires with `is_new = false, restored = true`.
   - User overrides (e.g., user overriding your command or disabling your function with `false`).

## 7. Publishing and Pinning Releases

1. Host the repository on GitHub (`owner/repo`) or any Git server reachable via HTTPS or SSH.
2. Tag releases with semantic versions (for example, `git tag v0.1.0 && git push --tags`).
3. Document installation and ref choices in your README:
   - **Install via `/reload` or offline clone**: tell users either to add the entry while Tuse is running and run `/reload`, or to run `git clone https://github.com/owner/repo ~/.tuse/plugins/owner/repo` before starting Tuse.
   - **Immutable commit pin**: `{ "owner/repo", ref = "<40-char-sha>" }` — truly immutable; once checked out, `/reload` skips fetching and keeps that exact commit fixed.
   - **Release tag ref**: `{ "owner/repo", ref = "v0.1.0" }` — follows a named release tag (still checked on `/reload` in case the tag was moved upstream; tags are not immutable pins).
   - **Branch / default branch**: omitting `ref` or setting `ref = "main"` updates automatically on `/reload` whenever upstream advances.
4. **Local edits safety**: `/reload` never overwrites local changes or unpublished commits in a checkout under `~/.tuse/plugins/`; document `dir = "..."` for contributors hacking locally.
5. **Monorepo note**: if a repository provides multiple plugins under `plugins/*`, every enabled entry from that repository in a user's config must agree on both URL and `ref` (shared repositories sync once per reload).

See [plugins.md](plugins.md) for the complete plugin specification and [hooks.md](hooks.md) for hook payloads.
