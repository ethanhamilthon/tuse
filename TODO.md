# TODO

## 1. Navigate trace entries with Left and Right

- [x] In a `/trace` detail panel, use Left/Right to open the previous/next entry in the filtered list, without going back to the picker. Start each entry at the top and stop at the ends of the list.
- [x] Keep the search query and update the selection so Esc returns to the entry last viewed. Leave arrow behavior outside trace details unchanged.

## 2. Support four API types

- [x] Support OpenAI-compatible Chat Completions (already supported), OpenAI Responses, Anthropic Messages, and the native Google Gemini API. Document which capabilities each API supports.
- [x] Document how a provider chooses its API type and how existing provider records keep working. Use separate adapters for the API-specific protocol details and keep the same session, streaming, and tool behavior across TUI and JSON mode where supported.
- [x] Define how unsupported capabilities are reported, and test the request, streaming, tool-call, usage, and error behavior of each adapter.

## 3. Make `docs/` the source of truth for development

- [x] Audit the current app against `docs/` and the README. Fix stale descriptions and fill gaps so `docs/` covers the whole app. Start with CLI and TUI workflows, installation and updates, sessions and persistence, the agent loop and tool lifecycle, model/provider selection, usage and compaction, JSON-RPC, config, and Lua (tools, tasks, hooks, and plugins).
- [x] Describe observable behavior and public contracts at a high level: normal flows, defaults, state changes, and important edge cases. Do not prescribe internal source files or functions. Keep public commands, config keys, and API names where readers need them.
- [x] Add a docs index and review the baseline against the app and its tests. Keep planned changes separate from descriptions of how the app works today. The baseline is ready when an agent can use `docs/` to reconstruct the app's behavior without reverse-engineering the source.
- [x] For each future change, edit `docs/` first. Hand the reviewed docs diff to a separate implementation agent to update the code and tests. Check the result against the documented behavior.

## 4. Publish the docs on GitHub Pages

- [x] Publish the same Markdown from `docs/` rather than maintaining a second copy. Use the docs index as the entry point to the docs section, separate from the product landing page (see #14). Preserve relative links.
- [x] Add a GitHub Actions build and link check for pull requests; deploy to GitHub Pages from the default branch.

## 5. Simplify the working indicator

- [x] Remove transient activity labels such as `running` and `reading` from the TUI working indicator. Keep the spinner and elapsed time, the separate queued-message indication, and tool output and logs unchanged.

## 6. Install and update plugins on reload

- [x] On every reload (`/reload` and JSON-RPC `config.reload`), re-read the config and sync all enabled Git plugins. Clone newly configured plugins that are missing locally. For existing checkouts, fetch and compare commits; if the configured ref (or default branch when no ref is set) resolves to a different commit, load that revision automatically. Keep commit pins fixed and sync shared repositories only once per reload.
- [x] Re-read local `dir` plugins from disk without Git or network access. Skip disabled plugins and unload removed ones on successful reload. Never overwrite local changes.
- [x] Validate the refreshed plugins and merged config before switching. If fetching, installing, or validation fails, report which plugin failed and keep the previous working config and callbacks active.
- [x] Update the plugin and reload docs first. Cover new installs, updated and unchanged refs, pinned commits, local edits, and failed reloads in tests.

## 7. Add a bundled default plugin: `theme` and `subs`

- [x] Bundle a default plugin with two parts: `theme` for the default Tuse palette (user overrides still win), and `subs` as a wrapper around CLIProxyAPI.
- [x] Define how `subs` sets up the proxy, connects it as a Tuse provider, reports its status and errors, and stops any process it started. Make installation, login, and proxy startup explicit opt-ins; loading or reloading the plugin alone must not trigger them.
- [x] Document the default plugin and the `subs` setup flow in `docs/` before implementation.

## 8. Persist `show`, `pick`, and `ask` in UI history

- [x] Save `show` blocks, completed `pick` selections, and `ask` questions with their answers in the session's UI history. Restore them in the chat in their original positions when resuming, and expose the restored history to JSON-RPC clients.
- [x] Preserve `show`'s named update, removal, and pinning behavior across resume. Do not restore pending prompts or create history entries for canceled prompts. Keep these records out of the model's conversation context.
- [x] Document the behavior first and test save/resume in TUI and JSON mode, including old sessions without UI history.

## 9. Make configured functions agent-only and undiscoverable by default

- [x] Accept `return { functions = { websearch = function(...) ... end } }`. Make `websearch(...)` callable from the agent's `lua` tool without automatically listing its name, signature, or description in the system prompt or in agent-facing function docs. Users can describe how to call it explicitly in a custom prompt or system instructions.
- [x] Expose configured functions only in the agent's Lua environment, not to config code, plugins, hooks, commands, other configured functions, or their background jobs. Use local Lua helpers for code shared by configured functions.
- [x] Allow configured functions to override built-in host functions such as `sh` and `read` in the agent's Lua tool. A wrapper can call the original through a documented handle available to trusted config code, without recursing or exposing that handle to the agent. Overrides must not change what config code, hooks, or plugins can call.
- [x] Allow disabling a built-in for the agent, including access through `require("tuse")` or other aliases. In a returned Lua table, `sh = nil` is indistinguishable from omitting `sh`, so use an explicit marker such as `functions = { sh = false }`. Omitting a built-in leaves it enabled. Keep agent-facing docs in sync with disabled built-ins.
- [x] Document the scope and the migration from `run`, `args`, `description`, and `show_agent` before implementation. Test visibility, isolation, wrapper delegation, disabled aliases, and restoring defaults on reload.

## 10. Document configuration and plugin best practices

- [x] Add a setup guide with a minimal working config, provider and model setup, and examples of splitting personal and project settings. Show how to use environment variables for secrets, organize larger configs, and verify changes with `/config` and `/reload`.
- [x] Add a plugin authoring guide with a small working example. Cover plugin layout, options and defaults, config merge rules, safe side effects, local testing, and publishing and pinning releases. Link both guides from the docs index.

## 11. Add first-run setup to the installer

- [x] Install the `tuse` binary first. Only after that succeeds, offer the bundled default plugin options (`theme` and `subs`; see #7), with a way to skip them.
- [x] Create `~/.config/tuse/init.lua` only as the final step, with the selected plugins or a minimal config if none were selected. Preserve existing global configs (`~/.config/tuse.lua` or `~/.config/tuse/init.lua`) without modification.
- [x] Support interactive setup when piped to `sh` and a non-interactive binary-only path for CI or when no terminal is available. Document the install flow first; test selections, skips, existing configs, and failed binary installation.

## 12. Hide the entire diff block when `diff_line = 0`

- [x] When `settings.ui.diff_line = 0`, render no diff block in the TUI chat: no file path, no `… N more lines` footer, and no leftover spacing. Currently the setting suppresses the body but still renders the header and footer.
- [x] Apply the setting to existing chat entries after `/reload` and when resuming a session. Keep stored diffs and ordinary tool output unchanged. Add a regression test for `0`, and preserve truncation for positive limits and full output for `-1`.

## 13. Rewrite the README

- [x] Make the README a concise introduction for GitHub visitors: what Tuse is, what makes it useful, how to install it, and how to start a first session.
- [x] Keep detailed setup and reference material in `docs/`. Link to the landing page, docs, and releases; check that commands and examples match current behavior.

## 14. Build a Tuse landing page on GitHub Pages

- [x] Design a product homepage that explains Tuse at a glance. Show a terminal demo or screenshot and make installation, documentation, and the GitHub repository easy to find.
- [x] Host the landing page and docs on the same GitHub Pages site. Use the site root for the landing page and `/docs/` for the docs index, relative to the site's base URL. Share the build and deployment from #4.
- [x] Check the mobile layout and verify navigation and asset URLs under the GitHub Pages repository path.

## 15. Use accent colors for interactive links

- [x] Render clickable URLs, local paths, and Markdown links in `accent` by default and `fg` on hover or click. Return to `accent` when the pointer leaves; do not add a persistent visited color or change how links open.
- [x] Update the theme docs first and test default and hover colors, including links that wrap across lines.
