## Why
Picker rows are built as one joined string, so the columns drift. In `/history` the role and text start at a different column on every row (`0 system  …`, `10 hook  …`), and in the `/` command list each description starts right after its own name.

## What Changes
- Every picker list draws its items as aligned columns: each column starts at the same display column on every row, padded to its widest cell.
- `/history` rows get three columns: index, role, summary.
- Items with a label and a detail (commands, models, sessions, tasks, Lua pickers) align the detail the same way.
- Cells are left-aligned, indexes included.
- The `Models` section of `/config` aligns the `window=… prices=…` details in a column after the widest model name.
- Search keeps matching the whole row text.

## Impact
- Specs: tui
- Code: `crates/tui/src/ui/picker.rs`, `crates/tui/src/ui/screen/list.rs`, `crates/tui/src/ui/commands.rs`, `crates/tui/src/ui/panel.rs`, `crates/tui/src/ui/screen/config_view.rs`
