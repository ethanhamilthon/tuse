## Why
A finished background task never reached an idle agent: its notice waited for the next user message, although the prompt says task completion wakes the agent. The input rule labels were also clipped to half of the width.

## What Changes
- A task notice starts a turn when the session is idle.
- The input rule labels (scroll mark, working status, model) are no longer limited to 50% of the width.

## Impact
- Specs: session
- Code: session submit, TUI input rule
