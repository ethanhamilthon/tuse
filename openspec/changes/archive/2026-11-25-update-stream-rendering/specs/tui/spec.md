## ADDED Requirements

### Requirement: Streaming renders incrementally
While a reply, reasoning, or `lua` tool call streams, the TUI SHALL re-render only the unfinished tail and reuse the lines of the finished part. The streamed view MUST equal a full render of the same text at every point. Once the reply ends, the entry SHALL be rendered once in full.

#### Scenario: Long list or code block
- **WHEN** the agent streams a long list, a long fenced code block, or long tool code
- **THEN** each frame renders only the last item or the last code lines, and the output matches a full render

#### Scenario: Path probes
- **WHEN** streamed text mentions file paths
- **THEN** each path is checked on disk at most once per second

### Requirement: Markdown images
A Markdown image `![alt](target)` SHALL render as a link to `target` with `alt` as its text, or with `target` as its text when `alt` is empty.

#### Scenario: Image in a reply
- **WHEN** a reply contains `![Photo](/tmp/a.png)`
- **THEN** the chat shows `Photo` as a clickable link that opens `/tmp/a.png`
