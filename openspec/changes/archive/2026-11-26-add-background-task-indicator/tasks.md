## Tasks

- [x] 1. Count running background tasks for the current session in the TUI without scanning storage on every frame.
- [x] 2. Build the `⠋ n task(s)` label in the accent color, with the singular for 1.
- [x] 3. Show it in place of the directory when idle, and after the timer as `· n tasks` during a turn, in every input style.
- [x] 4. Keep animating while tasks run and stop redrawing when none are left; clear the count when switching sessions.
- [x] 5. Update `docs/usage.md`.
- [x] 6. Add minimal tests: idle with one and two tasks, and no tasks after `/stop`.
