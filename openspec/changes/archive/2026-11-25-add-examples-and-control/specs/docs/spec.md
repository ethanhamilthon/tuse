## MODIFIED Requirements

### Requirement: Documentation topics
Tuse SHALL ship exactly eight documentation topics, in this order: `readme`, `usage`, `connect`, `config`, `lua`, `session`, `json`, `examples`. `readme` SHALL be the content of `README.md` and every other topic the content of `docs/<topic>.md`, embedded at build time. The build MUST fail when a listed page is missing or when `docs/` holds any other `.md` file.

#### Scenario: Listing topics
- **WHEN** the agent calls `docs()`
- **THEN** it gets one `- <topic> — <summary>` line per topic in the fixed order, ending with `examples`, followed by the `Call docs("<topic>")` hint

#### Scenario: Reading examples
- **WHEN** the agent calls `docs("examples")`
- **THEN** it gets the content of `docs/examples.md`

#### Scenario: Stray page
- **WHEN** `docs/extra.md` exists or `docs/examples.md` is missing
- **THEN** `cargo build` fails
