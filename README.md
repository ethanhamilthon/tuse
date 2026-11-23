# Tuse

A minimal AI coding agent for the terminal. Rust + Crossterm, any OpenAI
Chat Completions compatible API, one tool: `lua`.

- Full-screen TUI with Markdown rendering and pickers for `/` commands, `#` prompts and `@` files.
- The agent works in a persistent Lua 5.4 VM with `read()`, `write()`, `edit()`, `grep()`,
  `sh()`, `sleep()` and `spawn()`. Each call has a 120-second deadline; output is not capped.
- One VM per session runs the config, the agent's code, hooks and background tasks as
  coroutines: while one waits on `sh`, the others go on.
- `read()` shows images to the model; files changed by `write()` and `edit()` are shown as a
  colored diff at the end of the call; Lua code is syntax highlighted.
- A JSON-RPC mode (`tuse json`) for editors; sessions, providers and the last picks in SQLite
  (`~/.tuse/tuse.db`).
- 50/25/25 context compaction, retries.
- The config is a Lua file that returns a table: hooks and helpers are plain Lua functions.

## Benchmark

Tuse, Claude Code and pi on 5 hard coding tasks (a SQL engine, an MVCC store, a regex engine, a
dependency solver, a cron scheduler), 6 runs each, 90 runs in total. Model `claude-haiku-5-5` at
low effort behind one proxy, every agent in its own TUI inside `tmux`, hidden unit tests
(107 per round) grade each run, 360 s limit.

| Agent | Full pass | Avg score | Avg time | Input tokens / run | Est. cost / run |
| --- | ---: | ---: | ---: | ---: | ---: |
| Tuse 0.5.1 | 20/30 | 98.5% | 135 s | 142k | $0.20 |
| Claude Code 2.1.329 | 21/30 | 98.6% | 174 s | 462k | $0.30 |
| pi 1.5.2 | 13/30 | 72.3% | 160 s | 264k | $0.26 |

Tuse matches Claude Code on quality (one test of 150 apart on `mini_sql`), finishes 22% faster, sends 3.3x fewer
input tokens (a system prompt of about 2k tokens against about 20k) and costs 32% less. pi lost
points on one failure: on `mini_sql` the model spent all 32,000 reasoning tokens and the reply was
cut (`max_tokens`), so nothing was written (0/6); on the other tasks pi scores the same as the rest.
Cost is an estimate at Haiku-class prices. One model, one setup.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/ethanhamilthon/tuse/main/release/install.sh | sh
```

The script installs `tuse` into `~/.local/bin` without prompting or changing your config.
`TUSE_VERSION=0.7.3` pins a release; `TUSE_BIN_DIR` changes the destination.

Add a provider explicitly; quote `$OPENAI_API_KEY` to store a reference instead of the key:

```sh
tuse connect --base https://api.openai.com/v1 --key '$OPENAI_API_KEY' --name openai
```

`tuse update` replaces the binary without touching your config. You can also build from
source with `cargo install --locked --git https://github.com/ethanhamilthon/tuse tuse`,
or run `make install` from a checkout. The defaults are built in.

## Usage

```
tuse [-c|--config PATH] [-r|--resume ID] open the TUI
tuse json [-c PATH] [-r ID]               JSON-RPC 2.0 over stdin/stdout (docs/json.md)
tuse connect --base URL --key KEY --name NAME
                                          verify and save a provider
tuse providers | tuse disconnect NAME     list | remove providers
tuse update                              update the binary
```

`--config` picks the config file (else `./tuse.lua`, else the global config). `--resume ID`
opens a saved session: the ID is printed to stderr when Tuse exits.

The agent's tool is `lua(code)`, a persistent Lua 5.4 VM with these functions:

- `read(path)`: numbered text lines in pages, or an image the model sees. `write(path, content)`
  creates or replaces a file atomically. `edit(path, old, new)` replaces exact, unique text. The
  user sees what `write` and `edit` changed as a diff at the end of the call.
- `grep(pattern, {path = "."})`: a regex search that respects `.gitignore`.
- `sh(cmd)` runs `bash -c` and returns `.out`, `.code`, `.ok`; it is killed at its timeout.
- `spawn(fn, name)` runs a function as a background task in the same VM; `task_get`, `task_stop`
  and `task_list` manage them (listed in `/tasks`; the agent gets a message when one ends).

The VM lives as long as the process. A call is stopped after 120 seconds.

`tuse connect` verifies the provider by fetching its models, then saves it in `~/.tuse/tuse.db`.

### TUI

| Key | Action |
| --- | --- |
| Enter | send |
| Shift+Enter, Alt+Enter, Ctrl+J | new line |
| Esc | close the picker, or stop the running turn |
| mouse wheel | scroll the chat (or the open list) |
| Up / Down, Left / Right, Home / End | move in the input |
| click in the input | put the cursor there |
| hover / click a link | highlight it / open it in the browser |
| Ctrl+V, Cmd+V | paste; an image becomes `[Image #n]` in the input and is attached when the message is sent |
| Ctrl/Cmd+C | copy selected text; otherwise clear the input or quit if empty |
| SIGINT, SIGTERM, SIGHUP | quit and reset the terminal (SIGKILL cannot; run `reset` after it) |
| Ctrl/Cmd+A / Ctrl/Cmd+X | select all input / cut selected input |
| Shift+arrows / Shift+Home/End | extend the input selection |
| Home / Ctrl+E / Ctrl+U / Ctrl+K / Ctrl+W | line start / end / kill back / kill forward / delete word |

Shortcuts also work with the Russian keyboard layout when the terminal reports modified Cyrillic keys (for example, with the kitty keyboard protocol).

A message sent while the agent is working stays dimmed in the input until the agent takes it.
Editing it withdraws it from the queue without interrupting the turn. Press Enter to send
the edited draft. Esc still stops the turn.

Commands: `/new`, `/resume`, `/models`, `/tasks`, `/usage`, `/trace`, `/config`, `/reload`,
`/quit`. `/trace` opens a scrollable log of the conversation, tool calls and hooks. Pickers
filter by substring; the input becomes the search box and restores your draft on close.

`#name` inserts a prompt from the config; `@` picks a file or folder from the current folder
(`@dir/name` filters the entries inside `dir`), or by absolute path: `@/etc/` and `@~/` list those
folders as you type. The message keeps `#name` and `@path` as typed; the prompt
text and the file go to the model in a hidden block ([docs/prompts.md](docs/prompts.md)).
Files, prompts and images are one unit in the input: arrows and Backspace move over them whole.
`/models` remembers your choice in the database; it wins over the config file.

## Config

Tuse runs `--config PATH`, else `./tuse.lua`, else the global config (`$TUSE_GLOBAL_CONFIG`,
`~/.config/tuse.lua` or `~/.config/tuse/init.lua`). See [docs/config.md](docs/config.md) for every key.

```lua
local t = require("tuse")              -- sh, read, edit... and t.env.TUSE_MODE
local shared = require("shared")       -- ./shared.lua or ./shared/init.lua

return {
  include = { t.env.TUSE_GLOBAL_CONFIG }, -- optional: merge the global config
  model = "x/claude-sonnet-5-5",
  effort = "high",                     -- default | low | medium | high | xhigh | max
  systems = shared.systems,

  models = {
    { name = "x/claude-sonnet-5-5", display = "Sonnet 5.5", window = 1000000,
      compact_at = 600000, prices = { ["in"] = 3, out = 15, cache = 0.3, cache_write = 3.75 } },
  },

  functions = {
    checks = {
      args = "lines",
      description = "Run project checks and return the last lines.",
      run = function(lines)
        local out = sh("make check"):lines()
        return table.concat(out, "\n", math.max(1, #out - (lines or 30) + 1))
      end,
    },
  },

  hooks = {
    notify = {
      events = { "agent_message" },  -- process_start, process_end, user_message, agent_message
      run = function(e)
        if e.status == "finished" then
          sh([[osascript -e 'display notification "done"']])
        end
      end,
    },
  },
}
```

## Development

The workspace is split into crates: `config`, `api`, `lua`, `core`, `tui`, `rpc` and
`cli`. The Rust toolchain is pinned in `rust-toolchain.toml`. See [AGENTS.md](AGENTS.md)
for the rules and run `make check` before committing. Release builds target macOS 11+
and Linux with glibc 2.35 or newer.

## License

MIT
