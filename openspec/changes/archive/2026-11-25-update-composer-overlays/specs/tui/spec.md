## ADDED Requirements
### Requirement: Lists open below the input
A picker, question, or panel SHALL open below the input, never between the chat and the input. With `lines` and `boxed` its rows SHALL follow the last row of the composer. With `fill` and `bar` its rows SHALL sit between the editor block and the status row, followed by one row drawn as a rule in the `border` color. Its height SHALL stay limited by `list_max_height` and the viewport, and the chat SHALL keep the rows that remain. The wheel over the list and paging SHALL use the rows where the list is drawn.

#### Scenario: Picker with boxed input
- **WHEN** the style is `boxed` and the user types `/`
- **THEN** the command list is drawn directly under the bottom border of the box and the chat above is not covered

#### Scenario: Picker with fill input
- **WHEN** the style is `fill` and the user types `/`
- **THEN** the order from the top is the filled block, the command list, one border rule row, and the status row

### Requirement: Blank row above the composer
Every input style SHALL draw one empty row above the first row of the composer: the top rule, the top border, the filled block, or the first bar row. The row holds no text and no color other than the terminal background. It MUST NOT count toward `input_max_height`. When the viewport is too small, this row SHALL be the first one dropped.

#### Scenario: All styles
- **WHEN** the style is `lines`, `boxed`, `fill`, or `bar`
- **THEN** one blank row separates the chat from the composer

#### Scenario: Tiny terminal
- **WHEN** the viewport has no room for the blank row
- **THEN** the composer is drawn without it and the draft is kept

### Requirement: Bar input composer
When `settings.ui.input_style` is `bar` and the viewport fits, the composer SHALL draw each editor row as `▌` in the `accent` color at column 0 with no padding before it, one space, then the text, without a prompt arrow. The height SHALL follow the draft, limited by `input_max_height` and the viewport, and scroll beyond that with the cursor visible. One blank row SHALL follow the editor rows, then the status row as in `fill`, with the activity and notices on the left and the model/context label on the right. The lines composer SHALL be used when the bar cannot fit, without changing the configured style or losing the draft.

#### Scenario: Multiline draft
- **WHEN** bar input is configured and the draft wraps to three rows
- **THEN** three rows each start with `▌ `, one blank row follows, and then the status row

#### Scenario: Small terminal
- **WHEN** the terminal is too small for the bar
- **THEN** the lines composer is drawn and the configured style is unchanged

## MODIFIED Requirements
### Requirement: Fill input composer
When `settings.ui.input_style` is `fill` and the viewport fits, the composer SHALL draw one blank row, the editor rows, and one blank row, all with the `border` color as background across the full width. The first editor row SHALL start with the prompt arrow. One row without background SHALL follow, then the status row with the activity on the left and the model/context label on the right. While a picker, question, or panel is open, its rows and one border rule row SHALL take the place of that unfilled row.

#### Scenario: Fill composer with a multiline draft
- **WHEN** fill input is configured in a terminal with sufficient space and the draft wraps to more than one row
- **THEN** the filled block grows by one row per draft row, one unfilled row separates it from the status row, and the cursor and mouse hit testing match the editor rows

#### Scenario: Fill composer with an open list
- **WHEN** fill input is configured and a picker is open
- **THEN** the list is drawn between the filled block and the status row, and one border rule row closes it
