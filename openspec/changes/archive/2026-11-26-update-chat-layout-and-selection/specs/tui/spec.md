## ADDED Requirements

### Requirement: Aligned chat entry text
User and agent message rows SHALL reserve two display columns on each side for padding, inside the chat width and separate from the outer `pad_x` margin. Messages SHALL wrap at the remaining content width. The top-level text of every chat entry SHALL start at the same left content column after any entry marker, on both first and continuation rows. This applies to messages, reasoning, Lua code and output, diff blocks, notices, errors, turn summaries, pinned and unpinned `show()` blocks, and recorded picker and question results. Intentional indentation within an entry, such as Markdown nesting or code indentation, SHALL be preserved. When the chat is too narrow to fit the padding and content, the padding SHALL yield as needed without overflowing the row.

#### Scenario: Mixed chat history
- **WHEN** the chat shows user and agent messages alongside Lua blocks, events, `show()` blocks, and picker or question results
- **THEN** their top-level text starts two display columns past the outer left margin, with any entry marker in those two columns
- **AND** user and agent messages leave two columns of right padding even when a row fills the available content width
- **AND** wrapped lines keep the same left inset, while nested lists and indented code keep their internal indentation

#### Scenario: Narrow terminal
- **WHEN** the chat width is too small for both two-column insets and message text
- **THEN** the insets shrink so the row stays within the viewport
- **AND** widening the terminal restores the two-column insets

#### Scenario: Restore history
- **WHEN** a session is reopened
- **THEN** restored entries use the same text alignment and wrapping as live entries at the same width

### Requirement: Selection is limited to one text target
Ctrl+A and Cmd+A SHALL clear any chat selection and select the entire editable input, including draft lines outside the viewport. Mouse selection SHALL be limited to the input or a single user or agent message. A drag moving outside its starting target SHALL stop at that target's boundary and MUST NOT add text from another target. Starting a new selection SHALL replace the previous one. Reasoning, Lua tool blocks, event entries, UI output, overlays, and layout decorations MUST NOT be selectable. The highlight SHALL match the selected text rather than the whole screen row.

#### Scenario: Select all with a message selected
- **WHEN** a chat message is selected and the user presses Ctrl+A or Cmd+A with an editable multiline draft
- **THEN** the chat selection clears and only the entire draft is selected, including any hidden editor lines

#### Scenario: Drag across messages or into the input
- **WHEN** the user starts a drag in an agent message and drags through a different message, a Lua block, a UI result, or the input
- **THEN** only the range within the original agent message is highlighted, in either drag direction

#### Scenario: Drag out of the input
- **WHEN** the user starts a drag in the input and moves into the chat or a panel
- **THEN** the selection stays in the input and no other text is highlighted

#### Scenario: Drag from non-selectable UI
- **WHEN** the user starts a selection drag on reasoning, Lua output, an event, a `show()` block, or an overlay and moves into a message
- **THEN** neither that UI text nor the message is selected

### Requirement: Copy logical text without layout whitespace
Ctrl+C and Cmd+C with a selection SHALL copy only the selected text. Chat selections SHALL copy plain text as displayed, without styles, layout padding, message gaps, or newlines introduced solely by visual wrapping. A soft wrap SHALL preserve the original word separation without inserting a space inside a wrapped word. Logical line breaks, paragraph breaks, and intentional indentation within the selected range SHALL be preserved. Editor selections SHALL copy the exact selected draft text, unaffected by wrapping or composer style.

#### Scenario: Copy a visually wrapped range
- **WHEN** the user selects a message paragraph that wraps at a space and also wraps a long word across rows
- **THEN** copying produces one logical line with the original word separation and no extra whitespace inside the long word
- **AND** the same logical range copies identically at a different terminal width

#### Scenario: Copy intentionally multiline text
- **WHEN** the user selects across a paragraph break or across indented lines of a code block in an agent message
- **THEN** copying preserves the paragraph break or code line breaks and indentation, without adding leading or trailing padding from the highlighted rows

#### Scenario: Copy a draft
- **WHEN** the user selects a wrapped draft containing explicit newlines, spaces, or indentation
- **THEN** copying returns exactly that part of the draft, without the composer prompt, borders, or visual wraps
