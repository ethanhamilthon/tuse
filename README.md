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
cargo install --git https://github.com/ethanhamilthon/tuse tuse
```

or download a binary from the [releases](https://github.com/ethanhamilthon/tuse/releases).

## Usage

```
tuse [--config PATH] [--resume ID]        open the TUI
tuse -p [PROMPT|-] [--config PATH] [--resume ID]
                                          headless: session ID to stderr, replies to stdout;
                                          runs until killed and takes `tuse steer` messages
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

### TUI

| Key | Action |
| --- | --- |
| Enter | send |
| Shift+Enter, Alt+Enter, Ctrl+J | new line |
| Esc | close the picker, or stop the running turn |
| Up / Down, PageUp / PageDown, mouse wheel | scroll the chat |
| Ctrl+C | clear the input, or quit when it is empty |
| Ctrl+A / Ctrl+E / Ctrl+U / Ctrl+K / Ctrl+W | line start / end / kill back / kill forward / delete word |

Commands: `/new`, `/resume` (sessions started in this folder), `/models` (model, then
effort), `/tasks` (Enter shows output, Ctrl+X stops), `/usage`, `/reload`, `/quit`.

`#name` inserts a prompt from the config; it is expanded to its text when sent.
`@` picks a file or folder and inserts its path.

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
