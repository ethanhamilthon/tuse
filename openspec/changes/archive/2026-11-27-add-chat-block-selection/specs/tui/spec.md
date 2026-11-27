## MODIFIED Requirements

### Requirement: Selection is limited to one text target
Ctrl+A and Cmd+A SHALL clear any chat or panel selection and select the entire editable input, including draft lines outside the viewport. Mouse selection SHALL be limited to the input, a single user or agent message, a single `show()` block (pinned or not), a single `pick` or `ask` result in the chat, or the open panel (`/config`, `/session`, a `/history` entry, task details, and the Lua `pick` and `ask` panels, including their question text and choices). A drag moving outside its starting target SHALL stop at that target's boundary and MUST NOT add text from another target. Starting a new selection SHALL replace the previous one. Prompt atoms in user messages SHALL be selectable as their displayed text. The marker icon and the padding of a `show()` block or a `pick` or `ask` result MUST NOT be selected or copied. Reasoning, Lua tool blocks, event entries, built-in pickers, and layout decorations MUST NOT be selectable. The highlight SHALL match the selected text rather than the whole screen row. Closing a panel or stepping to another entry SHALL clear the panel selection. Scrolling clears the selection.

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

#### Scenario: Select a show block
- **WHEN** Lua calls `show("report", "line one\n\nline two")` and the user drags across the block and presses Ctrl+C
- **THEN** only the block text is highlighted and the clipboard holds `line one\n\nline two`, without the marker icon or padding

#### Scenario: Select a pinned show block
- **WHEN** a pinned `show()` block sits at the end of the chat and the user drags across it
- **THEN** its text is highlighted and copies the same way as an unpinned block

#### Scenario: Select a pick result
- **WHEN** the chat shows the result of `pick(options, title)` and the user drags across it
- **THEN** the displayed question and answer are highlighted and copy as displayed, without the marker icon

#### Scenario: Drag between a show block and a message
- **WHEN** the user starts a drag in a `show()` block and moves into an agent message, or starts in the message and moves into the block
- **THEN** only the range inside the starting target is highlighted

#### Scenario: Drag from non-selectable UI
- **WHEN** the user starts a selection drag on reasoning, Lua output, an event, or a built-in picker and moves into a message, a `show()` block, or a panel
- **THEN** neither that UI text nor the target is selected
