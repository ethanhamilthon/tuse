-- Handlers for clicked file paths: Markdown in glow, images and PDFs in the OS viewer.
-- Use: require("open").apply(g)
local t = require("tuse")

local M = {}

local function os_open(path)
  t.sh((t.sh("command -v open").ok and "open" or "xdg-open") .. " \"$P\"", { env = { P = path } })
end

function M.apply(g)
  g.open = g.open or {}
  g.open.md = function(path)
    if t.sh("command -v glow").ok then
      t.show("open", t.sh("glow -s dark \"$P\"", { env = { P = path } }).out, false)
    else
      t.show("open", t.read(path), false)
    end
  end
  g.open.png = os_open
  g.open.pdf = os_open
  return g
end

return M
