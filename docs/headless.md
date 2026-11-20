# Headless mode

`tuse -p PROMPT` runs one prompt to the end without the TUI, for scripts and CI.

```sh
tuse -p "fix the failing test"            # the prompt as arguments
git diff | tuse -p -                      # `-` reads the prompt from stdin
tuse -p "go on" --resume ID               # continue a saved session
tuse -p "review" -c ci.yaml               # another config file
```

- **stdout** gets the text of every assistant message, in order (also the text written before a
  tool call). Nothing else goes there.
- **stderr** gets `tuse: session ID` first, then notices, retries and errors, each prefixed with
  `tuse:`.
- **Exit code** is `0`, or `1` when the turn ended with an error (or there was no prompt).
- `ask` and `draw_ui` are unavailable: they raise `RuntimeError` ("ask is unavailable in headless
  mode..."), also inside `task_run` tasks and hooks. The system prompt tells the agent it runs
  headless, so it decides by itself instead of asking.
- When the turn ends, background tasks are stopped, `process_end` hooks run and Tuse exits.
- The session is saved in `~/.tuse/<id>/` like any other: open it later in the TUI with
  `tuse --resume ID`, or continue a TUI session headless with `tuse -p ... --resume ID`.
- Ctrl+C (SIGINT), SIGTERM and SIGHUP stop the turn and close the session.
