-- Census: the answers to the player's own /who. Ryna never sends /who itself: no timer, no button,
-- no key binding (the automated sweep it once had is gone). When the player types /who, or asks
-- from the game's own Who window, the client fires WHO_LIST_UPDATE with the answer, and this file
-- reads that answer and records it, the same way it recorded a sweep's answers: one `who` journal
-- event { query, count, batch } and a `seen` event per row (source `who`, the same batch id).
--
-- Reading only. This file calls no /who function that asks the server anything (no SendWho), and
-- doesn't touch the game's who-to-window setting (SetWhoToUi) or the Who window: it reads the
-- list the client already holds (C_FriendList.GetNumWhoResults and GetWhoInfo, or the old
-- globals). The query text comes from a post-hook on SendWho (hooksecurefunc), which runs after
-- the client sent the player's query and only notes the words; it never sends anything. An
-- answer the game prints into chat instead of the Who window (a short one, with the who-to-window
-- setting off) may come without WHO_LIST_UPDATE on some clients, and then nothing is recorded. A
-- /who another addon sends is answered the same way, so it is read too.
--
-- Privacy: nothing is sent at all. A /who row has no GUID, so the shared 10-minute window keys it
-- by name (Sightings.lua), and a name met twice in one answer is one sighting. Sorting the Who
-- window fires WHO_LIST_UPDATE again with the same rows: they fall in that window, and no second
-- `who` event is written for an answer already recorded.

local _, ns = ...
if ns.off then return end

local C = {}
ns.Census = C

local NonEmpty = ns.NonEmpty

local CAP = 50 -- rows the server returns at most per query

-- Localized class and race names <-> English tokens, from the client where it has them.
local classToken, raceToken
local function buildNames()
  if classToken then return end
  classToken, raceToken = {}, {}
  for _, list in ipairs({ _G.LOCALIZED_CLASS_NAMES_MALE, _G.LOCALIZED_CLASS_NAMES_FEMALE }) do
    if type(list) == "table" then
      for token, name in pairs(list) do
        if type(name) == "string" then classToken[name] = token end
      end
    end
  end
  local classInfo = ns.Fn("C_CreatureInfo", "GetClassInfo")
  local raceInfo = ns.Fn("C_CreatureInfo", "GetRaceInfo")
  for id = 1, 40 do
    local ci = classInfo and ns.Try(classInfo, id)
    if type(ci) == "table" and NonEmpty(ci.className) and NonEmpty(ci.classFile) then
      classToken[ci.className] = ci.classFile
    end
    local ri = raceInfo and ns.Try(raceInfo, id)
    if type(ri) == "table" and NonEmpty(ri.raceName) and NonEmpty(ri.clientFileString) then
      raceToken[ri.raceName] = ri.clientFileString
    end
  end
end

-- The English class token for a row: the client's file name when it gave one, else a lookup of the
-- localized name, else the localized name upper-cased (English clients give the token that way).
local function rowClass(fileName, localized)
  local token = NonEmpty(fileName)
  if token then return token:upper() end
  localized = NonEmpty(localized)
  if not localized then return nil end
  buildNames()
  return classToken[localized] or localized:upper()
end

-- The race as the other sources record it (the client file string, e.g. "Scourge") where the
-- client can map the localized name, else the localized name as given.
local function rowRace(localized)
  localized = NonEmpty(localized)
  if not localized then return nil end
  buildNames()
  return raceToken[localized] or localized
end

-- The who list's readers: C_FriendList first, the old globals as the fallback. Read only.
local function api()
  local F = {}
  F.num = ns.Fn("C_FriendList", "GetNumWhoResults")
  F.info = ns.Fn("C_FriendList", "GetWhoInfo")
  F.modern = F.num and F.info and true or false
  if not F.modern then
    F.num = ns.Fn("GetNumWhoResults")
    F.info = ns.Fn("GetWhoInfo")
  end
  return F
end

-- One row of the current who list as a sighting's fields, or nil.
local function readRow(F, i)
  if F.modern then
    local r = ns.Try(F.info, i)
    if type(r) ~= "table" then return nil end
    return {
      fullName = NonEmpty(r.fullName), level = r.level, class = rowClass(r.filename, r.classStr),
      race = rowRace(r.raceStr), guild = NonEmpty(r.fullGuildName), zone = NonEmpty(r.area),
    }
  end
  local name, guild, level, race, class, zone, classFileName = ns.Try(F.info, i)
  if not NonEmpty(name) then return nil end
  return {
    fullName = NonEmpty(name), level = level, class = rowClass(classFileName, class),
    race = rowRace(race), guild = NonEmpty(guild), zone = NonEmpty(zone),
  }
end

local function readResults()
  local F = api()
  if not (F.num and F.info) then return nil end
  local n, total = ns.Try(F.num)
  if type(n) ~= "number" or ns.IsSecret(n) then return nil end
  if type(total) ~= "number" or ns.IsSecret(total) then total = n end
  local rows = {}
  for i = 1, math.min(n, CAP) do
    local row = readRow(F, i)
    if row then
      if type(row.level) ~= "number" or ns.IsSecret(row.level) or row.level <= 0 then row.level = nil end
      rows[#rows + 1] = row
    end
  end
  return rows, n, total
end

-- The player's last query, noted by the SendWho post-hook: { text, answered }. A new query is a
-- new answer (and a new `who` event); WHO_LIST_UPDATE without one (a sort, a redraw) is the same
-- answer again.
local pending
local count = 0

local function noteQuery(text)
  if type(text) ~= "string" or ns.IsSecret(text) then text = nil end
  pending = { text = NonEmpty(text), answered = false }
end

-- Post-hooks only: hooksecurefunc runs after the original, with its arguments, and can't change
-- or repeat the call. The modern function and the old global, whichever the client has.
local function hookSendWho()
  local hook = _G.hooksecurefunc
  if type(hook) ~= "function" then return end
  if type(_G.C_FriendList) == "table" and type(_G.C_FriendList.SendWho) == "function" then
    pcall(hook, _G.C_FriendList, "SendWho", noteQuery)
  end
  if type(_G.SendWho) == "function" then pcall(hook, "SendWho", noteQuery) end
end
hookSendWho()

-- Records the who list the client holds now: a `who` event for a new answer, and a sighting per
-- player in it not already seen in the window.
function C.Record()
  if not ns.db or not ns.rec then return end
  if not ns.Setting("census") then
    ns.Debug("census off: not recording a /who answer")
    return
  end
  local rows, n, total = readResults()
  if not rows then
    ns.Debug("census: a /who answer came, but the who list couldn't be read")
    return
  end
  local fresh = not (pending and pending.answered)
  if fresh then
    count = count + 1
    C.batch = string.format("%d-%d", ns.Now(), count)
    ns.Journal.Add("who", { query = pending and pending.text or nil, count = n, batch = C.batch }, true)
  end
  if pending then pending.answered = true end
  local names, recorded, skipped, repeated = {}, 0, 0, 0
  for _, row in ipairs(rows) do
    local name = ns.CanonicalName(row.fullName)
    if name and not ns.Sightings.IsSelf(name) then
      if names[name] then
        repeated = repeated + 1
      else
        names[name] = true
        local earlier = ns.Sightings.RecentByName(name)
        if earlier and (earlier.level or not row.level) then
          skipped = skipped + 1
        else
          row.fullName, row.batch = name, C.batch
          if ns.Sightings.Seen("who", row, true) then recorded = recorded + 1 end
        end
      end
    end
  end
  ns.Debug("census: your /who %q answered %d (of %d), %d recorded, %d already seen in the last 10 minutes, "
    .. "%d twice in the answer%s", tostring(pending and pending.text), n, total, recorded, skipped, repeated,
    fresh and "" or " (the same answer again)")
end

ns.On("WHO_LIST_UPDATE", function() C.Record() end)
