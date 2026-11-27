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

### Requirement: Existing provider error suggests replace
When `tuse connect` is invoked without `--replace` for a provider name that already exists in SQLite, the error message SHALL mention both `--replace` (for atomic in-place updates) and `tuse disconnect <name>`.

#### Scenario: Duplicate provider name hint
- **WHEN** a saved provider named `openai` exists and the user runs `tuse connect ... --name openai` without `--replace`
- **THEN** the CLI fails with an error naming `openai` and mentioning both `--replace` and `tuse disconnect openai`

### Requirement: Wire effort encoding is decoupled from model availability policy
Provider adapters SHALL NOT enforce hardcoded provider-API effort allowlists as an availability policy; model effort availability SHALL be governed exclusively by the config/session model effort resolver (`config.models[].efforts`, else `low`, `medium`, `high`, `xhigh`, `max`). When no effort is active (`efforts = {}`, `Request.effort = None`), every adapter MUST omit reasoning/thinking effort parameters completely. When an effort string is active (`Request.effort = Some(effort)`):
- Adapters with string effort fields (`openai-chat` `reasoning_effort`, `openai-responses` `reasoning.effort`, `anthropic-messages` `output_config.effort`, and non-2.5 `google-gemini` `thinkingLevel`) SHALL pass the effort string verbatim with no renaming, clamping, or extra local availability matrix; upstream endpoint rejections SHALL surface as normal provider errors.
- For `google-gemini` `gemini-2.5-*` models (which use numeric `thinkingBudget` and have no equivalent string field), the adapter SHALL encode `"low" => 1024`, `"medium" => 8192`, `"high" => 24576`, and MUST NOT invent numeric mappings or silently clamp unmapped levels (`xhigh`, `max`) to `24576` — instead returning an explicit wire-encoding error advising the user to configure `config.models[].efforts`.

#### Scenario: Verbatim pass-through on string-field adapters
- **WHEN** a model with no `efforts` restriction is invoked on `openai-chat`, `openai-responses`, `anthropic-messages`, or a non-2.5 `google-gemini` model with `xhigh` or `max`
- **THEN** the adapter sends that exact effort string verbatim on its protocol field without local allowlist blocking, renaming, or clamping, and surfaces any upstream HTTP error verbatim

#### Scenario: Explicitly empty efforts omits reasoning wire parameters
- **WHEN** `config.models` declares `{ name = "openai/gpt-4o", efforts = {} }` and a turn or `llm()` call runs
- **THEN** the outgoing provider request omits reasoning/thinking effort fields completely

#### Scenario: Gemini 2.5 thinkingBudget does not clamp unmapped efforts
- **WHEN** `gemini/gemini-2.5-pro` is dispatched with `effort = "max"` (without restricting `config.models[].efforts`)
- **THEN** the `google-gemini` adapter returns a wire-encoding error rather than clamping `max` to `24576` or inventing a budget

#### Scenario: Per-model restriction enforced before dispatch
- **WHEN** `config.models` declares `{ name = "gemini/gemini-2.5-pro", efforts = { "low", "medium", "high" } }`
- **THEN** requesting `xhigh` or `max` for that model is rejected by the shared model effort resolver before any network request is sent
