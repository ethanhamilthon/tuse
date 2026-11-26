## Why

To try another direction from an earlier point, the user must start over or edit history from Lua. A fork should branch the session at any user or agent message and leave the original untouched.

## What Changes

- New `/fork` command opens a picker of the user and agent messages of the current session, newest first, with the role and a short preview.
- Picking one creates a new session in the same folder with the same model and effort, whose history is a copy of the original through the picked message and opens it. Forking at a user message leaves that message out and puts its text in the input.
- The original session is not changed and stays in `/resume`.
- The fork title is `[fork] ` followed by the original title.
- JSON-RPC gets `session.fork` with the same behavior, for parity; its result carries the `draft` text.
- `fork` becomes a reserved command name.

## Impact

- Specs: `tui` (new "Fork command"), `session` (new "Forked sessions").
- Code: `crates/core/src/session/launch.rs`, `crates/core/src/storage/store.rs`, `crates/tui/src/ui/commands.rs`, `crates/tui/src/ui/picker.rs`, `crates/rpc/src/methods.rs`, `crates/rpc/src/lib.rs`, `docs/usage.md`, `docs/json.md`.
