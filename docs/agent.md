# Tuse for the agent

You run inside Tuse. This page is what you need to work with it. Read it with `tuse docs`.

## Commands

`-s ID` picks the session; it defaults to `$TUSE_SESSION`, which is set for your bash calls.

- `tuse ask [--async] QUESTION OPTION...` asks the user to pick an option and prints the answer.
  With `--async` it prints an ID right away and the answer comes later as a message.
- `tuse task run [--name NAME] -- COMMAND...` starts a background process and prints its ID.
  Use it for servers and watchers. You get a message when it exits.
  `tuse task get ID [--lines N]` shows its status and output, `tuse task stop ID` stops it,
  `tuse task list` lists them.
- `tuse print [--id ID] [TEXT|-]` shows plain multiline text in the user's UI, without Markdown,
  and prints a block ID. `tuse print --id ID TEXT` replaces the text of that block, so a script
  can redraw its own interface in place. The text may start with `-` (only `-s` and `--id` are
  flags). It fails in headless sessions or with empty text.
- `tuse steer [--file PATH]... [TEXT|-]` sends a message to a session as if the user typed it.
- `tuse connect` adds a provider (the user runs it).
- `tuse reset [--yes]` replaces the global config folder with the latest `main.yaml` and
  `default.yaml`. It deletes everything in that folder.
- `tuse update` updates the binary and `default.yaml`; `main.yaml` is not touched.
- `tuse docs` prints this page and the config reference.
- `tuse -p [PROMPT|-]` runs a headless session: replies go to stdout, the session ID to stderr.

## User commands

`/new`, `/resume`, `/models`, `/theme`, `/tasks`, `/usage`, `/trace`, `/reload`, `/quit`.
`/usage`, `/trace` and a task's output open in a panel over the chat (Esc closes it); they are
not part of the conversation. A panel has a rule on top. `/trace` is a list, one row per
item (`system`, `user`, `agent`, `thinking`, `bash`, `output`); Up and Down move the selection,
Enter opens the item in full, Esc goes back to the list and then closes the panel.

A session keeps its own model and effort. `/models` changes them in the config and in the
current session; a new session takes them from the config; `/resume` and `/reload` do not
change the model of an old session.

## Environment

| Variable | Meaning |
| --- | --- |
| `TUSE_SESSION` | The current session ID; the default for `-s`. |
| `TUSE_GLOBAL_CONFIG` | Path of the global config file. Default `~/.config/tuse/main.yaml`. |
| `TUSE_HOME` | Where sessions live. Default `~/.tuse`. |
| `TUSE_RELEASE_URL` | Where `tuse reset` and `tuse update` download from. Default: the latest GitHub release. |
| `TUSE_CURRENT_CONFIG` | Path of the config file in use (`--config`, `./tuse.yaml` or the global one); not set without a file. Tuse does not read it. |
| `TUSE_CWD`, `TUSE_MODEL`, `TUSE_EVENT`, `TUSE_HOOK` | Set for hooks, see the config reference. |

## Config lookup

Tuse reads one config file:

1. `--config PATH`, if given.
2. `./tuse.yaml` in the working directory.
3. `$TUSE_GLOBAL_CONFIG`, else `~/.config/tuse/main.yaml`.

The app knows nothing about `default.yaml`. The shipped `main.yaml` loads it with
`include: [default.yaml]`; it holds the system prompt and the default settings, and `tuse update`
replaces it. Put your own settings in `main.yaml`. `/reload` rebuilds the config after an edit.
