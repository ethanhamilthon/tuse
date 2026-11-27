## Context

The loader evaluates the primary config, the global config selected by the primary config, and the selected plugins. Local and global namespaces are separate contributions until the merge. The session retains the merged Rust config and Lua callback table, which do not preserve namespace boundaries.

## Approach

1. Keep an active snapshot of the ordered contributions in the session's Lua VM, with explicit namespace name and layer for local and global entries. Keep plugin entries separate. Initialize the snapshot at launch and replace it on successful full reload.
2. Under the existing reload lock, check `ns` against the active snapshot and use the existing trusted config evaluation and cache invalidation path for one normal pass. Honor normal primary/global options and plugin selection for evaluation. Do not evaluate the config a second time to rebuild the session config. A scoped reload is not an execution sandbox: top-level code and modules can run as on full reload, and arbitrary Lua side effects are not rolled back.
3. Select freshly returned local and global contributions with the exact requested name. Fail if any previously active contribution for that name is missing from its layer; a full reload is required to remove it. A matching contribution can appear in another layer as long as none of the active contributions disappeared. Use the active snapshot for every other contribution, plugin metadata, and saved provider inputs; do not apply changes to those from the evaluation pass.
4. Validate the selected contributions and merge the resulting candidate using the normal ordering, duplicate checks, and UI precedence. A merge conflict with an unchanged contribution is still an error. Unselected contribution contents are not validated as replacements; evaluation failures and invalid loader controls can still abort the pass.
5. Publish the Rust config, Lua callbacks and contribution snapshot together on success. Preserve the active config and restore the prior module cache and preload state on failure. Keep the successful reload lifecycle, including system prompt rebuild, `config_load` and `reloaded`.
