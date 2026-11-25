-- Wraps a stdio MCP server as an agent function. The agent calls mcp_call("tool", { ... }).
-- Use: g.functions = require("mcp").functions({ "npx", "-y", "@modelcontextprotocol/server-everything" })
local t = require("tuse")

local M = {}

local function request(cmd, method, params)
  local init = t.json.encode({ jsonrpc = "2.0", id = 1, method = "initialize", params = {
    protocolVersion = "2025-06-18", capabilities = {}, clientInfo = { name = "tuse", version = "1" } } })
  local inited = t.json.encode({ jsonrpc = "2.0", method = "notifications/initialized" })
  local call = t.json.encode({ jsonrpc = "2.0", id = 2, method = method, params = params or {} })
  local input = init .. "\n" .. inited .. "\n" .. call .. "\n"
  local r = t.sh("printf '%s' \"$IN\" | " .. cmd .. " | grep -m1 '\"id\":2'", { env = { IN = input }, timeout = 60 })
  local ok, msg = pcall(t.json.decode, r.out)
  if not ok then
    error("mcp: " .. r.out)
  end
  if msg.error then
    error("mcp: " .. (msg.error.message or "error"))
  end
  return msg.result
end

function M.functions(argv, functions)
  local quoted = {}
  for i, a in ipairs(argv) do
    quoted[i] = string.format("%q", a)
  end
  local cmd = table.concat(quoted, " ")
  functions = functions or {}
  functions.mcp_tools = function()
    local out = {}
    for _, tool in ipairs(request(cmd, "tools/list").tools or {}) do
      out[#out + 1] = tool.name .. ": " .. (tool.description or "")
    end
    return table.concat(out, "\n")
  end
  functions.mcp_call = function(name, args)
    local res = request(cmd, "tools/call", { name = name, arguments = args or {} })
    local out = {}
    for _, c in ipairs(res.content or {}) do
      out[#out + 1] = c.text or t.json.encode(c)
    end
    return table.concat(out, "\n")
  end
  return functions
end

M.system = "You have `mcp_tools()` to list MCP tools and `mcp_call(name, args)` to call one."

return M
