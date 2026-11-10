-- Session: this play session's numbers, for the session strip (Strip.lua), Ryna's window
-- (Window.lua) and the Status tab. In memory while you play: kills by creature (count and
-- experience), experience gained (PLAYER_XP_UPDATE deltas, level-ups included), money gained and
-- spent, quests turned in, deaths, items looted and the time since login. Rates per hour are worked
-- out when S.Get() is called, never on a timer.
--
-- A /reload keeps the session: at logout (which a /reload is too) the session is written to
-- RynaDB.chronicle.session, and the first PLAYER_ENTERING_WORLD after a /reload takes it back for
-- the same character. A real login starts a new one. Nothing else is saved, and the lifetime record
-- (Chronicle.lua) is untouched.
--
-- S.Get() returns a plain table (see there); S.Watch(isShown, redraw) is how the strip and the
-- window ask to be redrawn, at most every half second and only while one of them is on screen.

local _, ns = ...
if ns.off then return end

local S = {}
ns.Session = S

local REDRAW_DELAY = 0.5
local MIN_SECONDS = 60 -- rates over less than a minute read as over a minute

S.version = 0       -- every change
S.killsVersion = 0  -- the session's creatures
S.levelVersion = 0  -- a level gained

local cur  -- the session (saved as it is: plain numbers, strings and the creatures map)
local base = { xp = 0, xpMax = 0, level = 0, money = nil, pendingLevel = nil }

local function num(v)
  if type(v) == "number" and not ns.IsSecret(v) then return v end
  return nil
end

local function fresh(guid)
  return {
    guid = guid, startAt = ns.Now(), startClock = GetTime(),
    kills = 0, xp = 0, levels = 0, goldGained = 0, goldSpent = 0, quests = 0, deaths = 0, items = 0,
    creatures = {}, -- creature id (or "?<name>" while its id isn't known) -> { name, n, xp }
  }
end

-- Redraws -----------------------------------------------------------------------------------

local watchers = {}

function S.Watch(isShown, redraw)
  watchers[#watchers + 1] = { shown = isShown, redraw = redraw }
end

local function anyShown()
  for i = 1, #watchers do
    if watchers[i].shown() then return true end
  end
  return false
end

local function redrawAll()
  for i = 1, #watchers do
    local w = watchers[i]
    if w.shown() then ns.Run("chronicle redraw", w.redraw) end
  end
end

-- Something the strip or the window shows changed (the session, or the lifetime kills and loot).
function S.Changed()
  S.version = S.version + 1
  if anyShown() then ns.Debounce("chronicle-live", REDRAW_DELAY, redrawAll) end
end

-- The session ---------------------------------------------------------------------------------

local function readBase()
  base.xp = num(ns.Try(UnitXP, "player")) or 0
  base.xpMax = num(ns.Try(UnitXPMax, "player")) or 0
  base.level = num(ns.Try(UnitLevel, "player")) or 0
  base.money = num(ns.Try(GetMoney))
  base.pendingLevel = nil
end

-- At login (Init.lua's onLogin, after the character record is built).
function S.Start(guid)
  cur = fresh(guid)
  S.adoptChecked = false
  readBase()
  S.killsVersion = S.killsVersion + 1
  S.Changed()
end

-- A saved session is taken back only for the same character, and only when the clock it was
-- started on is still running (GetTime is the computer's uptime, so it survives a /reload).
local function usable(saved, guid)
  if type(saved) ~= "table" or saved.guid ~= guid then return false end
  if type(saved.creatures) ~= "table" or not num(saved.startClock) or not num(saved.startAt) then return false end
  return saved.startClock <= GetTime()
end

local function adopt(saved)
  for _, key in ipairs({ "kills", "xp", "levels", "goldGained", "goldSpent", "quests", "deaths", "items" }) do
    cur[key] = (num(saved[key]) or 0) + (cur[key] or 0)
  end
  for id, e in pairs(cur.creatures) do
    local s = saved.creatures[id]
    if type(s) == "table" then
      s.n, s.xp = (num(s.n) or 0) + e.n, (num(s.xp) or 0) + e.xp
    else
      saved.creatures[id] = e
    end
  end
  cur.creatures = saved.creatures
  cur.startAt, cur.startClock = saved.startAt, saved.startClock
  S.killsVersion = S.killsVersion + 1
  S.Changed()
end

-- The first PLAYER_ENTERING_WORLD after login: a /reload takes the saved session back. The
-- event's second argument says so; a client that doesn't pass it is taken to have reloaded when the
-- session was saved under a minute ago on this clock.
function S.OnEnteringWorld(isInitialLogin, isReloadingUi)
  if not cur or S.adoptChecked then return end
  S.adoptChecked = true
  local root = ns.db and ns.db.chronicle
  local saved = type(root) == "table" and root.session or nil
  if type(root) == "table" then root.session = nil end
  if not usable(saved, cur.guid) then return end
  local reload = isReloadingUi
  if type(reload) ~= "boolean" then
    local savedClock = num(saved.savedClock)
    reload = isInitialLogin ~= true and savedClock ~= nil and GetTime() - savedClock < 60
  end
  if not reload then return end
  saved.savedClock = nil
  adopt(saved)
  ns.Debug("session: kept across the reload, %d kills since %s", cur.kills, date("%H:%M", cur.startAt))
end

-- At logout (and so at a /reload): the session, for the next PLAYER_ENTERING_WORLD.
function S.Save()
  local db = ns.db
  if not cur or not db then return end
  if type(db.chronicle) ~= "table" then db.chronicle = {} end
  cur.savedClock = GetTime()
  db.chronicle.session = cur
end

-- Kills (from Chronicle.lua): n kills of creature `id` (nil while its id isn't known yet) worth `xp`.
-- n is 0 when an experience line only adds to a kill already counted.
function S.Kill(id, name, xp, n)
  if not cur then return end
  local key = id or ("?" .. tostring(name))
  local e = cur.creatures[key]
  if not e then
    e = { name = name, n = 0, xp = 0 }
    cur.creatures[key] = e
  elseif name and not e.name then
    e.name = name
  end
  e.n = e.n + (n or 0)
  e.xp = e.xp + (xp or 0)
  cur.kills = cur.kills + (n or 0)
  S.killsVersion = S.killsVersion + 1
  S.Changed()
end

-- A kill counted by name, before its creature was seen, now has its id.
function S.Settle(name, id)
  if not cur or not name or not id then return end
  local key = "?" .. name
  local e = cur.creatures[key]
  if not e then return end
  cur.creatures[key] = nil
  local into = cur.creatures[id]
  if into then
    into.n, into.xp = into.n + e.n, into.xp + e.xp
  else
    cur.creatures[id] = e
  end
  S.killsVersion = S.killsVersion + 1
  S.Changed()
end

-- An item looted (from Chronicle.lua), `qty` of it.
function S.Loot(qty)
  if not cur then return end
  cur.items = cur.items + (qty or 1)
  S.Changed()
end

-- Experience: the change since the last update. A level-up (PLAYER_LEVEL_UP, a higher UnitLevel or
-- the bar going backwards, whichever the client shows first) adds what was left of the old bar.
function S.OnLevelUp(level)
  level = tonumber(level)
  if level and level > base.level then base.pendingLevel = level end
end

function S.UpdateXp()
  if not cur then return end
  local xp, xpMax = num(ns.Try(UnitXP, "player")), num(ns.Try(UnitXPMax, "player"))
  if not xp then return end
  local level = math.max(num(ns.Try(UnitLevel, "player")) or 0, base.pendingLevel or 0)
  local gained
  if level > base.level or xp < base.xp then
    gained = math.max(0, base.xpMax - base.xp) + xp
    cur.levels = cur.levels + math.max(1, level - base.level)
    base.level = math.max(level, base.level + 1)
    base.pendingLevel = nil
    S.levelVersion = S.levelVersion + 1
  else
    gained = xp - base.xp
  end
  base.xp, base.xpMax = xp, xpMax or base.xpMax
  if gained > 0 then
    cur.xp = cur.xp + gained
    S.Changed()
  end
end

-- Money, after Character.lua's own PLAYER_MONEY handler has read it into the record.
function S.UpdateMoney()
  if not cur then return end
  local money = num(ns.rec and ns.rec.gold) or num(ns.Try(GetMoney))
  if not money then return end
  local delta = base.money and (money - base.money) or 0
  base.money = money
  if delta > 0 then
    cur.goldGained = cur.goldGained + delta
  elseif delta < 0 then
    cur.goldSpent = cur.goldSpent - delta
  else
    return
  end
  S.Changed()
end

-- Reading it ----------------------------------------------------------------------------------

-- The session's creatures, most kills first: { id, name, n, xp } (cached until a kill changes it;
-- read it, don't change it). id is nil for one whose id isn't known yet.
local sorted = { version = -1, list = {} }

local function byKills(a, b)
  if a.n ~= b.n then return a.n > b.n end
  if a.xp ~= b.xp then return a.xp > b.xp end
  return tostring(a.name) < tostring(b.name)
end

function S.Creatures()
  if sorted.version == S.killsVersion and sorted.cur == cur then return sorted.list end
  sorted.version, sorted.cur = S.killsVersion, cur
  local list = sorted.list
  for i = #list, 1, -1 do list[i] = nil end
  if cur then
    for key, e in pairs(cur.creatures) do
      if type(e) == "table" and (num(e.n) or 0) > 0 then
        local id = key:sub(1, 1) ~= "?" and key or nil
        list[#list + 1] = { id = id, name = e.name or ("?" .. key), n = e.n, xp = e.xp or 0 }
      end
    end
  end
  table.sort(list, byKills)
  return list
end

-- The session as a plain table, or nil before login:
--   startAt (server time), elapsed (seconds), kills, xp, levels, goldGained, goldSpent, gold (the
--   net change, copper), quests, deaths, items (looted, counting stack sizes),
--   killsPerHour, xpPerHour, goldPerHour (net), creatures (S.Creatures()).
function S.Get()
  if not cur then return nil end
  local elapsed = math.max(0, GetTime() - cur.startClock)
  local hours = math.max(elapsed, MIN_SECONDS) / 3600
  local net = cur.goldGained - cur.goldSpent
  return {
    startAt = cur.startAt, elapsed = elapsed,
    kills = cur.kills, xp = cur.xp, levels = cur.levels,
    goldGained = cur.goldGained, goldSpent = cur.goldSpent, gold = net,
    quests = cur.quests, deaths = cur.deaths, items = cur.items,
    killsPerHour = cur.kills / hours, xpPerHour = cur.xp / hours, goldPerHour = net / hours,
    creatures = S.Creatures(),
  }
end

-- Text helpers shared by the strip, the window and the Status tab ----------------------------

local L = ns.L

-- 3120 -> "3,120" (the client's own separators where it has BreakUpLargeNumbers).
function S.Number(n)
  n = math.floor((tonumber(n) or 0) + 0.5)
  local f = ns.Fn("BreakUpLargeNumbers")
  local text = f and ns.Try(f, n)
  if type(text) == "string" then return text end
  local s = tostring(math.abs(n))
  local out = s:reverse():gsub("(%d%d%d)", "%1,"):reverse()
  if out:sub(1, 1) == "," then out = out:sub(2) end
  return (n < 0 and "-" or "") .. out
end

-- Copper -> "1g 20s" (whole silver once it is a gold or more), "45s 3c" below.
function S.Money(copper)
  copper = math.floor(tonumber(copper) or 0)
  if math.abs(copper) >= 10000 then copper = copper - math.fmod(copper, 100) end
  return ns.MoneyText(copper)
end

-- Seconds -> "1h 12m", "12m", "under a minute"; "2d 3h" past a day.
function S.Duration(seconds)
  local m = math.floor((tonumber(seconds) or 0) / 60)
  if m < 1 then return L["under a minute"] end
  local d, h = math.floor(m / 1440), math.floor(m / 60) % 24
  if d > 0 then return string.format(L["%dd %dh"], d, h) end
  if h > 0 then return string.format(L["%dh %dm"], h, m % 60) end
  return string.format(L["%dm"], m)
end

-- Events ------------------------------------------------------------------------------------

ns.On("PLAYER_ENTERING_WORLD", function(_, isInitialLogin, isReloadingUi)
  S.OnEnteringWorld(isInitialLogin, isReloadingUi)
end)
ns.On("PLAYER_XP_UPDATE", function() S.UpdateXp() end)
ns.On("PLAYER_LEVEL_UP", function(_, level) S.OnLevelUp(level) end)
ns.On("PLAYER_MONEY", function() S.UpdateMoney() end)
ns.On("QUEST_TURNED_IN", function()
  if not cur then return end
  cur.quests = cur.quests + 1
  S.Changed()
end)
ns.On("PLAYER_DEAD", function()
  if not cur then return end
  cur.deaths = cur.deaths + 1
  S.Changed()
end)
