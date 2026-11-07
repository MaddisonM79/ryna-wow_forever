-- Quests: accepting and turning in quests, and skipping the gossip of an NPC who offers nothing but
-- quests. Each step is its own toggle (Settings.lua), all off by default.
--
-- The guards, for every event here:
--   - Shift held when any of the NPC's frames opens pauses the whole conversation: nothing automatic
--     happens until it ends, even after Shift is let go (Questie's rule; AutoQuests checks only the
--     moment). Combat pauses it the same way.
--   - Each action runs at most once per conversation (`once`, keyed on the action and its quest), so
--     an NPC who shows the same frame twice, or a frame that bounces back, can't loop. A conversation
--     ends when every gossip and quest frame has been closed for half a second (GOSSIP_CLOSED and
--     QUEST_FINISHED also fire between one quest and the next with the same NPC, so they alone can't
--     be the end).
--   - The action runs a frame after the event, and only if the game's own frame for it (QuestFrame,
--     GossipFrame) is still shown and still on the same quest: another addon that handled it first
--     (AutoQuests, Questie's auto-accept) closes or moves it, and then this does nothing. With a
--     replacement quest UI that hides QuestFrame (Immersion), nothing here acts at all.
--
-- The 12.x engine Forever runs on: C_GossipInfo takes quest ids (SelectAvailableQuest(questID)), a
-- gossip option's `flags` has bit 1 set when the option is a quest's ("(Quest)"; AutoQuests relies on
-- the same bit), and every client function is looked up and called under pcall (ns.G, ns.C), since
-- this client's surface differs from retail's.

local _, ns = ...
if ns.off then return end
local L = ns.L
local S = ns.Settings

-- This conversation: the actions done (by key), whether it's paused, and what the gossip told us
-- about quests the detail frame can't (trivial, by quest id).
local done, paused, trivialById = {}, false, {}
-- What has been said once this conversation (the reward-price line).
local said = {}

local function once(key)
  if done[key] then return false end
  done[key] = true
  return true
end

local function sayOnce(key, msg)
  if said[key] then return end
  said[key] = true
  ns.Report(msg)
end

-- The quest the quest frame is on, or nil (GetQuestID is 0 when the frame has none).
local function questID()
  local id = ns.G("GetQuestID")
  if type(id) ~= "number" or ns.IsSecret(id) or id <= 0 then return nil end
  return id
end

-- Every frame a conversation with a quest giver uses.
local FRAMES = { "GossipFrame", "QuestFrame" }

local function anyFrameShown()
  for _, name in ipairs(FRAMES) do
    local f = _G[name]
    if type(f) == "table" and type(f.IsShown) == "function" then
      local ok, shown = pcall(f.IsShown, f)
      if ok and shown then return true end
    end
  end
  return false
end

local function endOfConversation()
  ns.After(0.5, "automation conversation", function()
    if anyFrameShown() then return end
    done, paused, trivialById, said = {}, false, {}, {}
  end)
end

-- Shift or combat at any frame of the conversation pauses the rest of it, whichever features are on.
-- These run before the features' handlers (they are registered first).
for _, event in ipairs({ "GOSSIP_SHOW", "QUEST_GREETING", "QUEST_DETAIL", "QUEST_PROGRESS", "QUEST_COMPLETE" }) do
  ns.On(event, function()
    if paused then return end
    local held, why = ns.Held()
    if held then
      paused = true
      ns.Debug("automation: paused for this conversation (%s)", why)
    end
  end)
end

local function stop()
  return paused
end

-- Runs fn a frame from now, if the conversation isn't paused by then, `frame` is still shown and,
-- for a quest frame (`sameQuest`), it's still on the quest the event was for.
local function soon(label, frame, fn, sameQuest)
  local at = sameQuest and questID() or nil
  ns.After(0, "automation " .. label, function()
    if paused or ns.Held() then return end
    if not ns.Shown(frame) or (sameQuest and questID() ~= at) then
      ns.Debug("automation: %s: the %s closed or moved on first (another addon?), leaving it", label, frame)
      return
    end
    fn()
  end)
end

-- Low level ----------------------------------------------------------------------------------------

-- Whether a quest is trivial (grey) for this character: the quest log's word (C_QuestLog.IsQuestTrivial),
-- else what the gossip list said about it. nil when neither can say; then it's treated as not trivial,
-- since the alternative is never accepting anything on a client without either.
local function isTrivial(id)
  local t = ns.C("C_QuestLog", "IsQuestTrivial", id)
  if type(t) == "boolean" then return t end
  return trivialById[id]
end

-- Escorts -------------------------------------------------------------------------------------------

-- What could be detected: nothing in this client's API marks a quest as an escort. QUEST_ACCEPT_CONFIRM
-- (an escort someone else started, asking to join) is the one real signal, and nothing here answers
-- it. For a quest offered by its NPC, the quest tags (C_QuestLog.GetQuestTagInfo) have no escort tag,
-- so this goes by words: the title or the objective text naming an escort, per client language. That
-- misses escorts whose text doesn't say so ("Protect ...", "Help X to safety") and may catch a quest
-- that only mentions one; a missed one is accepted as any quest is, a caught one is left to the player.
local ESCORT_WORDS = {
  enUS = { "escort" }, enGB = { "escort" },
  deDE = { "eskort", "geleit" },
  frFR = { "escort" },
  esES = { "escolt" }, esMX = { "escolt" },
  itIT = { "scort" },
  ptBR = { "escolt" },
  ruRU = { "сопровод", "Сопровод", "проводи", "Проводи" },
  koKR = { "호위" },
  zhCN = { "护送" }, zhTW = { "護送" },
}

local function looksLikeEscort()
  local words = ESCORT_WORDS[ns.Locale()] or ESCORT_WORDS.enUS
  local text = table.concat({
    ns.NonEmpty(ns.G("GetTitleText")) or "",
    ns.NonEmpty(ns.G("GetObjectiveText")) or "",
  }, "\n"):lower()
  for _, w in ipairs(words) do
    if text:find(w:lower(), 1, true) or text:find(w, 1, true) then return true end
  end
  return false
end

-- A quest a player shared (the "questnpc" is a player), rather than one an NPC, an item or a place
-- offers. Those are left to the player. A GUID the client keeps secret counts as a player's, since
-- it can't be checked.
local function sharedByPlayer()
  local guid = ns.G("UnitGUID", "questnpc")
  if guid == nil then return false end
  if ns.IsSecret(guid) then return true end
  if type(guid) ~= "string" or not guid:match("^Player%-") then return false end
  -- A quest an item starts names the player themself.
  return not ns.G("UnitIsUnit", "questnpc", "player")
end

local function title()
  return ns.NonEmpty(ns.G("GetTitleText")) or L["this quest"]
end

-- Accepting -----------------------------------------------------------------------------------------

ns.On("QUEST_DETAIL", function()
  if not S.On("autoAcceptQuests") or stop() then return end
  soon("accept", "QuestFrame", function()
    local id = questID()
    if not id or not once("accept:" .. id) then return end
    -- A quest the game accepted by itself (an auto-accept popup): only the acknowledgement is left.
    if ns.G("QuestGetAutoAccept") then
      ns.G("AcknowledgeAutoAcceptQuest")
      ns.Debug("automation: acknowledged auto-accepted quest %s", id)
      return
    end
    if sharedByPlayer() then
      ns.Debug("automation: quest %s was shared by a player, leaving it", id)
      return
    end
    if S.On("autoSkipLowLevel") and isTrivial(id) then
      ns.Debug("automation: quest %s is low level, leaving it", id)
      return
    end
    if looksLikeEscort() then
      ns.Report(string.format(L["%s looks like an escort, so it's yours to accept."], title()))
      return
    end
    ns.Debug("automation: accepting quest %s%s", id,
      ns.G("QuestIsFromAreaTrigger") and " (offered by a place)" or "")
    ns.G("AcceptQuest")
  end, true)
end)

-- Turning in ----------------------------------------------------------------------------------------

ns.On("QUEST_PROGRESS", function()
  if not S.On("autoTurnInQuests") or stop() then return end
  soon("turn in", "QuestFrame", function()
    local id = questID()
    if not id or not ns.G("IsQuestCompletable") or not once("progress:" .. id) then return end
    -- A quest that takes money to hand in is the player's call.
    local cost = ns.G("GetQuestMoneyToGet")
    if type(cost) == "number" and cost > 0 then
      ns.Report(string.format(L["%s costs %s to hand in, so that's yours to do."], title(), ns.MoneyText(cost)))
      return
    end
    ns.Debug("automation: completing quest %s", id)
    ns.G("CompleteQuest")
  end, true)
end)

-- The choice with the highest vendor price (sell price times how many), or nil and why not.
local function bestReward(n)
  local best, bestValue, bestLink = nil, -1, nil
  local itemInfo = ns.Fn("C_Item", "GetItemInfo") or ns.Fn("GetItemInfo")
  for i = 1, n do
    local link = ns.G("GetQuestItemLink", "choice", i)
    local _, _, count, _, _, itemID = ns.G("GetQuestItemInfo", "choice", i)
    local price = link and select(11, ns.Try(itemInfo, link))
    if type(price) ~= "number" then
      -- Not cached yet: ask for it, so it's known next time.
      if type(itemID) == "number" then ns.C("C_Item", "RequestLoadItemDataByID", itemID) end
      return nil, "unknown"
    end
    local value = price * math.max(tonumber(count) or 1, 1)
    if value > bestValue then best, bestValue, bestLink = i, value, link end
  end
  if not best or bestValue <= 0 then return nil, "worthless" end
  return best, nil, bestLink, bestValue
end

ns.On("QUEST_COMPLETE", function()
  if not S.On("autoTurnInQuests") or stop() then return end
  soon("reward", "QuestFrame", function()
    local id = questID()
    if not id or not once("complete:" .. id) then return end
    local n = tonumber(ns.G("GetNumQuestChoices")) or 0
    if n <= 1 then
      -- As the game's own button does: the one choice is 1, no choice is 0.
      ns.Debug("automation: taking the reward for quest %s", id)
      ns.G("GetQuestReward", n == 1 and 1 or 0)
      return
    end
    -- A choice of rewards stops here, unless the player asked for the best-selling one.
    if not S.On("autoPickReward") then return end
    local pick, why, link, value = bestReward(n)
    if not pick then
      if why == "unknown" then
        sayOnce("prices", L["The rewards' prices aren't known yet, so this choice is yours."])
      else
        sayOnce("prices", L["None of the rewards sells for anything, so this choice is yours."])
      end
      return
    end
    ns.Report(string.format(L["Took %s, the reward that sells for the most (%s)."], tostring(link),
      ns.MoneyText(value)))
    ns.G("GetQuestReward", pick)
  end, true)
end)

-- Quest-only gossip ---------------------------------------------------------------------------------

local function isQuestOption(option)
  local flags = type(option) == "table" and tonumber(option.flags) or 0
  return flags % 2 == 1
end

local function list(v)
  return type(v) == "table" and v or {}
end

ns.On("GOSSIP_SHOW", function()
  if not S.On("autoSkipGossip") or stop() then return end
  soon("gossip", "GossipFrame", function()
    local options = list(ns.C("C_GossipInfo", "GetOptions"))
    local available = list(ns.C("C_GossipInfo", "GetAvailableQuests"))
    local active = list(ns.C("C_GossipInfo", "GetActiveQuests"))
    for _, q in ipairs(available) do
      if type(q) == "table" and type(q.questID) == "number" then
        trivialById[q.questID] = q.isTrivial and true or false
      end
    end
    -- Nothing but quests: a finished one to hand in first, then one to pick up.
    if #options == 0 and #available + #active > 0 then
      for _, q in ipairs(active) do
        if type(q) == "table" and q.isComplete and type(q.questID) == "number"
          and once("gossip:active:" .. q.questID) then
          ns.Debug("automation: gossip: handing in quest %s", q.questID)
          return ns.C("C_GossipInfo", "SelectActiveQuest", q.questID)
        end
      end
      local skipLow = S.On("autoSkipLowLevel")
      for _, q in ipairs(available) do
        if type(q) == "table" and type(q.questID) == "number" and not (skipLow and q.isTrivial)
          and once("gossip:available:" .. q.questID) then
          ns.Debug("automation: gossip: opening quest %s", q.questID)
          return ns.C("C_GossipInfo", "SelectAvailableQuest", q.questID)
        end
      end
      return
    end
    -- One option and nothing else, and it's a quest's. A lone option that isn't a quest's is left
    -- alone: plain gossip can start an event, a fight or a teleport, and vendors, trainers, flight
    -- masters, bankers, innkeepers, battlemasters and stable masters are never touched.
    local only = options[1]
    if #options == 1 and #available + #active == 0 and isQuestOption(only)
      and type(only.gossipOptionID) == "number" and once("gossip:option:" .. only.gossipOptionID) then
      ns.Debug("automation: gossip: the one option is a quest's (%s)", only.gossipOptionID)
      ns.C("C_GossipInfo", "SelectOption", only.gossipOptionID)
    end
  end)
end)

-- The older quest greeting (an NPC with only quests, no gossip): the same order.
ns.On("QUEST_GREETING", function()
  if not S.On("autoSkipGossip") or stop() then return end
  soon("greeting", "QuestFrame", function()
    for i = 1, tonumber(ns.G("GetNumActiveQuests")) or 0 do
      local name, isComplete = ns.G("GetActiveTitle", i)
      if isComplete and once("greeting:active:" .. i .. ":" .. tostring(name)) then
        ns.Debug("automation: greeting: handing in %s", tostring(name))
        return ns.G("SelectActiveQuest", i)
      end
    end
    for i = 1, tonumber(ns.G("GetNumAvailableQuests")) or 0 do
      local skip = S.On("autoSkipLowLevel") and ns.G("IsAvailableQuestTrivial", i)
      local name = ns.G("GetAvailableTitle", i)
      if not skip and once("greeting:available:" .. i .. ":" .. tostring(name)) then
        ns.Debug("automation: greeting: opening %s", tostring(name))
        return ns.G("SelectAvailableQuest", i)
      end
    end
  end)
end)

ns.On("GOSSIP_CLOSED", endOfConversation)
ns.On("QUEST_FINISHED", endOfConversation)
