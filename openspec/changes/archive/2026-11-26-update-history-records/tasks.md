## 1. Records
- [x] 1.1 Rename `Record` variants and tag to the new roles; split `ui_result` into `pick`/`ask` and `job` into `task`/`command`/`open`
- [x] 1.2 Replace `Tool` with `Lua { id, code, output, exit, images, diffs }`
- [x] 1.3 Replace `Flash`, `Notice`, `Error` with `Message { level, text }`
- [x] 1.4 Replace `Title` and `Settings` with `Meta { title, model, effort }`
- [x] 1.5 Drop `removed` from `Show`; empty `text` removes the block
- [x] 1.6 Remove `Record::Hook`; hook failures record only a `notice` message

## 2. History view and validation
- [x] 2.1 Make `HistoryItem` the stored JSON plus `summary`; drop the role/field mapping
- [x] 2.2 Simplify `decode` to serde plus ordering checks
- [x] 2.3 Leave tool calls out of `agent` summary and detail
- [x] 2.4 Update `HISTORY_ROLES`

## 3. Storage
- [x] 3.1 Reset schema below version 3: drop `sessions` and `history`, keep `providers` and `state`
- [x] 3.2 Test the reset

## 4. TUI and JSON-RPC
- [x] 4.1 Update chat rendering for `message`, `meta`, `show`
- [x] 4.2 `/history` and `/fork` hide `agent` entries with no text and no reasoning
- [x] 4.3 `history.hide` accepts message levels `flash`, `notice`, `error`
- [x] 4.4 Update RPC parity tests

## 5. Docs
- [x] 5.1 Update `docs/session.md`, `docs/json.md`, `docs/config.md`, `docs/lua.md`
- [x] 5.2 Drop `hook` from `history.hide` in `examples/project.lua`
