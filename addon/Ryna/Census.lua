-- Census: the /who sweep. A sweep asks the server who is online with a list of generic queries
-- (level brackets, split by class and then by zone where an answer is capped) and records each
-- answer as one `who` journal event { query, count, batch } and a `seen` event per row (source
-- `who`, the same batch id). Runs from /ryna census, the panel's "Run census now" button, and,
-- with the censusSweep setting on, every 30 minutes while the player is parked in a city.
--
-- Privacy: the queries are level brackets, class names and zone names, never a person's name, and
-- nothing about the player is sent. A /who row has no GUID, so the shared 10-minute window keys it
-- by name (Sightings.lua), and a name met twice in one sweep is one sighting.
--
-- The server caps an answer at 50 rows and throttles /who, so one query goes out every 4 seconds,
-- 10 after an empty, errored or missing answer. Every client call is looked up and pcall'd; a
-- client that blocks the call from a timer stops the sweep for the session instead of retrying.

local _, ns = ...
local L, N = ns.L, ns.N

local C = {}
ns.Census = C

local NonEmpty = ns.NonEmpty

-- "412 players", in the client's language.
local function players(n)
  return ns.Plural(n, L["%d player"], L["%d players"])
end

local CAP = 50              -- rows the server returns at most per query
local GAP = 4               -- seconds between queries
local BACKOFF = 10          -- seconds after an empty or errored answer
local ANSWER_TIMEOUT = 8    -- seconds to wait for WHO_LIST_UPDATE before calling it errored
local MAX_QUERIES = 80      -- per sweep, whatever the splits ask for
local MAX_ZONE_SPLITS = 10  -- zone queries per capped bracket-and-class answer
local AUTO_EVERY = 1800     -- seconds between automatic sweeps (and since any sweep)
local CITY_DWELL = 60       -- seconds in a city before an automatic sweep
local TICK = 60             -- seconds between automatic-sweep checks
local PROGRESS_EVERY = 4    -- a manual sweep prints progress every this many answers

local BRACKETS = { "1-9", "10-19", "20-29", "30-39", "40-49", "50-59", "60" }
local CLASSES = { "WARRIOR", "PALADIN", "HUNTER", "ROGUE", "PRIEST", "SHAMAN", "MAGE", "WARLOCK", "DRUID" }
local CLASS_ENGLISH = {
  WARRIOR = "Warrior", PALADIN = "Paladin", HUNTER = "Hunter", ROGUE = "Rogue", PRIEST = "Priest",
  SHAMAN = "Shaman", MAGE = "Mage", WARLOCK = "Warlock", DRUID = "Druid",
}
C.CITIES = {
  ["Orgrimmar"] = true, ["Thunder Bluff"] = true, ["Undercity"] = true,
  ["Stormwind City"] = true, ["Ironforge"] = true, ["Darnassus"] = true,
  ["Booty Bay"] = true, ["Gadgetzan"] = true, ["Everlook"] = true, ["Ratchet"] = true,
}

-- The running sweep, or nil. Callbacks hold the table they were scheduled for and do nothing once
-- it is no longer C.sweep, so a cancelled sweep's timers die quietly.
C.sweep = nil
C.blocked = false -- set when the client refused /who from the addon; no sweep for the session

-- Localized class and race names <-> English tokens, from the client where it has them.
local classLocal, classToken, raceToken
local function buildNames()
  if classLocal then return end
  classLocal, classToken, raceToken = {}, {}, {}
  for _, list in ipairs({ _G.LOCALIZED_CLASS_NAMES_MALE, _G.LOCALIZED_CLASS_NAMES_FEMALE }) do
    if type(list) == "table" then
      for token, name in pairs(list) do
        if type(name) == "string" then
          classToken[name] = token
          if not classLocal[token] then classLocal[token] = name end
        end
      end
    end
  end
  local classInfo = ns.Fn("C_CreatureInfo", "GetClassInfo")
  local raceInfo = ns.Fn("C_CreatureInfo", "GetRaceInfo")
  for id = 1, 40 do
    local ci = classInfo and ns.Try(classInfo, id)
    if type(ci) == "table" and NonEmpty(ci.className) and NonEmpty(ci.classFile) then
      classToken[ci.className] = ci.classFile
      if not classLocal[ci.classFile] then classLocal[ci.classFile] = ci.className end
    end
    local ri = raceInfo and ns.Try(raceInfo, id)
    if type(ri) == "table" and NonEmpty(ri.raceName) and NonEmpty(ri.clientFileString) then
      raceToken[ri.raceName] = ri.clientFileString
    end
  end
end

local function className(token)
  buildNames()
  return classLocal[token] or CLASS_ENGLISH[token] or token
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

-- The /who API: C_FriendList first, the old globals as the fallback.
local function api()
  local F = {}
  F.send = ns.Fn("C_FriendList", "SendWho")
  F.toUi = ns.Fn("C_FriendList", "SetWhoToUi") or ns.Fn("C_FriendList", "SetWhoToUI")
  F.getToUi = ns.Fn("C_FriendList", "GetWhoToUi") or ns.Fn("C_FriendList", "GetWhoToUI")
  F.num = ns.Fn("C_FriendList", "GetNumWhoResults")
  F.info = ns.Fn("C_FriendList", "GetWhoInfo")
  F.modern = F.send and F.num and F.info and true or false
  if not F.modern then
    F.send = ns.Fn("SendWho")
    F.num = ns.Fn("GetNumWhoResults")
    F.info = ns.Fn("GetWhoInfo")
  end
  if not F.toUi then
    local old = ns.Fn("SetWhoToUI") or ns.Fn("SetWhoToUi")
    if old then F.toUi = function(on) return old(on and 1 or 0) end end
  end
  if not F.getToUi then F.getToUi = ns.Fn("GetWhoToUI") or ns.Fn("GetWhoToUi") end
  return F
end

function C.Available()
  local F = api()
  return F.send ~= nil and F.num ~= nil and F.info ~= nil
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

-- The Who frame: open (by the player) means no sweep; while a sweep runs, the friends frame stops
-- listening for WHO_LIST_UPDATE so the addon's answers never pop it open or redraw it, and gets
-- the event back when the sweep ends, along with SetWhoToUi as it was.
local function whoFrameOpen()
  local wf = _G.WhoFrame
  if type(wf) ~= "table" or type(wf.IsVisible) ~= "function" then return false end
  local ok, shown = pcall(wf.IsVisible, wf)
  return ok and shown and true or false
end

local function quietWhoUi(sweep)
  local F = api()
  if F.getToUi then
    local was = ns.Try(F.getToUi)
    sweep.toUiWas = (was == true or was == 1) and true or false
  end
  if F.toUi then pcall(F.toUi, true) end
  local ff = _G.FriendsFrame
  if type(ff) == "table" and type(ff.IsEventRegistered) == "function" then
    local ok, registered = pcall(ff.IsEventRegistered, ff, "WHO_LIST_UPDATE")
    if ok and registered and pcall(ff.UnregisterEvent, ff, "WHO_LIST_UPDATE") then sweep.friendsMuted = ff end
  end
end

local function restoreWhoUi(sweep)
  local F = api()
  if F.toUi then pcall(F.toUi, sweep.toUiWas and true or false) end
  local ff = sweep.friendsMuted
  sweep.friendsMuted = nil
  if ff then pcall(ff.RegisterEvent, ff, "WHO_LIST_UPDATE") end
end

local function inInstance()
  local yes, kind = ns.Try(ns.Fn("IsInInstance"))
  return (yes and kind ~= "none") and true or false
end

local function inCombat()
  if ns.Try(ns.Fn("InCombatLockdown")) then return true end
  return ns.Try(ns.Fn("UnitAffectingCombat"), "player") and true or false
end

local function refreshPanel()
  local P = ns.Panel
  if not (P and P.panel and P.Refresh) then return end
  local ok, shown = pcall(P.panel.IsVisible, P.panel)
  if ok and shown then ns.Run("panel", P.Refresh) end
end

local function store()
  if type(ns.db.census) ~= "table" then ns.db.census = {} end
  return ns.db.census
end

-- The last sweep's summary: { at, players, new, queries, seconds, complete, stopped }.
function C.LastSweep()
  local c = ns.db and ns.db.census
  local l = type(c) == "table" and c.lastSweep
  if type(l) == "table" and type(l.at) == "number" then return l end
  return nil
end

function C.Running()
  return C.sweep ~= nil
end

-- "Last sweep: 412 players, 20 minutes ago", or the running sweep's progress, for the panel.
function C.StatusLine()
  local sw = C.sweep
  if sw then
    return string.format(L["Sweeping: %d of %d queries, %s so far"], sw.done, #sw.queue, players(sw.players))
  end
  if C.blocked then return L["The game blocked /who from Ryna this session."] end
  local l = C.LastSweep()
  if not l then return L["No sweep yet."] end
  return string.format(l.complete and L["Last sweep: %s, %s"] or L["Last sweep: %s, %s (stopped early)"],
    players(l.players or 0), ns.Status.Ago(l.at) or "?")
end

local finish, sendNext

local function schedule(sweep, delay)
  ns.After(delay, "census sweep", function()
    if C.sweep == sweep then sendNext(sweep) end
  end)
end

-- "8 of 12 queries, 214 players so far", in the client's language (only manual sweeps print it).
local function progress(sweep)
  return string.format(L["%d of %d queries, %s so far"], sweep.done, #sweep.queue, players(sweep.players))
end

-- Splits a capped answer: a bracket by class, a bracket-and-class by the zones the answer named.
local function split(sweep, item, rows)
  local added = 0
  if not item.class then
    for _, token in ipairs(CLASSES) do
      if #sweep.queue < MAX_QUERIES then
        sweep.queue[#sweep.queue + 1] = {
          bracket = item.bracket, class = token,
          text = string.format('%s c-"%s"', item.bracket, className(token)),
        }
        added = added + 1
      end
    end
  elseif not item.zone then
    local zones, seen = {}, {}
    for _, row in ipairs(rows) do
      if row.zone and not seen[row.zone] and #zones < MAX_ZONE_SPLITS then
        seen[row.zone] = true
        zones[#zones + 1] = row.zone
      end
    end
    table.sort(zones)
    for _, zone in ipairs(zones) do
      if #sweep.queue < MAX_QUERIES then
        sweep.queue[#sweep.queue + 1] = {
          bracket = item.bracket, class = item.class, zone = zone,
          text = string.format('%s z-"%s"', item.text, zone),
        }
        added = added + 1
      end
    end
  end
  if added > 0 then
    ns.Debug("census sweep: %q is capped at %d, split into %d more queries", item.text, CAP, added)
  else
    ns.Debug("census sweep: %q is capped at %d and can't be split further", item.text, CAP)
  end
end

local function onAnswer(sweep, item)
  sweep.awaiting = nil
  local rows, n, total = readResults()
  sweep.done = sweep.done + 1
  if not rows then
    ns.Debug("census sweep: %q answered, but the who list couldn't be read", item.text)
    schedule(sweep, BACKOFF)
    return
  end
  ns.Journal.Add("who", { query = item.text, count = n, batch = sweep.batch }, true)
  local fresh, repeated, skipped = 0, 0, 0
  for _, row in ipairs(rows) do
    local name = ns.CanonicalName(row.fullName)
    if name and not ns.Sightings.IsSelf(name) then
      if sweep.names[name] then
        repeated = repeated + 1
      else
        sweep.names[name] = true
        sweep.players = sweep.players + 1
        local earlier = ns.Sightings.RecentByName(name)
        if earlier and (earlier.level or not row.level) then
          skipped = skipped + 1
        else
          row.fullName, row.batch = name, sweep.batch
          if ns.Sightings.Seen("who", row, true) then
            fresh = fresh + 1
            sweep.new = sweep.new + 1
          end
        end
      end
    end
  end
  ns.Debug("census sweep: %q answered %d (of %d), %d recorded, %d already seen in the last 10 minutes, "
    .. "%d already in this sweep", item.text, n, total, fresh, skipped, repeated)
  if n >= CAP or total > n then split(sweep, item, rows) end
  if sweep.manual and (sweep.done % PROGRESS_EVERY == 0) and sweep.done < #sweep.queue then
    ns.Print(string.format(L["census sweep: %s"], progress(sweep)))
  end
  refreshPanel()
  schedule(sweep, n == 0 and BACKOFF or GAP)
end

-- Why a sweep can't go on right now, or nil. Reasons stay English (they are kept in
-- RynaDB.census.lastSweep.stopped and debug lines) and are translated with L[why] where printed.
local function stopReason()
  if not ns.Setting("census") then return N("the census setting is off") end
  if inCombat() then return N("in combat") end
  if inInstance() then return N("in an instance") end
  if whoFrameOpen() then return N("the Who window is open") end
  return nil
end

sendNext = function(sweep)
  local why = stopReason()
  if why then return finish(sweep, why) end
  local item = sweep.queue[sweep.next]
  if not item then return finish(sweep, nil) end
  local F = api()
  if not F.send then return finish(sweep, N("this client has no /who")) end
  if F.toUi then pcall(F.toUi, true) end
  local ok, err = pcall(F.send, item.text)
  if not ok then
    ns.Debug("census sweep: sending %q failed: %s", item.text, tostring(err))
    sweep.errors = sweep.errors + 1
    if sweep.errors >= 2 then
      C.blocked = true
      return finish(sweep, N("the game refused /who from Ryna"))
    end
    schedule(sweep, BACKOFF)
    return
  end
  sweep.next = sweep.next + 1
  sweep.awaiting = item
  sweep.sentAt = GetTime()
  ns.Debug("census sweep: sent %q (%d of %d)", item.text, sweep.next - 1, #sweep.queue)
  local sentAt = sweep.sentAt
  ns.After(ANSWER_TIMEOUT, "census sweep", function()
    if C.sweep ~= sweep or sweep.awaiting ~= item or sweep.sentAt ~= sentAt then return end
    sweep.awaiting = nil
    sweep.done = sweep.done + 1
    sweep.missed = sweep.missed + 1
    ns.Debug("census sweep: no answer to %q in %ds", item.text, ANSWER_TIMEOUT)
    -- The first two queries got no answer: the server is throttling or the client is dropping
    -- the call; either way, stop rather than keep asking.
    if sweep.missed >= 2 and sweep.answered == 0 then
      return finish(sweep, N("the server didn't answer /who"))
    end
    schedule(sweep, BACKOFF)
  end)
end

finish = function(sweep, why)
  if C.sweep ~= sweep then return end
  C.sweep = nil
  restoreWhoUi(sweep)
  local seconds = math.floor(GetTime() - sweep.startedAt + 0.5)
  local complete = why == nil
  store().lastSweep = {
    at = ns.Now(), players = sweep.players, new = sweep.new, queries = sweep.done, seconds = seconds,
    complete = complete, stopped = why, manual = sweep.manual,
  }
  if sweep.manual and complete then
    ns.Print(string.format(L["census sweep done: %s, %d new to this sweep's window, in %ds"],
      players(sweep.players), sweep.new, seconds))
  elseif sweep.manual then
    ns.Print(string.format(L["census sweep stopped (%s): %s, %d new to this sweep's window, in %ds"],
      L[why], players(sweep.players), sweep.new, seconds))
  elseif complete then
    ns.Debug("census sweep done: %d players, %d new to this sweep's window, in %ds",
      sweep.players, sweep.new, seconds)
  else
    ns.Debug("census sweep stopped (%s): %d players, %d new to this sweep's window, in %ds",
      why, sweep.players, sweep.new, seconds)
  end
  refreshPanel()
end

-- Why C.Start didn't start: printed in the client's language for a manual sweep, a debug line
-- (English) for an automatic one. msg is an English N(...) key with at most one %s, and arg (if
-- any) is English too, translated for the printed line.
local function refuse(manual, msg, arg)
  if manual then
    ns.Print(string.format(L[msg], arg and L[arg] or nil))
  else
    ns.Debug(msg, arg)
  end
  return false
end

-- Starts a sweep. manual: from /ryna census or the panel's button (prints progress and the
-- summary); otherwise automatic (debug lines only). Returns true when a sweep started.
function C.Start(manual)
  if not ns.db or not ns.rec then return refuse(manual, N("census sweep: not ready yet.")) end
  if C.sweep then
    -- Only a manual start gets here (the automatic check skips a running sweep).
    ns.Print(string.format(L["census sweep: one is already running. %s"], progress(C.sweep)))
    return false
  end
  if C.blocked then
    return refuse(manual, N("census sweep: the game blocked /who from Ryna earlier this session; "
      .. "/reload to try again."))
  end
  if not C.Available() then return refuse(manual, N("census sweep: this client has no /who for addons.")) end
  local why = stopReason()
  if why then return refuse(manual, N("census sweep: not now, %s."), why) end
  C.count = (C.count or 0) + 1
  local sweep = {
    batch = string.format("%d-%d", ns.Now(), C.count), manual = manual and true or false,
    queue = {}, next = 1, done = 0, players = 0, new = 0, names = {}, errors = 0, missed = 0,
    answered = 0, startedAt = GetTime(),
  }
  for _, bracket in ipairs(BRACKETS) do
    sweep.queue[#sweep.queue + 1] = { bracket = bracket, text = bracket }
  end
  C.sweep = sweep
  store().lastAttemptAt = ns.Now()
  quietWhoUi(sweep)
  ns.Debug("census sweep %s started (%s): %d level brackets to ask", sweep.batch,
    manual and "by you" or "automatic, in a city", #sweep.queue)
  if manual then ns.Print(L["census sweep started: asking the server who is online, one query every 4s."]) end
  refreshPanel()
  sendNext(sweep)
  return true
end

-- Stops the running sweep (it isn't resumed).
function C.Stop(why)
  if C.sweep then finish(C.sweep, why or N("stopped")) end
end

ns.On("WHO_LIST_UPDATE", function()
  local sweep = C.sweep
  if not (sweep and sweep.awaiting) then return end
  sweep.answered = sweep.answered + 1
  onAnswer(sweep, sweep.awaiting)
end)

-- The client refusing the call (an action only the Blizzard UI may take, e.g. /who outside a
-- click): stop, and don't try again this session.
local function onBlocked(_, addon, fn)
  if addon ~= ns.NAME or not C.sweep then return end
  if type(fn) == "string" and not fn:find("Who", 1, true) then return end
  C.blocked = true
  finish(C.sweep, N("the game blocked /who from Ryna"))
end
ns.On("ADDON_ACTION_BLOCKED", onBlocked)
ns.On("ADDON_ACTION_FORBIDDEN", onBlocked)

ns.On("PLAYER_REGEN_DISABLED", function() C.Stop(N("combat started")) end)

-- Automatic sweeps: checked every minute; one runs when the setting is on, the player has been in
-- a city for a minute, isn't in combat or an instance, the Who window is closed, and no sweep
-- started in the last 30 minutes.
local city = { zone = nil, since = nil }

function C.AutoCheck()
  local rec = ns.rec
  if not rec or not ns.db then return end
  -- Booty Bay, Gadgetzan, Everlook and Ratchet are subzones, so the live subzone counts too.
  local zone = NonEmpty(ns.Try(ns.Fn("GetRealZoneText"))) or rec.zone
  local sub = NonEmpty(ns.Try(ns.Fn("GetSubZoneText")))
  if not (zone and C.CITIES[zone]) then zone = sub and C.CITIES[sub] and sub or nil end
  if not zone or inInstance() then
    city.zone, city.since = nil, nil
    return
  end
  if city.zone ~= zone then city.zone, city.since = zone, GetTime() end
  if not (ns.Setting("census") and ns.Setting("censusSweep")) or C.sweep or C.blocked then return end
  if GetTime() - city.since < CITY_DWELL then return end
  local c = ns.db.census
  local lastAt = type(c) == "table" and type(c.lastAttemptAt) == "number" and c.lastAttemptAt or nil
  if lastAt and ns.Now() - lastAt < AUTO_EVERY then return end
  if inCombat() or whoFrameOpen() then return end
  ns.Debug("census sweep: in %s for %s, starting the automatic sweep", zone,
    ns.DurationText(GetTime() - city.since))
  C.Start(false)
end

local ticking = false
ns.On("PLAYER_ENTERING_WORLD", function()
  if inInstance() then C.Stop(N("entered an instance")) end
  if ticking or not (_G.C_Timer and _G.C_Timer.NewTicker) then return end
  ticking = true
  _G.C_Timer.NewTicker(TICK, function() ns.Run("census auto", C.AutoCheck) end)
end)
