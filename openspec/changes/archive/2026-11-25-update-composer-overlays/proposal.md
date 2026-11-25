## Why
Pickers, questions, and panels open above the input and push the chat up, away from where the user is typing. The composer also touches the chat with no gap, and there is no light style with a plain left bar.

## What Changes
- Pickers, questions, and panels open below the input. In `lines` and `boxed` they sit right under the composer. In `fill` and `bar` they sit between the editor block and the status row, with one border rule row under them.
- Every composer style gets one blank row above it.
- New `settings.ui.input_style = "bar"`: a `▌` in the `accent` color at column 0 with no padding before it, one space, then the text; the height follows the text; one blank row, then the status row as in `fill`.
- Update the config validation, `/config`, and docs for `bar`.

## Impact
- Specs: tui, config
- Code: `crates/config/src/theme.rs`, `crates/config/src/load.rs`, `crates/tui/src/ui/screen/compose.rs`, `crates/tui/src/ui/screen/input_view.rs`, `crates/tui/src/ui/mouse.rs`, `docs/config.md`, `docs/usage.md`
