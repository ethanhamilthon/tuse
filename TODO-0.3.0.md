# TODO — 0.3.0

## Config lookup

- [x] Load one entry point in this order:
  1. Use `--config PATH` when provided.
  2. Otherwise, use `./tuse.yaml` if it exists in the current directory.
  3. Otherwise, look at `TUSE_GLOBAL_CONFIG`. It points to the global config file. If unset, look for `~/.config/tuse/main.yaml`.
- [x] Remove implicit `default.yaml` loading. The config loader has no special knowledge of `default.yaml`. The shipped `main.yaml` loads it through `include:`.

## Installer

- [x] Install the binary.
- [x] Run `tuse reset` only if `TUSE_GLOBAL_CONFIG` is unset and `~/.config/tuse` does not exist.
- [x] Run `tuse connect` after installation and after `tuse reset` if it ran.

## `tuse reset`

- [x] Download fresh `main.yaml` and `default.yaml` from the latest release.
- [x] Delete and recreate the directory containing the global config file (`~/.config/tuse` if `TUSE_GLOBAL_CONFIG` is unset). This removes existing config and other files in that directory.
- [x] Install both downloaded files there.

## `tuse update`

- [x] Update the installed binary to the latest release.
- [x] Replace only `default.yaml` in the global config directory with the copy from that release. Leave `main.yaml` and other files untouched.

## Panels

- [x] Show `/usage`, `/tasks` output (for example, a task's log), and other info output in a separate panel, not as a message in the chat.
  - Currently they are printed into the chat as notice lines (for example `· t1 config-smoke [exited]` with the command and output).
  - The panel opens over the chat, does not become part of the conversation, and closes with Esc.
  - Panel look (colors, spacing, limits, symbols) comes from theme tokens.

## `tuse docs`

- [x] Add the `tuse docs` command. It prints the Tuse documentation for the agent, so the agent knows how to work with Tuse.
  - Covers env variables (`TUSE_SESSION`, `TUSE_GLOBAL_CONFIG`, ...), how configs are found and loaded, and the other Tuse commands.
  - Output is plain text that the agent reads through `bash`.

## Input animation

- [x] While the agent works, a gradient animation runs along the two lines of the input (top and bottom).
  - The gradient colors are theme tokens.
  - `motion: false` in a theme turns the animation off.

## Theme key

- [x] Move `theme: some-theme` inside `settings` (`settings.theme`).
  - `/theme` saves the choice there.
  - Update `docs/config.md`.

## `/trace`

- [x] Add the `/trace` command. It shows everything in the session, in order:
  - the system prompt;
  - user and agent messages;
  - thinking (reasoning);
  - tool calls and their output.
  - Shown in a panel, like `/usage` and `/tasks` (see "Panels").

## `tuse print`

- [x] Add the `tuse print` command. It prints a multiline text to the UI as plain text, without Markdown.
  - It prints back an ID.
  - `tuse print --id ID` replaces the text of that block in place.
  - Purpose: lets scripts and the agent draw their own interface in the UI.

## Input designs

- [x] Ship 4 input designs in the app. Each has the input and two status bars: working (time, calls) and model (model, effort, context, cost).
  1. Lines (current): a rule above and below the input; working status on the top rule, model status on the bottom rule, both at the right end.
  2. Box: a rounded frame around the input; working status on the top border, model status on the bottom border.
  3. Fill: no lines; the input has its own background; below it one status row, working on the left, model on the right.
  4. Bar: no lines; a colored vertical bar on the left of the input; below it one status row, working on the left, model on the right.
- [x] The theme picks the design with a token (for example `input_design: lines | box | fill | bar`, default `lines`).
- [x] The gradient animation (see "Input animation") fits each design: the rules in 1, the border in 2, the input background edge in 3, the vertical bar in 4.

## Input prefix

- [x] Add the theme token `input_prefix`, default `› `. Set it to `""` to remove the prefix.
