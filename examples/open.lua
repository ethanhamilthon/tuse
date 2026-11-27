-- Handlers for clicked file paths: Markdown in glow, images and PDFs in the OS viewer.
-- Use: return { open = require("open") }
local t = require("tuse")

local function os_open(path)
  t.sh((t.sh("command -v open").ok and "open" or "xdg-open") .. " \"$P\"", { env = { P = path } })
end

return {
  open = {
    md = function(path)
      if t.sh("command -v glow").ok then
        t.show("open", t.sh("glow -s dark \"$P\"", { env = { P = path } }).out, false)
      else
        t.show("open", t.read(path), false)
      end
    end,
    png = os_open,
    pdf = os_open,
  },
}
