## Why
The `boxed` composer puts draft text and status labels directly against its side borders. One empty terminal column on each side makes both rows easier to read.

## What Changes
- Reserve one blank display column just inside each side border of `boxed`, on every editor row and the status row.
- Use the narrower content area for draft wrapping, cursor placement, mouse hit testing, and status clipping. Fall back to `lines` if the padded box cannot fit.
- Keep the outer box width, vertical spacing, and other input styles unchanged.

## Impact
- Specs: `tui`
- Implementation: boxed composer layout and rendering, editor mouse hit testing, and TUI tests.
