# Environment & Config Lookup

| Variable | Meaning |
| --- | --- |
| `TUSE_MODE` | The current front end: `tui` or `json`. Set by Tuse in Lua (`TUSE_MODE` and `os.getenv("TUSE_MODE")`) and in `sh`/tasks. |
| `TUSE_SESSION` | The current session ID, set in `sh` and tasks after session startup. |
| `TUSE_GLOBAL_CONFIG` | Global config file path. Default `~/.config/tuse/main.lua`. |
| `TUSE_HOME` | Where session data and providers live. Default `~/.tuse`. |
| `TUSE_RELEASE_URL` | Release download base for the installer and `tuse update`. Default: GitHub releases. |
| `TUSE_CURRENT_CONFIG` | Path of the selected config, when there is one; set in `sh` and tasks. |
| `TUSE_CWD`, `TUSE_MODEL` | The working directory and model of the session; set in `sh` and tasks. |

Tuse chooses `--config PATH` first, then `./tuse.lua`, then the global config. Use
`include = { os.getenv("TUSE_GLOBAL_CONFIG") }` inside a project config to load an external
config explicitly. `require` searches the session's working directory for Lua modules.
See [config](config.md) for the merge rules.
