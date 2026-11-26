## ADDED Requirements

### Requirement: Folded Lua strings
A multiline Lua long string in a tool call SHALL be shown with at most `ui.lines.strings` lines. With `0` it SHALL be shown as `…` alone. With `N > 0` it SHALL show its first N lines and end with `…` when it has more. With `-1` it SHALL show every line.

#### Scenario: Default
- **WHEN** a tool call holds a long string of three lines and `ui.lines.strings` is `1`
- **THEN** the first line followed by `…` is shown

#### Scenario: Hidden
- **WHEN** `ui.lines.strings` is `0`
- **THEN** the string is shown as `…`

### Requirement: Themed colors
The TUI SHALL take colors only from `ui.theme`. The prompt marker, spinners, the inline Lua star, and the bar mark SHALL use `icons`. The input text SHALL use `input`, and the input background or frame SHALL use `input_bg`. User messages SHALL use `user` on `user_bg`. Pickers, panels, and rules SHALL use `border`. Error text SHALL use `error`. Diff lines SHALL use `code.added`, `code.removed`, `muted` for context, and `border` for headers. Markdown SHALL use `markdown.heading`, `markdown.link`, `markdown.code`, `markdown.quote`, and `markdown.list`; hovered links use `fg`.

#### Scenario: Icon color
- **WHEN** `ui.theme.icons = "#FF0000"`
- **THEN** the spinner, the prompt marker, and the inline Lua star are drawn in `#FF0000`

#### Scenario: Markdown tokens
- **WHEN** `ui.theme.markdown.heading` differs from `fg` and an agent reply has a heading
- **THEN** the heading uses that color

#### Scenario: Diff colors
- **WHEN** a diff has an added line, a removed line, and a context line
- **THEN** they use `code.added`, `code.removed`, and `muted`

### Requirement: Spacing between entries
Blank rows between consecutive tool calls and reasoning blocks SHALL equal `ui.space.steps`. Blank rows above and below user and agent messages SHALL equal `ui.space.messages`. Side padding SHALL equal `ui.space.padding`.

#### Scenario: Steps gap
- **WHEN** `ui.space.steps = 2` and two tool calls follow each other
- **THEN** two blank rows separate them

#### Scenario: Messages gap
- **WHEN** `ui.space.messages = 0`
- **THEN** a user message and the next agent message have no blank row between them

## MODIFIED Requirements

### Requirement: History command
`/history` SHALL list the session history from the database, minus roles in `history.hide`; `flash` entries are listed like any other role. `trace` is no longer a built-in command and `history` is a reserved command name. JSON-RPC `session.trace` and `session.records` are replaced by `session.history`, which returns the same structured records as `session().history`, and `session.list` returns only started sessions.

#### Scenario: Open history
- **WHEN** the user types `/history`
- **THEN** a list of every history record opens, starting with `system`

#### Scenario: Old command
- **WHEN** the user types `/trace`
- **THEN** it is an unknown command

#### Scenario: Old method
- **WHEN** a client calls `session.trace` or `session.records`
- **THEN** it gets a method-not-found error

### Requirement: Lua syntax highlighting
Lua code in tool calls, panels, and `lua` code blocks SHALL be colored by the `ui.theme.code` tokens using the code's structure. Function definition and call names SHALL be `function`, names after `:` SHALL be `method`, names after `.` and table keys SHALL be `field`, names in a function's parameter list and their uses inside that function SHALL be `parameter`, standard library and Tuse functions SHALL be `builtin`, and other names SHALL be `variable`. `local` and `function` SHALL be `storage`; `nil`, `true`, and `false` SHALL be `constant`; `and`, `or`, `not`, and symbol operators SHALL be `operator`; brackets, `,`, `;`, `.`, and `:` SHALL be `punctuation`; escapes inside strings SHALL be `escape`. Long strings and long comments SHALL continue across lines.

#### Scenario: Assignment and calls
- **WHEN** the code is `x = read("a"):upper() .. y`
- **THEN** `=` and `..` are `operator`, `read` is `builtin`, `upper` is `method`, and `x` and `y` are `variable`

#### Scenario: Parameters and table keys
- **WHEN** the code defines `function M.go(self, n) return n end` and a table `{ run = function() local y = 1 end }`
- **THEN** `go` is `function`, `self` and both `n` are `parameter`, `run` is `field`, and `y` is `variable`

### Requirement: User message color
User messages SHALL use `ui.theme.user` on `ui.theme.user_bg`; the input text SHALL use `ui.theme.input`; agent text SHALL use `fg`.

#### Scenario: Different colors
- **WHEN** `ui.theme.user` differs from `ui.theme.fg`
- **THEN** a user message differs in color from an agent reply
- **WHEN** `ui.theme.input` differs from `ui.theme.user`
- **THEN** the draft in the input differs in color from a sent user message

### Requirement: Boxed input composer
When `ui.input` is `boxed` and the viewport fits, the composer SHALL consist of a top border, the editor rows, one blank row, a status row, and a bottom border. Editor rows SHALL have no prompt arrow. Every editor row and the status row SHALL reserve exactly one blank display column immediately inside each side border. The padding SHALL be inside the existing frame width and separate from the outer `ui.space.padding` margin. The editor SHALL use the remaining content width for wrapping, cursor placement, mouse hit testing, and selection. It SHALL grow with the draft, limited by `ui.lines.input` and the viewport, and scroll beyond that with the cursor visible. Exactly one blank row SHALL separate the last editor row from the status row. Side borders SHALL enclose all interior rows and use the `input_bg` color. The status row SHALL place activity and notices at the left edge of the content area and the model/context label at its right edge; labels MUST be clipped to fit without overlapping or using the padding. The box SHALL require room for the two side borders, the two padding columns, and at least two content columns after accounting for `ui.space.padding`. The `lines` composer SHALL be used when the box cannot fit, without changing the configured style or losing the draft. All styles SHALL retain editing, completion, mouse and selection behavior.

#### Scenario: Boxed composer with a multiline draft
- **WHEN** boxed input is configured in a terminal with sufficient space and the draft wraps to more than one row at the padded content width
- **THEN** every editor row keeps one blank column just inside each side border, including when the draft fills the content width
- **AND** the box grows by one row per draft row within its height limits, and one blank row stays above the status row
- **AND** the cursor and mouse selection align with the inset text on the wrapped rows

#### Scenario: Long status labels
- **WHEN** the left status and right model/context labels together exceed the content width in a visible box
- **THEN** the labels are clipped within the content area without overlapping, and the column next to each side border stays blank

#### Scenario: Resize or reload
- **WHEN** the terminal becomes too small for the padded box, grows again, or `/reload` changes the input style
- **THEN** insufficient space uses `lines`, and the padded box returns when it fits again if `boxed` remains configured
- **AND** layout and input hit testing update together, preserving the draft and logical cursor position

### Requirement: Lines input composer
When `ui.input` is `lines`, or a framed style does not fit, the composer SHALL be a rule with the activity on its right, the editor rows with the prompt arrow on the first row, and a rule with the model/context label on its right.

#### Scenario: Small terminal
- **WHEN** the style is `boxed` or `fill` and the terminal is too small for the frame
- **THEN** the lines composer is drawn and the configured style is unchanged

### Requirement: Fill input composer
When `ui.input` is `fill` and the viewport fits, the composer SHALL draw one blank row, the editor rows, and one blank row, all with the `input_bg` color as background across the full width. The first editor row SHALL start with the prompt arrow. One row without background SHALL follow, then the status row with the activity on the left and the model/context label on the right. While a picker, question, or panel is open, its rows and one border rule row SHALL take the place of that unfilled row.

#### Scenario: Fill composer with a multiline draft
- **WHEN** fill input is configured in a terminal with sufficient space and the draft wraps to more than one row
- **THEN** the filled block grows by one row per draft row, one unfilled row separates it from the status row, and the cursor and mouse hit testing match the editor rows

#### Scenario: Fill composer with an open list
- **WHEN** fill input is configured and a picker is open
- **THEN** the list is drawn between the filled block and the status row, and one border rule row closes it

### Requirement: Bar input composer
When `ui.input` is `bar` and the viewport fits, the composer SHALL draw each editor row as `▌` in the `icons` color at column 0 with no padding before it, one space, then the text, without a prompt arrow. The height SHALL follow the draft, limited by `ui.lines.input` and the viewport, and scroll beyond that with the cursor visible. One blank row SHALL follow the editor rows, then the status row as in `fill`, with the activity and notices on the left and the model/context label on the right. The lines composer SHALL be used when the bar cannot fit, without changing the configured style or losing the draft.

#### Scenario: Multiline draft
- **WHEN** bar input is configured and the draft wraps to three rows
- **THEN** three rows each start with `▌ `, one blank row follows, and then the status row

#### Scenario: Small terminal
- **WHEN** the terminal is too small for the bar
- **THEN** the lines composer is drawn and the configured style is unchanged
