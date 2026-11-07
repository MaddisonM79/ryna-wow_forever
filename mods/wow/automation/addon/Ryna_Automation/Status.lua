-- Status: Automation's line on the core's Status tab and in /ryna status (what's on), and
-- /ryna auto: the toggles, and /ryna auto <name> on|off.

local _, ns = ...
if ns.off then return end
local L = ns.L
local S = ns.Settings

-- "Automation: quests (accept, turn in), vendor (greys)", or "Automation: nothing on".
local function line()
  local on = S.Summary()
  return { on and string.format(L["Automation: %s"], on) or L["Automation: nothing on"] }
end

-- After the census (40), before the companion app (50).
ns.API.Panel.AddStatusLines(45, line)
ns.API.Slash.AddStatusLines(45, line)

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
    -- "  acceptQuests: off (Accept quests)"; the name is what /ryna auto takes, never translated.
    ns.Print(string.format("  %s: %s (%s)", f.name, state, S.Label(f.key)))
  end
  ns.Print(L["/ryna auto <name> on||off changes one. Hold Shift at an NPC to pause it all."])
end

-- run(arg, rest): rest is the whole line after "auto" (cores after 0.3.4; 0.3.4 passes only arg).
local function auto(arg, rest)
  if not arg or arg == "" then return list() end
  local f = S.Find(arg)
  if not f then
    ns.Print(string.format(L["no automation called %s. /ryna auto lists them."], arg))
    return
  end
  local value = type(rest) == "string" and rest:match("^%S+%s+(%S+)") or nil
  if value ~= "on" and value ~= "off" then
    ns.Print(string.format(L["%s is %s. /ryna auto %s on||off changes it."], f.name, onOff(ns.Setting(f.key)), f.name))
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

ns.API.Slash.Add({ word = "auto", order = 30, run = auto,
  help = L["/ryna auto: what Automation does for you (/ryna auto <name> on||off changes one)"] })
