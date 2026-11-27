## ADDED Requirements

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
