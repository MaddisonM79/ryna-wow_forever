-- Settings: Automation's toggles, each its own key in RynaDB.settings (the core keeps one flat table
-- for the core and every mod, so these all start with "auto"). Every feature is off by default:
-- turning the mod on in the companion app does nothing until a feature here is ticked too. Only
-- `autoReport`, which acts on nothing (it prints what the others did), starts on.
--
-- Each feature has the short name `/ryna auto <name> on|off` takes (the brief's words), its setting,
-- its group and, for the ones that only matter under another, that one (`under`).

local _, ns = ...
if ns.off then return end
local L = ns.L

local S = {}
ns.Settings = S

S.FEATURES = {
  { name = "acceptQuests", key = "autoAcceptQuests", group = "quests", label = L["accept"] },
  { name = "skipLowLevel", key = "autoSkipLowLevel", group = "quests", under = "autoAcceptQuests",
    label = L["not low-level ones"] },
  { name = "turnInQuests", key = "autoTurnInQuests", group = "quests", label = L["turn in"] },
  { name = "pickReward", key = "autoPickReward", group = "quests", under = "autoTurnInQuests",
    label = L["pick the reward"] },
  { name = "skipGossip", key = "autoSkipGossip", group = "quests", label = L["skip quest-only gossip"] },
  { name = "sellGreys", key = "autoSellGreys", group = "vendor", label = L["greys"] },
  { name = "repair", key = "autoRepair", group = "vendor", label = L["repair"] },
  { name = "repairGuild", key = "autoRepairGuild", group = "vendor", under = "autoRepair",
    label = L["guild bank first"] },
  { name = "report", key = "autoReport", group = "chat", label = L["chat lines"] },
}

local defaults = {}
for _, f in ipairs(S.FEATURES) do defaults[f.key] = false end
defaults.autoReport = true
ns.API.Settings.AddDefaults(defaults)

-- Whether a feature is on: its own toggle, and the one it sits under (if any).
function S.On(key)
  if not ns.Setting(key) then return false end
  for _, f in ipairs(S.FEATURES) do
    if f.key == key and f.under then return ns.Setting(f.under) and true or false end
  end
  return true
end

-- The feature `/ryna auto <word>` names: its short name or its setting, in any case.
function S.Find(word)
  word = tostring(word or ""):lower()
  for _, f in ipairs(S.FEATURES) do
    if f.name:lower() == word or f.key:lower() == word then return f end
  end
  return nil
end

-- "quests (accept, turn in), vendor (greys)": what's on, by group; nil when nothing acts.
function S.Summary()
  local groups = { { id = "quests", label = L["quests (%s)"] }, { id = "vendor", label = L["vendor (%s)"] } }
  local parts = {}
  for _, g in ipairs(groups) do
    local on = {}
    for _, f in ipairs(S.FEATURES) do
      if f.group == g.id and S.On(f.key) then on[#on + 1] = f.label end
    end
    if #on > 0 then parts[#parts + 1] = string.format(g.label, table.concat(on, ", ")) end
  end
  if #parts == 0 then return nil end
  return table.concat(parts, ", ")
end
