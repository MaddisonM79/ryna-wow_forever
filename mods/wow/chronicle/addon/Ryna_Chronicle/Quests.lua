-- Quests: the active log (with objectives) and the completed set on the record, and
-- `questAccept` / `questDone` journal events. Modern C_QuestLog first, the classic API after.
-- Collapsed headers in the classic log hide their quests; the addon doesn't expand them.

local _, ns = ...
if ns.off then return end

local Q = {}
ns.Quests = Q

local NonEmpty = ns.NonEmpty

local function modernLog()
  local QL = _G.C_QuestLog
  if type(QL) == "table" and QL.GetNumQuestLogEntries and QL.GetInfo then return QL end
  return nil
end

local function objectivesFor(questId, logIndex)
  local list = {}
  local QL = _G.C_QuestLog
  if type(QL) == "table" and QL.GetQuestObjectives and questId then
    local objectives = ns.Try(QL.GetQuestObjectives, questId)
    if type(objectives) == "table" and #objectives > 0 then
      for _, o in ipairs(objectives) do
        if type(o) == "table" then
          list[#list + 1] = { text = NonEmpty(o.text), done = o.finished and true or false }
        end
      end
      return list
    end
  end
  local count = logIndex and ns.Try(ns.Fn("GetNumQuestLeaderBoards"), logIndex)
  local board = ns.Fn("GetQuestLogLeaderBoard")
  if type(count) == "number" and board then
    for j = 1, count do
      local text, _, finished = ns.Try(board, j, logIndex)
      list[#list + 1] = { text = NonEmpty(text), done = finished and true or false }
    end
  end
  return list
end

-- Every quest in the log: { id, title, level, logIndex }.
local function readLog()
  local quests = {}
  local QL = modernLog()
  if QL then
    local n = ns.Try(QL.GetNumQuestLogEntries)
    for i = 1, (type(n) == "number" and n or 0) do
      local info = ns.Try(QL.GetInfo, i)
      if type(info) == "table" and not info.isHeader and info.questID and info.questID ~= 0 then
        quests[#quests + 1] = { id = info.questID, title = NonEmpty(info.title), level = info.level, logIndex = i }
      end
    end
    return quests
  end
  local n = ns.Try(ns.Fn("GetNumQuestLogEntries"))
  local title = ns.Fn("GetQuestLogTitle")
  for i = 1, (type(n) == "number" and title and n or 0) do
    local name, level, _, isHeader, _, _, _, questId = ns.Try(title, i)
    if name and not isHeader and questId and questId ~= 0 then
      quests[#quests + 1] = { id = questId, title = NonEmpty(name), level = level, logIndex = i }
    end
  end
  return quests
end

local function acceptedMap()
  if Q.acceptedAt then return Q.acceptedAt end
  Q.acceptedAt = {}
  local rec = ns.rec
  for _, q in ipairs(rec and rec.quests.active or {}) do
    if type(q) == "table" and q.id and q.acceptedAt then Q.acceptedAt[q.id] = q.acceptedAt end
  end
  return Q.acceptedAt
end

function Q.ScanLog()
  local rec = ns.rec
  if not rec then return end
  local accepted = acceptedMap()
  local active = {}
  for _, q in ipairs(readLog()) do
    active[#active + 1] = {
      id = q.id, title = q.title, level = q.level, acceptedAt = accepted[q.id],
      objectives = objectivesFor(q.id, q.logIndex),
    }
  end
  rec.quests.active = active
  -- QUEST_LOG_UPDATE comes with every objective tick, so an unchanged line isn't repeated.
  ns.DebugChanged("quest-log", "quests: log read, %d in the log, %d completed", #active, #rec.quests.completed)
  if Q.completedDirty then
    Q.completedDirty = false
    Q.RefreshCompleted()
  end
end

local function setCompleted(set)
  local list = {}
  for id in pairs(set) do
    if type(id) == "number" then list[#list + 1] = id end
  end
  table.sort(list)
  ns.rec.quests.completed = list
end

function Q.RefreshCompleted()
  if not ns.rec then return end
  local all = ns.Fn("C_QuestLog", "GetAllCompletedQuestIDs")
  if all then
    local ids = ns.Try(all)
    if type(ids) == "table" then
      local set = {}
      for _, id in ipairs(ids) do set[id] = true end
      setCompleted(set)
      ns.Debug("quests: completed set read, %d quests", #ns.rec.quests.completed)
      return
    end
  end
  local legacy = ns.Fn("GetQuestsCompleted")
  if legacy then
    local passed = {}
    local got = ns.Try(legacy, passed)
    local set = type(got) == "table" and got or passed
    if next(set) then
      setCompleted(set)
      ns.Debug("quests: completed set read (classic API), %d quests", #ns.rec.quests.completed)
      return
    end
    -- Old clients answer only after QueryQuestsCompleted -> QUEST_QUERY_COMPLETE.
    local query = ns.Fn("QueryQuestsCompleted")
    if query and not Q.queried then
      Q.queried = true
      pcall(query)
      ns.Debug("quests: asked the server for the completed set")
    end
  end
end

local function find(questId)
  for _, q in ipairs(readLog()) do
    if q.id == questId then return q end
  end
  return nil
end

-- Modern clients send (questID); classic ones send (questLogIndex, questID).
function Q.OnAccepted(a, b)
  local rec = ns.rec
  if not rec then return end
  local questId = tonumber(b or a)
  if not questId then return end
  acceptedMap()[questId] = ns.Now()
  local q = find(questId)
  local title = (q and q.title) or NonEmpty(ns.Try(ns.Fn("C_QuestLog", "GetTitleForQuestID"), questId))
  ns.Journal.Add("questAccept", { questId = questId, title = title, level = q and q.level, zone = rec.zone })
  ns.Debounce("quests", 1, Q.ScanLog)
end

function Q.OnTurnedIn(questId, xp, money)
  local rec = ns.rec
  questId = tonumber(questId)
  if not rec or not questId then return end
  ns.Journal.Add("questDone", { questId = questId, xp = xp, gold = money, zone = rec.zone })
  local done = rec.quests.completed
  local present = false
  for _, id in ipairs(done) do
    if id == questId then present = true break end
  end
  if not present then
    done[#done + 1] = questId
    table.sort(done)
  end
  acceptedMap()[questId] = nil
  Q.completedDirty = true
  ns.Debounce("quests", 1, Q.ScanLog)
end

ns.On("QUEST_ACCEPTED", function(_, a, b) Q.OnAccepted(a, b) end)
ns.On("QUEST_TURNED_IN", function(_, questId, xp, money) Q.OnTurnedIn(questId, xp, money) end)
ns.On("QUEST_LOG_UPDATE", function() ns.Debounce("quests", 1, Q.ScanLog) end)
ns.On("QUEST_QUERY_COMPLETE", function() Q.RefreshCompleted() end)
