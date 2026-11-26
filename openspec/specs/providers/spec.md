# providers

## Purpose
Named API connections for model discovery and generation.

## Requirements

### Requirement: Additional provider headers
`connect` SHALL accept repeatable `--header 'Name: value'`. Header names SHALL be case-insensitive and unique, and custom headers MUST NOT override adapter auth, protocol, or HTTP framing headers. Values MAY use whole-value environment references. Saved profiles SHALL retain unresolved references. Public provider listings and config views MUST NOT expose header values or keys.

#### Scenario: Compatible gateway
- **WHEN** a provider is saved with additional headers
- **THEN** the probe, model discovery, normal completion, and `llm()` send those headers as well as the standard adapter headers

### Requirement: Config-only provider profiles
Lua `providers` SHALL map names to the same connection settings as `connect`: `base`, `key`, optional `api`, and optional `headers`. Config loading SHALL validate these profiles locally, merge them with saved profiles, and fail on name collisions without writing the profiles to the database. `tuse providers` and `tuse disconnect` SHALL operate on saved entries only.

#### Scenario: Reload a project provider
- **WHEN** a valid project provider is added or removed and the config is reloaded
- **THEN** it becomes available or unavailable to that session without affecting stored providers

#### Scenario: Duplicate name
- **WHEN** config and database define the same provider name
- **THEN** loading fails without silently selecting an endpoint

### Requirement: OpenAI Responses stream finalization
The OpenAI Responses adapter SHALL finalize the reply after processing `response.completed` or `response.incomplete`, including terminal status and usage, without waiting for the HTTP body to close. `response.failed` SHALL remain an error. EOF without a recognized terminal event SHALL NOT be treated as a successful reply.

#### Scenario: Open body after a terminal event
- **WHEN** the provider sends `response.completed` or `response.incomplete` and leaves the HTTP body open
- **THEN** Tuse returns the reply with its terminal metadata without waiting for another event or EOF

#### Scenario: Premature EOF
- **WHEN** a Responses stream ends after a text delta without a terminal event
- **THEN** Tuse treats the stream as truncated rather than returning a successful reply
