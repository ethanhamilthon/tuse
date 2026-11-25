## MODIFIED Requirements

### Requirement: Boxed input composer
When `settings.ui.input_style` is `boxed` and the viewport fits, the composer SHALL consist of a top border, the editor rows, one blank row, a status row, and a bottom border. Editor rows SHALL have no prompt arrow. Every editor row and the status row SHALL reserve exactly one blank display column immediately inside each side border. The padding SHALL be inside the existing frame width and separate from the outer `pad_x` margin. The editor SHALL use the remaining content width for wrapping, cursor placement, mouse hit testing, and selection. It SHALL grow with the draft, limited by `input_max_height` and the viewport, and scroll beyond that with the cursor visible. Exactly one blank row SHALL separate the last editor row from the status row. Side borders SHALL enclose all interior rows. The status row SHALL place activity and notices at the left edge of the content area and the model/context label at its right edge; labels MUST be clipped to fit without overlapping or using the padding. The box SHALL require room for the two side borders, the two padding columns, and at least two content columns after accounting for `pad_x`. The `lines` composer SHALL be used when the box cannot fit, without changing the configured style or losing the draft. All styles SHALL retain editing, completion, mouse and selection behavior.

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
