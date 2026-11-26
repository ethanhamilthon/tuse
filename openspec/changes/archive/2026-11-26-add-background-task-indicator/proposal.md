## Why

Background tasks run out of sight. The status spot that shows the working indicator while a turn runs, and the project directory when idle, says nothing about them, so the user has to open `/tasks` to learn that something is still running.

## What Changes

- While background tasks are running, the status spot shows an animated spinner and the count in the accent color: `⠋ 2 tasks`, or `⠋ 1 task`.
- Idle: the indicator replaces the project directory.
- During a turn: only the spinner of the activity is shown, and the count follows the elapsed timer, as `⠋ 12s · 2 tasks`.
- Notices and flashes still win over it. With no running tasks, nothing changes.
- Only running tasks count. Finished, failed, and stopped ones do not.

## Impact

- Specs: `tui` (modifies "Idle status shows the project directory" and "Show the foreground activity in the composer").
- Code: `crates/tui/src/ui/screen/status.rs`, the redraw tick in `crates/tui/src/ui/`, `docs/usage.md`.
- No JSON-RPC or session changes.
