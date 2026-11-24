local tuse, SCHED = ...
local type, tostring, error, pcall, select = type, tostring, error, pcall, select
local pairs, ipairs, getmetatable = pairs, ipairs, getmetatable
local str_format, str_gsub, str_match, str_sub, str_gmatch =
  string.format, string.gsub, string.match, string.sub, string.gmatch
local tbl_sort, tbl_concat = table.sort, table.concat
local math_type = math.type
local co_create, raw_resume, raw_yield, status, isyieldable =
  coroutine.create, coroutine.resume, coroutine.yield, coroutine.status, coroutine.isyieldable

local function pass(co, ok, ...)
  if ok and (...) == SCHED and status(co) == "suspended" then
    return pass(co, raw_resume(co, raw_yield(...)))
  end
  if ok and status(co) == "suspended" and select("#", ...) == 0 and isyieldable() and tuse.nested_yield(co) then
    raw_yield(SCHED, "ready")
    return pass(co, raw_resume(co))
  end
  return ok, ...
end
local function wrapped_resume(co, ...)
  return pass(co, raw_resume(co, ...))
end
coroutine.resume = wrapped_resume
function coroutine.wrap(f)
  local co = co_create(f)
  return function(...)
    local function check(ok, ...)
      if not ok then error((...), 2) end
      return ...
    end
    return check(wrapped_resume(co, ...))
  end
end

local function wait(kind, arg)
  local ok, v
  if isyieldable() then
    ok, v = raw_yield(SCHED, kind, arg)
  else
    ok, v = tuse.block(kind, arg)
  end
  if not ok then error(v, 3) end
  return v
end

local sh_mt = {}
sh_mt.__index = sh_mt
sh_mt.__name = "sh"
sh_mt.__metatable = "sh"
function sh_mt:__tostring()
  local note = ""
  if self.timed_out then note = "[timed out]"
  elseif self.code == nil then note = "[killed by signal]"
  elseif self.code ~= 0 then note = "[exit code " .. self.code .. "]" end
  if note == "" then return self.out end
  if self.out ~= "" and str_sub(self.out, -1) ~= "\n" then return self.out .. "\n" .. note end
  return self.out .. note
end
function sh_mt:lines()
  local t = {}
  for l in str_gmatch(self.out .. "\n", "(.-)\r?\n") do t[#t + 1] = l end
  if t[#t] == "" then t[#t] = nil end
  return t
end
tuse.sh_mt = sh_mt

function sh(cmd, opts)
  return wait("sh", tuse.sh_start(cmd, opts))
end

local function key_order(a, b)
  local ta, tb = type(a), type(b)
  if ta ~= tb then return ta < tb end
  if ta == "number" or ta == "string" then return a < b end
  return tostring(a) < tostring(b)
end

local function inspect_val(v, depth, seen)
  depth, seen = depth or 0, seen or {}
  local t = type(v)
  if t == "string" then return depth == 0 and v or str_format("%q", v) end
  if t ~= "table" then return tostring(v) end
  local mt = getmetatable(v)
  if mt == "sh" or (type(mt) == "table" and mt.__tostring) then return tostring(v) end
  if seen[v] then return "<cycle>" end
  if depth > 6 then return "{...}" end
  seen[v] = true
  local parts, n = {}, #v
  for i = 1, n do parts[#parts + 1] = inspect_val(v[i], depth + 1, seen) end
  local keys = {}
  for k in pairs(v) do
    if not (math_type(k) == "integer" and k >= 1 and k <= n) then keys[#keys + 1] = k end
  end
  tbl_sort(keys, key_order)
  for _, k in ipairs(keys) do
    local name = type(k) == "string" and str_match(k, "^[%a_][%w_]*$") and k or "[" .. inspect_val(k, depth + 1, seen) .. "]"
    parts[#parts + 1] = name .. " = " .. inspect_val(v[k], depth + 1, seen)
  end
  seen[v] = nil
  return "{" .. tbl_concat(parts, ", ") .. "}"
end

inspect = inspect_val

function tuse.render(...)
  local out = {}
  for i = 1, select("#", ...) do
    local v = select(i, ...)
    if v ~= nil then out[#out + 1] = inspect_val(v) end
  end
  return tbl_concat(out, "\t")
end

local function clean(e)
  if type(e) ~= "userdata" then return e end
  local s = str_gsub(tostring(e), "^runtime error: ", "")
  return (str_gsub(s, "\nstack traceback:.*$", ""))
end
local function unwrap(ok, ...)
  if ok then return ... end
  error(clean((...)), 2)
end
function tuse.wrap(f)
  return function(...) return unwrap(pcall(f, ...)) end
end
function tuse.yield_wait(start)
  return function(...)
    if not isyieldable() then error("interactive wait cannot run here", 2) end
    local ok, key = pcall(start, ...)
    if not ok then error(clean(key), 2) end
    if key == nil then return nil end
    return wait("wait", key)
  end
end
tuse.sh_start = tuse.wrap(tuse.sh_start)

local smt = getmetatable("")
if type(smt) == "table" then smt.__metatable = "string" end
local fmt = io and io.stdout and getmetatable(io.stdout)
if type(fmt) == "table" then fmt.__metatable = "FILE*" end

os.exit = nil
