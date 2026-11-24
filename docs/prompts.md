# Prompts, files and images in a message

A user message keeps its submitted text (`#review`, `@notes.txt`, and `[Image #1]` stay visible in chat, `user_message` hooks, and auto-titles—note that the TUI trims leading and trailing whitespace on submit, and auto-titles take the first line up to 80 characters). What picked atoms refer to is sent to the model in a hidden `context` block appended after the message text (separated by a blank line, or alone when the visible text is empty):

```text
fix this #review @notes.txt [Image #1]

<prompt name="review">Check the change for bugs.</prompt>
<image id="1" path="/home/me/.tuse/ID/images/image-1.png"/>
<file path="notes.txt">hello world
</file>
```

## Picked atoms vs plain text

- Only picked or pasted atoms generate hidden context: a prompt chosen from the `#` picker, a file or folder chosen from the `@` picker, or an image pasted from the clipboard (`Ctrl+V` / `Cmd+V`). Typing `#name` or `@path` by hand leaves it as plain text.
- In the TUI input, each picked prompt, file, directory, or pasted image is a single atomic unit highlighted in `accent` (except while dimmed as a queued steering message): arrow keys, Backspace, Delete, and selection move over or remove it whole.
- **Image numbering:** Atom IDs (`[Image #N]`) count image atoms inserted in the **current input draft** (deleting `[Image #2]` and pasting again creates `[Image #3]`; numbering resets when the draft is sent or cleared). Saved PNG filenames (`~/.tuse/<session-id>/images/image-<k>.png`) are numbered independently by the entry count in that session's `images/` directory and keep incrementing across drafts.
- When the clipboard contains an image and you paste during a modal search picker or `ask()` prompt, the TUI discards atom insertion—though it reads the clipboard first, which can still write the PNG file under `~/.tuse/<session-id>/images/`. When the input is disabled (`pick()`), clipboard paste returns immediately without writing a file.

## `#` prompts

- Configured in Lua as `prompts = { name = "text..." }`.
- Typing `#` at a token boundary opens the `#` picker (when at least one prompt is configured), showing each `#name` and the first line of its body as detail. Filtering matches `#name` labels only.
- Selecting an entry inserts a `#name` atom followed by a space.
- A prompt is emitted once per message even if referenced multiple times (first occurrence wins); trailing whitespace is trimmed from the prompt body.

## `@` files and directories

- Typing `@` at a token boundary opens the file picker, listing up to 20,000 entries of the target directory sorted by name, skipping `.git` (directories end with `/`).
- `@query` searches the session working directory; `@dir/query` lists `dir/` and filters its entries by `query`.
- Paths starting with `/` or `~` list outside the project (`@/etc/ho` filters `/etc/` by `ho`; `@~` or `@~/` lists your home directory). Picked `~/` paths are expanded to the full home path in the hidden context.
- **Tab** on a directory completes it (`@some` -> `@some/`) so you can browse inside it; **Tab** on a file attaches the file atom and closes the picker.
- **Enter** attaches the highlighted entry—file or directory—as an atom followed by a space and closes the picker.

## Context block format and limits

Blocks are emitted in order: prompts, then pasted images, then picked files and directories (deduplicated by path). Attribute values and directory listings XML-escape `&`, `"`, and `<`.

- **Prompts:** `<prompt name="NAME">BODY</prompt>`.
- **Images (`.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, case-insensitive):**
  - Pasted images keep their `[Image #N]` atom ID.
  - Image files picked with `@` receive the next free ID (`max(pasted atom IDs) + 1`, incrementing).
  - Each image is encoded as a base64 `data:<mime>;base64,...` URL attached to the user message and referenced by `<image id="N" path="PATH"/>`. (The 5 MiB limit for Lua `read()` does not apply to message image attachments.)
- **Directories:** `<dir path="PATH">\nentry1\nentry2/\n</dir>`, sorted by name with `/` on subdirectories. Listings longer than 1,024 bytes stop early with `\n…`.
- **Text files:** Tuse reads up to the first 1,024 bytes, cutting cleanly on a UTF-8 character boundary if a multi-byte character straddles byte 1,024. Files up to 1,024 bytes emit `<file path="PATH">CONTENT</file>`; larger files emit `<file path="PATH" truncated="true" size="BYTES">HEAD</file>`.
- **Binary files:** Any file whose read head contains a NUL byte or invalid UTF-8 emits `<file path="PATH" binary="true" size="BYTES"/>` without content.
- **Missing or unreadable references:** In the TUI, missing prompts, files, or images are silently skipped. In `tuse json`, `turn.submit` validates `prompts`, `files`, and `images` strictly and fails the request if any is missing or unreadable ([json.md](json.md)).

## Trace and compaction

- `/trace` shows the user message text joined with its hidden `context`, plus `[N image(s)]` when images were attached.
- Once context compaction moves a turn into the dropped (oldest) zone, only the visible conversation `text` is sent to the model; hidden `context` and `images` are omitted ([usage.md](usage.md)).
