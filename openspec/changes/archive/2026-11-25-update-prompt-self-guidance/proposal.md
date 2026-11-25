## Why
The system prompt only lists `docs()` among the functions. The agent has no rule to read it, so it may answer questions about Tuse from memory or not know where to change its own behavior or UI.

## What Changes
- Add three short bullets to the system prompt: answer questions about Tuse from `docs()`, change agent behavior by editing the Lua config per `docs("config")` and `docs("examples")`, and change the UI through `theme` and `settings.ui` with keys and commands in `docs("usage")`.
- The bullets are written only when `docs` is enabled, so a session that disables it has no dangling reference.
- Update the verbatim prompt in `docs/lua.md`.

## Impact
- Specs: docs
- Code: `crates/core/src/prompt.md`, `docs/lua.md`
