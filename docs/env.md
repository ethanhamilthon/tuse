# Environment & Config Lookup

## Environment

| Variable | Meaning |
| --- | --- |
| `TUSE_SESSION` | The current session ID (set in `sh`, tasks and `os.environ`). |
| `TUSE_GLOBAL_CONFIG` | Path of the global config file. Default `~/.config/tuse/main.yaml`. |
| `TUSE_HOME` | Where sessions live. Default `~/.tuse`. |
| `TUSE_RELEASE_URL` | Where `tuse reset` and `tuse update` download from. Default: the latest GitHub release. |
| `TUSE_CURRENT_CONFIG` | Path of the config file in use (`--config`, `./tuse.yaml` or the global one); not set without a file. Tuse does not read it. |
| `TUSE_CWD`, `TUSE_MODEL` | The working directory and the model of the session. |

## Config lookup

Tuse reads one config file:

1. `--config PATH`, if given.
2. `./tuse.yaml` in the working directory.
3. `$TUSE_GLOBAL_CONFIG`, else `~/.config/tuse/main.yaml`.

The system prompt (this tool and its functions) and the default
settings are built into the binary; the `systems` of the config come after them. Put your own
settings in `main.yaml`. `/reload` rebuilds the config after an edit.
