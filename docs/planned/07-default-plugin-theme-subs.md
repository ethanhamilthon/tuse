# Planned: Bundled Default Plugin — `theme` and `subs` (TODO #7)

Status: **Planned** (not yet implemented). Baseline plugin behavior is documented in [../plugins.md](../plugins.md); baseline theme tokens are in [../theme.md](../theme.md).

## Goal

Bundle a default plugin with two optional parts:

1. **`theme`** — the bundled Tuse color palette (user `theme` overrides still win).
2. **`subs`** — a wrapper around [CLIProxyAPI](https://github.com/router-for-me/CLIProxyAPI) so users can use existing AI subscriptions through a local loopback-only OpenAI-compatible proxy wired as a Tuse provider.

Loading or reloading the plugin must **never** install software, start a login flow, modify providers, or launch a background proxy process. Every side effect in `subs` is an explicit user opt-in.

## Packaging, Reserved ID & Configuration

- **Decision (single reserved offline plugin ID):** Ship the default plugin embedded in the `tuse` binary (source under `plugins/default/`) under **one** reserved built-in plugin ID: `"tuse/default"`.
  - `"tuse/default"` is resolved exclusively from the binary (never cloned from GitHub and never synced over the network by `/reload`).
  - `ref`, `dir`, and `path` are rejected on `"tuse/default"` during validation (clear diagnostic: `plugin tuse/default: bundled plugin does not accept ref, dir, or path`).
  - **Installer capability gate (for [install.md](../install.md)):** Running `tuse models -c <temp_config>` with `{ plugins = { { "tuse/default", opts = { theme = false, subs = false } } } }` exits `0` on binaries that bundle `"tuse/default"` and exits non-zero on older binaries (which treat `"tuse/default"` as a missing Git checkout), allowing the installer to gate first-run setup cleanly even when `TUSE_VERSION` pins an older release.
- **Configuration syntax**:
  ```lua
  return {
    plugins = {
      {
        "tuse/default",
        opts = {
          theme = true,
          subs = false, -- set true or { ... } to opt into CLIProxyAPI commands
        },
      },
    },
  }
  ```
- **Defaults when `opts` or individual flags are omitted**:
  - `opts.theme`: `true` (boolean). Because `opts.theme` defaults to `true`, a **`subs`-only** config must set `theme = false` explicitly (`opts = { theme = false, subs = true }`).
  - `opts.subs`: `false` by default (**Decision:** `subs` defaults to `false` unless explicitly enabled with `subs = true` or `subs = { ... }`, so users who only enable `"tuse/default"` do not get proxy commands they did not ask for).
- **Validation**: unknown keys inside `opts` or `opts.subs`, wrong types, non-loopback `host`, invalid port numbers (`1..=65535`), or invalid `provider` names (`/` or whitespace) fail config validation cleanly with a diagnostic naming `"tuse/default"` and the full key path.

## Part 1: `theme`

- Returns a `theme` table defining all 10 standard tokens (`bg`, `surface`, `fg`, `dim`, `muted`, `border`, `accent`, `ok`, `err`, `info`).
- **Decision (palette vs. core defaults):** Keep the binary's built-in core fallback defaults unchanged (avoiding any breaking visual change for existing plugin-free configs), and define a distinct curated 10-token palette in `"tuse/default"`'s `theme` part (exact 10 hex values specified in the plugin table and documented in `docs/theme.md` before implementation) so `theme = true` vs `theme = false` produces an observable, testable difference. *(Open product question: alternatively, move today's dark hex palette exclusively into `"tuse/default"` and switch binary core fallback to terminal-default `none`/ANSI colors—tracked for confirmation before handoff.)*
- Follows standard plugin deep-merge rules: any color token set in the user's own `theme = { ... }` overrides the plugin's token while keeping the remaining tokens from `theme`.
- Pure static configuration: no hooks, no commands, no I/O, no side effects on startup or `/reload`. Works identically in TUI and `tuse json`.

## Part 2: `subs` (CLIProxyAPI Wrapper)

### Pinned Upstream Version & CLI / Config / Auth Contract

- **Upstream project**: `router-for-me/CLIProxyAPI`.
- **Decision (pinned release + hardcoded artifact digests):** Pin a single tested CLIProxyAPI release version (e.g., `vX.Y.Z`) and embed the exact expected **SHA-256 digest** for every supported platform archive (`darwin/arm64`, `darwin/amd64`, `linux/arm64`, `linux/amd64`) directly inside the bundled plugin manifest. Never trust a checksum file fetched from the same mutable release URL as the archive.
- **Exact CLI contract** (verified against the pinned release before handoff):
  - Proxy server: `<binary> --config <config_dir>/config.yaml`
  - OAuth login flows: `<binary> --config <config_dir>/config.yaml --no-browser <login_flag>`, supporting `--codex-login` (OpenAI/Codex), `--claude-login` (Anthropic/Claude), and `--gemini-login` (Google/Gemini).
  - Version inspection: `<binary> --version` (or the pinned release's version flag).
- **Exact file layout and permissions** under `<config_dir>` (default `~/.tuse/subs`):
  - `<config_dir>`: directory, mode `0700`
  - `<config_dir>/bin/cli-proxy-api`: downloaded binary (if installed by `setup`), mode `0755`
  - `<config_dir>/auth`: directory for CLIProxyAPI OAuth credentials, mode `0700`; all credential files created inside it tightened to mode `0600` after login
  - `<config_dir>/config.yaml`: mode `0600` (contains local bearer key), setting:
    - `host`: loopback address (`127.0.0.1`, `::1`, or `localhost`)
    - `port`: configured integer port (`8317` default)
    - `auth-dir`: `"<config_dir>/auth"`
    - `api-keys`: `["<local_api_key>"]`
    - `remote-management`: `{ allow-remote: false, disable-control-panel: true, secret-key: "" }`
  - `<config_dir>/api.key`: randomly generated 32-byte hex/base64url local API key, mode `0600`

### Options (`opts.subs`)

When `opts.subs` is `true`, all defaults below apply. When it is a table, any field may be overridden:

| Option | Type | Default | Meaning |
| --- | --- | --- | --- |
| `provider` | string | `"subs"` | Tuse provider name registered with `tuse connect` / `tuse disconnect`. Cannot contain `/` or whitespace. |
| `host` | string | `"127.0.0.1"` | **Loopback-only** bind address. **Decision:** Strictly restricted to `"127.0.0.1"`, `"::1"`, or `"localhost"`; non-loopback addresses (`0.0.0.0`, `::`, LAN IPs) fail config validation to prevent exposing subscription credentials on the network. |
| `port` | integer | `8317` | Local TCP port (`1..=65535`) for CLIProxyAPI. |
| `binary` | string \| nil | `nil` | Explicit path to the `cli-proxy-api` binary. When unset, searches `<config_dir>/bin/cli-proxy-api` then `PATH`. |
| `config_dir` | string | `"~/.tuse/subs"` | Directory for generated CLIProxyAPI config, auth credentials, local key, and optional installed binary. |

### Explicit Opt-In Guarantee

Loading the plugin, opening a session, resuming a session, or reloading (`/reload`, `reload()`, `config.reload`) MUST NOT:

- Download or install CLIProxyAPI
- Create or overwrite `<config_dir>/config.yaml` or `<config_dir>/api.key`
- Launch an OAuth/browser login flow
- Start, restart, or probe external proxy processes automatically
- Add, modify, or delete providers in `~/.tuse/tuse.db`

All state-changing actions happen **only** when the user explicitly invokes `/subs` and confirms the requested action.

### User Interface (`/subs`)

- **Decision:** Register one custom slash command `/subs` (callable from TUI `/subs` and JSON-RPC `commands.run { name = "subs" }`). Invoking `/subs` opens an interactive `pick()` menu of actions:
  1. `status` — inspect binary, config, proxy process, endpoint health, and Tuse provider wiring
  2. `setup` — locate or install CLIProxyAPI and create/verify local proxy config (with explicit confirmation for each write)
  3. `login` — authenticate a subscription account with CLIProxyAPI
  4. `start` — start the local CLIProxyAPI proxy as a managed session process
  5. `stop` — stop the proxy process started by `subs` in this session
  6. `connect` — verify and register (or atomically update) the running proxy as a Tuse provider and reload
  7. `disconnect` — verify ownership and remove the `subs` provider from Tuse and reload
- Pressing Esc on any picker cancels with no side effects and no chat clutter.
- Output and diagnostics are rendered via `show("subs", ..., false)` (UI-only Markdown block, never sent to the model, and always redacted before calling `show()`).

### Setup Flow (`setup`)

1. **Binary step**: checks for an existing binary (`opts.subs.binary`, then `<config_dir>/bin/cli-proxy-api`, then `PATH`).
   - If missing, asks for explicit confirmation via `pick({"Install CLIProxyAPI <pinned-version>", "Cancel"})` before making any network request.
   - Downloads the release archive for the current OS/arch into a staging file inside `<config_dir>` (mode `0700`), verifies its SHA-256 digest against the **hardcoded digest pinned in the bundled plugin manifest**, extracts the binary, sets mode `0755`, and atomically moves it to `<config_dir>/bin/cli-proxy-api`. On mismatch or unsupported platform, removes staging files, reports the error, and suggests setting `opts.subs.binary`.
2. **Config step (confirmed even when binary is already installed)**:
   - If `<config_dir>/config.yaml` does **not** exist: asks for explicit confirmation via `pick({"Create local config in <config_dir>", "Cancel"})` before creating `<config_dir>` (`0700`), `<config_dir>/auth` (`0700`), `<config_dir>/api.key` (`0600`), and `<config_dir>/config.yaml` (`0600`).
   - If `<config_dir>/config.yaml` **already exists**: asks `pick({"Keep existing config", "Reset config (rotates local API key)", "Cancel"})`; preserves it untouched unless reset is explicitly chosen.
3. Shows next steps (`login` -> `start` -> `connect`) in the `subs` block.

### Login Flow (`login`)

Because `sh()` redirects stdin from `/dev/null` and buffers all output until process exit, `login` **cannot** rely on a single blocking `sh()` call to display authorization URLs or device codes while waiting for OAuth completion.

- **Decision (streaming login transport):** `subs` runs the login subprocess via direct argv execution (no shell string interpolation) with a streaming output transport (either a host-provided streaming process primitive for the bundled plugin, or a `0600` temp log file inside `<config_dir>` tailed every 200 ms from the `/subs` coroutine while the login process runs and deleted unconditionally on exit/cancel):
  1. Requires binary and `config.yaml` to exist; otherwise reports an error pointing to `setup`.
  2. Prompts `pick({"OpenAI / Codex", "Anthropic / Claude", "Google / Gemini", "Cancel"})`.
  3. Runs `<binary> --config <config_dir>/config.yaml --no-browser <provider-login-flag>` with a 5-minute timeout.
  4. Streams output incrementally while running: as soon as an authorization `https://...` URL (and optional device code) appears, displays **only** the sanitized URL / device code instructions via `show("subs", ..., false)` (clickable in TUI) without printing raw output or tokens.
  5. If the user cancels (Esc / `turn.cancel`) or the 5-minute timeout elapses, kills the login process group immediately and cleans up temporary files.
  6. On success, tightens permissions on files in `<config_dir>/auth` to `0600` and reports login complete (never printing tokens or cookies). Note: flows requiring interactive stdin paste of a redirect URL containing a one-time OAuth `code=` are avoided in favor of `--no-browser` localhost-callback / device-code flows so secrets never enter `ask()` or persisted UI history (#8).

### Proxy Process Lifecycle, Ownership & Reload Reconciliation

- **Unforgeable ownership tracking**:
  - When `subs` starts CLIProxyAPI, it creates a session-scoped ownership record keyed by a unique runtime `owner_id` bound to the exact spawned `task_id` and its effective `{ host, port, config_dir, binary }`.
  - `subs` **never** identifies or claims a running task by display name (e.g., `"subs-proxy"`), so an unrelated user or agent task with the same name can never be hijacked or stopped.
- **`start`**:
  1. Verifies binary and `config.yaml` exist; if missing, reports an error pointing to `setup`.
  2. If the session already holds a live ownership record whose `task_id` is still running and healthy at `http://<host>:<port>/v1/models`, reports already running (idempotent; no duplicate spawn).
  3. Probes `http://<host>:<port>/v1/models` with the local API key:
     - If HTTP 200: reports an externally managed proxy is already listening on `<host>:<port>` and does **not** spawn a duplicate or record ownership.
     - If another unrelated service holds the port (non-200 / invalid JSON): reports a port-conflict error suggesting changing `opts.subs.port`.
  4. Spawns CLIProxyAPI as a managed background task invoking `<binary> --config <config_dir>/config.yaml` directly (no secrets in argv), wrapping stdout/stderr through the secret redactor before anything is written to the task log, and returning a fixed sanitized summary string (`"subs-proxy stopped"` / `"subs-proxy exited with code N"`) so task completion notifications sent to the model never contain raw stderr or credentials.
  5. Polls `http://<host>:<port>/v1/models` for up to 10 seconds until ready (failing fast if the task exits early). On failure, stops the task, clears ownership, and displays a redacted error summary via `show("subs", ..., false)`.
- **`stop`**:
  - Stops ONLY the exact `task_id` in the active ownership record (killing its process group) and clears ownership.
  - If an external proxy is listening on the port, does **not** stop it and explains that it is externally managed.
  - Idempotent when no managed proxy is running.
- **Session close and `/reload` reconciliation**:
  - **Session close** (`exit`, `new`, `resume`, SIGINT/SIGTERM/SIGHUP, `shutdown`): stops the owned `task_id` and clears ownership so no proxy outlives the session.
  - **Failed reload** (syntax error, schema error, or plugin sync failure under #6): reload aborts before commit; the running managed proxy and its ownership record remain completely untouched.
  - **Successful reload (post-validation reconciliation)**: because an old `close` hook runs before the new config is committed and cannot inspect the new config, reconciliation happens **after** the new config commits:
    - If the committed config still enables `subs` with identical `{ host, port, config_dir, resolved_binary }`: preserve the running owned `task_id` and transfer the ownership record cleanly to the reloaded `subs` state without restarting.
    - If the committed config **removed/disabled** `subs` OR **changed** `host`, `port`, `config_dir`, or `resolved_binary`: stop the previously owned `task_id`, clear ownership, and emit a notice explaining that the previous managed proxy was stopped (never auto-starting a proxy on reload; the user must explicitly run `/subs` -> `start`).

### Provider Wiring (`connect` / `disconnect`)

- **Secret-safe, verify-before-replace `connect`**:
  - **Decision (atomic verify-before-replace & secret-safe CLI):** Extend `tuse connect` with two opt-in flags (keeping existing `tuse connect` behavior unchanged when omitted):
    - `--replace`: verifies `GET <base_url>/models` **first**, and only if verification succeeds atomically updates the existing provider row in SQLite (so verification failure never deletes or corrupts the previous provider entry!).
    - `--key-stdin`: reads the API key value from stdin instead of command-line argv (so the secret never appears in shell command strings, `/proc/<pid>/cmdline`, or `ps`).
  - `connect` flow:
    1. Probes `http://<host>:<port>/v1/models` with the local key from `<config_dir>/api.key`; if the proxy is not running or unhealthy, fails clearly instructing the user to run `start` first (never auto-starts silently).
    2. Inspects existing providers (`tuse providers`, parsing `<NAME><tab><BASE_URL>` via split on the first tab, or `tuse providers --json` once #2 lands):
       - If `<provider>` does not exist: registers it via `tuse connect --base http://<host>:<port>/v1 --key-stdin --name <provider>` piping the key on stdin.
       - If `<provider>` exists with the **same** `base_url` (`http://<host>:<port>/v1`): atomically refreshes it via `tuse connect --replace --base http://<host>:<port>/v1 --key-stdin --name <provider>`.
       - If `<provider>` exists with a **different** `base_url` (owned by another provider): requires explicit confirmation via `pick({"Replace existing provider '<name>' (<other_url>)", "Cancel"})`, and only then uses `--replace` (still verifying before overwriting).
    3. Calls `reload()` so the active session loads the new provider snapshot immediately.
    4. **If `reload()` fails** (for example, due to an unrelated config error or a broken Git plugin sync under #6): catches the error and reports the exact state cleanly in the `subs` block — *"Saved provider `<name>` in `~/.tuse/tuse.db`, but config reload failed: `<redacted error>`. Fix the config/plugin issue and run `/reload` to activate it in this session."*
- **Ownership-checked `disconnect`**:
  - Looks up `<provider>` in saved providers:
    - If absent: reports not connected (idempotent).
    - If present with `base_url != http://<host>:<port>/v1`: refuses silent deletion and requires explicit confirmation via `pick({"Disconnect '<name>' (<other_url>)", "Cancel"})`.
    - If present with matching URL (or confirmed): runs `tuse disconnect <provider>` and calls `reload()`, reporting any `reload()` failure with the same explicit partial-state message above.

### Secret Redaction & Status Reporting (`status`)

- **Mandatory redaction boundary (before logs, notifications, and #8 persisted UI)**:
  - All output produced by `subs` — proxy task logs, task return summaries/notifications, login output, status output, and error messages passed to `show("subs", ...)` (which TODO #8 persists in SQLite) — MUST pass through a redactor first.
  - The redactor replaces with `[REDACTED]`: the exact value of `<config_dir>/api.key`, `Authorization: Bearer ...` headers, `x-api-key` / `x-goog-api-key` / `api-key` values, OAuth `access_token` / `refresh_token` / `id_token` / `client_secret` / `cookie` fields, OAuth `code=` query params, and JWT-shaped strings (`eyJ...`).
- **`status`** renders a redacted Markdown block via `show("subs", ..., false)`:
  - **Binary**: resolved path and version (and whether it matches the pinned version), or `not installed`
  - **Config**: `<config_dir>/config.yaml` (`present` / `missing`) and `<config_dir>/auth` account presence summary (filenames/provider count only, never file contents)
  - **Proxy**: `running (managed, task <id>)`, `running (external)`, `stopped`, or `failed`
  - **Endpoint**: `http://<host>:<port>/v1` and live health check (`reachable, N models` or redacted error summary)
  - **Tuse provider**: `connected as <provider>` (noting whether its URL matches this endpoint) or `not connected`
  - **Last error**: redacted last failure message if any

## Baseline Docs to Update Before Implementation (Handoff Gate)

Keep current baseline docs describing today's plugin behavior until work begins on #7. Before handing off implementation to a coding agent (per TODO #3), update and review:

- [../plugins.md](../plugins.md): document the reserved bundled plugin `"tuse/default"` (`theme` and `subs` options, offline embedded behavior, rejection of `ref`/`dir`/`path`) and link to the `subs` reference.
- [../theme.md](../theme.md): document the bundled `theme` palette, its relation to core defaults, and user override precedence.
- [../commands.md](../commands.md) and [../cli.md](../cli.md): document `--replace` and `--key-stdin` on `tuse connect`, and `/subs` when enabled.
- [../providers.md](../providers.md): cross-link `subs` as an optional local loopback proxy provider workflow.
- Add a dedicated baseline doc (e.g., `docs/subs.md`, linked from [../index.md](../index.md)) covering the full `subs` setup, login, lifecycle, provider wiring, and troubleshooting contract.

## Acceptance Tests

1. **Reserved ID & theme behavior**: `"tuse/default"` loads completely offline without Git or network; `ref`/`dir`/`path` on `"tuse/default"` fail validation; `theme = true` applies the bundled 10-token palette, user `theme` overrides win token-by-token, and `theme = false` leaves core defaults unchanged; `tuse models -c <probe>` validates the capability gate.
2. **Zero side effects on load/reload**: opening a session, resuming, and running `/reload` (`config.reload`) with `subs = true` performs zero downloads, zero config writes, zero logins, zero provider DB mutations, and spawns zero tasks/processes.
3. **Strict validation**: rejects unknown `opts`/`opts.subs` keys, invalid ports, invalid provider names, and any non-loopback `host` (`0.0.0.0`, `::`, `192.168.1.5`); verifies `subs`-only config requires explicit `theme = false`.
4. **Setup confirmation, pinned digest & permissions**: confirms before downloading binary AND confirms before creating/resetting `config.yaml` even when binary is already installed; rejects a mismatched SHA-256 digest against the hardcoded pinned digest; enforces `0700` on `<config_dir>` and `<config_dir>/auth`, `0600` on `config.yaml`, `api.key`, and auth files.
5. **Streaming login transport**: mock login binary printing an auth URL while staying alive waiting for callback delivers the clickable URL to `show("subs", ...)` **before** the process exits, does not leak tokens, tightens `auth/` permissions on success, and kills the login process group cleanly on Esc or timeout.
6. **Ownership & reload reconciliation**: `start` spawns a managed proxy bound by ownership ID (an unrelated user task named `"subs-proxy"` is never claimed or stopped); `stop` and session close (`exit`, `/new`, `/resume`, `shutdown`) stop only the owned task; failed `/reload` leaves the running proxy untouched; successful `/reload` preserves it when `{host, port, config_dir, binary}` match and stops it (without auto-restarting) when `subs` is removed/disabled or bind settings change; externally running proxies are detected without taking ownership.
7. **Atomic verify-before-replace & disconnect safety**: `connect` fails without touching an existing provider row if proxy verification fails; `connect` uses `--key-stdin` (key absent from argv) and confirms before replacing a provider with a different URL; `disconnect` refuses silent removal of a provider pointing to a different URL; a failed `reload()` after `connect`/`disconnect` reports the partial state cleanly without losing the saved provider.
8. **Secret redaction**: mock proxy emitting bearer keys, OAuth tokens, and JWTs to stdout/stderr has every secret redacted **before** task log writes, task completion notifications (fixed sanitized return string), and `show("subs", ...)` blocks (including when persisted under #8).
9. **TUI and JSON-RPC parity**: `/subs` actions (`status`, `setup`, `login`, `start`, `stop`, `connect`, `disconnect`) work through both TUI and `tuse json` (`commands.run`, `ui_request`, `ui.reply`, `ui_markdown`).
