-- Chronicle: kills and loot (docs/wow.md section 12.1). Lifetime tallies in the character record,
-- never journal events: a session has thousands of kills, and a row each would make the file large
-- and logout slow. Three string-keyed maps on the record (Character.lua carries them over):
--
--   kills[npcId]    { name, n, own, xp, looted, last, zone }   CHAT_MSG_COMBAT_XP_GAIN (+ units seen), LOOT_OPENED
--   loot[itemId]    { name, link, quality, n, qty, last, from = { [source] = n } }   LOOT_SLOT_CLEARED
--   objects[id]     { n, last, zone }                          LOOT_OPENED from a game object
--
-- A loot source is "c:<npcId>" (a creature), "o:<objectId>" (a chest, vein, herb or pool),
-- "i:<itemId>" (a container item; "i" when the client can't say which) or "f" (no source: fishing).
-- Everything is gated on the `chronicle` setting; with it off nothing is counted and the maps stay
-- as they are. Nothing here touches the combat log: this client forbids it to addons (see Kills).

local _, ns = ...

local Ch = {}
ns.Chronicle = Ch

local NonEmpty = ns.NonEmpty

local CAP = { kills = 2000, loot = 3000, objects = 2000 }
local MAX_FROM = 8
local XP_WINDOW = 5      -- seconds between a kill and its experience message
local RECENT_KILLS = 16  -- kills remembered for the experience message (parallel arrays, reused)
local SEEN_RING = 200    -- loot source GUIDs remembered, so a corpse reopened isn't counted twice
local LOOT_SLOT_ITEM = 1 -- GetLootSlotType: 0 none, 1 item, 2 money, 3 currency

local function chronicleOn()
  return ns.rec and ns.Setting("chronicle")
end

-- The record's maps, made when an older record has none.
local function map(name)
  local rec = ns.rec
  if not rec then return nil end
  local m = rec[name]
  if type(m) ~= "table" then
    m = {}
    rec[name] = m
  end
  return m
end

-- Entry counts per map, recounted when the record changes (login) and kept as entries come and go.
local counted = { rec = nil, kills = 0, loot = 0, objects = 0 }

local function recount()
  if counted.rec == ns.rec then return end
  counted.rec = ns.rec
  for name in pairs(CAP) do
    local n = 0
    local m = ns.rec and ns.rec[name]
    if type(m) == "table" then
      for _ in pairs(m) do n = n + 1 end
    end
    counted[name] = n
  end
end

local function num(v)
  if type(v) == "number" then return v end
  return tonumber(v) or 0
end

-- A new entry in map `name` under `key`; past the cap, the entry with the oldest `last` goes first.
local function newEntry(name, key)
  local m = map(name)
  if not m then return nil end
  recount()
  if counted[name] >= CAP[name] then
    local oldestKey, oldest
    for k, e in pairs(m) do
      local t = type(e) == "table" and num(e.last) or 0
      if not oldest or t < oldest then oldestKey, oldest = k, t end
    end
    if oldestKey ~= nil then
      m[oldestKey] = nil
      counted[name] = counted[name] - 1
    end
  end
  local e = {}
  m[key] = e
  counted[name] = counted[name] + 1
  return e
end

local function entry(name, key)
  local m = map(name)
  if not m then return nil end
  local e = m[key]
  if type(e) == "table" then return e end
  if e ~= nil then
    m[key] = nil
    counted[name] = counted[name] - 1
  end
  return newEntry(name, key)
end

-- GUIDs: "Creature-0-<server>-<instance>-<zoneUID>-<npcId>-<spawnUID>"; GameObject and Vehicle
-- have the same fields. Returns the kind and the id (as a string key, "6" not "06"), or nil.
local function guidId(guid)
  if type(guid) ~= "string" then return nil end
  local kind, id = guid:match("^(%a+)%-[^%-]*%-[^%-]*%-[^%-]*%-[^%-]*%-(%d+)")
  if not kind then return nil end
  id = tonumber(id)
  if not id then return nil end
  return kind, tostring(id)
end
Ch.GuidId = guidId

-- The creature id of a Creature- or Vehicle- GUID, or nil.
function Ch.CreatureId(guid)
  local kind, id = guidId(guid)
  if kind == "Creature" or kind == "Vehicle" then return id end
  return nil
end

function Ch.ItemIdFromLink(link)
  if type(link) ~= "string" then return nil end
  local id = tonumber(link:match("|Hitem:(%d+)"))
  return id and tostring(id) or nil
end

-- Kills ---------------------------------------------------------------------------------------
--
-- Not from the combat log: this client refuses COMBAT_LOG_EVENT_UNFILTERED to addons (the "blocked
-- from an action only available to the Blizzard UI" dialog at login, 2026-11-07, 0.2.2), so a kill is
-- the chat line the game prints for it, COMBATLOG_XPGAIN_FIRSTPERSON ("%s dies, you gain %d
-- experience."), which names the creature and gives the experience. The creature's id comes from the
-- units the player sees: the target, the mouseover and nameplates give a GUID and a name, and the
-- name is the join. A kill whose name isn't known yet waits (in memory, a few at most) until the name
-- is seen. Solo kills are the player's own; in a group the chat line can't say whose blow it was.

-- Creature name -> id, learned from units seen. Names are localised, so a map per session is right.
local namesToId = {}
-- Kills named before their creature was seen: name -> { n, xp }.
local pendingByName = {}
local PENDING_MAX = 32
local pendingCount = 0

local function solo()
  local n = ns.Try(ns.Fn("GetNumGroupMembers"))
  return type(n) ~= "number" or n <= 1
end

local function addKill(id, name, xp, n, own)
  local e = entry("kills", id)
  if not e then return end
  e.n = num(e.n) + n
  e.own = num(e.own) + (own and n or 0)
  e.xp = num(e.xp) + xp
  e.looted = num(e.looted)
  e.last = ns.Now()
  e.zone = ns.rec.zone
  if name then e.name = name end
  ns.Debug("chronicle: killed %s (%s)%s, %d in all", name or "?", id, own and ", mine" or "", e.n)
end

-- A unit seen: remember its creature id by name, and settle any kills waiting on that name.
function Ch.LearnUnit(unit)
  if not chronicleOn() or type(unit) ~= "string" then return end
  local guid = ns.Try(UnitGUID, unit)
  if not guid or ns.IsSecret(guid) then return end
  local id = Ch.CreatureId(guid)
  if not id then return end
  local name = ns.Try(UnitName, unit)
  if type(name) ~= "string" or ns.IsSecret(name) or name == "" then return end
  if namesToId[name] ~= id then
    namesToId[name] = id
    local m = ns.rec and type(ns.rec.kills) == "table" and ns.rec.kills[id]
    if type(m) == "table" and not NonEmpty(m.name) then m.name = name end
  end
  local waiting = pendingByName[name]
  if waiting then
    pendingByName[name] = nil
    pendingCount = pendingCount - 1
    addKill(id, name, waiting.xp, waiting.n, waiting.own)
  end
end

-- COMBATLOG_XPGAIN_FIRSTPERSON ("%s dies, you gain %d experience.") as a Lua pattern, anchored at
-- the start only, so the group, raid and rested variants (the same sentence with a bonus after it)
-- match too. Positional specifiers (%1$s) are allowed; nameAt and xpAt say which capture is which.
local xpPattern, nameAt, xpAt

function Ch.FormatToPattern(fmt)
  if type(fmt) ~= "string" or fmt == "" then return nil end
  local order = {}
  for spec in fmt:gsub("%%%%", ""):gmatch("%%%d*%$?([sd])") do order[#order + 1] = spec end
  local p = fmt:gsub("%%%%", "\003"):gsub("%%%d*%$?s", "\001"):gsub("%%%d*%$?d", "\002")
  p = p:gsub("([%%%(%)%.%+%-%*%?%[%]%^%$])", "%%%1")
  p = p:gsub("\001", "(.+)"):gsub("\002", "(%%d+)"):gsub("\003", "%%%%")
  local s, d
  for i, kind in ipairs(order) do
    if kind == "s" and not s then s = i end
    if kind == "d" and not d then d = i end
  end
  return "^" .. p, s, d
end

local function buildXpPattern()
  if xpPattern ~= nil then return xpPattern end
  local p, s, d = Ch.FormatToPattern(_G.COMBATLOG_XPGAIN_FIRSTPERSON)
  if p and s and d and s <= 2 and d <= 2 then
    xpPattern, nameAt, xpAt = p, s, d
  else
    xpPattern = false
  end
  return xpPattern
end

-- The kill line in chat. Returns the creature's name and the experience, or nil.
function Ch.ParseXpMessage(text)
  if type(text) ~= "string" or ns.IsSecret(text) then return nil end
  local pattern = buildXpPattern()
  if not pattern then return nil end
  local a, b = text:match(pattern)
  if not a then return nil end
  local name = nameAt == 1 and a or b
  local xp = tonumber(xpAt == 1 and a or b)
  if not name or not xp then return nil end
  return name, xp
end

function Ch.OnXpMessage(text)
  local name, xp = Ch.ParseXpMessage(text)
  if not name then return end
  local own = solo()
  -- The target is usually what just died; learn it before looking the name up.
  Ch.LearnUnit("target")
  local id = namesToId[name]
  if id then
    addKill(id, name, xp, 1, own)
    return
  end
  local w = pendingByName[name]
  if w then
    w.n, w.xp, w.own = w.n + 1, w.xp + xp, w.own and own
    return
  end
  if pendingCount >= PENDING_MAX then
    -- Drop the oldest waiting name rather than grow without bound.
    local oldestName, oldestAt
    for k, v in pairs(pendingByName) do
      if not oldestAt or v.at < oldestAt then oldestName, oldestAt = k, v.at end
    end
    if oldestName then
      pendingByName[oldestName] = nil
      pendingCount = pendingCount - 1
    end
  end
  pendingByName[name] = { n = 1, xp = xp, own = own, at = GetTime() }
  pendingCount = pendingCount + 1
  ns.Debug("chronicle: killed %s, waiting to learn its id", name)
end

-- Loot ----------------------------------------------------------------------------------------

-- Recently counted source GUIDs: a ring of SEEN_RING, with a set for the lookup.
local seenRing, seenSet, seenPos = {}, {}, 0

local function firstSeen(guid)
  if seenSet[guid] then return false end
  seenPos = seenPos % SEEN_RING + 1
  local old = seenRing[seenPos]
  if old then seenSet[old] = nil end
  seenRing[seenPos] = guid
  seenSet[guid] = true
  return true
end

-- The item id of a container item's GUID ("Item-..." holds no item id), where the client says.
local function containerItemId(guid)
  local id = ns.Try(ns.Fn("C_Item", "GetItemIDByGUID"), guid)
  if type(id) == "number" then return tostring(id) end
  return nil
end

-- The record key of a loot source GUID.
local function sourceKey(guid)
  if type(guid) ~= "string" or guid == "" or ns.IsSecret(guid) then return "f" end
  local kind, id = guidId(guid)
  if kind == "Creature" or kind == "Vehicle" then return "c:" .. id end
  if kind == "GameObject" then return "o:" .. id end
  if guid:find("^Item%-") then
    local itemId = containerItemId(guid)
    return itemId and ("i:" .. itemId) or "i"
  end
  return "f"
end

-- One source GUID opened: a corpse counts towards its creature's `looted`, an object towards
-- `objects`; each GUID once while it is in the ring.
local function countSource(guid)
  if type(guid) ~= "string" or guid == "" or ns.IsSecret(guid) or not firstSeen(guid) then return end
  local kind, id = guidId(guid)
  if kind == "Creature" or kind == "Vehicle" then
    local e = entry("kills", id)
    if not e then return end
    e.n, e.own, e.xp = num(e.n), num(e.own), num(e.xp)
    e.looted = num(e.looted) + 1
  elseif kind == "GameObject" then
    local e = entry("objects", id)
    if not e then return end
    e.n = num(e.n) + 1
    e.last = ns.Now()
    e.zone = ns.rec.zone
  end
end

-- GetLootSourceInfo returns guid1, quantity1, guid2, quantity2, ...: every GUID is counted, and
-- the first (the slot's source) is returned.
local function countSources(guid, _, ...)
  if guid == nil then return nil end
  countSource(guid)
  if select("#", ...) > 0 then countSources(...) end
  return guid
end

-- The open loot window's item slots, remembered until LOOT_CLOSED. The tables are reused.
local slots, slotCount = {}, 0

local function rememberSlot(i, sourceGuid)
  local s = slots[i]
  if not s then
    s = {}
    slots[i] = s
  end
  s.open = false
  local slotType = ns.Try(ns.Fn("GetLootSlotType"), i)
  local link = ns.Try(ns.Fn("GetLootSlotLink"), i)
  if type(link) ~= "string" or ns.IsSecret(link) then link = nil end
  if slotType ~= nil and slotType ~= LOOT_SLOT_ITEM then return end
  local itemId = Ch.ItemIdFromLink(link)
  if not itemId then return end
  local _, name, quantity, _, quality = ns.Try(ns.Fn("GetLootSlotInfo"), i)
  s.open = true
  s.itemId, s.link, s.source = itemId, link, sourceKey(sourceGuid)
  s.name = NonEmpty(name)
  s.quantity = type(quantity) == "number" and not ns.IsSecret(quantity) and quantity or 1
  s.quality = type(quality) == "number" and not ns.IsSecret(quality) and quality or nil
end

function Ch.OnLootOpened()
  slotCount = 0
  if not chronicleOn() then return end
  Ch.LearnUnit("target") -- the corpse being looted is usually the target: its name for the kills table
  local n = ns.Try(ns.Fn("GetNumLootItems"))
  if type(n) ~= "number" or ns.IsSecret(n) then return end
  local sourceInfo = ns.Fn("GetLootSourceInfo")
  for i = 1, n do
    local first = sourceInfo and countSources(ns.Try(sourceInfo, i)) or nil
    rememberSlot(i, first)
  end
  slotCount = n
  ns.Debug("chronicle: loot window, %d slots", n)
end

-- Adds 1 to from[source], keeping the MAX_FROM biggest (a tie goes against the older source).
local function addFrom(e, source)
  if type(e.from) ~= "table" then e.from = {} end
  local from = e.from
  if from[source] ~= nil then
    from[source] = num(from[source]) + 1
    return
  end
  from[source] = 1
  local n, minKey, minN = 0, nil, nil
  for k, v in pairs(from) do
    n = n + 1
    if k ~= source and (minN == nil or num(v) < minN) then minKey, minN = k, num(v) end
  end
  if n > MAX_FROM and minKey ~= nil then from[minKey] = nil end
end

function Ch.OnSlotCleared(i)
  if type(i) ~= "number" or i > slotCount then return end
  local s = slots[i]
  if not s or not s.open then return end
  s.open = false
  if not chronicleOn() then return end
  local e = entry("loot", s.itemId)
  if not e then return end
  e.n = num(e.n) + 1
  e.qty = num(e.qty) + s.quantity
  e.last = ns.Now()
  if s.name then e.name = s.name end
  if s.link then e.link = s.link end
  if s.quality then e.quality = s.quality end
  addFrom(e, s.source)
  Ch.lootVersion = (Ch.lootVersion or 0) + 1
  ns.Debug("chronicle: looted %s x%d from %s, %d in all", s.name or s.itemId, s.quantity, s.source, e.n)
end

function Ch.OnLootClosed()
  for i = 1, slotCount do
    if slots[i] then slots[i].open = false end
  end
  slotCount = 0
end

-- For the panel's status line: how many creatures and items the record holds.
function Ch.Counts()
  if not ns.rec then return 0, 0 end
  recount()
  return counted.kills, counted.loot
end

ns.On("CHAT_MSG_COMBAT_XP_GAIN", function(_, text)
  if chronicleOn() then Ch.OnXpMessage(text) end
end)
ns.On("PLAYER_TARGET_CHANGED", function() Ch.LearnUnit("target") end)
ns.On("UPDATE_MOUSEOVER_UNIT", function() Ch.LearnUnit("mouseover") end)
ns.On("NAME_PLATE_UNIT_ADDED", function(_, unit) Ch.LearnUnit(unit) end)
ns.On("LOOT_OPENED", function() Ch.OnLootOpened() end)
ns.On("LOOT_SLOT_CLEARED", function(_, slot) Ch.OnSlotCleared(slot) end)
ns.On("LOOT_CLOSED", function() Ch.OnLootClosed() end)
