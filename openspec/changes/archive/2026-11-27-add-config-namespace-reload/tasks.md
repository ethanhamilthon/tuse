## 1. Core config reload

- [x] 1.1 Retain active contributions and their layer/name identity alongside the merged session config.
- [x] 1.2 Add the scoped reload path: evaluate once, select the named contributions, check for missing active contributions, and validate and merge against the retained snapshot.
- [x] 1.3 Commit the scoped candidate with the existing reload lifecycle and keep the prior config, callbacks, and module-cache state on failure.

## 2. JSON-RPC and documentation

- [x] 2.1 Accept and validate optional `params.ns` on `config.reload`; preserve full reload and the existing response and event shapes.
- [x] 2.2 Document the RPC parameter and scoped reload semantics in `docs/json.md` and `docs/config.md`.

## 3. Verification

- [x] 3.1 Cover a single evaluation pass, refresh of a matching name in both layers, and preservation of unrelated contributions.
- [x] 3.2 Cover unknown or removed namespaces and a merge conflict without changing the active config.
- [x] 3.3 Cover RPC parameter validation, the successful reload event, and the no-argument full reload path; run the relevant config and RPC tests.
