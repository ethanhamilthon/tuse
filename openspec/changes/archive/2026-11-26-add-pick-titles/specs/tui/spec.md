## ADDED Requirements

### Requirement: Lua picker questions
Lua `pick(options, title)` SHALL accept an optional Markdown question as its second argument. A missing, `nil`, empty, or whitespace-only title SHALL be treated as untitled. The TUI SHALL render a nonblank title above the choices with the same Markdown and link treatment as `ask`, without a text input. Only the choices SHALL be selectable. `pick` SHALL still return the selected option string unchanged, or `nil` on cancellation or an empty options list. Built-in pickers SHALL be unaffected.

#### Scenario: Ask a multiple-choice question
- **WHEN** Lua calls `pick({"Retry", "Skip"}, "**Build failed.**\n\nWhat next?")`
- **THEN** the question is rendered as Markdown above the two choices, there is no answer editor, and selecting `Retry` returns `"Retry"`

#### Scenario: Untitled picker
- **WHEN** Lua calls `pick({"Retry", "Skip"})` or passes `nil` or a blank title
- **THEN** the choices appear without a question or an extra row

#### Scenario: Empty options or cancellation
- **WHEN** Lua passes an empty options list, or the user cancels an active titled picker
- **THEN** `pick` returns `nil`, and an empty options list does not open a picker

### Requirement: Titled picker layout and navigation
The title and choices SHALL share the picker overlay height budget. The title SHALL stay above the choices when they scroll. If the whole title does not fit, the TUI SHALL display only the leading title rows that fit while reserving at least one choice row whenever the overlay is visible. Paging and choice scrolling SHALL use the available choice rows, not the combined height.

#### Scenario: One available row
- **WHEN** only one picker row fits below the composer
- **THEN** the selected choice is visible and the title is hidden

#### Scenario: Paging beneath the question
- **WHEN** a multi-line title wraps and the user pages through a long list of choices
- **THEN** the visible title rows stay fixed and the page size is based only on the visible choice rows

### Requirement: JSON-RPC picker questions
An active titled picker SHALL include the original Markdown source in a `title` string in both the `ui_request` event and `ui.state.active`. Untitled pickers SHALL omit `title`. Replies SHALL continue to accept only an offered option string.

#### Scenario: Observe and answer a titled picker remotely
- **WHEN** a titled picker becomes active over JSON-RPC and a client calls `ui.state`
- **THEN** both the event and the active snapshot include the same Markdown `title` and unchanged `options`, and `ui.reply` still validates the answer against `options`

#### Scenario: Untitled request shape
- **WHEN** an untitled picker becomes active over JSON-RPC
- **THEN** neither the request event nor the active snapshot includes `title`
