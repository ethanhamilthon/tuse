## ADDED Requirements

### Requirement: Document scoped config reload
The `json` and `config` topics SHALL document the optional `config.reload` parameter `ns`, that it selects an active namespace across local and global layers, that the config still executes in one normal load pass, and that adding or removing a namespace requires a full reload. They SHALL distinguish scoped reload from the default full reload.

#### Scenario: Discover namespace reload
- **WHEN** a reader looks up `config.reload` in `docs("json")` or the reload section of `docs("config")`
- **THEN** they can see how to reload one namespace and when a full reload is required
