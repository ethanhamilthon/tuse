## Context
`Item` holds `label`, `detail`, `value`; `/history` joins index, role, and summary into one `label`, so no renderer can align them.

## Decision
`Item` gets a list of cells in place of `label` and `detail`. `item(label, detail, value)` keeps working and builds one or two cells; `/history` builds three. The renderer measures the widest cell of each column over all items of the picker, not only the visible ones, so columns do not jump while scrolling or filtering. Search matches the cells joined with a space.
