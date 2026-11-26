## 1. Lua and shared UI

- [x] 1.1 Accept an optional Markdown `title` in `pick(options, title)` and carry nonblank titles through the shared prompt queue.
- [x] 1.2 Expose `title` in JSON-RPC `ui_request` and `ui.state.active`, omitting it for untitled calls.
- [x] 1.3 Save the original title as `prompt` in the existing `pick` result; keep `Picker` as the untitled fallback.

## 2. TUI

- [x] 2.1 Render the Markdown title above the choices with the same link behavior as `ask`, keeping at least one choice row whenever the overlay can be drawn.
- [x] 2.2 Use the remaining choice rows for scrolling and paging; keep existing choice selection and cancellation behavior.
- [x] 2.3 Render saved titled picker questions as Markdown in chat, including on session resume, while leaving answers as plain text.

## 3. Documentation and tests

- [x] 3.1 Update `docs/lua.md`, the agent system prompt, and `docs/json.md`.
- [x] 3.2 Cover Lua compatibility and cancellation, JSON-RPC request and history payloads, and TUI rendering and paging with a constrained viewport.
