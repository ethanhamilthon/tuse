## MODIFIED Requirements

### Requirement: Selection is limited to one text target
Ctrl+A and Cmd+A SHALL clear any chat or panel selection and select the entire editable input, including draft lines outside the viewport. Mouse selection SHALL be limited to the input, a single user or agent message, or the open panel (`/config`, `/session`, a `/history` entry, task details, and the Lua `pick` and `ask` panels, including their question text and choices). A drag moving outside its starting target SHALL stop at that target's boundary and MUST NOT add text from another target. Starting a new selection SHALL replace the previous one. Prompt atoms in user messages SHALL be selectable as their displayed text. Reasoning, Lua tool blocks, event entries, `show()` blocks, built-in pickers, and layout decorations MUST NOT be selectable. The highlight SHALL match the selected text rather than the whole screen row. Closing a panel or stepping to another entry SHALL clear the panel selection. Scrolling clears the selection.

#### Scenario: Select all with a message selected
- **WHEN** a chat message is selected and the user presses Ctrl+A or Cmd+A with an editable multiline draft
- **THEN** the chat selection clears and only the entire draft is selected, including any hidden editor lines

#### Scenario: Drag across messages or into the input
- **WHEN** the user starts a drag in an agent message and drags through a different message, a Lua block, a UI result, or the input
- **THEN** only the range within the original agent message is highlighted, in either drag direction

#### Scenario: Drag out of the input
- **WHEN** the user starts a drag in the input and moves into the chat or a panel
- **THEN** the selection stays in the input and no other text is highlighted

#### Scenario: Select in a panel
- **WHEN** `/config` is open and the user drags across several lines of it
- **THEN** only that text is highlighted, and Ctrl+C copies it

#### Scenario: Drag out of a panel
- **WHEN** the user starts a drag in a panel and moves into the chat or the input
- **THEN** the selection stays in the panel and no other text is highlighted

#### Scenario: Select in a Lua question
- **WHEN** a Lua `pick` or `ask` panel is open and the user drags across its question text or choices
- **THEN** that text is highlighted and Ctrl+C copies it, and nothing is picked

#### Scenario: Link click in a panel
- **WHEN** the user clicks a URL or file path in a panel without dragging
- **THEN** it opens as before and nothing is selected

#### Scenario: Prompt atom in a message
- **WHEN** the user selects a user message that contains the atom `#openspec propose`
- **THEN** the atom is highlighted and copies as `#openspec propose`

#### Scenario: Select all with a panel selected
- **WHEN** a panel text is selected and the user presses Ctrl+A or Cmd+A
- **THEN** the panel selection clears and only the entire draft is selected

#### Scenario: Drag from non-selectable UI
- **WHEN** the user starts a selection drag on reasoning, Lua output, an event, a `show()` block, or a built-in picker and moves into a message or panel
- **THEN** neither that UI text nor the target is selected

### Requirement: Copy logical text without layout whitespace
Ctrl+C and Cmd+C with a selection SHALL copy only the selected text. Chat and panel selections SHALL copy plain text as displayed, without styles, layout padding, message gaps, or newlines introduced solely by visual wrapping. A soft wrap SHALL preserve the original word separation without inserting a space inside a wrapped word. Logical line breaks, paragraph breaks, and intentional indentation within the selected range SHALL be preserved. Editor selections SHALL copy the exact selected draft text, unaffected by wrapping or composer style.

#### Scenario: Copy a visually wrapped range
- **WHEN** the user selects a message or panel paragraph that wraps at a space and also wraps a long word across rows
- **THEN** copying produces one logical line with the original word separation and no extra whitespace inside the long word
- **AND** the same logical range copies identically at a different terminal width

#### Scenario: Copy intentionally multiline text
- **WHEN** the user selects across a paragraph break or across indented lines of a code block in an agent message or panel
- **THEN** copying preserves the paragraph break or code line breaks and indentation, without adding leading or trailing padding from the highlighted rows

#### Scenario: Copy a draft
- **WHEN** the user selects a wrapped draft containing explicit newlines, spaces, or indentation
- **THEN** copying returns exactly that part of the draft, without the composer prompt, borders, or visual wraps
