# Environment & Config Lookup

Tuse leaves ordinary process environment variables untouched in Lua (`os.getenv`), `sh()`, and background tasks. It does not inject `TUSE_*` variables into child processes or Lua.

## Environment variables

| Variable | Read by | Meaning |
| --- | --- | --- |
| `THIS_PATH` | Lua `os.getenv` | Path of the Lua file calling `os.getenv("THIS_PATH")`; `nil` for inline code (such as the `lua` tool). Not exported to `sh()` or subprocesses. |
| `TUSE_RELEASE_URL` | installer, `tuse update` | Release download base URL (ignored when empty). Default: `https://github.com/ethanhamilthon/tuse/releases/latest/download`. |
| `TUSE_VERSION` | installer only | Release version to install, with or without a leading `v` (for example `0.9.2` or `v0.9.2`). Default: latest release. |
| `TUSE_BIN_DIR` | installer only | Destination directory for the `tuse` binary. Default: `~/.local/bin`. |
| `HOME` | Tuse, installer | Standard home directory used to expand `~/` and locate `~/.tuse`, `~/.config/tuse.lua`, `~/.config/tuse/init.lua`, and the default installer target. |

In the installer, a non-empty `TUSE_RELEASE_URL` takes precedence over `TUSE_VERSION`: when both are set, the installer downloads from `TUSE_RELEASE_URL` and ignores `TUSE_VERSION`. `tuse update` reads only `TUSE_RELEASE_URL` and replaces the running executable in place.

### Provider key references

When saving a provider with `tuse connect --key '$VAR'` (or `'${VAR}'`), where `VAR` consists of ASCII letters, digits, or `_`, Tuse stores the literal reference in SQLite and resolves it from the process environment during `tuse connect` verification, on session startup, and on config reload. An unset variable resolves to an empty string.

## Config file lookup

Tuse selects one entry config file using this precedence:

1. `--config PATH` (`-c PATH`), resolved relative to the current directory (`~/` expands to `HOME`). If specified and missing or unreadable, startup fails rather than falling back.
2. `./tuse.lua` in the working directory (if it is a regular file).
3. `~/.config/tuse.lua` (if it is a regular file).
4. `~/.config/tuse/init.lua` (if it is a regular file).
5. Built-in defaults (no config file loaded; `session().configpath` is `nil`).

Inside a project config, `require("global")` checks `~/.config/tuse.lua` first, then `~/.config/tuse/init.lua`. See [config.md](config.md) for all `require` resolution rules and config keys.
