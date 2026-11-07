-- Status: Automation's line on the core's Status tab and in /ryna status (what's on), and
-- /ryna auto: the toggles, /ryna auto <name> on|off, /ryna auto traincap <gold> (the trainer's cap)
-- and /ryna auto junk [itemID] (marks or unmarks the item on the cursor, or that item, as junk).

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
  ns.Print(string.format(L["  traincap: %s (/ryna auto traincap <gold> changes it; 0 trains nothing)"],
    ns.MoneyText(S.TrainCap())))
  local n = #ns.Vendor.JunkIDs()
  ns.Print(string.format(n == 1
    and L["  junk: %d item marked (/ryna auto junk marks or unmarks the item on the cursor)"]
    or L["  junk: %d items marked (/ryna auto junk marks or unmarks the item on the cursor)"], n))
  ns.Print(L["/ryna auto <name> on||off changes one. Hold Shift at an NPC to pause it all."])
end

-- /ryna auto traincap [gold]
local function trainCap(value)
  local gold = tonumber(value)
  if not gold then
    ns.Print(string.format(L["The trainer's cap is %s. /ryna auto traincap <gold> changes it (0 trains nothing)."],
      ns.MoneyText(S.TrainCap())))
    return
  end
  if not ns.API.DB() then return ns.Print(L["not initialised yet"]) end
  S.SetTrainCap(math.max(0, gold) * 10000)
  ns.Print(string.format(L["The trainer's cap is now %s."], ns.MoneyText(S.TrainCap())))
  ns.API.Panel.RefreshIfShown()
end

local JUNK_REFUSALS = {
  quest = L["%s is a quest item, so it can't be junk."],
  noprice = L["%s has no vendor price, so it can't be sold as junk."],
}

-- Marks or unmarks an item (the one on the cursor when no ID is given) and says which. Also the tab's
-- button (Tab.lua).
function S.MarkJunk(itemID)
  local link
  if not itemID then
    local what, id, cursorLink = ns.G("GetCursorInfo")
    if what ~= "item" or type(id) ~= "number" then
      ns.Print(L["Pick up an item first (or give its item ID): /ryna auto junk marks or unmarks it."])
      return
    end
    itemID, link = id, cursorLink
  end
  link = ns.NonEmpty(link) or select(2, ns.ItemInfo(itemID)) or string.format(L["item %d"], itemID)
  local marked, why = ns.Vendor.ToggleJunk(itemID)
  if marked == nil then
    ns.Print(why == "nodb" and L["not initialised yet"] or string.format(JUNK_REFUSALS[why], link))
  elseif marked then
    ns.Print(string.format(L["%s is marked as junk: it's sold with the greys when selling junk is on."], link))
  else
    ns.Print(string.format(L["%s is no longer junk."], link))
  end
  ns.API.Panel.RefreshIfShown()
end

-- run(arg, rest): rest is the whole line after "auto" (cores after 0.3.4; 0.3.4 passes only arg).
local function auto(arg, rest)
  if not arg or arg == "" then return list() end
  local value = type(rest) == "string" and rest:match("^%S+%s+(%S+)") or nil
  if arg:lower() == "traincap" then return trainCap(value) end
  if arg:lower() == "junk" then return S.MarkJunk(tonumber(value)) end
  local f = S.Find(arg)
  if not f then
    ns.Print(string.format(L["no automation called %s. /ryna auto lists them."], arg))
    return
  end
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
  S.Changed(f.key, value == "on")
  ns.API.Panel.RefreshIfShown()
end

ns.API.Slash.Add({ word = "auto", order = 30, run = auto,
  help = L["/ryna auto: what Automation does for you (/ryna auto <name> on||off changes one)"] })
