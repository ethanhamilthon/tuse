# Environment & Config Lookup

Tuse leaves ordinary process environment variables alone in both Lua (`os.getenv`) and `sh`/tasks.
Tuse itself reads these:

| Variable | Read by | Meaning |
| --- | --- | --- |
| `THIS_PATH` | Lua | The path of the Lua file calling `os.getenv("THIS_PATH")`; `nil` for inline code. |
| `TUSE_RELEASE_URL` | installer, `tuse update` | Release download base URL. Default: the latest GitHub release. |
| `TUSE_VERSION` | installer only | Release to install, like `0.9.2`. Default: the latest. |
| `TUSE_BIN_DIR` | installer only | Folder for the binary. Default: `~/.local/bin`. |

`TUSE_RELEASE_URL` takes precedence over `TUSE_VERSION`: with both set, the installer downloads
from the URL and ignores the version pin.

Tuse chooses `--config PATH` first, then `./tuse.lua`, then `~/.config/tuse.lua`, then
`~/.config/tuse/init.lua`. Use `require("global")` inside a project config to load the global one.
See [config](config.md) for `require` lookup rules.

