# Planned: Agent-Only, Undiscoverable Configured Functions (TODO #9)

## Goal

Simplify configured `functions` to plain Lua functions (`functions = { websearch = function(...) ... end }`), scope them strictly to the agent's `lua` tool environment without automatically exposing them in the system prompt or agent-facing docs, allow overriding built-in host functions with clean non-recursive delegation to originals, and allow disabling built-ins for the agent across all aliases, cached references, and agent-readable docs.

> Note: Configured `functions` overrides and disables are **capability-scoping and ergonomic controls** for cooperative agents, not a hostile-code security sandbox (the agent still runs standard Lua 5.4 libraries such as `os.execute` and `io.open`).

## Target Behavior

### Config Schema

`functions` maps a Lua identifier to either a **Lua function** or **`false`** (`nil` is indistinguishable from omitting the key):

```lua
local t = require("tuse")

return {
  functions = {
    -- Custom agent-only function (undiscoverable unless described in systems/prompts)
    websearch = function(query)
      return t.sh('curl -fsSL "$URL"', { env = { URL = "https://example.com/?q=" .. query } }).out
    end,

    -- Override a built-in for the agent while delegating to the original (e.g., default timeout/env)
    sh = function(cmd, opts)
      opts = opts or {}
      local env = { CI = "1" }
      for k, v in pairs(opts.env or {}) do env[k] = v end
      return t.sh(cmd, { timeout = opts.timeout or 30, cwd = opts.cwd, env = env })
    end,

    -- Disable a built-in completely for the agent
    delete = false,
  },
}
```

- **Allowed keys**: valid Lua identifiers (`[_A-Za-z][_A-Za-z0-9]*`) excluding:
  - Lua keywords (`and`, `break`, `do`, `else`, `elseif`, `end`, `false`, `for`, `function`, `goto`, `if`, `in`, `local`, `nil`, `not`, `or`, `repeat`, `return`, `then`, `true`, `until`, `while`), and
  - **Decision:** Lua base/stdlib globals (`_G`, `_VERSION`, `assert`, `collectgarbage`, `dofile`, `error`, `getmetatable`, `ipairs`, `load`, `loadfile`, `next`, `pairs`, `pcall`, `print`, `rawequal`, `rawget`, `rawlen`, `rawset`, `require`, `select`, `setmetatable`, `tonumber`, `tostring`, `type`, `warn`, `xpcall`, `coroutine`, `io`, `math`, `os`, `package`, `string`, `table`, `utf8`), so `functions` can never shadow or disable core Lua globals such as `os` or `package`.
  - All built-in host names (`sh`, `sleep`, `spawn`, `read`, `write`, `append`, `delete`, `edit`, `grep`, `docs`, `set_title`, `session`, `task_get`, `task_list`, `task_stop`, `show`, `pick`, `ask`, `reload`, `llm`, `json`, `inspect`) **are allowed**.
- **Allowed values**: Lua `function` or boolean `false` (or `nil`, meaning omitted).
  - **Decision (strict `false` target check):** `false` is valid ONLY on a built-in host name OR a function name defined by a loaded plugin in the current merged config (allowing a user to disable a plugin's configured function). Setting `false` on an unknown name (neither a built-in nor provided by an enabled plugin) fails strict validation so typos are caught immediately.
  - **Decision (`json` table contract):** `json` is a module table (`{ encode, decode }`), not a function. `functions = { json = false }` is allowed and disables `json` for the agent; setting `functions = { json = function(...) ... end }` fails validation (`functions.json: 'json' is a table and can only be disabled with false`).
  - **Decision (hard break from old schema):** Passing any value other than `function` or `false` (including the removed `{ run, args, description, show_agent }` table form) fails validation with an actionable migration message.

### Undiscoverability by Default

- Configured functions are **never** listed automatically in the system prompt (the `# Your functions in lua` section is removed).
- Configured functions are **never** appended to `docs("functions")`.
- Users who want the agent to call a configured function describe it explicitly in `systems` or `prompts`.
- **User-facing inspection**:
  - **Decision:** `/config` lists configured `functions` for the human user without Lua bodies: `<name>` for custom functions, `<name>  [override]` for built-in overrides, and `<name>  [disabled]` for `false` entries.
  - **Decision:** JSON-RPC `config.get` reports `functions` as a map of name to `"custom" | "override" | "disabled"`, replacing the old `{ args, description, show_agent }` object.

### Strict Environment Isolation and Callback Provenance

Tuse maintains two isolated Lua environments within the session VM:

1. **Trusted environment** (config files, plugins, modules `require`d by trusted code, hooks, custom `commands`, `open` handlers, configured `functions` bodies themselves, and background tasks spawned by them):
   - Built-in host functions are **not** globals; they are imported via `require("tuse")`, which **always** returns the unmodified original built-in host table.
   - Configured `functions` are **never** visible here — not as globals, not on `require("tuse")`, and not to other configured functions.
   - Code shared between configured functions, hooks, or commands uses plain Lua `local` functions or `require`d helper modules.
2. **Agent environment** (`lua(code)` tool calls, modules `require`d from `lua(code)`, `load`/`loadfile`/`dofile` called from `lua(code)`, and background tasks `spawn`ed by `lua(code)`):
   - Contains the safe Lua 5.4 standard libraries (without `debug` or `os.exit`), live-gated built-in host functions (subject to overrides and `false` disables), live-gated configured custom functions, and the agent's own persistent globals across calls.
   - **Isolated shared tables**:
     - **Decision:** `_G`/`_ENV`, `package`, `package.loaded`, `package.preload`, `require`, `load`, `loadfile`, `dofile`, and `tuse` are strictly separate between trusted and agent environments. Agent code cannot inspect or mutate trusted `package.loaded`, cannot share module cache entries with trusted config/plugin code, and cannot obtain the raw trusted `tuse` table via `require("tuse")` or `package.loaded.tuse`. (Standard library tables such as `string`/`table`/`math` remain ordinary Lua 5.4 libraries; this boundary scopes config capabilities rather than sandboxing hostile Lua.)
   - **Agent `require("tuse")` proxy**:
     - Returns a live, read-only agent-scoped proxy table (also installed as `package.loaded.tuse` in the agent's isolated `package.loaded`). Lookups on this table dynamically read the session's current active agent capability map (overrides applied, disabled built-ins returning `nil`, custom non-built-in configured functions **not** exposed on `tuse`).

#### Callback Provenance and Spawned Closures

- **Lexical `_ENV` rule**: every Lua function's globals and `require` binding are determined lexically where the function was defined (standard Lua semantics):
  - If the agent passes a callback `cb` into a configured wrapper `wrap(cb)`, `cb` retains the **agent `_ENV`** (it cannot see trusted `require("tuse")` or call disabled built-ins), while `wrap` executes with the **trusted `_ENV`**.
  - If a configured wrapper returns a closure created in trusted config code back to the agent, that closure retains its trusted lexical `_ENV` (explicitly delegated by the config author).
- **Spawned task ownership (`spawn` vs `t.spawn`)**:
  - **Decision:** A task spawned by the agent's `spawn(fn, name)` runs in the **agent job context**; `fn` retains its lexical `_ENV` (agent `_ENV` for agent closures, seeing agent globals, configured functions, overrides, and disabled built-ins).
  - A task spawned by trusted code (config, hook, command, `open`, or inside a configured function) via `t.spawn(fn, name)` runs in the **trusted job context**; trusted closures retain trusted `_ENV` (no configured functions, unmodified `require("tuse")`). If a configured function passes an agent-created closure `cb` to `t.spawn(cb)`, `cb` still has agent `_ENV` and cannot bypass disabled built-ins or reach trusted `require("tuse")`.

### Overriding Built-Ins and Wrapper Delegation

- Any function-valued built-in in `functions` (all `HOST_FUNCTIONS` except table-valued `json`) replaces both global `<name>` and `require("tuse").<name>` in the agent environment.
- **Documented original handle**:
  - **Decision:** `local t = require("tuse")` in trusted config/plugin code is the documented handle to original built-ins. Calling `t.sh(...)` / `t.read(...)` inside a wrapper invokes the real built-in without recursing and without exposing `t` to the agent.
- When a wrapper executes during an agent `lua(code)` tool call and delegates to `t.write(...)`, `t.read(...)`, `t.sh(...)`, `t.pick(...)`, etc., job-scoped side effects (diffs, model images, 120-second deadline, timeout pausing, Esc cancellation) remain bound to the active agent tool job.
- Overrides never alter what config code, plugins, hooks, commands, `open` handlers, or trusted tasks call via `require("tuse")`.

### Disabling Built-Ins (`false`), Saved Aliases Across Reload, and Private Runtime Helpers

- **Live-gated handles (revoking saved aliases across reload)**:
  - **Decision:** Every built-in function, `json.encode`/`json.decode`, and configured `functions.<name>` entry exposed to the agent is wrapped in a thin live-dispatch proxy backed by the session's active config:
    - If the agent caches `saved_sh = sh`, `saved_enc = json.encode`, `saved_tuse = require("tuse")`, or `saved_fn = plugin_fn`, and a subsequent reload sets `sh = false`, `json = false`, or `plugin_fn = false` (or removes `plugin_fn`), then:
      - `saved_tuse.sh` and `saved_tuse.json` immediately read `nil`;
      - calling `saved_sh(...)`, `saved_enc(...)`, or `saved_fn(...)` immediately raises `"<name> is disabled"` (or `"<name> is unavailable"` if removed) instead of executing the stale closure;
      - if a reload installs a new override wrapper for `sh`, calling `saved_sh(...)` dispatches to the new active wrapper.
- **Private runtime helpers stay intact**:
  - Disabling `inspect = false`, `json = false`, `sh = false`, etc. for the agent **never** breaks Tuse's own internal runtime helpers (return-value rendering via private `inspect`, `sh` result formatting, session snapshotting, or trusted hooks/commands).
- **Agent-facing docs stay in sync everywhere**:
  - **Decision:** Disabled built-ins are filtered out of **every** agent-facing capability surface:
    1. **System prompt**: `# Tool`, `# Rules`, and `# Functions in lua` omit disabled built-ins (including `spawn` / `json` / `inspect` / `grep` / `sh` mentions in `# Tool`).
    2. **`lua` tool schema description**: omits mentions of disabled built-ins (`read()`, `grep()`, `write()`, `edit()`, `append()`, `delete()`, `sh(cmd)`).
    3. **`docs("functions")`**: omits every disabled built-in entry (and never lists configured custom functions).
    4. **`docs("tool")` and all other embedded `docs(topic)` pages**: `docs("tool")` filters out disabled built-ins; other embedded topics (`README.md`, `commands`, `config`, `json`, `hooks`, `plugins`, etc.) either omit disabled-only sections or prepend an explicit session notice listing disabled built-ins (`Disabled in this session: ...`) so no agent-readable doc instructs the agent to call a disabled function. If `docs = false`, `docs` is disabled and omitted from the system prompt.
  - Overridden (not disabled) built-ins remain listed in agent-facing docs.

### Plugin Merging and Reload Semantics

- **Plugin merging**: two plugins defining the same key in `functions` (whether function or `false`) remain a merge error (`plugins A and B both define functions.<name>`). User config silently overrides any plugin entry (including disabling a plugin's function with `false`).
- **On successful reload**:
  - New/updated configured functions and overrides activate immediately in the agent environment (and in live-gated saved aliases).
  - Removed custom functions are unbound from the agent global environment (unless the agent rebound that global to a different value), and any saved handle to the removed function fails when called.
  - Removed built-in overrides or `false` disables restore the default built-in across globals, `require("tuse")`, saved aliases, the system prompt, the `lua` tool description, and all `docs()` topics.
  - Agent scratch globals persist unchanged across reload.
- **On failed reload**: nothing changes; previous functions, overrides, disables, and docs filtering remain active.

## Error Reporting

- Table passed for a function entry: `<source>: functions.<name>: expected function or false, got table (the { run, description, args, show_agent } form was removed; pass a function directly or false to disable)`
- Function passed for `json`: `<source>: functions.json: 'json' is a table and can only be disabled with false`
- Other invalid value type: `<source>: functions.<name>: expected function or false, got <type>`
- `false` on unknown name: `<source>: functions.<name>: cannot disable unknown function`
- Invalid or reserved key (keyword or Lua stdlib global): `<source>: functions: '<name>': invalid Lua name` or `<source>: functions: '<name>': reserved name`

## Migration from `run`, `args`, `description`, and `show_agent`

### 1. Agent-callable function

Before:
```lua
local t = require("tuse")
return {
  functions = {
    checks = {
      args = "lines",
      description = "Run project checks and return the last lines.",
      run = function(lines)
        local out = t.sh("make check"):lines()
        return table.concat(out, "\n", math.max(1, #out - (lines or 30) + 1))
      end,
    },
  },
}
```

After (explicitly documented in `systems` if you want the agent to discover it):
```lua
local t = require("tuse")
return {
  systems = {
    "You have `checks(lines)` in `lua`: runs `make check` and returns the last `lines` lines (default 30).",
  },
  functions = {
    checks = function(lines)
      local out = t.sh("make check"):lines()
      return table.concat(out, "\n", math.max(1, #out - (lines or 30) + 1))
    end,
  },
}
```

### 2. Hidden helper (`show_agent = false`) shared with hooks/commands/functions

Move internal helpers out of `functions` into a `local function` in `tuse.lua` or a shared module imported with `require("./helpers")`.

### 3. Configured function calling another configured function

Extract shared logic into a `local` helper or sibling module; configured functions cannot see other configured functions as globals.

### 4. Wrapping or disabling built-ins (newly supported)

```lua
local t = require("tuse")
return {
  functions = {
    sh = function(cmd, opts)
      opts = opts or {}
      return t.sh(cmd, { timeout = opts.timeout or 30, cwd = opts.cwd, env = opts.env })
    end,
    delete = false,
  },
}
```

## Docs-First Handoff Gate

Per TODO #3, keep baseline docs describing current behavior until ready to implement this spec. Before handing off to an implementation agent, update and review **all** affected public docs, embedded docs, tool/prompt text, and shipped examples first:

- `docs/functions.md`: replace "Functions from Config" with plain function / `false` syntax, agent-only isolation, live-gated aliases, built-in overrides via `require("tuse")`, `json = false` rules, and dynamic docs filtering.
- `docs/config.md`: update `functions` key description, reserved-name validation, `/config` display, and reload behavior.
- `docs/lua.md`: document isolated trusted vs agent environments, isolated `package`/`require`, lexical callback provenance, and `spawn` vs `t.spawn` ownership.
- `docs/tool.md`, built-in system prompt, `lua` tool schema description, and embedded `docs()` topics: align capability descriptions and disabled-function filtering.
- `docs/hooks.md`: remove mention that hooks can call configured helpers; point to `local`/module helpers.
- `docs/plugins.md`: document plugin `functions` merge/disable rules.
- `docs/json.md`: update `config.get` `functions` map schema.
- Shipped examples (including `README.md` config example during #13 or as part of this handoff): update away from `{ run, args, description }`.

## Acceptance Tests

1. **Plain function syntax**: `functions = { websearch = function(q) return "res:" .. q end }` works in `lua(code)`.
2. **Validation & reserved names**: old `{ run, description, args, show_agent }` table fails with migration error; `functions = { os = false }`, `functions = { package = false }`, `functions = { require = fn }`, `functions = { typo = false }`, and `functions = { json = function() end }` all fail validation cleanly.
3. **Undiscoverable by default**: neither system prompt nor `docs("functions")` mentions configured custom functions.
4. **Strict environment isolation**: configured `websearch` is `nil` in top-level config, plugins, hooks, custom `commands`, `open` handlers, another configured function, and trusted `t.spawn` tasks. Agent mutating `package.loaded` does not affect trusted `require`.
5. **Callback provenance & spawned closures**: wrapper receiving an agent callback `cb` runs `wrap` with trusted `_ENV` while `cb` stays in agent `_ENV` (cannot see disabled `sh`); `t.spawn(cb)` inside a wrapper keeps `cb` in agent `_ENV`; agent `spawn(fn)` keeps `fn` in agent `_ENV`.
6. **Built-in override & delegation**: overriding `sh` and `write` routes agent `sh(...)` and `require("tuse").sh(...)` to the wrapper; wrapper delegates to `t.sh`/`t.write` without recursing; diffs/images/deadline bind to the agent tool call; hooks/commands/plugins still call unmodified built-ins.
7. **Disabled built-ins, saved aliases, and private helpers**: with `sh = false`, `read = false`, `json = false`, `inspect = false`:
   - `sh`, `_G.sh`, `require("tuse").sh`, `package.loaded.tuse.sh`, `load(...)`, agent-`require`d `.lua` files, and agent-`spawn`ed tasks all see `nil`.
   - Saved aliases captured **before** reload (`saved_sh = sh`, `saved_enc = json.encode`, `saved_tuse = require("tuse")`, `saved_plug = plugin_fn`) immediately observe the reload (`saved_tuse.sh == nil`; calling `saved_sh()`/`saved_enc()`/`saved_plug()` raises disabled/unavailable error).
   - Returning a table `{ a = 1 }` from `lua(code)` while `inspect = false` still renders `{a = 1}` using Tuse's private runtime `inspect`.
8. **All agent-readable docs stay in sync**: with `sh = false` and `delete = false`, verify the system prompt, `lua` tool description, `docs("functions")`, `docs("tool")`, and other embedded `docs(topic)` outputs do not advertise `sh` or `delete`.
9. **Reload restores defaults & failed reload preserves state**: removing overrides/`false` disables on reload restores original built-ins everywhere (globals, `require("tuse")`, saved aliases, prompt, tool description, `docs()`); failed reload keeps prior state intact.
10. **Plugin merge & user disable**: plugin defines `functions = { tool_a = fn }`; user sets `functions = { tool_a = false }`; `tool_a` is disabled for agent (including saved aliases across reload). Two plugins defining `tool_a` fail merge.
