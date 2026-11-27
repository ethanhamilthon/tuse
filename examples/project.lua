-- A per-project tuse.lua using the namespaced config format (global config loads automatically).
-- Use: copy to <repo>/tuse.lua
return {
  project = {
    systems = {
      "This is a Rust workspace. Run `cargo test` before you finish.",
    },
    prompts = {
      release = "Bump the patch version, update the changelog, commit, tag, and push.",
    },
    history = { hide = { "usage", "flash" } },
    models = {
      { name = "anthropic/claude-sonnet-4-5" },
      { name = "openai/gpt-5" },
    },
  },
}
