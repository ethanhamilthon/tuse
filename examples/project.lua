-- A per-project tuse.lua that builds on your global config.
-- Use: copy to <repo>/tuse.lua
local ok, g = pcall(require, "global")
if not ok or type(g) ~= "table" then
  g = {}
end

g.systems = g.systems or {}
g.prompts = g.prompts or {}
g.settings = g.settings or {}
g.settings.ui = g.settings.ui or {}

g.systems[#g.systems + 1] = "This is a Rust workspace. Run `cargo test` before you finish."
g.prompts.release = "Bump the patch version, update the changelog, commit, tag, and push."
g.settings.ui.hide_trace = { "usage", "hook" }
g.settings.ui.scope_models = { "anthropic/*", "openai/gpt-5" }

return g
