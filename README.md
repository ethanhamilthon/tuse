# Tuse

A minimal AI coding agent for the terminal. Rust + Crossterm, any OpenAI
Chat Completions compatible API, one tool: `bash`.

- Full-screen TUI with Markdown rendering and pickers for `/` commands, `#` prompts and `@` files.
- Headless mode (`tuse -p`) driven from scripts with `tuse steer`.
- Sessions in `~/.tuse/<id>/` as JSONL; a socket per session for `steer`, `ask` and `task`.
- Background tasks, hooks, `bins` aliases, 50/25/25 context compaction, retries.
- YAML config with `include`, `$VAR` and `#{{command}}` expansion and a full theme.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/ethanhamilthon/tuse/main/release/install.sh | sh
```

The script puts `tuse` into `~/.local/bin`. On a first install (no `TUSE_GLOBAL_CONFIG` and
no `~/.config/tuse`) it runs `tuse reset`, which installs `main.yaml` and `default.yaml`
(the system prompt about Tuse and the default settings), and then `tuse connect`.
`TUSE_VERSION=0.3.0` picks a version, `TUSE_BIN_DIR` the folder.

`tuse update` updates the binary and replaces `default.yaml`; your `main.yaml` is not touched.
`tuse reset` deletes the config folder and installs fresh `main.yaml` and `default.yaml`.

Or build it: `cargo install --git https://github.com/ethanhamilthon/tuse tuse`, then copy
[main.yaml](release/main.yaml) and [default.yaml](release/default.yaml) to `~/.config/tuse/`.

## Usage

```
tuse [--config PATH] [--resume ID]        open the TUI
tuse -p [PROMPT|-] [--config PATH] [--resume ID]
                                          headless: session ID to stderr, replies to stdout;
                                          runs until killed and takes `tuse steer` messages
tuse connect [--config PATH]              add a provider (base url, api key, name)
tuse print [-s ID] [--id ID] [TEXT|-]     show plain multiline text in the TUI; prints a block ID
tuse reset [--yes] | tuse update          reset the config folder | update the binary and default.yaml
tuse docs                                 the docs for the agent
tuse steer [-s ID] [--file PATH]... [TEXT|-]
tuse ask [-s ID] [--async] QUESTION OPTION...
tuse task run [-s ID] [--name NAME] -- COMMAND...
tuse task stop|get [--lines N]|list [-s ID] [ID]
```

`-s` defaults to `$TUSE_SESSION`, which is set for the agent's bash calls, so the
agent can use `tuse ask` and `tuse task` itself.

- `tuse steer` sends a message exactly as if it were typed in the TUI. While a turn
  runs, it joins the turn after the current step. `--file` attaches images as images
  and any other file as text.
- `tuse ask` shows a picker in the TUI and prints the answer. With `--async` it
  prints an ID right away and the answer reaches the agent later as a message.
- `tuse task run` starts a background process owned by the session. Tasks are listed
  in `/tasks`, and the agent gets a message when one exits.

`tuse connect` asks for the base url, the API key (`$ENV` works too) and a name, fetches
the provider's models, says how many it got and adds the `connect` entry to the config.

### TUI

| Key | Action |
| --- | --- |
| Enter | send |
| Shift+Enter, Alt+Enter, Ctrl+J | new line |
| Esc | close the picker, or stop the running turn |
| Up / Down, PageUp / PageDown, mouse wheel | scroll the chat |
| Ctrl+V, Cmd+V | paste; an image becomes `[Image #n: PATH]` and goes to the model with the message |
| Ctrl+C | clear the input, or quit when it is empty |
| Ctrl+A / Ctrl+E / Ctrl+U / Ctrl+K / Ctrl+W | line start / end / kill back / kill forward / delete word |

Commands: `/new`, `/resume` (sessions started in this folder), `/models` (every model the
providers report, then effort), `/theme`, `/tasks` (Enter shows output, Ctrl+X stops), `/usage`,
`/trace` (the system prompt, messages, thinking, tool calls and outputs), `/reload`, `/quit`.
`/usage`, `/trace` and a task's output open in a panel over the chat; Esc closes it.

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
```

## Development

The workspace is split into crates: `config`, `api`, `core`, `socket`, `tui` and
`cli`. See [AGENTS.md](AGENTS.md) for the rules. Run `make check` before committing.

## License

MIT
