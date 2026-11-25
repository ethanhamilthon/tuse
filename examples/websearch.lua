-- A websearch(query) agent function backed by a SearXNG instance.
-- Use: require("websearch").apply(g, "http://localhost:8888")
local t = require("tuse")

local M = {}

function M.apply(g, base)
  g.functions = g.functions or {}
  g.systems = g.systems or {}
  g.functions.websearch = function(q, n)
    local r = t.sh("curl -fsS --get \"$BASE/search\" --data-urlencode \"q=$Q\" -d format=json",
      { env = { BASE = base, Q = q }, timeout = 30 })
    if not r.ok then
      error("websearch: " .. r.out)
    end
    local out = {}
    for i, it in ipairs(t.json.decode(r.out).results or {}) do
      if i > (n or 8) then
        break
      end
      out[#out + 1] = string.format("%d. %s\n   %s\n   %s", i, it.title or "", it.url or "", it.content or "")
    end
    return table.concat(out, "\n")
  end
  g.systems[#g.systems + 1] = "Call `websearch(query, n)` to search the web; it returns titles, URLs, and snippets."
  return g
end

return M
