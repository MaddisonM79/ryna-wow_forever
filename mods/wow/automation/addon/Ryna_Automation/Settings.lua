-- Settings: Automation's toggles, each its own key in RynaDB.settings (the core keeps one flat table
-- for the core and every mod, so these all start with "auto"). Every feature is off by default:
-- turning the mod on in the companion app does nothing until a feature here is ticked too, and
-- `autoReport` (a chat line for what the others did) is off like the rest: nothing in chat unasked.
--
-- Each feature has the short name `/ryna auto <name> on|off` takes (the brief's words), its setting,
-- its group and, for the ones that only matter under another, that one (`under`). `changed(on)`, where
-- there is one, runs when the player ticks or unticks it (the tab or /ryna auto): auto-loot sets the
-- game's own option then.
--
-- The trainer's gold cap is `autoTrainCap` in the same table, in copper (10 gold until it's set with
-- /ryna auto traincap): the core's defaults hold booleans only, so it's read here with its default.

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
  { name = "sellJunk", key = "autoSellJunk", group = "vendor", label = L["marked junk"] },
  { name = "repair", key = "autoRepair", group = "vendor", label = L["repair"] },
  { name = "repairGuild", key = "autoRepairGuild", group = "vendor", under = "autoRepair",
    label = L["guild bank first"] },
  { name = "trainAll", key = "autoTrainAll", group = "npcs", label = L["train"] },
  { name = "mail", key = "autoMail", group = "npcs", label = L["mail"] },
  { name = "acceptRes", key = "autoAcceptRes", group = "world", label = L["resurrections"] },
  { name = "acceptInvites", key = "autoAcceptInvites", group = "world", label = L["invites"] },
  { name = "confirmDialogs", key = "autoConfirmDialogs", group = "world", label = L["dialogs"] },
  { name = "skipCinematics", key = "autoSkipCinematics", group = "world", label = L["cinematics"] },
  { name = "trackQuests", key = "autoTrackQuests", group = "world", label = L["track quests"] },
  { name = "releaseInBG", key = "autoReleaseInBG", group = "world", label = L["release in battlegrounds"] },
  { name = "lootOn", key = "autoLootOn", group = "world", label = L["auto-loot"],
    changed = function(on) return ns.Loot and ns.Loot.SetAutoLoot(on, true) end },
  { name = "roll", key = "autoRoll", group = "world", label = L["loot rolls"] },
  { name = "report", key = "autoReport", group = "chat", label = L["chat lines"] },
}

local defaults = {}
for _, f in ipairs(S.FEATURES) do defaults[f.key] = false end
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

-- Ticks or unticks a feature, and runs its `changed` (the tab and /ryna auto both come here).
function S.Changed(key, on)
  for _, f in ipairs(S.FEATURES) do
    if f.key == key and f.changed then ns.Run("automation " .. f.name, f.changed, on and true or false) end
  end
end

-- The trainer's gold cap, in copper: 10 gold until it's set; 0 means train nothing.
S.TRAIN_CAP_DEFAULT = 100000
function S.TrainCap()
  local all = ns.API.Settings.All and ns.API.Settings.All()
  local v = type(all) == "table" and tonumber(all.autoTrainCap) or nil
  if not v or v < 0 then return S.TRAIN_CAP_DEFAULT end
  return math.floor(v)
end

function S.SetTrainCap(copper)
  ns.API.Settings.Set("autoTrainCap", math.max(0, math.floor(copper)))
end

-- "quests (accept, turn in), vendor (greys)": what's on, by group; nil when nothing acts.
function S.Summary()
  local groups = { { id = "quests", label = L["quests (%s)"] }, { id = "vendor", label = L["vendor (%s)"] },
    { id = "npcs", label = L["trainers and mail (%s)"] }, { id = "world", label = L["world (%s)"] } }
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
