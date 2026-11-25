## Why
The renderer only redraws rows that changed and trusts that the terminal kept the rest. Anything else that writes to the terminal at the same time, such as a hook that writes an escape sequence to `/dev/tty`, can cut one of the renderer's escape sequences in two. The tail of the cut sequence is then printed as text (`;218;218;218m`), the row grows past the screen width, the terminal wraps it on the last row and scrolls the screen by one row, and the old composer and status rows stay on screen above the new ones. Rows that the renderer thinks are unchanged are never fixed.

## What Changes
- A frame is built in memory and written with one write.
- The terminal's automatic line wrap is off while the TUI runs, so overflowing text is clipped and never scrolls the screen. It is turned back on when the TUI exits, also after a panic.
- Every row is repainted at least every five seconds, which removes stray text left by another writer.

## Impact
- Specs: tui
- Code: `crates/tui/src/frame/draw.rs`, `crates/tui/src/lib.rs`
