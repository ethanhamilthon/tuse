# Tuse

A minimal AI coding agent for the terminal. Rust + Crossterm, any OpenAI
Chat Completions compatible API, one tool: `python`.

- Full-screen TUI with Markdown rendering and pickers for `/` commands, `#` prompts and `@` files.
- The agent works in a persistent Python REPL ([Monty](https://github.com/pydantic/monty), a
  Python subset in Rust) with `read()`, `write()`, `edit()`, `grep()`, `sh()`, `ask()`, `draw_ui()`
  and background `task_*()` functions.
- Hooks are Python too: each call runs in a fresh interpreter with the same access.
- `read()` shows images to the model; files changed by `write()` and `edit()` are shown as a
  colored diff at the end of the call; Python code is syntax highlighted.
- Headless mode (`tuse -p`) for scripts; sessions in `~/.tuse/<id>/` as JSONL.
- Background tasks, 50/25/25 context compaction, retries.
- YAML config with `include`, `$VAR` and `#{{command}}` expansion and a full theme.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/ethanhamilthon/tuse/main/release/install.sh | sh
```

The script puts `tuse` into `~/.local/bin`. On a first install (no `TUSE_GLOBAL_CONFIG` and
no `~/.config/tuse`) it runs `tuse reset`, which installs `main.yaml`, and then `tuse connect`.
The system prompt and the default settings are built into the binary.
`TUSE_VERSION=0.3.0` picks a version, `TUSE_BIN_DIR` the folder.

`tuse update` updates the binary; your `main.yaml` is not touched.
`tuse reset` deletes the config folder and installs a fresh `main.yaml`.

Or build it: `cargo install --git https://github.com/ethanhamilthon/tuse tuse`, then copy
[main.yaml](release/main.yaml) to `~/.config/tuse/`.

From a checkout, `make install` builds the release binary into `~/.local/bin` (`BIN_DIR`) and
creates `main.yaml` in `~/.config/tuse` (`CONFIG_DIR`) when it is missing.

## Usage

```
tuse [--config PATH] [--resume ID]        open the TUI
tuse -p [PROMPT|-] [--config PATH] [--resume ID]
                                          headless: runs the prompt to the end, replies to
                                          stdout, session ID to stderr, exit code 1 on an error
tuse connect [--config PATH]              add a provider (base url, api key, name)
tuse reset [--yes] | tuse update          reset the config folder | update the binary
```

The agent's tool is `python`, a persistent REPL with these functions:

- `read(path)`: numbered text lines in pages, or an image the model sees. `write(path, content)`
  creates or replaces a file atomically. `edit(path, old, new)` replaces exact, unique text. The
  user sees what `write` and `edit` changed as a diff at the end of the call.
- `grep(pattern, path)`: a regex search that respects `.gitignore`.
- `sh(cmd)` runs `bash -c` and returns `.out`, `.code`, `.ok`; a command that runs over the timeout
  goes on as a background task instead of being killed.
- `ask(question, options)` shows a list in the TUI and waits; the user can also write their own
  answer. `draw_ui(text)` draws a plain text block that `draw_ui(text, id=ID)` redraws.
- `task_run(code)` starts a background Python REPL, `task_add(id, code)` queues more code in it;
  `task_get`, `task_stop` and `task_list` manage them (listed in `/tasks`; the agent gets a
  message when one goes idle or exits).

The REPL lives as long as the process. Every call has a `title` that the working indicator shows.

`tuse connect` asks for the base url, the API key (`$ENV` works too) and a name, fetches
the provider's models, says how many it got and adds the `connect` entry to the config.

### TUI

| Key | Action |
| --- | --- |
| Enter | send |
| Shift+Enter, Alt+Enter, Ctrl+J | new line |
| Esc | close the picker, or stop the running turn |
| mouse wheel | scroll the chat (or the open list) |
| Up / Down, Left / Right, Home / End | move in the input |
| click in the input | put the cursor there |
| Ctrl+V, Cmd+V | paste; an image becomes `[Image #n: PATH]` and goes to the model with the message |
| Ctrl/Cmd+C | copy selected text; otherwise clear the input or quit if empty |
| Ctrl/Cmd+A / Ctrl/Cmd+X | select all input / cut selected input |
| Shift+arrows / Shift+Home/End | extend the input selection |
| Home / Ctrl+E / Ctrl+U / Ctrl+K / Ctrl+W | line start / end / kill back / kill forward / delete word |

A message sent while the agent is working stays dimmed in the input until the agent takes it.
Editing it withdraws it from the queue without interrupting the turn. Press Enter to send
the edited draft. Esc still stops the turn.

Commands: `/new`, `/resume` (sessions started in this folder), `/models` (every model the
providers report, then effort), `/theme`, `/tasks` (Enter shows output, Ctrl+X stops), `/usage`,
`/trace` (a list of the system prompt, messages, thinking, tool calls and outputs; Enter opens
an item in full), `/reload`, `/quit`.
Every list looks the same and is drawn over the bottom of the chat; typing filters it. `/usage`
and a task's output open as text in the same frame; Esc closes it. Drag to select text in the chat, panels or input.
A session keeps its own model: `/models` also saves it to the config for new sessions.

`#name` inserts a prompt from the config; it is expanded to its text when sent.
`@` picks a file or folder from the current folder; `@dir/*` lists what is inside `dir`.
Files, prompts and images are one unit in the input: arrows and Backspace move over them whole.
`/models` and `/theme` write your choice to the active config.

## Config

Tuse reads `--config PATH`, else `./tuse.yaml`, else `~/.config/tuse/main.yaml`.
See [docs/config.md](docs/config.md) for every key.

```yaml
include:
  - prompts.yaml

model: claude-sonnet-5-5
provider: x
effort: high          # default | low | medium | high | xhigh | max

connect:
  - name: x
    base_url: http://127.0.0.1:8317/v1
    api_key: $API_KEY

models:
  - name: x/claude-sonnet-5-5
    display: Sonnet 5.5
    window: 1000000
    compact_at: 600000
    prices: {in: 3, out: 15, cache: 0.3, cache_write: 3.75}

hooks:
  notify:
    events: [turn_end]
    run: |                      # Python, a fresh interpreter per call
      if not cancelled:
          sh("osascript -e 'display notification \"done\"'")
```

## Development

The workspace is split into crates: `config`, `api`, `py`, `core`, `tui` and
`cli`. See [AGENTS.md](AGENTS.md) for the rules. Run `make check` before committing.

## License

MIT
