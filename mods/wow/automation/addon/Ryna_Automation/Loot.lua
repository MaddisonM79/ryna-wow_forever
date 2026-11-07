-- Loot: group loot rolls, and the game's own auto-loot option. Two toggles (Settings.lua), both off
-- by default.
--
-- autoRoll answers every group loot roll and never with Need: a bind-on-pickup item the character
-- can't Need on is passed, anything else gets Greed when Greed is offered, else a pass. One roll per
-- roll ID, a frame after START_LOOT_ROLL and only while the roll is still open. Shift held when the
-- roll starts leaves it to the player; a roll that starts in combat waits for the end of combat (it
-- is rolled then if it's still open and Shift isn't held). It affects the group: a pass or a greed
-- is the player's word to everyone rolling.
--
-- autoLootOn sets the game's "Auto Loot" option (the autoLootDefault CVar) to on at every login and
-- when it's ticked; unticking it turns the option off once, then leaves it to the game's settings.

local _, ns = ...
if ns.off then return end
local L = ns.L
local S = ns.Settings

local Loot = {}
ns.Loot = Loot

-- Rolls ------------------------------------------------------------------------------------------

-- RollOnLoot's types: 0 pass, 2 greed (1, Need, is never sent).
local PASS, GREED = 0, 2
local rolled, waiting = {}, {}

local function open(rollID)
  local left = ns.Fn("GetLootRollTimeLeft")
  if not left then return true end
  local t = ns.Try(left, rollID)
  return type(t) ~= "number" or t > 0
end

-- What to roll: never Need. Returns the roll type and why (for the chat line), or nil.
function Loot.Choose(rollID)
  local _, name, _, _, bindOnPickUp, canNeed, canGreed = ns.G("GetLootRollItemInfo", rollID)
  if not name then return nil end
  if bindOnPickUp and not canNeed then return PASS, "bop" end
  if canGreed then return GREED, "greed" end
  return PASS, "nogreed"
end

local function roll(rollID)
  if rolled[rollID] or not open(rollID) then return end
  local choice, why = Loot.Choose(rollID)
  if not choice then
    ns.Debug("automation: roll %s: the item isn't known, leaving it", tostring(rollID))
    return
  end
  rolled[rollID] = true
  local link = ns.NonEmpty(ns.G("GetLootRollItemLink", rollID)) or select(2, ns.G("GetLootRollItemInfo", rollID))
  ns.G("RollOnLoot", rollID, choice)
  if why == "greed" then
    ns.Report(string.format(L["Greed on %s."], tostring(link)))
  elseif why == "bop" then
    ns.Report(string.format(L["Passed on %s (bind on pickup, and you can't need it)."], tostring(link)))
  else
    ns.Report(string.format(L["Passed on %s (no greed on it)."], tostring(link)))
  end
end

ns.On("START_LOOT_ROLL", function(_, rollID)
  if not S.On("autoRoll") or type(rollID) ~= "number" or rolled[rollID] then return end
  if not (ns.Need("roll", "RollOnLoot") and ns.Need("roll", "GetLootRollItemInfo")) then return end
  if ns.G("IsShiftKeyDown") then
    ns.Debug("automation: roll %s: left to you (shift)", rollID)
    return
  end
  if ns.InCombat() then
    waiting[rollID] = true
    ns.Debug("automation: roll %s: after combat", rollID)
    return
  end
  ns.Soon("roll:" .. rollID, function() return open(rollID) end, function() roll(rollID) end)
end)

ns.On("PLAYER_REGEN_ENABLED", function()
  if not next(waiting) then return end
  local ids = {}
  for id in pairs(waiting) do ids[#ids + 1] = id end
  waiting = {}
  table.sort(ids)
  if not S.On("autoRoll") then return end
  for _, id in ipairs(ids) do
    ns.Soon("roll:" .. id, function() return open(id) end, function() roll(id) end)
  end
end)

-- A roll that ended (someone won it, or it timed out) is forgotten.
ns.On("CANCEL_LOOT_ROLL", function(_, rollID)
  if type(rollID) == "number" then waiting[rollID] = nil end
end)

-- Auto-loot ---------------------------------------------------------------------------------------

local CVAR = "autoLootDefault"

-- The game's auto-loot option: true, false, or nil when this client can't say.
function Loot.AutoLoot()
  local v = ns.C("C_CVar", "GetCVar", CVAR)
  if v == nil then v = ns.G("GetCVar", CVAR) end
  if v == nil then return nil end
  return tostring(v) == "1"
end

-- on: turn the option on. Off, and only when the player just unticked it (`changed`): turn it off.
function Loot.SetAutoLoot(on, changed)
  if not on and not changed then return end
  local value = on and "1" or "0"
  local set = ns.Fn("C_CVar", "SetCVar") or ns.Fn("SetCVar")
  if not set then return ns.Missing("lootOn", "C_CVar.SetCVar") end
  if Loot.AutoLoot() == on then return end
  ns.Try(set, CVAR, value)
  ns.Debug("automation: auto-loot %s", on and "on" or "off")
  if changed then ns.Report(on and L["The game's auto-loot is on."] or L["The game's auto-loot is off."]) end
end
