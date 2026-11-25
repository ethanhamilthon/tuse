## ADDED Requirements

### Requirement: Lua syntax highlighting
Lua code in tool calls, panels, and `lua` code blocks SHALL be colored by the `theme.lua` tokens using the code's structure. Function definition and call names SHALL be `function`, names after `:` SHALL be `method`, names after `.` and table keys SHALL be `field`, names in a function's parameter list and their uses inside that function SHALL be `parameter`, standard library and Tuse functions SHALL be `builtin`, and other names SHALL be `variable`. `local` and `function` SHALL be `storage`; `nil`, `true`, and `false` SHALL be `constant`; `and`, `or`, `not`, and symbol operators SHALL be `operator`; brackets, `,`, `;`, `.`, and `:` SHALL be `punctuation`; escapes inside strings SHALL be `escape`. Long strings and long comments SHALL continue across lines.

#### Scenario: Assignment and calls
- **WHEN** the code is `x = read("a"):upper() .. y`
- **THEN** `=` and `..` are `operator`, `read` is `builtin`, `upper` is `method`, and `x` and `y` are `variable`

#### Scenario: Parameters and table keys
- **WHEN** the code defines `function M.go(self, n) return n end` and a table `{ run = function() local y = 1 end }`
- **THEN** `go` is `function`, `self` and both `n` are `parameter`, `run` is `field`, and `y` is `variable`

### Requirement: User message color
User messages and the input text SHALL use `theme.user`; agent text SHALL use `fg`.

#### Scenario: Different colors
- **WHEN** `theme.user` differs from `theme.fg`
- **THEN** a user message and the draft in the input differ in color from an agent reply
