# Examples

Copyable Lua snippets for the global config: project instructions, reusable prompts, custom commands, and themes. Combine their fields in one returned table, or split reusable pieces into sibling Lua modules with `require`.

## Project instructions

To attach a project's `AGENTS.md` without executing project Lua, read it as text from the session directory. Put this in your global config:

```lua
local t = require("tuse")
local ok, rules = pcall(t.fs.read, "AGENTS.md")

return {
  systems = { project = ok and rules or "" },
  work = {
    prompts = {
      review = "Review the diff for bugs, edge cases, and missing tests.",
      test = "Run the relevant tests and report any failures.",
    },
    functions = {
      status = function()
        return t.sh("git status --short").out
      end,
    },
    commands = {
      diff = function()
        t.ui.show("diff", "```diff\n" .. t.sh("git diff").out .. "\n```")
      end,
      rename = function()
        local title = t.ui.ask("New session title?")
        if title and title ~= "" then t.session.set { title = title } end
      end,
    },
  },
}
```

Select `#work:review` or `#work:test` in the prompt picker. Run `/work:diff` to show the diff or `/work:rename` to set a title. The agent can call `work.status()` in its Lua VM.

## Theme

Use `ui` at the top level to style the composer and code blocks:

```lua
return {
  ui = {
    input = "bar",
    picker = "top",
    theme = { accent = "#FFB86C", code = { keyword = "accent" } },
  },
}
```

See [Config](config.md#theme-and-ui) for all tokens and defaults.

## External plugins

Plugins are not bundled. Install one from a source you trust with `/plugins add <source>`, then run `/reload` to activate it in the current session. See [Installed plugins](config.md#installed-plugins) and the [Lua API](lua.md#plugins-plugins).
