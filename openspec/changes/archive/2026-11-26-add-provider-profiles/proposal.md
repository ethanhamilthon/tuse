## Why

Compatible API gateways need additional headers, and project configs need providers without changing the global database.

## What Changes

- Add repeatable `tuse connect --header` and equivalent Lua `providers` profiles.
- Merge config-only and saved profiles for session use without silently overriding a name.
- Use the attached screenshot in both README and the docs site.

## Impact

CLI, config loading, provider storage, API transport, docs and site assets.
