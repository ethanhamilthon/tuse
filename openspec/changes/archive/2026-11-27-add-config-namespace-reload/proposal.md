## Why

JSON-RPC clients can only reload the entire session config today. A client should be able to apply a change to one active namespace without also applying pending changes to other namespaces or plugins.

## What Changes

- Add optional `params.ns` to JSON-RPC `config.reload`. Omitting it keeps full reload behavior.
- Evaluate the normal config sources once per scoped reload, then apply only the named namespace to the current session. If the name is contributed by both local and global config, update both contributions and merge them using the existing rules.
- Reject names not in the active config and reject removal of an active namespace contribution. Use a full reload to add or remove namespaces.
- Keep Lua `reload()` and TUI `/reload` as full reloads.

## Impact

- Affected capabilities: `config`, `tui` (JSON-RPC v2), `docs`.
- Affected code: core config loading, merge and session commit; RPC parameter dispatch; config and RPC tests; `docs/config.md` and `docs/json.md`.
- No protocol version bump or change to the RPC success response or reload event shape.
