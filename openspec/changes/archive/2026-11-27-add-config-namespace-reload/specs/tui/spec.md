## ADDED Requirements

### Requirement: Namespace parameter for JSON-RPC config reload
In protocol version 2, `config.reload` SHALL accept optional `params.ns` as a non-empty string naming the active namespace to reload. Omitting `ns` or setting it to `null` SHALL perform a full reload. Other types and the empty string SHALL fail with `-32602`. Scoped reload SHALL use the existing response contract: `null` on success and `-32000` with the reason when reload fails. A successful scoped reload SHALL rebuild the session system prompt, trigger `config_load`, and emit the existing `reloaded` event. A failed reload SHALL NOT emit `reloaded`. TUI `/reload` and Lua `reload()` SHALL continue to perform full reloads.

#### Scenario: Reload one namespace over JSON-RPC
- **WHEN** a client calls `config.reload` with `{"ns":"work"}` and the reload succeeds
- **THEN** it receives `null`, `config.get` reflects the resulting merged config, the system prompt is rebuilt, `config_load` is triggered, and the usual `reloaded` event is emitted

#### Scenario: Optional parameter preserves full reload
- **WHEN** a client calls `config.reload` without `ns` or with `{"ns":null}`
- **THEN** it performs the same full reload as before

#### Scenario: Invalid namespace parameter
- **WHEN** a client passes `{"ns":42}` or `{"ns":""}`
- **THEN** the server returns `-32602` without running a reload

#### Scenario: Unknown namespace
- **WHEN** a client passes a non-empty namespace name not present in the active config
- **THEN** the server returns `-32000` with a reason and does not emit `reloaded`
