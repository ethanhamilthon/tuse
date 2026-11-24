# Planned: Install and Update Plugins on Reload (TODO #6)

## Goal

Synchronize enabled Git plugins automatically on every config reload (`/reload`, JSON-RPC `config.reload`, and Lua `reload()`): clone newly configured plugins, update moving refs (branches, tags, default branch) when upstream advances, keep commit pins fixed, never overwrite local work, stay safe across concurrent Tuse processes, and keep the previous working config active if anything fails.

## Target Behavior

### Triggers and Scope

- **Sync triggers**: `/reload` (TUI), `config.reload` (JSON-RPC), and `reload()` (Lua) all execute the same reload pipeline and synchronize enabled Git plugins before loading and validating them.
- **Decision:** Initial startup (`tuse`, `tuse json`), session switches (`/new`, `/resume`, `session.new`, `session.resume`), and `tuse models` remain offline and read existing checkouts on disk without touching the network. Only reload triggers Git network operations.
- **Disabled plugins** (`enabled = false`): schema and path syntax are still validated as today, while checkout checks, ref/URL conflict checks, Git sync, loading, and listing are skipped.
- **Removed plugins**: unloaded on successful reload.
  - **Decision:** Checkouts for removed plugins remain on disk under `~/.tuse/plugins/` rather than being auto-deleted, avoiding accidental loss of local files.
- **Local `dir` plugins**: re-read directly from disk on every reload with no Git or network commands.

### Repository Identity, Conflict Checks, and Deduplication

1. Evaluate the user config file and validate the user config table (excluding `plugins`).
2. Parse all `plugins` entries (validating syntax on all entries, then filtering to enabled entries).
3. Check repository identity conflicts across enabled Git entries **before** running any Git command:
   - **URL normalization**: normalize a repository URL by stripping trailing `/` and an optional trailing `.git` suffix (preserving scheme, host, user, and path so HTTPS and SSH URLs remain distinct).
   - **URL conflict**: two enabled entries mapping to the same checkout root (`~/.tuse/plugins/...`) with different normalized URLs (for example, `https://github.com/a/b.git` and `git@github.com:a/b.git`, which both map to `~/.tuse/plugins/github.com/a/b`) fail validation with `plugins: <a> and <b> map to the same checkout with different URLs`.
   - **Ref conflict**: two enabled entries mapping to the same checkout root with conflicting non-nil `ref` values fail validation before any Git command runs. If one entry sets `ref` and another omits `ref`, the non-nil `ref` applies to that repository.
4. Group enabled Git entries by checkout root and synchronize each unique repository **at most once** per reload.
5. Load and validate every enabled plugin, merge plugins under the user config, and validate the final merged config **before** firing `close` hooks or committing the reload.

### Cross-Process Locking and Non-Interactive Git Execution

Because multiple Tuse processes share `~/.tuse/plugins/`, every repository sync acquires an exclusive advisory lock before inspecting or mutating that checkout:

- **Decision:** Acquire an exclusive per-repository advisory file lock (for example, a sibling lock file under `~/.tuse/plugins/.locks/`) for the duration of that repository's sync (blocking up to the per-repository timeout; failing clearly if the lock cannot be acquired in time).
- **Non-interactive environment**: invoke `git` with stdin closed (`/dev/null`) and:
  - `GIT_TERMINAL_PROMPT=0`
  - `GCM_INTERACTIVE=never`
  - `GIT_ASKPASS=/usr/bin/true` (if not already set)
  - `GIT_SSH_COMMAND="ssh -o BatchMode=yes"` (when the user has not set `GIT_SSH_COMMAND`)
  - **Decision:** Enforce a 60-second per-repository timeout (killing the Git process group on timeout). Note that environment variables cannot disable every third-party GUI credential helper configured in a user's Git config; the bounded timeout ensures reload fails cleanly rather than hanging indefinitely.

### Unified Ref Resolution (Identical for Fresh Clone and Reload)

Both fresh clones and reloads resolve the target commit identically from the fetched repository state:

- **When `ref` is omitted**: resolve the remote's default branch (`refs/remotes/origin/HEAD`, refreshing the remote `HEAD` symref on fetch so default-branch renames such as `master` -> `main` are detected).
- **When `ref` is set**: resolve in order:
  1. Exact tag `refs/tags/<ref>` (peeled to its target commit).
  2. Exact remote-tracking branch `refs/remotes/origin/<ref>`.
  3. Commit SHA (40-char hex, or unique 7–39-char hex resolving to a commit object).
  4. Otherwise fail with `plugin <name>: ref '<ref>' not found`.
- **Commit pin rule**:
  - **Decision:** Treat `ref` as an immutable commit pin only when it is a 40-char hex SHA (or 7–39-char hex string that does not match any tag or remote-tracking branch name) AND the local checkout's `HEAD` already resolves to that exact commit. In that case, skip fetching and leave the checkout untouched. Tags and branches are moving refs and always fetch on reload.
- **Detached `HEAD` target checkout**:
  - **Decision:** On both fresh clone and reload updates, check out the resolved target commit in detached `HEAD` mode (without `--force`). This guarantees identical behavior across default branch, named branch, tag, and commit pin, avoids stale local branch divergence, and never overwrites colliding untracked/ignored files.

### Per-Repository Sync Rules

#### 1. Missing Checkout (New Install)

When the checkout root does not exist on disk (holding the per-repository lock):

- **Decision:** Clone into a sibling temporary staging directory under `~/.tuse/plugins/`, resolve `ref` (or remote default branch) using the unified rules above, check out the target commit in detached `HEAD`, and atomically rename the staging directory into the target checkout root.
- If any step fails (network, auth, timeout, invalid `ref`), delete the temporary staging directory so no broken checkout is left behind.

#### 2. Existing Checkout

When the checkout root already exists (holding the per-repository lock):

1. Verify it is a valid Git repository (a non-Git directory fails without modifying it).
2. Verify `origin` remote URL matches the entry's normalized URL; mismatch fails with `plugin <name>: checkout remote URL '<actual>' does not match '<expected>'` rather than overwriting `origin`.
3. If `ref` satisfies the immutable commit pin rule and `HEAD` already matches it, return immediately without fetching.
4. Fetch from `origin` while pruning deleted remote branches and tags (`git fetch --prune --prune-tags --tags origin`, plus refreshing `origin/HEAD` when `ref` is omitted so deleted/renamed remote refs and moved tags stay accurate).
5. Resolve the target commit using the unified ref resolution rules above.
6. Compare target commit SHA with current `HEAD` SHA:
   - **Same commit**: do not touch the working tree or `HEAD`.
   - **Different commit**: run local-work safety checks below, then check out the target commit (without `--force`).

#### 3. Local-Work Safety Checks (Never Overwrite Local Changes)

Before moving `HEAD` to a different commit on an existing checkout:

1. **No destructive Git commands**: never run `git reset --hard`, `git clean`, or `git checkout --force`.
2. **Clean working tree**: check Git status for any staged changes, unstaged tracked changes, or untracked non-ignored files.
3. **No in-progress Git operation**: reject if merge, rebase, cherry-pick, revert, or bisect state is active.
4. **Reachability check for `HEAD` (covers both local branches AND detached `HEAD`)**: verify the current `HEAD` commit is reachable from at least one remote-tracking ref or tag (`refs/remotes/origin/*` or `refs/tags/*`). Any unpublished local commit—whether on a local branch or on detached `HEAD`—fails this check and is never orphaned.
5. **Colliding ignored files**: because checkout runs without `--force`, if an ignored file on disk would be overwritten by a tracked file in the target commit, Git refuses the checkout and reload fails without overwriting that file.
- **Decision:** If a checkout needs to move to a different commit and fails any safety check above, refuse to switch, leave the working tree untouched, and fail the reload naming the plugin. If the target commit already equals `HEAD` (or is an already-checked-out commit pin), leave local files untouched and load the checkout as-is.

### Failure Handling and Cross-Process Safety

- Syncing, plugin evaluation, merging, and schema validation all run before `close` hooks fire or the active config swaps.
- **Decision (safer cross-process model):** Do **not** roll back already-synced sibling repositories on disk when a later plugin fails sync or validation. Instead:
  - Each individual repository sync is self-contained (fresh clones stage and rename atomically; existing checkouts switch only after passing all safety checks).
  - If any plugin fails sync, checkout, load, or validation, the reloading session performs **in-memory rollback**: previous config, Lua module cache, helpers, hooks, commands, and `open` handlers stay active.
  - This avoids a failed reload in one Tuse process reverting a valid checkout update made by another concurrent Tuse process, and avoids double-fault disk rollback errors.
  - If a single repository's checkout step itself fails unexpectedly, report the error clearly and leave the working tree untouched without attempting a destructive reset.

## Error Reporting

Errors identify the failing plugin (`plugin <name>: ...`):

- Missing `git` executable: `plugin <name>: git is required to sync plugins`
- Lock timeout: `plugin <name>: timed out waiting for plugin repository lock`
- Clone / fetch / checkout failure: `plugin <name>: git <op> failed: <stderr summary>`
- Remote URL mismatch: `plugin <name>: checkout remote URL '<actual>' does not match '<expected>'`
- Unknown `ref`: `plugin <name>: ref '<ref>' not found`
- Local changes or unpublished commits (branch or detached): `plugin <name>: checkout has local changes or unpublished commits; commit, push, stash, or discard them before updating`
- Non-Git directory at checkout path: `plugin <name>: <path> exists and is not a git repository`
- Plugin evaluation / validation / merge error: `<source>: <path>: <message>`

Surface behavior:

- TUI `/reload`: shows `Reload failed: <message>`; previous config stays active.
- JSON-RPC `config.reload`: returns `-32000` with `<message>`; previous config stays active.
- Lua `reload()`: raises `<message>` (or emits `error` event `reload: <message>` when deferred from a hook); previous config stays active.

## Docs-First Handoff Gate

Per TODO #3, keep baseline docs describing current behavior until ready to implement this spec. Before handing off to an implementation agent, update and review these baseline doc sections first:

- `docs/plugins.md`: update `ref` semantics (active sync vs immutable commit SHA pins), repository URL conflict rules, and replace "Managing Checkouts" with reload sync, cross-process locking, local-work safety, and offline startup behavior.
- `docs/config.md`: update "Reloading (`/reload`, `config.reload`, `reload()`)" to include Git plugin synchronization and failure semantics.
- `docs/json.md`: update `config.reload` to note Git plugin sync and sync error reporting.
- `docs/commands.md`: update `/reload` to note automatic plugin install/update.

## Acceptance Tests

1. **New install (GitHub shorthand, Git URL, subpath)**: reload clones via staging into `~/.tuse/plugins/...`, resolves default branch or `ref` identically to reload, loads plugin, and reports commit in `/config` and `config.get`; failed clone leaves no partial checkout directory.
2. **Updated branch / default branch (including default-branch rename)**: advance upstream commit (and test remote default-branch rename); reload fetches, updates checkout, and loads new code.
3. **Updated and pruned tag / deleted branch**: move an upstream tag and delete a remote branch; reload prunes deleted remote refs, loads the updated tag, and fails cleanly if a configured branch was deleted upstream.
4. **Unchanged ref**: reload when upstream commit matches `HEAD` is a no-op on the working tree.
5. **Immutable commit pin**: configure a 40-char commit SHA `ref`; advance remote branch; reload skips fetch and keeps the pinned commit fixed.
6. **Shared repository synced once**: two enabled entries from subpaths of one repository trigger only one Git fetch/sync per reload.
7. **Conflicting refs and conflicting URLs**: two entries for one checkout root with different `ref`s or different URLs (e.g., HTTPS vs SSH) fail before Git runs; an existing checkout with a mismatched `origin` URL fails without modifying `origin`.
8. **Cross-process lock**: two concurrent reloads targeting the same checkout serialize cleanly via the advisory lock without corrupting `.git` or failing spuriously.
9. **Local `dir` plugin**: edit a local `dir` plugin on disk (no `.git`, no network); reload picks up changes immediately.
10. **Disabled and removed plugins**: `enabled = false` still validates entry syntax while skipping Git sync/load; removing a loaded plugin unloads its callbacks without deleting its checkout directory.
11. **Local edits and unpublished commits never overwritten**: test modified tracked file, untracked file, colliding ignored file, unpushed branch commit, and unpublished **detached `HEAD`** commit when upstream advances; every case fails naming the plugin, leaves local files/commits untouched, and keeps previous config active.
12. **Failed reload preserves active session config**: fail fetch, unknown `ref`, or validation on a second plugin; reload reports which plugin failed and keeps previous hooks, commands, prompts, and helpers active in both TUI and JSON-RPC without reverting a concurrent session's repo update.
