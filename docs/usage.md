# Usage

How to install, update, and roll back Tuse, use the terminal UI, and run the `tuse` command line. For providers and models see [Connect](connect.md), for config files and plugins see [Config](config.md).

## Install

Prebuilt binaries exist for macOS 11+ (`arm64`, `amd64`) and Linux with glibc 2.35+ (`amd64`, `arm64`):

```sh
curl -fsSL https://raw.githubusercontent.com/ethanhamilthon/tuse/main/release/install.sh | sh
```

The script picks the archive for your OS and CPU, verifies its SHA-256 against the release's `checksums.txt`, and installs only the `tuse` binary into `~/.local/bin`. It never prompts for plugins and never creates or changes config files. Settings:

| Variable | Meaning |
| --- | --- |
| `TUSE_BIN_DIR` | Install directory (default `~/.local/bin`). |
| `TUSE_VERSION` | Version to install, such as `0.15.0` or `v0.15.0` (default: latest). |
| `TUSE_RELEASE_URL` | Base download URL. Wins over `TUSE_VERSION`. |

To build from source you need the Rust toolchain from `rust-toolchain.toml`. Run `cargo install --locked --git https://github.com/ethanhamilthon/tuse tuse`, or `make install` in a checkout (`BIN_DIR` defaults to `~/.local/bin`). To build this documentation site locally, install mdBook (Pages CI uses v0.5.4) and run `make site`; the HTML is written to `target/book`.

### Updating and rolling back

- `tuse update` compares your installed version with the latest release inline, offers to show all intervening changelog sections before asking for confirmation, verifies the release SHA-256 checksum, backs up the current binary to `$TUSE_HOME/backups/<version>-tuse-bin` (default `~/.tuse/backups/`), and atomically replaces the executable. Declining or failing verification leaves the executable and rollback tracker unchanged.
- `tuse rollback` reads the backup recorded by the latest successful update (independent of semantic version order), shows the target version, and restores that binary after confirmation.
- Neither `tuse update` nor `tuse rollback` touches configs, plugins, or `tuse.db`.

### Developer install

Run `make install-dev` in a checkout to build and install `tusedev` in `BIN_DIR` (default `~/.local/bin`) alongside `tuse`. The launcher sets `TUSE_HOME` to `~/.tusedev` for sessions, providers, preferences, backups, and plugins, and `TUSE_GLOBAL_CONFIG` to `~/.config/tusedev/init.lua`. It replaces any inherited values for those two variables without changing `HOME`. The developer global file is its only config entrypoint.

The installer creates an empty global config (`return {}`) if it does not exist and preserves it on reinstall. Run `make install-dev` again to rebuild from the checkout; `tusedev update` and `tusedev rollback` are disabled.

For a fresh developer installation, run `make install-dev CLEAN=1` (or `sh release/install-dev.sh --clean`). This deletes `~/.tusedev` and `~/.config/tusedev` and recreates the empty developer config without offering or installing plugins. The regular `tuse` installation is untouched.

## First run

1. Run `tuse` in your project.
2. Run `/providers connect` and follow the staged wizard: API type, base URL, masked API key (or `$ENV_VAR`), and name ([Connect](connect.md)).
3. Pick a discovered model with `/models` and send your first message.

## Command line

Every command accepts `-h`/`--help`, which prints its usage and exits without reading config or state. Valued flags take `--flag VALUE` or `--flag=VALUE` (values starting with `-` are accepted unless matching `-h`, `--help`, or a known flag of that command). Plain boolean flags (`--json`, `--no-global`, `--no-plugins`) reject inline `=value`. When a flag is repeated the last value wins, and `--` ends flag parsing. Invalid commands, flags, and arguments print `tuse: <message>` to stderr and exit with 1. The diagnostic may also include usage.

- `tuse [-r|--resume ID] [--no-global] [--no-plugins]` opens the TUI in the current folder. `-r ID` opens a saved session instead, using its saved folder if that still exists. On exit it prints `tuse: session <ID>` to stderr.
- `tuse json [-r|--resume ID] [--no-global] [--no-plugins]` serves one session over JSON-RPC on stdin/stdout ([JSON mode](json.md)).
- `tuse providers [--json]` lists saved providers (read-only) as `NAME<TAB>BASE_URL`, or as JSON objects with `{ name, type, base }`. Keys and headers are never printed.
- `tuse models` prints the saved default model and effort and the `models` metadata from the effective config.
- `tuse update` checks for updates, offers intervening changelogs, verifies checksums, saves a backup, and updates the binary after confirmation.
- `tuse rollback` restores the binary saved by the last successful update after confirmation.
- `tuse version` (`-V`, `--version`) prints `tuse <version>`. Extra flags or arguments are rejected.
- `tuse help [COMMAND]` (`-h`, `--help`) prints the full usage or the usage of one command. Unknown commands or extra arguments are rejected.

Startup exclusion flags can be combined freely on `tuse` and `tuse json` and persist across `/reload`, `/new`, `/fork`, and `/resume` in the same process:

- `--no-global` skips all global config lookup and execution.
- `--no-plugins` blocks installed plugin activation on startup, switch, and reload while leaving `/plugins` and Lua/RPC `plugins.*` management working.

Global config lookup: `$TUSE_GLOBAL_CONFIG` (if nonblank; directory loads `<dir>/init.lua`; missing target fails) > `~/.config/tuse.lua` > `~/.config/tuse/init.lua`. Project files are never discovered automatically. Eligible plugins are loaded from `$TUSE_HOME/plugins/` (default `~/.tuse/plugins/`).

## The screen

From top to bottom:

- **Chat.** Your messages sit on a `user_bg` block. Both your messages and agent replies leave two columns on each side and line up with the rest of the chat text. Agent replies render as Markdown, and a Markdown image `![alt](path)` shows as a clickable `alt` (or the path when `alt` is empty). Reasoning is shown after `∴`. Each `lua` tool call shows its code with a star spinner while it runs and `*` once it is done, then its output and per-file diffs. `ui.show()` blocks start with `◆`, and answered `ui.pick()`/`ui.ask()` prompts with `?`. Turn summaries look like `✓ 12s · Model/effort`.
- **Gap.** One empty row above the input in every style.
- **Top rule.** Shows `↑N` while scrolled up. On the right: a short flash (`Copied.`, retry countdowns), the busy spinner with elapsed time, or `steering…`. While background tasks run, the count follows the timer (`12s · 2 tasks`), and when idle it replaces the project path with an `icons`-colored spinner and `accent`-colored task count (`⠋ 2 tasks`).
- **Input.** Grows with the text up to `ui.lines.input`.
- **Bottom rule.** `<model> · <effort> · <used>/<window> · $<cost>` (the effort segment is omitted for `efforts = {}` models, `unavailable` appears in error color when the selected model's provider is disconnected, and context and cost are hidden while unknown).
- **Overlay.** At most one picker, panel, wizard step, or `ui.ask()` question. With `ui.picker = "bottom"` (the default), overlays open below the input with exactly one blank row separating the input from the overlay across all input styles (`lines`, `boxed`, `fill`, `bar`); with `fill` and `bar` the overlay sits between that blank row and the closing rule above the status line. With `ui.picker = "top"`, overlays open above the composer ([Config](config.md#theme-and-ui)).

How many lines of code, output, diffs, and reasoning appear is set by `ui.lines`, and the colors by `ui.theme` ([Config](config.md#theme-and-ui)).

## Typing a message

- `Enter` accepts a highlighted picker item, runs a standalone `/command`, or sends the message. `Shift+Enter`, `Alt+Enter`, or `Ctrl+J` inserts a newline.
- `#` groups literal prompts by namespace. Pick `#work:review` to attach that prompt. There are no prompt arguments.
- `@` opens a file picker for the session folder. It matches any part of the file or folder name, ignoring case. Prefix matches come first, then other matches. Each group is newest first by modification time (creation time if unavailable). With no query, the newest entries come first, and older entries are dropped first in large folders. `@dir/q` browses `dir/`, and `@/` or `@~/` go outside the project. `Tab` completes a folder or attaches a file. `Enter` attaches the highlighted file or folder.
- `Ctrl+V`/`Cmd+V` pastes an image from the clipboard as `[Image #N]` (saved under `$TUSE_HOME/<id>/images/`, default `~/.tuse/<id>/images/`), or pastes text otherwise.

Picked prompts, files, and images are atoms: arrows and Backspace treat each one as a single unit. Typing `#work:review` or `@path` by hand is plain text. The model receives the attachments in a hidden block after your text:

```text
fix this #work:review @notes.txt [Image #1]

<prompt name="work:review">Check the change for bugs.</prompt>
<image id="1" path="/home/me/.tuse/ID/images/image-1.png"/>
<file path="notes.txt">hello world
</file>
```

Text files send their first 1,024 bytes (`truncated="true" size="…"` when longer), and binary files only their size. Folders send a sorted listing of up to 1,024 bytes. Images are attached as base64 data. Missing references are skipped.

## While the agent works

- **Steering.** Press `Enter` while a turn runs and your message is queued (dimmed, with `steering…` in the top rule). The agent gets it at its next step. If you edit the dimmed text, it leaves the queue.
- **Steering** hands a running `lua` call off to a background task in `/tasks`, so the agent can read your message right away ([Lua](lua.md#background-tasks)).
- **Esc** closes the open picker, wizard step, question, or panel first. Cancelling a provider wizard clears any typed secret and saves nothing. Otherwise Esc stops everything: the turn, the running `lua` call, background tasks, hooks, custom commands, and open `ui.pick`/`ui.ask` prompts. `/stop` does the same.
- **Scrolling.** The chat follows new output only while you are at the bottom. Scrolled up, the view stays where it is until you scroll back down.
- **Quit** with `/quit`, or `Ctrl+C`/`Ctrl+D` on an empty input.

## Keys

| Key | Action |
| --- | --- |
| `PageUp` / `PageDown` | Page any open picker (clamping at the ends without scrolling chat), or scroll the chat. `Shift+` scrolls the open panel. |
| `Up` / `Down` | Move through wrapped lines, picker items, or (with empty input) scroll a panel. |
| `Ctrl+P` / `Ctrl+N` | Move in a picker. |
| `Home` / `End`, `Ctrl+E` | Start / end of line. |
| `Ctrl` or `Alt` + `Left`/`Right`, `Alt+B`/`Alt+F` | Move by word. |
| `Shift` + arrows / `Home` / `End` | Extend the draft selection. |
| `Ctrl+A` / `Cmd+A` | Select the entire draft, not the chat (disabled on masked secret fields). |
| `Ctrl+C` / `Cmd+C` | Copy the selection, else clear the input, else quit (never copies masked secrets). |
| `Ctrl+X` / `Cmd+X` | Cut (disabled on masked secret fields). In `/tasks`, stop the selected task. |
| `Ctrl+W`, `Alt+Backspace`, `Ctrl+Backspace` | Delete the previous word. |
| `Ctrl+U` / `Ctrl+K` | Delete to start / end of line. |
| `Esc` | Close or cancel the active overlay, or stop everything (see above). |

Shortcuts with `Ctrl`, `Alt`, or `Cmd` also work on a Russian keyboard layout. With the mouse, the wheel scrolls whatever is under the pointer, and dragging selects text within one target at a time: the draft, a user or agent message, a `ui.show()` block (pinned or not), a `ui.pick` or `ui.ask` result, or the open text panel. Reasoning, Lua tool blocks, events, masked secret fields, and built-in pickers are not selectable. Copying with `Ctrl+C` or `Cmd+C` leaves out visual wraps and layout padding but preserves intentional line breaks. Clicking a URL or an existing file path in agent messages, `ui.show()` blocks, `/config`, `/session`, or `/history` opens it, using the optional `open(target, kind)` callback or the OS opener.

## Slash commands

Type `/` anywhere in a draft to open the command picker (tokens with a second `/`, like `/usr/bin`, do not open it). The flat `/` and grouped `#` pickers match qualified name prefixes ignoring case. Pickers show aligned columns; `/history`, `/resume`, `/fork`, `/models`, and `/tasks` search the whole row.

- With the command picker open, `Enter` (or `Tab`) accepts the highlighted command immediately—executing built-in or zero-arg commands (removing only the command token and preserving surrounding draft text) or showing subcommands for `/providers` and `/plugins`.
- Typing a space after a zero-argument command (such as `/new `) disengages the picker; pressing ordinary `Enter` on `"/new "` or `"/new foo"` shows `/new takes no words`, preserves the draft, and neither runs `/new` nor sends to the model.
- Plain mid-draft prose (`explain /new works`, ` /new`) and paths (`/usr/bin/env`, `/etc/hosts is broken`) send cleanly as normal user messages.
- Standalone unknown single-segment commands at column 0 (`/trace`) and invalid command arguments fail immediately, preserve the draft untouched, and never send to the model.

Built-in commands:

- `/new` starts a fresh session in the current folder (preserving launch exclusion flags).
- `/resume` picks another started session from this folder, newest first.
- `/fork` picks one of the user and agent messages of this session and opens a new session in the same folder with the history up to it, titled `[fork] ` plus the old title. Forking at an agent message keeps it; forking at a user message leaves it out and puts its text in the input. The original session stays in `/resume`.
- `/models` picks a discovered model (annotated with any matching `config.models` metadata), then an effort from that model's resolved `efforts` (`low`, `medium`, `high`, `xhigh`, `max`, or skips effort selection when `efforts = {}`). The choice applies to this session and becomes the default for new ones.
- `/providers` manages saved API profiles (`list`, `connect`, `update`, `disconnect`). `list` is read-only; `connect` collects API type, base URL, masked non-recording API key, then name; `update` selects a saved profile and edits through a staged form; `disconnect` removes a profile.
- `/plugins` manages installed plugins under `$TUSE_HOME/plugins/` (`list`, `get`, `add`, `delete`, `update`). Mutations update installed files immediately (even under `--no-plugins`); active session contributions update only after an explicit `/reload`.
- `/tasks` lists background tasks. `Enter` shows the task: id, name, status, start and end, how long it has run, the source of its function, and the last 200 log lines. `Ctrl+X` or `Cmd+X` stops one.
- `/session` shows the session: ID, title, folder, config, model, context, turns, usage, cost.
- `/history` lists everything in the session, starting with the system prompt. `Enter` opens an entry, `Left`/`Right` step through entries, `Esc` goes back. The list always hides `meta`, `usage`, and empty agent replies. Lua and RPC still see the full history.
- `/config` shows the resolved config (including active plugins), without secrets or Lua bodies.
- `/reload` runs the config and eligible plugins again (preserving launch exclusion flags). If it fails, the old config stays.
- `/stop` stops the turn, background tasks, hooks, and commands, also when the session is idle.
- `/quit` exits.

Custom callbacks are listed as `/namespace:name`, accept no words, and run as Lua jobs.
