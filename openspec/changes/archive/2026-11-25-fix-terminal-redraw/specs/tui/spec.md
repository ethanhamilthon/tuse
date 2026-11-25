## ADDED Requirements
### Requirement: Frames stay in sync with the terminal
The TUI SHALL write each frame with a single write. It SHALL turn the terminal's automatic line wrap off while it runs and on again when it exits or panics, so text that overflows a row is clipped and MUST NOT scroll the screen. It SHALL repaint every row, not only the changed ones, at least every five seconds.

#### Scenario: One write per frame
- **WHEN** the TUI draws a frame
- **THEN** the whole frame, from the synchronized-update start to its end, reaches the output in one write

#### Scenario: Stray text
- **WHEN** another writer leaves text on a row that the renderer considers unchanged
- **THEN** the row is repainted within five seconds

#### Scenario: Line wrap
- **WHEN** the TUI starts
- **THEN** it sends the sequence that turns line wrap off, and it sends the one that turns it on when it exits or panics
