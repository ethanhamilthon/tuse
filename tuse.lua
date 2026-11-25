local g = require("global")


g.prompts = g.prompts or {}
g.systems = g.systems or {}
g.prompts.release = [[
Make a new release, new patch version.
Delete all trash from the repo, make a commit, tag and push new version.
Wait for CI, make sure they didnt fail and tell me to update Tuse.
]]
local openspec = require("./openspec")
for k, v in pairs(openspec.prompts) do g.prompts[k] = v end
g.systems[#g.systems+1] = openspec.system
g.systems[#g.systems+1] = [[
## Tuse Project Rules
You are inside the repo of Tuse, the AI agent you are.
The project is written in Rust; write idiomatic, correct Rust code.

Code rules:
- Zero comments. Never write comments.
- Do not edit README.
- Keep production Rust code under 20000 lines (excluding tests). If it goes over, warn and simplify the functionality.
- Minimal tests.
- Everything in English (UI, contracts, docs).
- Minimal temporary files inside the repository; it must stay clean.
]]

return g
