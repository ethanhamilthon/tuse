# Usage

How to install and update Tuse, use the terminal UI, and run the `tuse` command line. For providers and models see [Connect](connect.md), for config files see [Config](config.md).

## Install

Prebuilt binaries exist for macOS 11+ (`arm64`, `amd64`) and Linux with glibc 2.35+ (`amd64`, `arm64`):

```sh
curl -fsSL https://raw.githubusercontent.com/ethanhamilthon/tuse/main/release/install.sh | sh
```

The script picks the archive for your OS and CPU, checks its SHA-256 against the release's `checksums.txt`, and installs `tuse` into `~/.local/bin`. It only installs the binary and never creates or changes config files. Settings:

| Variable | Meaning |
| --- | --- |
| `TUSE_BIN_DIR` | Install directory (default `~/.local/bin`). |
| `TUSE_VERSION` | Version to install, such as `0.10.3` or `v0.10.3` (default: latest). |
| `TUSE_RELEASE_URL` | Base download URL. Wins over `TUSE_VERSION`. |

To build from source you need the Rust toolchain from `rust-toolchain.toml`. Run `cargo install --locked --git https://github.com/ethanhamilthon/tuse tuse`, or `make install` in a checkout (`BIN_DIR` defaults to `~/.local/bin`).

`tuse update` downloads the latest release (or `TUSE_RELEASE_URL`) and checks and unpacks it in-process. It then atomically replaces the running binary and prints `<old> -> <new>`. It never touches `~/.tuse` or your config.

## First run

1. Connect a provider: `tuse connect --base https://api.openai.com/v1 --key '$OPENAI_API_KEY' --name openai` ([Connect](connect.md)).
2. Run `tuse` in your project and pick a model with `/models`.
3. Optionally add a config: `tuse init` creates `./tuse.lua` that extends your global config ([Config](config.md)).

## Command line

Every command accepts `-h`/`--help`, which prints its usage and exits without reading config or state. Flags take `--flag VALUE` or `--flag=VALUE`. When a flag is repeated the last value wins, and `--` ends flag parsing. Unknown commands, unknown flags, and missing or extra arguments print `tuse: <message>` with the usage line to stderr and exit with 1.

- `tuse [-c|--config PATH] [-r|--resume ID]` opens the TUI in the current folder. `-r ID` opens a saved session instead, using its saved folder if that still exists. On exit it prints `tuse: session <ID>` to stderr.
- `tuse json [-c|--config PATH] [-r|--resume ID]` serves one session over JSON-RPC on stdin/stdout ([JSON mode](json.md)).
- `tuse connect --base URL (--key '$ENV' | --key-stdin) --name NAME [--api TYPE] [--replace]` checks a provider and saves it ([Connect](connect.md)).
- `tuse disconnect NAME` removes a provider.
- `tuse providers [--json]` lists providers as `NAME<TAB>BASE_URL`, or as JSON with `api`. Keys are never printed.
- `tuse models [-c|--config PATH]` prints the saved default model and effort and the `models` metadata from the effective config.
- `tuse init` writes `./tuse.lua`, which loads the global config with `pcall(require, "global")`. It fails if the file exists.
- `tuse update` updates the binary (see above).
- `tuse version` (`-V`, `--version`) prints `tuse <version>`.
- `tuse help` (`-h`, `--help`) prints the full usage. `tuse COMMAND -h` prints the usage of one command.

`-c PATH` is resolved from the current folder, and `~/` expands to your home. Without it, Tuse uses `./tuse.lua`, then `~/.config/tuse.lua`, then `~/.config/tuse/init.lua`, then built-in defaults.

## The screen

From top to bottom:

- **Chat.** Your messages sit on a `surface` block. Agent replies render as Markdown. Reasoning is shown after `∴`. Each `lua` tool call shows its code with a star spinner while it runs and `*` once it is done, then its output and per-file diffs. `show()` blocks start with `◆`, and answered `pick()`/`ask()` prompts with `?`. Turn summaries look like `✓ 12s · Model/effort`.
- **Overlay.** At most one picker, panel, or `ask()` question, shown above the input.
- **Top rule.** Shows `↑N` while scrolled up. On the right: a short flash (`Copied.`, retry countdowns), the busy spinner with elapsed time, or `steering…`.
- **Input.** Grows with the text up to `settings.ui.input_max_height`.
- **Bottom rule.** `<model> · <effort> · <used>/<window> · $<cost>`. The context and cost parts are hidden while unknown.

How many lines of code, output, diffs, and reasoning appear is set by `settings.ui`, and the colors by `theme` ([Config](config.md#theme-and-ui)).

## Typing a message

- `Enter` sends the message, or runs an exact `/command`. `Shift+Enter`, `Alt+Enter`, or `Ctrl+J` inserts a newline.
- `#` opens a picker of your configured `prompts`. The picked `#name` is sent with the prompt body attached. A prompt with `args` then opens a picker of its args, and a space after the prompt opens it again. The result is one atom such as `#openspec propose`, drawn in the accent color.
- `@` opens a file picker for the session folder. `@dir/q` browses `dir/`, and `@/` or `@~/` go outside the project. `Tab` completes a folder or attaches a file. `Enter` attaches the highlighted file or folder.
- `Ctrl+V`/`Cmd+V` pastes an image from the clipboard as `[Image #N]` (saved under `~/.tuse/<id>/images/`), or pastes text otherwise.

Picked prompts, files, and images are atoms: arrows and Backspace treat each one as a single unit. Typing `#name` or `@path` by hand is plain text. The model receives the attachments in a hidden block after your text:

```text
fix this #review @notes.txt [Image #1]

<prompt name="review">Check the change for bugs.</prompt>
<image id="1" path="/home/me/.tuse/ID/images/image-1.png"/>
<file path="notes.txt">hello world
</file>
```

Text files send their first 1,024 bytes (`truncated="true" size="…"` when longer), and binary files only their size. Folders send a sorted listing of up to 1,024 bytes. Images are attached as base64 data. Missing references are skipped.

## While the agent works

- **Steering.** Press `Enter` while a turn runs and your message is queued (dimmed, with `steering…` in the top rule). The agent gets it at its next step. If you edit the dimmed text, it leaves the queue.
- **Steering** hands a running `lua` call off to a background task in `/tasks`, so the agent can read your message right away ([Lua](lua.md#background-tasks)).
- **Esc** closes the open picker, question, or panel first. Otherwise it stops everything: the turn, the running `lua` call, background tasks, hooks, custom commands, and open `pick`/`ask` prompts. `/stop` does the same.
- **Scrolling.** The chat follows new output only while you are at the bottom. Scrolled up, the view stays where it is until you scroll back down.
- **Quit** with `/quit`, or `Ctrl+C`/`Ctrl+D` on an empty input.

## Keys

| Key | Action |
| --- | --- |
| `PageUp` / `PageDown` | Scroll the chat. `Shift+` scrolls the open panel. |
| `Up` / `Down` | Move through wrapped lines, picker items, or (with empty input) scroll a panel. |
| `Ctrl+P` / `Ctrl+N` | Move in a picker. |
| `Home` / `End`, `Ctrl+E` | Start / end of line. |
| `Ctrl` or `Alt` + `Left`/`Right`, `Alt+B`/`Alt+F` | Move by word. |
| `Shift` + arrows / `Home` / `End`, `Ctrl+A` | Select input. |
| `Ctrl+C` / `Cmd+C` | Copy the selection, else clear the input, else quit. |
| `Ctrl+X` / `Cmd+X` | Cut. In `/tasks`, stop the selected task. |
| `Ctrl+W`, `Alt+Backspace`, `Ctrl+Backspace` | Delete the previous word. |
| `Ctrl+U` / `Ctrl+K` | Delete to start / end of line. |
| `Esc` | Close, or stop everything (see above). |

Shortcuts with `Ctrl`, `Alt`, or `Cmd` also work on a Russian keyboard layout. With the mouse, the wheel scrolls whatever is under the pointer, and dragging selects text (copy with `Ctrl+C`). Clicking a URL or an existing file path in agent messages, `show()` blocks, `/config`, `/session`, or `/history` opens it, using a matching `open.<ext>` handler from config or the OS opener.

## Slash commands

Type `/` to get a picker. Pickers filter by label, ignoring case.

- `/new` starts a fresh session in the current folder.
- `/resume` picks another titled session from this folder, newest first.
- `/models` picks a model (limited by `settings.ui.scope_models`), then an effort (`default`, `low`, `medium`, `high`, `xhigh`, `max`). The choice applies to this session and becomes the default for new ones.
- `/tasks` lists background tasks. `Enter` shows the task: id, name, status, start and end, how long it has run, the source of its function, and the last 200 log lines. `Ctrl+X` stops one.
- `/session` shows the session: ID, name, folder, config, model, context, turns, usage, cost.
- `/history` lists everything in the session, starting with the system prompt. `Enter` opens an entry, `Left`/`Right` step through entries, `Esc` goes back. Roles in `settings.ui.hide_history` are left out.
- `/config` shows the resolved config, without secrets or Lua bodies.
- `/reload` runs the config again. If it fails, the old config stays.
- `/stop` stops the turn, background tasks, hooks, and commands, also when the session is idle.
- `/quit` exits.

Custom commands from `commands = { name = function() ... end }` show up in the same picker and run as Lua jobs ([Config](config.md#commands-and-settingsopen)). Built-in names cannot be overridden.
