# Environment & Config Lookup

| Variable | Meaning |
| --- | --- |
| `TUSE_MODE` | The current front end: `tui` or `json`. Read it with `os.getenv("TUSE_MODE")` or `require("tuse").env.TUSE_MODE`; also set in `sh`/tasks. |
| `TUSE_SESSION` | The current session ID, set in `sh` and tasks after session startup. |
| `TUSE_GLOBAL_CONFIG` | Global config: a file, or a module path (`~/x` loads `~/x.lua` or `~/x/init.lua`). Default `~/.config/tuse.lua`, else `~/.config/tuse/init.lua`. In Lua and `sh` it is the file found, unset when there is none. |
| `TUSE_HOME` | Where session data and providers live. Default `~/.tuse`. |
| `TUSE_RELEASE_URL` | Release download base for the installer and `tuse update`. Default: GitHub releases. |
| `TUSE_CURRENT_CONFIG` | Path of the selected config, when there is one; set in `sh` and tasks. |
| `TUSE_CWD`, `TUSE_MODEL` | The working directory and model of the session; set in `sh` and tasks. |

Every variable here is visible in Lua through `os.getenv(NAME)` and `require("tuse").env.NAME`.

Tuse chooses `--config PATH` first, then `./tuse.lua`, then the global config. Use
`require("global")` inside a project config to load the global one. See [config](config.md) for
`require` lookup rules.
