## ADDED Requirements

### Requirement: Persist picker questions with answers
A completed titled `pick` SHALL save the original Markdown question in `prompt` and the selected string in `value` in the existing `pick` history entry. The result SHALL be available through both TUI and JSON-RPC history, including after session resume. TUI chat SHALL render the question as Markdown like an `ask` question and the answer as plain text. An untitled picker SHALL retain the `Picker` prompt. A cancelled picker SHALL NOT create a result entry.

#### Scenario: Read a picker result after resuming
- **WHEN** the user answers `Retry` to a picker titled `**Build failed.** What next?` and resumes the session
- **THEN** the `pick` history entry retains that exact Markdown source and `Retry`, and TUI chat renders the formatted question above the plain-text answer

#### Scenario: Untitled or cancelled picker
- **WHEN** a user answers an untitled picker and then cancels a titled picker
- **THEN** only the first picker produces a history result, with `Picker` as its prompt
