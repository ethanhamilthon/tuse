# Prompts, files and images in a message

A message keeps the text as typed: `#review`, `@src/main.rs` and `[Image #1]` stay as they are
in the chat, the session title and the `user_message` hook. What they refer to goes to the
model in a hidden `context` after the text:

```
fix this #review @src/main.rs [Image #1]

<prompt name="review">Check the change for bugs.</prompt>
<image id="1" path="/home/me/.tuse/ID/images/1.png"/>
<file path="src/main.rs">fn main() {}
</file>
```

- Only what is picked in the input counts: a prompt chosen with the `#` picker, a file chosen
  with `@`, an image pasted with Ctrl+V. A `#name` or `@path` typed by hand is plain text.
- `@` lists the current folder (`@dir/*` goes into `dir`). A path starting with `/` or `~/` lists
  that folder outside the project: `@/etc/ho` shows the entries of `/etc/` matching `ho`. `~/` is
  sent as the full home path.
- A prompt is sent once, however often it appears.
- In config, each `prompts` entry is a plain string or `{text, modes: [tui]}` (no `modes` or
  empty means all modes; entries excluded by the active mode are hidden from `#` and ignored).
- Images (`.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`) are attached as pictures. A pasted image keeps
  its number `N`; one picked with `@` gets the next free one.
- A text file sends its first 1 KB; a longer one is marked `truncated="true" size="BYTES"`.
  A binary file is only named: `<file path="..." binary="true" size="BYTES"/>`.
  Missing files and folders are skipped.

`/trace` shows the text with its context. After compaction, the dropped messages are summarized
from their text only. `tuse json` builds the context from
`files` and `images` in `turn.submit` ([json.md](json.md)).
