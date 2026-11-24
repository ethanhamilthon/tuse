# Commands

Quick reference for CLI subcommands and TUI slash commands (also available as `docs("commands")`). Canonical specifications live in [cli.md](cli.md) and [tui.md](tui.md).

## CLI

- `tuse [-c|--config PATH] [-r|--resume ID]` opens the full-screen TUI. `--config` (`-c`) picks the config file; `--resume` (`-r`) opens a saved session. Normal exit prints `tuse: session <ID>` to stderr.
- `tuse json [-c|--config PATH] [-r|--resume ID]` serves one session over [JSON-RPC 2.0 stdio](json.md).
- `tuse connect --base URL (--key '$ENV' | --key-stdin) --name NAME [--api TYPE] [--replace]` verifies a provider (`openai-chat`, `openai-responses`, `anthropic-messages`, or `google-gemini`) via its model-listing endpoint and saves (or atomically replaces with `--replace`) it in SQLite ([providers.md](providers.md)).
- `tuse providers [--json]` lists saved providers (`<NAME>\t<BASE_URL>`, or JSON with `api` via `--json`); `tuse disconnect NAME` removes one.
- `tuse models [-c|--config PATH]` prints the saved default model/effort and lists configured model metadata from the effective Lua config.
- `tuse init` creates `./tuse.lua` extending the global config.
- `tuse update` downloads, verifies, and replaces the `tuse` binary in place without touching your config ([install.md](install.md)).
- `tuse version` (`--version`, `-V`) prints the version; `tuse help` (`--help`, `-h`) prints usage; `tuse COMMAND --help` prints one command's usage.

See [cli.md](cli.md) for flag syntax, validation rules, error formats, and exit behavior.

## TUI

Built-in slash commands run when typed as `/name` with no arguments or selected from the `/` picker:

- `/new` starts a fresh session in the current directory.
- `/resume` opens a searchable picker of other named sessions for the current directory.
- `/models` opens a two-step picker (model, then reasoning effort) and saves the selection for the active session and as the SQLite default for new sessions.
- `/tasks` lists background tasks (`spawn()`); Enter views the log (last 200 lines) and `Ctrl+X` stops the selected task.
- `/session` opens a scrollable view of `require("tuse").session()` (all fields except `trace`).
- `/trace` opens a searchable list of all trace entries (`index  role  first line`); Enter opens the full entry panel, `Left`/`Right` (when input is empty) step to the previous/next filtered entry, and Esc returns to the list with search and selection intact.
- `/config` opens a scrollable view of the resolved config (without provider secrets or Lua function bodies).
- `/reload` re-evaluates and validates the config in the same VM (keeping the previous working config on failure).
- `/quit` closes the session and exits.

Custom slash commands from `commands = { name = function() ... end }` appear in `/` autocomplete and run as non-blocking Lua jobs (built-in names cannot be overridden).

Picker search filters item **labels** (not details) by case-insensitive substring; modal searchable pickers reuse the input as a search box and restore your draft on close. See [tui.md](tui.md) for complete keyboard shortcuts, mouse selection, interactive links, steering, and overlay workflows.
