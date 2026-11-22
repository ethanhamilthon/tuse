# Environment & Config Lookup

## Environment

| Variable | Meaning |
| --- | --- |
| `TUSE_SESSION` | The current session ID (set in `sh` and tasks). |
| `TUSE_PARENT` | In: the ID of the parent session; a session started with it records `parent` in its meta. Out: every tuse process sets it to its own session ID in `sh` and tasks (replacing an inherited value), so nested runs form a tree for `/childrens`. |
| `TUSE_GLOBAL_CONFIG` | Path of the global config file. Default `~/.config/tuse/main.lua`. |
| `TUSE_HOME` | Where sessions live. Default `~/.tuse`. |
| `TUSE_RELEASE_URL` | Where `tuse reset` and `tuse update` download from. Default: the latest GitHub release. |
| `TUSE_CURRENT_CONFIG` | Path of the config file in use (`--config`, `./tuse.lua` or the global one); not set without a file. Tuse does not read it. |
| `TUSE_CWD`, `TUSE_MODEL` | The working directory and the model of the session. |

## Config lookup

Tuse reads one config file:

1. `--config PATH`, if given.
2. `./tuse.lua` in the working directory.
3. `$TUSE_GLOBAL_CONFIG`, else `~/.config/tuse/main.lua`.

The system prompt (this tool and its functions) and the default
settings are built into the binary; the `systems` of the config come after them. Put your own
settings in `main.lua`. `/reload` runs the config again after an edit.
