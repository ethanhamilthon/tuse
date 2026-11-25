## Why
Streaming got slower with long replies: every frame re-rendered whole lists, code blocks, reasoning, and tool code, and probed every path-like word on disk. Markdown images also lost their target and showed only the alt text.

## What Changes
- Incremental streaming for lists, fenced code, reasoning, and `lua` tool code, checked against a full render.
- Path existence probes are cached for one second.
- Markdown images render as links.
- Docs: images, `settings.open` example, `e.event` in hooks.

## Impact
- Specs: tui
- Code: `kit/rich/stream.rs`, `kit/links.rs`, `kit/rich/markdown`, `kit/rich/tool_view.rs`, `ui/chat.rs`
