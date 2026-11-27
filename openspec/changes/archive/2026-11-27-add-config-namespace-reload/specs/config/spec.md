## ADDED Requirements

### Requirement: Reload one active namespace
A scoped reload SHALL target an exact namespace name in the current session's active config. For an active name, it SHALL evaluate the normal config sources in one load pass, including global config and plugins when selected by the usual loader rules. From that pass it SHALL apply only the local and global contributions with the requested name. It SHALL retain all other active contributions, plugin metadata, and saved provider settings. It SHALL apply the existing merge order, precedence, and conflict checks to the resulting config. Scoped reload limits what is applied, not which Lua code executes.

#### Scenario: Refresh a namespace contributed by both layers
- **WHEN** the active local and global configs both contribute to `work`, their files change `work` and other namespaces, and a client reloads `work`
- **THEN** the normal config sources are evaluated in one pass, both new `work` contributions take effect using the existing merge rules, and unrelated namespace and plugin contributions remain unchanged

#### Scenario: Unselected changes are not applied
- **WHEN** a fresh contribution outside the selected namespace would clash with another contribution if applied, but config evaluation succeeds and the selected contribution does not clash with the active config
- **THEN** the scoped reload succeeds without applying the unrelated change

### Requirement: Scoped reload failures preserve active configuration
A scoped reload SHALL fail if the target name is not active, if any previously active local or global contribution for that name is missing from the newly evaluated config, or if evaluation, validation of the selected contributions, or merging the candidate fails. It SHALL leave the previous active config, including its Lua callbacks, in place and preserve the existing failed-reload module-cache and preload restoration semantics. Adding or removing a namespace name SHALL require a full reload.

#### Scenario: Namespace was added on disk
- **WHEN** a client requests a namespace that exists on disk but is not active in the current session
- **THEN** the request fails and the new namespace does not become active

#### Scenario: Namespace was removed from one layer
- **WHEN** an active namespace has both local and global contributions and the fresh config no longer returns its local contribution
- **THEN** scoped reload fails, both active contributions remain, and a full reload is required to apply the removal

#### Scenario: Selected namespace conflicts with an unchanged namespace
- **WHEN** the new selected contribution defines a command already defined by another active namespace
- **THEN** scoped reload fails and the previous config and callbacks remain active
