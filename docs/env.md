# Environment & Config Lookup

Tuse leaves ordinary process environment variables alone in both Lua (`os.getenv`) and `sh`/tasks.
Only two Tuse-specific variables exist:

| Variable | Meaning |
| --- | --- |
| `THIS_PATH` | Lua only: the path of the Lua file calling `os.getenv("THIS_PATH")`; `nil` for inline code. |
| `TUSE_RELEASE_URL` | Release download base URL override for the installer and `tuse update`. Default: GitHub releases. |

Tuse chooses `--config PATH` first, then `./tuse.lua`, then `~/.config/tuse.lua`, then
`~/.config/tuse/init.lua`. Use `require("global")` inside a project config to load the global one.
See [config](config.md) for `require` lookup rules.

## Migrating from 0.8.x environment variables

`require("tuse").env` and the old `TUSE_*` environment variables (`TUSE_MODE`, `TUSE_SESSION`,
`TUSE_CWD`, `TUSE_MODEL`, `TUSE_CURRENT_CONFIG`, `TUSE_HOME`, `TUSE_GLOBAL_CONFIG`) were removed
in 0.9.0 and are no longer injected into Lua, `sh`, or background tasks.

- **Runtime session values (`TUSE_MODE`, `TUSE_SESSION`, `TUSE_CWD`, `TUSE_MODEL`, `TUSE_CURRENT_CONFIG`, `tuse.env`):**
  Call `require("tuse").session()` instead. For example:
  - `os.getenv("TUSE_MODE")` -> `require("tuse").session().mode`
  - `os.getenv("TUSE_CWD")` -> `require("tuse").session().cwd`
  - `os.getenv("TUSE_SESSION")` -> `require("tuse").session().id`
  - `os.getenv("TUSE_MODEL")` -> `require("tuse").session().model`
  - `os.getenv("TUSE_CURRENT_CONFIG")` -> `require("tuse").session().configpath`
  - `os.getenv("TUSE_HOME")` -> `require("tuse").session().home`
  If a shell command needs any of these values, pass them explicitly via `sh(cmd, { env = { ... } })`.
- **`TUSE_HOME`:** Tuse always stores state in `~/.tuse` (`$HOME/.tuse`). To keep state at a
  custom location, symlink `~/.tuse` to that directory or run with a dedicated `HOME`.
- **`TUSE_GLOBAL_CONFIG`:** Place or symlink your global config at `~/.config/tuse.lua` or
  `~/.config/tuse/init.lua`, or pass `--config PATH` (or keep `./tuse.lua` in your project).
