-- Places: the character's lifetime by zone (docs/ryna-agent-cores-mods.md 7.6), for the window's
-- Zones section. The journal keeps only 30 days, so the record keeps its own small map:
--
--   places[zone] = { seconds, quests, deaths, first, last }
--
--   seconds  time spent there: from entering the zone to the next zone change or the logout
--   quests   quests turned in there
--   deaths   deaths there
--   first    when the character was first seen there; last when it was last there
--
-- The zone is the record's (GetRealZoneText, Zone.lua), the same key Chronicle's tallies use. The
-- first time it runs for a character it is seeded from what the journal still holds (its `session`,
-- `zone`, `questDone` and `death` events), and `placesSince` says from when, so the window can say
-- "time since <date>" until the map covers everything. Gated on the `chronicle` setting like the
-- tallies: with it off nothing counts.

local _, ns = ...
if ns.off then return end

local P = {}
ns.Places = P

local function num(v)
  if type(v) == "number" then return v end
  return tonumber(v) or 0
end

local function on()
  return ns.rec ~= nil and ns.Setting("chronicle")
end

local function usable(zone)
  return type(zone) == "string" and zone ~= "" and not ns.IsSecret(zone)
end

-- places[zone] on the record, made when missing.
local function place(rec, zone)
  if type(rec.places) ~= "table" then rec.places = {} end
  local p = rec.places[zone]
  if type(p) ~= "table" then
    p = { seconds = 0, quests = 0, deaths = 0 }
    rec.places[zone] = p
  end
  return p
end

local function seen(p, at)
  if type(at) ~= "number" then return end
  if p.first == nil or at < num(p.first) then p.first = at end
  if p.last == nil or at > num(p.last) then p.last = at end
end

P.version = 0

-- The zone the character is in now, and since when (this session; nil until Enter).
local stay = { zone = nil, at = nil, rec = nil }

-- Adds the time since the stay began to its zone, and starts the stay again from now.
function P.Accrue()
  local now = ns.Now()
  if stay.zone and stay.rec == ns.rec and on() then
    local p = place(stay.rec, stay.zone)
    p.seconds = num(p.seconds) + math.max(0, now - num(stay.at))
    seen(p, now)
    P.version = P.version + 1
  end
  stay.at = now
end

-- The stay under way, for the window: its zone and the seconds not yet added (nil when none).
function P.Current()
  if not stay.zone or stay.rec ~= ns.rec or not on() then return nil, 0 end
  return stay.zone, math.max(0, ns.Now() - num(stay.at))
end

-- The character arrived in `zone` (a zone change, a login or a /reload).
function P.Enter(zone)
  P.Accrue()
  stay.rec = ns.rec
  if not usable(zone) then
    stay.zone = nil
    return
  end
  stay.zone = zone
  if on() then
    seen(place(ns.rec, zone), ns.Now())
    P.version = P.version + 1
  end
end

-- A quest turned in, or a death, in the zone the character is in.
local function count(field)
  if not on() then return end
  local zone = ns.rec.zone
  if not usable(zone) then return end
  local p = place(ns.rec, zone)
  p[field] = num(p[field]) + 1
  seen(p, ns.Now())
  P.version = P.version + 1
end
function P.Quest() count("quests") end
function P.Death() count("deaths") end

-- Seeds `places` from the journal the first time (rec.placesSince unset): every session of this
-- character but the one now running, in order. A stay runs from a zone (the login's or a zone event's)
-- to the next zone event or the session's logout; a session the game closed without a logout ends at
-- its last event. Returns how many events it read.
function P.Seed(rec)
  if type(rec) ~= "table" or rec.placesSince ~= nil or not ns.Setting("chronicle") then return 0 end
  local now = ns.Now()
  local current = ns.API.Journal and ns.API.Journal.Session and ns.API.Journal.Session() or nil
  local journal = ns.db and ns.db.journal
  local sessions, order = {}, {}
  local oldest
  if type(journal) == "table" then
    for i = 1, #journal do
      local e = journal[i]
      if type(e) == "table" and e.char == rec.guid and type(e.at) == "number" and e.session ~= current
        and (e.type == "session" or e.type == "zone" or e.type == "questDone" or e.type == "death") then
        local key = tostring(e.session)
        local list = sessions[key]
        if not list then
          list = {}
          sessions[key] = list
          order[#order + 1] = key
        end
        list[#list + 1] = { e = e, i = i }
        if oldest == nil or e.at < oldest then oldest = e.at end
      end
    end
  end
  local read = 0
  for _, key in ipairs(order) do
    local list = sessions[key]
    table.sort(list, function(a, b)
      if a.e.at ~= b.e.at then return a.e.at < b.e.at end
      return a.i < b.i
    end)
    local zone, since
    local function leave(at)
      if zone and since then
        local p = place(rec, zone)
        p.seconds = num(p.seconds) + math.max(0, at - since)
        seen(p, at)
      end
    end
    local lastAt
    for _, item in ipairs(list) do
      local e = item.e
      read = read + 1
      lastAt = e.at
      if e.type == "zone" or (e.type == "session" and e.kind == "login") then
        if usable(e.zone) then
          leave(e.at)
          zone, since = e.zone, e.at
          seen(place(rec, zone), e.at)
        elseif e.type == "zone" then
          leave(e.at)
          zone, since = nil, nil
        end
      elseif e.type == "session" and e.kind == "logout" then
        leave(e.at)
        zone, since = nil, nil
      elseif usable(e.zone) then
        local p = place(rec, e.zone)
        if e.type == "questDone" then p.quests = num(p.quests) + 1 else p.deaths = num(p.deaths) + 1 end
        seen(p, e.at)
      end
    end
    if lastAt then leave(lastAt) end
  end
  if type(rec.places) ~= "table" then rec.places = {} end
  rec.placesSince = oldest or now
  P.version = P.version + 1
  ns.Debug("places: seeded from %d journal events, since %s", read, date("%Y-%m-%d", rec.placesSince))
  return read
end

ns.On("QUEST_TURNED_IN", function() P.Quest() end)
ns.On("PLAYER_DEAD", function() P.Death() end)
