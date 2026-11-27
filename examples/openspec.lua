-- One #openspec prompt with an action argument: #openspec propose, #openspec apply, …
-- Use: return { openspec = require("openspec") }
local actions = {
  explore = "OpenSpec explore: map the problem in this message. Read `openspec/specs/` and active changes, inspect the code, report behavior, capabilities, constraints, and open questions. Write nothing.",
  propose = "OpenSpec propose: create a change under `openspec/changes/<change-id>/` with `proposal.md`, `tasks.md`, delta specs, and `design.md` if needed. Every requirement has a scenario. Do not touch the code.",
  apply = "OpenSpec apply: implement the named (or only active) change. Work through `tasks.md` in order and check off each task when done.",
  verify = "OpenSpec verify: check every requirement and scenario of the change against code and tests. Report covered, partial, or missing. Do not change code.",
  archive = "OpenSpec archive: merge the change's deltas into `openspec/specs/` and move it to `openspec/changes/archive/YYYY-MM-DD-<change-id>/`.",
}

return {
  systems = {
    [[
## OpenSpec workflow
This repo uses spec-driven development (OpenSpec layout):
- `openspec/specs/<capability>/spec.md`: current behavior.
- `openspec/changes/<change-id>/`: `proposal.md`, `design.md` (if needed), `tasks.md`, and delta `specs/<capability>/spec.md`.
- `openspec/changes/archive/YYYY-MM-DD-<change-id>/`: archived changes.
]],
  },
  prompts = {
    openspec = {
      args = { "explore", "propose", "apply", "verify", "archive" },
      run = function(action)
        return actions[action or "explore"]
      end,
    },
  },
}
