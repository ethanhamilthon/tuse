# TODO — 0.3.2

## `bar` input design

`lines`, `box` and `fill` stay as they are. Only `bar` changes.

- [x] New look of `bar`: a blank row between the input and the status row, the status row has no indent under the bar.

  ```
  ┃ › message

  ⠋ 12s · 3 tools      gpt-5 · high · 12k · $0.04
  ```

- [x] Theme tokens `input_top_padding` and `input_bottom_padding` (rows, default `0`).
  - Padding rows are part of the design: in `bar` the bar runs through them, and the gradient runs along the whole bar.
  - The blank row before the status row belongs to the design and is not padding. Padding goes between the text and that row.
  - Example, top 1 and bottom 2:

    ```
    ┃
    ┃ › message
    ┃
    ┃

    ⠋ 12s · 3 tools      gpt-5 · high · 12k · $0.04
    ```

- [x] Count the padding and the blank row in the input height (`frame` in `input_view.rs`).
- [x] Update "Input design" and the token list in `docs/config.md`.
- [x] `release/CHANGELOG.md`: add 0.3.2, bump `version` in `Cargo.toml`.

## `/trace` as a list

Now `/trace` prints the whole session as one long text. Make it a list, one row per item, with the details on demand.

- [x] A row per item: index, role, a short one-line text (cut with `more`).

  ```
  0 system  The start of the prompt...
  1 user    hey
  2 agent   what do you need?
  3 user    find me this file...
  4 bash    ls -la
  ```

  Roles: `system`, `user`, `agent`, `thinking`, `bash` (a tool call), `output` (its result).
- [x] Up and Down move the selection, Esc closes the panel (as now).
- [x] Enter on a row opens that item in full (the whole text, the full command, the full output). Esc goes back to the list.
- [x] Scrolling works in both views; the selected row stays visible.
- [x] Row look (role colors, index width, the one-line limit, the selection symbol) comes from theme tokens.
- [x] Update `/trace` in `README.md` and `docs/agent.md`.

## Panel separator

- [x] A line on top of a panel (`/usage`, `/trace`, a task's output) separates it from the chat behind it.
  - It is drawn above the title, over the chat background (not the panel background), so the panel's edge is visible.
  - It counts in the panel height (`panel_max_height`).
  - Its look (character, color) comes from theme tokens; the rule character can reuse `rule_char`.

## Model per session

- [x] `/models` changes the model and effort in the config and in the current session. This works now.
- [x] A new session takes the model and effort from the config and records them at once (`Record::Settings` at the start). Now a session records them only after a switch.
- [x] An old session keeps its own model. Neither `/resume` nor `/reload` changes it.
  - `/reload` already leaves the session model alone; keep it that way.
  - `/resume` of a session without a `Settings` record takes the model from `meta.json`, and only if that is empty, from the config. Now it always takes the config's.
- [x] A test: a session resumed after the config's `model` changed keeps its model.
- [x] Say it in `docs/agent.md`.
