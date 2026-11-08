-- Status: Prices' line on the core's Status tab and in /ryna status, and /ryna prices: how the
-- settings stand, and /ryna prices <name> on|off. It prints only when typed.

local _, ns = ...
if ns.off then return end
local L = ns.L
local S = ns.Settings

-- "Prices: sell line on (with the stack's total, everywhere)".
local function line()
  return { string.format(L["Prices: %s"], S.Summary()) }
end

-- After Automation (45), before the companion app (50).
ns.API.Panel.AddStatusLines(46, line)
ns.API.Slash.AddStatusLines(46, line)

local function onOff(v)
  return v and L["on"] or L["off"]
end

local function list()
  ns.Print(line()[1])
  for _, f in ipairs(S.FEATURES) do
    local state = onOff(ns.Setting(f.key))
    if f.under and ns.Setting(f.key) and not ns.Setting(f.under) then
      state = string.format(L["%s (but %s is off)"], state, S.Find(f.under).name)
    end
    -- "  sellLine: on (Show what a vendor pays on item tooltips)"; the name is what /ryna prices
    -- takes, never translated.
    ns.Print(string.format("  %s: %s (%s)", f.name, state, f.label))
  end
  ns.Print(L["/ryna prices <name> on||off changes one."])
end

-- run(arg, rest): rest is the whole line after "prices".
local function prices(arg, rest)
  if not arg or arg == "" then return list() end
  local f = S.Find(arg)
  if not f then
    ns.Print(string.format(L["no price setting called %s. /ryna prices lists them."], arg))
    return
  end
  local value = type(rest) == "string" and rest:match("^%S+%s+(%S+)") or nil
  value = value and value:lower() or nil
  if value ~= "on" and value ~= "off" then
    ns.Print(string.format(L["%s is %s. /ryna prices %s on||off changes it."], f.name, onOff(ns.Setting(f.key)),
      f.name))
    return
  end
  if not ns.API.DB() then
    ns.Print(L["not initialised yet"])
    return
  end
  ns.API.Settings.Set(f.key, value == "on")
  ns.Print(string.format(L["%s: %s"], f.name, onOff(value == "on")))
  ns.API.Panel.RefreshIfShown()
end

ns.API.Slash.Add({ word = "prices", order = 35, run = prices,
  help = L["/ryna prices: the price lines on item tooltips (/ryna prices <name> on||off changes one)"] })
