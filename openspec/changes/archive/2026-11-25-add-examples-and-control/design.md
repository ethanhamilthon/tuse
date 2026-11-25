# Design

## Examples
`examples/<name>.lua`, one self-contained module each, copied into `~/.config/tuse/` and loaded with `require`. The no-comments rule is for Rust, so each example may start with a few `--` lines on how to use it.

List:
| File | Shows |
| --- | --- |
| `skills.lua` | Reusable prompts, loading `SKILL.md` folders into `prompts`. |
| `mcp.lua` | Wrapping an MCP server (stdio JSON-RPC via `sh`/`spawn`) as `functions`. |
| `themes.lua` | A few full themes (light, gruvbox, solarized) to pick from. |
| `rename.lua` | Auto title with `llm()` on the first `user_message`, plus `/rename`. |
| `notify.lua` | Desktop notification on `agent_message` finished (macOS and Linux). |
| `tmux.lua`, `cmux.lua`, `herdr.lua`, `orca.lua`, `zellij.lua` | Pane/tab title and status from hooks for each multiplexer. |
| `websearch.lua` | A `websearch` function with a `systems` line. |
| `sandbox.lua` | Overriding `sh` with a timeout and a denylist, disabling `delete`. |
| `commands.lua` | `/commit`, `/review`, `/test` custom commands with `show()`. |
| `open.lua` | `settings.open` handlers for `md`, `png`, `pdf`. |
| `openspec.lua` | One `#openspec` prompt with args (below). |
| `rex.lua` | Rex block label and program status from hooks (`rex block rename`, `REX_BLOCK`). |
| `project.lua` | A per-project `tuse.lua` built on `require("global")`. |

`docs/examples.md` has one section per file: one sentence and a link `https://github.com/ethanhamilthon/tuse/blob/main/examples/<file>`. A test checks that every file in `examples/` has a link and every link points to an existing file.

## Task source
`spawn(fn)` reads `debug.getinfo(fn, "S")`. If `source` starts with `=`/`@` for a chunk Tuse loaded, Tuse keeps the chunk text it loaded (tool calls, config files) and slices `linedefined..lastlinedefined`. File chunks are read from disk. A handed-off tool call is gone (no more handoff), so the only code kinds are functions. Unknown or C functions give `(source unavailable)`. The source is stored on `TaskInfo.code` and returned by `task_get`, `tasks.list`, `tasks.get`.

## Trace roles
Fixed list, one role per thing that happens: `system`, `user`, `thinking`, `agent`, `lua`, `output`, `notification`, `notice`, `error`, `hook`, `task`, `command`, `open`, `compaction`, `usage`, `model`, `turn`, `title`, `show`, `pick`, `ask`. `task`, `command`, and `open` come from a new `Record::Job {id, kind, name, code, status, ms, error}` written when one ends. `title` comes from a new `Record::Title {previous, new}`. `trace()` matches `Record` without a `_` arm, so a new record kind does not compile until it is traced.

`hide_trace` only filters the TUI `/trace` list. `session().trace` and `session.trace` stay complete.

## scope_models
Entries are `provider/model` or `provider/*`. Non-empty means `/models` shows only matching names; the current model is always shown. It is a UI setting: `model.list` and `model.set` are unaffected.

## Prompt args
A `prompts` value is a string or a table `{ args?, run }`:

```lua
prompts = {
  review = "Check the change for bugs.",
  today = { run = function() return "Today is " .. os.date("%F") end },
  openspec = {
    args = { "explore", "propose", "apply", "verify", "archive" },
    run = function(arg) return require("./openspec").body(arg) end,
  },
}
```

- `args` is an optional list of unique non-empty strings without spaces. `run(arg)` gets the chosen string, or `nil` when the prompt has no `args`. It must return a string, the prompt body.
- TUI: picking `#openspec` from the `#` picker inserts the prompt atom and opens a second picker with its `args`. Typing a space after a prompt atom with `args` reopens it, filtered by what is typed. The chosen arg joins the atom: `#openspec propose`. Esc leaves the atom without an arg. A prompt with `args` sent without one gets `run(nil)`.
- The prompt atom and its arg are drawn in the accent color.
- On send, `run` runs as a config-side Lua job (no deadline, Esc cancels). The body goes into `<prompt name="openspec" arg="propose">…</prompt>`. An error or non-string result drops the send, keeps the draft, and shows the error.
- JSON-RPC: `turn.submit` `prompts` items are `"name"` or `"name arg"`. An arg not in `args` fails with `-32000`. `config.get` lists each prompt as `{kind: "text" | "function", args}`.
- One `#openspec` replaces five `openspec-*` prompts.

Rejected: typed arg schemas (more than needed for picking an action), LuaLS `---@param` comments (fragile).

## Stop
One `Session::stop_all()` used by Esc, `turn.cancel`, and `/stop`: cancel the turn and queued input, kill the running `lua` call (result `[cancelled]`), cancel other calls (`[interrupted: no output]`), stop all running tasks, hooks, and commands, close the active `pick`/`ask`. Steering keeps the handoff so the agent can answer at once.

Finished tasks push their `<task>` notice into a pending list. The next user message takes them all, recorded as `notification` records right before the `user` record. Stopped tasks still send nothing.
