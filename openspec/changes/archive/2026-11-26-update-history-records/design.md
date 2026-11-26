## Context
`Record` is serialized with serde tag `t`; `HistoryItem` renames tags and some fields to roles. `decode_record` maps them back. The DB stores `body` JSON per row in `history`.

## Decisions
- `Record` uses `#[serde(tag = "role")]` with variant names equal to roles, and split variants instead of a `kind` field. `pick`/`ask` and `task`/`command`/`open` share fields: `Pick`, `Ask`, `Task`, `Command`, `Open`. `HistoryItem` becomes the stored JSON plus `summary`; `decode` is plain serde plus the existing ordering checks.
- `lua` record fields: `id`, `code` (the Lua source), `output`, `exit`, `images`, `diffs`. `agent.tool_calls` keeps the raw call for the provider wire format; the duplicate is accepted because providers need it.
- `message { level, text }`; `meta { title?, model?, effort? }` with at least one field set.
- Reset: `user_version` below 3 drops `sessions` and `history` and recreates them; `providers` and `state` are kept.

## Risks
- Old sessions are lost. Accepted for the 0.13.0 breaking release.
