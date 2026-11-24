# Subscription Proxy (`subs`)

The optional `subs` part of the bundled [`tuse/default`](plugins.md#bundled-default-plugin-tusedefault) plugin wraps [CLIProxyAPI](https://github.com/router-for-me/CLIProxyAPI) so you can use existing AI subscriptions through a local loopback-only OpenAI-compatible proxy registered as a Tuse provider.

Loading or reloading `"tuse/default"` **never** installs software, creates config files, starts OAuth login flows, starts/probes background proxies, or modifies `~/.tuse/tuse.db`. Every state-changing action requires explicitly running `/subs` and confirming the action.

## Enabling `subs`

`opts.subs` defaults to `false`. Enable it with `subs = true` (all defaults) or an options table. Because `opts.theme` defaults to `true`, set `theme = false` explicitly for a `subs`-only setup:

```lua
return {
  plugins = {
    {
      "tuse/default",
      opts = {
        theme = false,
        subs = {
          provider = "subs",
          host = "127.0.0.1",
          port = 8317,
          config_dir = "~/.tuse/subs",
        },
      },
    },
  },
}
```

### Options (`opts.subs`)

| Option | Type | Default | Meaning |
| --- | --- | --- | --- |
| `provider` | string | `"subs"` | Tuse provider name registered with `tuse connect` / `tuse disconnect`. Must be non-empty and contain no `/` or whitespace. |
| `host` | string | `"127.0.0.1"` | **Loopback-only** bind address. Must be `"127.0.0.1"`, `"::1"`, or `"localhost"`; non-loopback addresses (`0.0.0.0`, `::`, LAN IPs) fail config validation. |
| `port` | integer | `8317` | Local TCP port (`1..=65535`) for CLIProxyAPI. |
| `binary` | string \| nil | `nil` | Explicit path to `cli-proxy-api`. When unset, searches `<config_dir>/bin/cli-proxy-api`, then `PATH`. |
| `config_dir` | string | `"~/.tuse/subs"` | Directory for generated CLIProxyAPI config, OAuth credentials, local bearer key, and optional installed binary. |

Unknown keys or invalid values fail config validation with a diagnostic naming `plugin tuse/default` and the full key path (for example `plugin tuse/default: opts.subs.host: ...`).

## Pinned CLIProxyAPI Release & File Layout

`subs` pins **CLIProxyAPI `v8.1.2`** (`router-for-me/CLIProxyAPI`) with hardcoded SHA-256 archive digests embedded in the bundled plugin:

| Platform | Archive | SHA-256 |
| --- | --- | --- |
| `darwin/arm64` | `CLIProxyAPI_8.1.2_darwin_aarch64.tar.gz` | `95d746c8d6cb418a88cb79937ca90194388faa707c30e3ec53e7d41f469cd6ec` |
| `darwin/amd64` | `CLIProxyAPI_8.1.2_darwin_amd64.tar.gz` | `3e4ae8dc26676ada1db8b5f8e95bf097670e7aefb0bf6c3f5a3340c00cb42f0e` |
| `linux/arm64` | `CLIProxyAPI_8.1.2_linux_aarch64.tar.gz` | `41245fa2af02d68c69376afc6360af49b58c3b2368d8cec1d61f0e43de14d580` |
| `linux/amd64` | `CLIProxyAPI_8.1.2_linux_amd64.tar.gz` | `851e41c9a087b60e6586fec0c6be821b61921e8da0e84b9490ef542042db0756` |

### Files and Permissions under `<config_dir>`

- `<config_dir>`: mode `0700`
- `<config_dir>/bin/cli-proxy-api`: installed binary (when installed by `setup`), mode `0755`
- `<config_dir>/auth`: OAuth credential directory, mode `0700`; files inside are tightened to mode `0600` after login
- `<config_dir>/api.key`: randomly generated 32-byte hex local bearer key, mode `0600`
- `<config_dir>/config.yaml`: mode `0600`, configuring loopback `host`, `port`, `auth-dir`, `api-keys` (the single local key), and `remote-management: { allow-remote: false, disable-control-panel: true, secret-key: "" }`

## The `/subs` Command

When `opts.subs` is enabled, `"tuse/default"` registers one custom slash command `/subs` (callable from TUI `/subs` and JSON-RPC `commands.run { "name": "subs" }`). Invoking `/subs` opens a `pick()` menu with seven actions:

1. `status` — inspect binary, config, proxy process, endpoint health, and Tuse provider wiring
2. `setup` — locate or install CLIProxyAPI and create/reset local proxy config
3. `login` — authenticate a subscription account with CLIProxyAPI
4. `start` — start the local CLIProxyAPI proxy as a managed session process
5. `stop` — stop the proxy process started by `subs` in this session
6. `connect` — verify and register (or atomically update) the running proxy as a Tuse provider and reload
7. `disconnect` — verify ownership and remove the `subs` provider from Tuse and reload

Pressing Esc (or cancelling via `ui.cancel`) cancels with no side effects. All diagnostics render in the UI-only `show("subs", ..., false)` block after secret redaction.

### `setup`

1. **Binary step**: checks `opts.subs.binary`, `<config_dir>/bin/cli-proxy-api`, and `PATH`. If missing, confirms via `pick({"Install CLIProxyAPI v8.1.2", "Cancel"})` before downloading into a staging folder under `<config_dir>`, verifying the SHA-256 digest against the hardcoded manifest (never trusting a remote checksum file), extracting `cli-proxy-api`, setting mode `0755`, and moving it atomically into `<config_dir>/bin/cli-proxy-api`. On mismatch or unsupported platform, removes staging files and suggests `opts.subs.binary`.
2. **Config step (confirmed even when the binary already exists)**:
   - If `<config_dir>/config.yaml` is missing: confirms via `pick({"Create local config in <config_dir>", "Cancel"})` before creating `<config_dir>` (`0700`), `<config_dir>/auth` (`0700`), `<config_dir>/api.key` (`0600`), and `<config_dir>/config.yaml` (`0600`).
   - If `<config_dir>/config.yaml` exists: prompts `pick({"Keep existing config", "Reset config (rotates local API key)", "Cancel"})`.
3. Shows next steps (`login` -> `start` -> `connect`).

### `login`

1. Requires binary and `config.yaml` to exist (otherwise points to `setup`).
2. Prompts `pick({"OpenAI / Codex", "Anthropic / Claude", "Google / Gemini", "Cancel"})`.
3. Runs `<binary> --config <config_dir>/config.yaml --no-browser <flag>` directly (no shell interpolation, 5-minute timeout) using CLIProxyAPI `v8.1.2` flags:
   - OpenAI / Codex: `--codex-login`
   - Anthropic / Claude: `--claude-login`
   - Google / Gemini: `--antigravity-login`
4. Streams output incrementally while running: as soon as an `https://...` authorization URL (and optional device code) appears, displays only the sanitized URL and instructions via `show("subs", ..., false)` (clickable in TUI) before the process exits, never printing raw output or tokens.
5. Cancelling (Esc / `turn.cancel`) or hitting the 5-minute timeout kills the login process group immediately.
6. On success, tightens permissions on files inside `<config_dir>/auth` to `0600` and reports completion.

### `start`, `stop`, Ownership, and Reload Reconciliation

- **Unforgeable ownership**: `start` spawns a managed background task (`subs-proxy`) and stores a session-scoped ownership record bound to the exact `task_id` and `{ host, port, config_dir, resolved_binary }`. `subs` never claims or stops a task by display name.
- **`start`**:
  - Requires binary and `config.yaml`.
  - If the owned `task_id` is already running and healthy, reports already running (idempotent).
  - Probes `http://<host>:<port>/v1/models` with the local key: if HTTP 200 with valid JSON, reports an externally managed proxy without spawning a duplicate or recording ownership; if another unrelated service holds the port, reports a port conflict.
  - Runs `<binary> --config <config_dir>/config.yaml` directly (no secrets in argv), redacts every stdout/stderr line before writing to the task log, and returns only `"subs-proxy stopped"` or `"subs-proxy exited with code N"` so model task notifications never carry raw stderr or credentials.
  - Polls readiness for up to 10 seconds (failing fast if the task exits early). On failure, stops the task and clears ownership.
- **`stop`**: stops only the owned `task_id` (killing its process group) and clears ownership. Never stops an externally managed proxy.
- **Session close**: closing the session (`exit`, `/new`, `/resume`, `shutdown`, signals) stops the owned proxy process group so nothing outlives the session.
- **Reload reconciliation**:
  - Failed `/reload` leaves the running proxy and ownership record untouched.
  - Successful `/reload` with identical `{ host, port, config_dir, resolved_binary }` preserves the running proxy and transfers ownership without restarting.
  - Successful `/reload` that removes/disables `subs` or changes `host`, `port`, `config_dir`, or resolved `binary` stops the previously owned task, clears ownership, and emits a notice (never auto-starting a new proxy).

### `connect` and `disconnect`

- **`connect`**:
  - Probes `http://<host>:<port>/v1/models` with `<config_dir>/api.key`; fails if the proxy is not running (never auto-starts).
  - Registers or updates `<provider>` via `tuse connect --base http://<host>:<port>/v1 --key-stdin --name <provider>` (piping the key on stdin so it never appears in argv), adding `--replace` when `<provider>` already exists with the same URL (atomic verify-before-replace). If `<provider>` exists with a **different** URL, asks confirmation first.
  - Calls `reload()` to activate the provider immediately in the session. If `reload()` fails (for example due to an unrelated config error), catches the error and reports that the provider was saved in `~/.tuse/tuse.db` and instructs the user to fix the config and run `/reload`.
- **`disconnect`**:
  - Idempotent when `<provider>` is absent.
  - If `<provider>` points to a different URL than `http://<host>:<port>/v1`, requires confirmation before running `tuse disconnect <provider>`.
  - Calls `reload()` and reports partial state cleanly if `reload()` fails.

### Secret Redaction and `status`

All `subs` output — proxy task logs, task completion notifications, login output, status panels, and errors passed to `show("subs", ...)` — passes through a redactor that replaces the exact local `api.key` value, `Authorization: Bearer ...` headers, `x-api-key` / `x-goog-api-key` / `api-key` values, OAuth `access_token` / `refresh_token` / `id_token` / `client_secret` / `cookie` fields, OAuth `code=` query parameters, and JWT-shaped strings (`eyJ...`) with `[REDACTED]`.

`status` reports binary path/version (compared against pinned `v8.1.2`), config and auth account presence (filenames/count only), proxy state (`running (managed, task <id>)`, `running (external)`, `stopped`, `failed`), endpoint health, Tuse provider wiring, and the redacted last error if any.
