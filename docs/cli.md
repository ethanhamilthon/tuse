# CLI

The `tuse` executable launches interactive TUI sessions, runs the JSON-RPC stdio server, manages API providers, inspects configured models, creates project configs, and updates itself.

## Invocation and argument parsing

Dispatch inspects the first argument:

- No arguments, or first argument starting with `-` (other than `-h`/`--help` and `-V`/`--version`): launches the TUI.
- `-V` or `--version`: runs `version`.
- `-h` or `--help`: runs `help`.
- Otherwise: matches one of the subcommands `json`, `connect`, `disconnect`, `providers`, `models`, `init`, `update`, `version`, `help`. Any other word fails immediately—**before** checking `-h`/`--help`—printing `tuse: unknown command \`<word>\`` plus full usage to stderr and exiting 1.

### Help and validation rules

- **Help precedence (known commands):** For the root command or any known subcommand, if `-h` or `--help` appears anywhere before `--`, Tuse prints usage to stdout and exits 0 without reading config files, touching `~/.tuse`, or running the command. Root `tuse -h` and `tuse help` print full usage; `tuse COMMAND -h` prints only that subcommand's usage line.
- **Flag syntax:** Both `--flag VALUE` and `--flag=VALUE` are accepted. A valued flag followed by no token or by a token starting with `-` fails with `tuse: <flag> needs a value` (without a usage line; pass `--flag=-value` if a value begins with `-`). When a flag is repeated, the last value wins.
- **End of flags:** `--` stops flag parsing; all remaining arguments are treated as positional arguments.
- **Strict checking:** Unknown flags (`unknown flag <flag>`), missing required positional arguments (`missing argument`), and unexpected extra arguments (`unexpected argument \`<arg>\``) print the error plus the subcommand's usage line to stderr and exit 1.
- **Error format:** All CLI failures print `tuse: <message>` to stderr and exit with status 1.
- **Config path resolution:** `-c`/`--config PATH` is resolved relative to the current working directory; `~/` expands to `HOME`. See [env.md](env.md) for lookup order.

## Subcommands

### `tuse [-c|--config PATH] [-r|--resume ID]`

Opens a full-screen TUI session in the current working directory (or in the saved session's working directory when `--resume` is given and that directory still exists).

- Loads and validates the effective Lua config before opening the terminal UI.
- When `--resume ID` is passed, verifies the session exists in SQLite and acquires its session lock; fails if missing or open in another process.
- On normal exit (`/quit`, `Ctrl+C` on empty input, `Ctrl+D` on empty input, or handled signal where the main loop shuts down), closes the session, restores the terminal, and prints `tuse: session <ID>` to stderr. If the TUI watchdog forces an emergency exit instead ([tui.md](tui.md)), no session ID is printed.

See [tui.md](tui.md) and [sessions.md](sessions.md).

### `tuse json [-c|--config PATH] [-r|--resume ID]`

Starts a single-session JSON-RPC 2.0 server over stdin/stdout (newline-delimited JSON).

- Registers `SIGINT`, `SIGTERM`, and `SIGHUP` handlers for graceful session shutdown.
- Tuse writes only JSON-RPC frames to stdout; diagnostic messages go to stderr.
- Shares the same SQLite database and session locks as the TUI.

See [json.md](json.md).

### `tuse connect --base URL (--key '$ENV' | --key-stdin) --name NAME [--api TYPE] [--replace]`

Verifies an API endpoint and saves the provider in `~/.tuse/tuse.db`. See [providers.md](providers.md) for credential storage, protocol details, and API types.

- **Required flags:** `--base`, `--name`, and either `--key` or `--key-stdin` (not both).
- **Optional flags:**
  - `--api TYPE` selects the API protocol (`openai-chat`, `openai-responses`, `anthropic-messages`, or `google-gemini`; defaults to `openai-chat`). An invalid value fails before any network request.
  - `--key-stdin` reads the API key value from stdin (trimmed of trailing `\r`/`\n`) instead of command-line argv so secrets do not appear in shell command strings or process listings.
  - `--replace` verifies the endpoint first and, only if verification succeeds, atomically updates an existing provider with the same `NAME` (a failed verification leaves the existing provider untouched).
- **Validation:**
  - `URL` must start with `http://` or `https://`.
  - `NAME` cannot contain `/` or any whitespace, and must not match an already-saved provider unless `--replace` is passed (without `--replace`, remove an existing one first with `tuse disconnect NAME`).
  - If the key value is `$VAR` or `${VAR}` (where `VAR` contains only ASCII letters, digits, and `_`), Tuse resolves it from the process environment for verification; an unset or empty resolved key fails before any network call. Quote `'$VAR'` in your shell so the reference—not the expanded secret—is stored in SQLite.
- **Verification:** Calls the selected adapter's model-listing endpoint (`GET <URL>/models`, with adapter-specific auth headers and pagination). Succeeds on any valid JSON response (even if it yields zero usable model IDs); it checks model listing and does not test generation.
- **Result:** Persists the provider and prints `Received <N> models. Saved \`<NAME>\`.` to stdout.

### `tuse providers [--json]`

Lists all saved providers from SQLite in insertion order. Without flags, prints one line per provider as `<NAME>\t<BASE_URL>` (strictly two columns; prints nothing if empty). With `--json`, prints a JSON array `[{"name": "...", "base_url": "...", "api": "..."}]`. Never prints API keys or key references.

### `tuse disconnect NAME`

Removes the named provider from SQLite. Prints `Removed \`<NAME>\`` on success, or fails with `no provider \`<NAME>\`` if not found.

### `tuse models [-c|--config PATH]`

Inspects the saved global default model/effort and the model metadata from the effective Lua config without creating a session directory or acquiring a session lock.

- First line: `Default: <model or (none)> (effort: <effort>)`.
- If no models are defined in the effective config: prints `No models configured.`.
- Otherwise: prints one line per distinct model `name` in alphabetical order (when duplicate `name` entries exist in config, the last definition wins):
  `<name>\t<parts joined by " · ">`
  where parts include (when set): display name, `<window> ctx`, `compact <threshold>` or `compact off`, and `$<in>/$<out> (cache $<cache_read>/$<cache_write>) per 1M`.

### `tuse init`

Creates `./tuse.lua` in the current working directory with a starter project config that loads the global config via `local ok, g = pcall(require, "global")`, falling back to `{}`.

- Fails with `<path> already exists` if `./tuse.lua` already exists.
- Prints whether an existing global config (`~/.config/tuse.lua` or `~/.config/tuse/init.lua`) was found.

### `tuse update`

Downloads, verifies, and atomically replaces the currently running `tuse` binary from the latest release (or `TUSE_RELEASE_URL`) without touching any user configuration or SQLite state. See [install.md](install.md).

### `tuse version` / `-V` / `--version`

Prints `tuse <version>` (for example `tuse 0.9.2`) to stdout and exits 0.

### `tuse help` / `-h` / `--help`

Prints the full CLI usage summary to stdout and exits 0.
