local layout = [[
## OpenSpec workflow
This repo uses spec-driven development (OpenSpec layout), managed by hand without the npm CLI:
- `openspec/specs/<capability>/spec.md`: source of truth for current behavior.
- `openspec/changes/<change-id>/`: an active change with `proposal.md` (Why, What Changes, Impact), `design.md` (only when non-trivial), `tasks.md` (numbered `- [ ]` checklist), and `specs/<capability>/spec.md` delta files.
- `openspec/changes/archive/YYYY-MM-DD-<change-id>/`: archived changes.
Spec format: `### Requirement: <name>` with SHALL/MUST wording, each followed by at least one `#### Scenario: <name>` with `- **WHEN** ...` / `- **THEN** ...` bullets.
Delta files group requirements under `## ADDED Requirements`, `## MODIFIED Requirements` (full updated text), `## REMOVED Requirements` (with Reason), `## RENAMED Requirements` (FROM/TO).
Change ids are kebab-case and verb-led (`add-...`, `update-...`, `remove-...`).
]]

return {
  system = layout,
  prompts = {
    ["openspec-explore"] = [[OpenSpec explore: map the problem described in this message. Read `openspec/specs/` and active `openspec/changes/`, then inspect the relevant code. Report the current behavior, affected capabilities, constraints, and open questions. Do not write any files.]],
    ["openspec-propose"] = [[OpenSpec propose: create a new change under `openspec/changes/<change-id>/` for the request in this message. Write `proposal.md`, `tasks.md`, the delta specs under `specs/<capability>/spec.md`, and `design.md` only if it is needed. Every requirement must have a scenario. Do not touch the code. Finish with the change id and any open questions.]],
    ["openspec-apply"] = [[OpenSpec apply: implement the change named in this message (or the only active change). Read its proposal, design, and delta specs, then work through `tasks.md` in order, checking off each `- [x]` as soon as it is done. Follow the project rules and run `cargo build` and `cargo test` before finishing.]],
    ["openspec-verify"] = [[OpenSpec verify: check that the implementation matches the change named in this message (or the only active change). For every requirement and scenario in its delta specs, find the code and tests that cover it. Report each one as covered, partial, or missing, and list unchecked tasks. Do not change the code.]],
    ["openspec-archive"] = [[OpenSpec archive: archive the change named in this message (or the only active change). Make sure every task is checked off. Merge its deltas into `openspec/specs/` (ADDED appends, MODIFIED replaces, REMOVED deletes, RENAMED renames), then move the folder to `openspec/changes/archive/YYYY-MM-DD-<change-id>/` using today's date.]],
  },
}
