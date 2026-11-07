-- World: tracking a quest as it's accepted, and releasing the spirit in a battleground. Two toggles
-- (Settings.lua), both off by default.
--
-- autoTrackQuests adds a newly accepted quest to the tracker (C_QuestLog.AddQuestWatch, by quest ID
-- as Forever's quest log does; else the older AddQuestWatch by log index). Forever's QUEST_ACCEPTED
-- carries the quest ID alone, older clients the log index and then the ID; both are read. A full
-- tracker is left as it is, and said once until it has room again.
--
-- autoReleaseInBG releases the spirit on death in a battleground only (IsInInstance's "pvp", or
-- C_PvP.IsBattleground): never in a dungeon, raid, arena or the open world, and never when a soulstone
-- or another way to come back on the spot is on offer. Shift held at death leaves it to the player; a
-- death the game still counts as in combat is released when combat ends, if the player is still dead.

local _, ns = ...
if ns.off then return end
local L = ns.L
local S = ns.Settings

local World = {}
ns.World = World

-- Quest tracking ----------------------------------------------------------------------------------

-- The quest ID QUEST_ACCEPTED is about: (questID) on Forever, (questLogIndex, questID) before.
function World.AcceptedID(a, b)
  if type(b) == "number" and b > 0 then return b end
  if type(a) ~= "number" or a <= 0 then return nil end
  -- One number: a quest ID, unless the log has no quest by that ID and a quest at that index.
  if ns.Fn("C_QuestLog", "GetLogIndexForQuestID") and not ns.C("C_QuestLog", "GetLogIndexForQuestID", a) then
    local info = ns.C("C_QuestLog", "GetInfo", a)
    if type(info) == "table" and type(info.questID) == "number" and info.questID > 0 then return info.questID end
  end
  return a
end

local function watchLimit()
  local max = ns.C("C_QuestLog", "GetMaxNumQuestWatches") or ns.G("GetMaxNumQuestsCanWatch")
    or _G.MAX_WATCHABLE_QUESTS
  if type(max) ~= "number" and type(_G.Constants) == "table" and type(_G.Constants.QuestWatchConsts) == "table" then
    max = _G.Constants.QuestWatchConsts.MAX_QUEST_WATCHES
  end
  return tonumber(max)
end

local function watchCount()
  return tonumber(ns.C("C_QuestLog", "GetNumQuestWatches") or ns.G("GetNumQuestWatches"))
end

local saidFull = false

function World.Track(id)
  if ns.C("C_QuestLog", "GetQuestWatchType", id) ~= nil then return end
  local max, count = watchLimit(), watchCount()
  if max and count and count >= max then
    if not saidFull then
      saidFull = true
      ns.Report(L["The quest tracker is full, so new quests aren't tracked until you untrack one."])
    end
    return
  end
  saidFull = false
  if ns.Fn("C_QuestLog", "AddQuestWatch") then
    ns.C("C_QuestLog", "AddQuestWatch", id)
  else
    local index = ns.C("C_QuestLog", "GetLogIndexForQuestID", id) or ns.G("GetQuestLogIndexByID", id)
    if not (ns.Fn("AddQuestWatch") and type(index) == "number" and index > 0) then
      return ns.Missing("trackQuests", "C_QuestLog.AddQuestWatch")
    end
    ns.G("AddQuestWatch", index)
  end
  ns.Debug("automation: tracking quest %s", id)
end

ns.On("QUEST_ACCEPTED", function(_, a, b)
  if not S.On("autoTrackQuests") then return end
  local id = World.AcceptedID(a, b)
  if not id then return end
  ns.Soon("track:" .. id, nil, function() World.Track(id) end)
end)

-- Releasing in battlegrounds -----------------------------------------------------------------------

function World.InBattleground()
  local inInstance, kind = ns.G("IsInInstance")
  if kind == "pvp" then return true end
  if kind == "arena" then return false end
  if ns.Fn("C_PvP", "IsBattleground") then return ns.C("C_PvP", "IsBattleground") and true or false end
  if inInstance == nil and kind == nil then ns.Missing("releaseInBG", "IsInInstance") end
  return false
end

-- Whether the player could come back on the spot: a soulstone, or any of the game's own options.
local function canComeBack()
  if ns.G("HasSoulstone") then return true end
  local options = ns.C("C_DeathInfo", "GetSelfResurrectOptions")
  return type(options) == "table" and #options > 0
end

local afterCombat = false

local function release()
  afterCombat = false
  if not S.On("autoReleaseInBG") or not World.InBattleground() or canComeBack() then return end
  if not ns.Need("releaseInBG", "RepopMe") then return end
  ns.Soon("release", function()
    return ns.G("UnitIsDead", "player") ~= false and not ns.G("UnitIsGhost", "player") and ns.PopupShown("DEATH")
  end, function()
    ns.G("RepopMe")
    ns.Report(L["Released your spirit (battleground)."])
  end)
end

ns.On("PLAYER_DEAD", function()
  if not S.On("autoReleaseInBG") then return end
  if ns.G("IsShiftKeyDown") then return ns.Debug("automation: release: left to you (shift)") end
  if ns.G("InCombatLockdown") then
    afterCombat = true
    return
  end
  release()
end)

ns.On("PLAYER_REGEN_ENABLED", function()
  if afterCombat and not ns.G("IsShiftKeyDown") then release() end
end)
