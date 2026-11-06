-- Sightings: `seen` journal events (other players this character saw) and `group` events.
-- Gated by RynaDB.settings: `census` (off: no `seen` events at all), `chatSightings` (chat),
-- `groupMembers` (group members, and the `group` event's member list) and `nearbyPlayers`
-- (players whose nameplate, target or mouseover the client shows).
--
-- Privacy: a chat message yields only who sent it and, for local chat, where: the sender's GUID and
-- name, class and race where the client's GUID cache has them, and the observer's own zone for
-- say, yell, emotes and the zone's General channel. The message text and the recipient are never
-- read into a variable, stored, buffered or printed. The channel's id and base name are read only
-- to decide whether the zone applies, and are never stored or printed.

local _, ns = ...

local S = {}
ns.Sightings = S

local NonEmpty = ns.NonEmpty
local WINDOW = 600 -- seconds: one sighting per player per 10 minutes, whatever the source

-- The last sighting per player, shared by every source: { at = GetTime(), level = n or nil }, keyed
-- by GUID and also by "name:<First-Last>" (a /who row has no GUID, so its key is the name; a
-- sighting with both is remembered under both, so /who skips someone chat or a nameplate just
-- saw). In memory only (never saved), pruned as it grows. Chat, nearby and /who sightings within
-- the window are skipped, except that one with a level is recorded after one without.
local last, lastCount = {}, 0

local function remember(guid, level)
  local now = GetTime()
  if not last[guid] then lastCount = lastCount + 1 end
  last[guid] = { at = now, level = level }
  if lastCount > 2000 then
    local kept, n = {}, 0
    for g, l in pairs(last) do
      if now - l.at < WINDOW then
        kept[g] = l
        n = n + 1
      end
    end
    last, lastCount = kept, n
  end
end

-- The earlier sighting of guid (or a name key) within the window, or nil.
local function recent(guid, now)
  local l = last[guid]
  if l and now - l.at < WINDOW then return l end
  return nil
end

local function nameKey(fullName)
  local name = ns.CanonicalName(NonEmpty(fullName))
  return name and ("name:" .. name) or nil
end

-- The earlier sighting of a player by name (First-Last) within the window, or nil. For /who rows.
function S.RecentByName(fullName)
  local key = nameKey(fullName)
  return key and recent(key, GetTime()) or nil
end

-- True when fullName is the player's own character (a /who answer lists you too).
function S.IsSelf(fullName)
  local mine = ns.rec and ns.CanonicalName(NonEmpty(ns.rec.fullName))
  return mine ~= nil and ns.CanonicalName(NonEmpty(fullName)) == mine
end

-- info: { guid, fullName, level, class, race, guild, zone, batch, faction }
-- Nothing is written while the census setting is off, whatever the source (chat, group, roster).
-- quiet: no debug line per sighting (group and roster print one summary). Returns the event.
-- batch is the /who sweep's id, carried only by `who` sightings. faction ("Horde" or "Alliance")
-- is carried only by nearby sightings, the one source that sees the other faction; chat, group,
-- roster and /who sightings are the observer's own faction by construction and leave it out.
function S.Seen(source, info, quiet)
  if not info then return nil end
  if not ns.Setting("census") then
    ns.Debug("census off: not recording %s", tostring(NonEmpty(info.fullName) or info.guid))
    return nil
  end
  local guid = NonEmpty(info.guid)
  if guid and guid == ns.playerGuid then return nil end
  if not guid and not NonEmpty(info.fullName) then return nil end
  if not guid and S.IsSelf(info.fullName) then return nil end
  local e = ns.Journal.Add("seen", {
    guid = guid, fullName = ns.CanonicalName(NonEmpty(info.fullName)), level = info.level, class = info.class,
    race = info.race, guild = info.guild, zone = info.zone, source = source, batch = info.batch,
    faction = info.faction,
  }, quiet)
  if e then
    if guid then remember(guid, info.level) end
    local key = nameKey(info.fullName)
    if key then remember(key, info.level) end
  end
  return e
end

local function guidInfo(guid)
  local _, class, _, race = ns.Try(ns.Fn("GetPlayerInfoByGUID"), guid)
  return NonEmpty(class), NonEmpty(race)
end

-- Debug only: a skipped sighting is noted once per player per window, not per message or event.
local skipNoted, skipNotedCount = {}, 0
local function noteSkip(guid, now)
  local t = skipNoted[guid]
  if t and now - t < WINDOW then return false end
  if skipNotedCount > 2000 then skipNoted, skipNotedCount = {}, 0 end
  if not t then skipNotedCount = skipNotedCount + 1 end
  skipNoted[guid] = now
  return true
end

-- Chat that is heard only nearby, or only in the observer's zone, gets the observer's zone. The
-- General channel is zone-bound (zone channel id 1, base name "General"). Trade, LocalDefense and
-- LookingForGroup would need a list of cities to tell, which the addon doesn't keep, so they get
-- no zone; nor do whispers, guild, party, raid and instance chat (group members come from the
-- group scan, with their zones).
local LOCAL_CHAT = {
  CHAT_MSG_SAY = true, CHAT_MSG_YELL = true, CHAT_MSG_EMOTE = true, CHAT_MSG_TEXT_EMOTE = true,
}
local function chatZone(event, ...)
  if LOCAL_CHAT[event] then return ns.rec.zone end
  if event ~= "CHAT_MSG_CHANNEL" then return nil end
  local zoneChannel = select(7, ...)
  if type(zoneChannel) == "number" and not ns.IsSecret(zoneChannel) and zoneChannel == 1 then
    return ns.rec.zone
  end
  if NonEmpty(select(9, ...)) == "General" then return ns.rec.zone end
  return nil
end

local function onChat(event, ...)
  if not ns.rec then return end
  local recording = ns.Setting("census") and ns.Setting("chatSightings")
  if not recording and not ns.Debugging() then return end
  local sender = select(2, ...)
  local guid = select(12, ...)
  if ns.IsSecret(guid) or ns.IsSecret(sender) then return end
  if type(guid) ~= "string" or not guid:find("^Player%-") or guid == ns.playerGuid then return end
  local now = GetTime()
  if not recording then
    if noteSkip(guid, now) then
      ns.Debug("%s: not recording %s from chat",
        ns.Setting("census") and "chat sightings off" or "census off", tostring(NonEmpty(sender) or guid))
    end
    return
  end
  local earlier = recent(guid, now)
  if earlier then
    if ns.Debugging() and noteSkip(guid, now) then
      ns.Debug("census: %s already seen %s ago, skipped until 10 minutes have passed",
        tostring(NonEmpty(sender) or guid), ns.DurationText(now - earlier.at))
    end
    return
  end
  local class, race = guidInfo(guid)
  S.Seen("chat", {
    guid = guid, fullName = NonEmpty(sender), class = class, race = race, zone = chatZone(event, ...),
  })
end

local CHAT_EVENTS = {
  "CHAT_MSG_SAY", "CHAT_MSG_YELL", "CHAT_MSG_EMOTE", "CHAT_MSG_TEXT_EMOTE", "CHAT_MSG_WHISPER",
  "CHAT_MSG_CHANNEL", "CHAT_MSG_GUILD", "CHAT_MSG_OFFICER", "CHAT_MSG_PARTY", "CHAT_MSG_PARTY_LEADER",
  "CHAT_MSG_RAID", "CHAT_MSG_RAID_LEADER", "CHAT_MSG_RAID_WARNING", "CHAT_MSG_INSTANCE_CHAT",
  "CHAT_MSG_INSTANCE_CHAT_LEADER",
}
for _, event in ipairs(CHAT_EVENTS) do ns.On(event, onChat) end

-- Group: the member set (GUIDs) is compared with the last one; any change writes a `group`
-- event (joined, changed or left) and a `seen` event for every member.
local function fullNameOf(unit)
  local name, realm = ns.Try(ns.Fn("UnitFullName"), unit)
  name, realm = NonEmpty(name), NonEmpty(realm)
  if not name then name = NonEmpty(ns.Try(UnitName, unit)) end
  if name and realm then return name .. "-" .. realm end
  return name
end

local function groupUnits()
  local units = {}
  local inRaid = ns.Try(ns.Fn("IsInRaid"))
  if inRaid then
    local n = ns.Try(ns.Fn("GetNumGroupMembers")) or ns.Try(ns.Fn("GetNumRaidMembers")) or 0
    for i = 1, n do units[#units + 1] = { unit = "raid" .. i, raidIndex = i } end
  else
    local n = ns.Try(ns.Fn("GetNumSubgroupMembers")) or ns.Try(ns.Fn("GetNumPartyMembers")) or 0
    for i = 1, n do units[#units + 1] = { unit = "party" .. i } end
  end
  return units
end

function S.ScanGroup()
  local rec = ns.rec
  if not rec then return end
  local members, infos, set = {}, {}, {}
  for _, u in ipairs(groupUnits()) do
    local guid = NonEmpty(ns.Try(UnitGUID, u.unit))
    if guid and guid ~= ns.playerGuid and not set[guid] then
      set[guid] = true
      members[#members + 1] = guid
      local zone
      if u.raidIndex then
        zone = NonEmpty(select(7, ns.Try(ns.Fn("GetRaidRosterInfo"), u.raidIndex)))
      elseif ns.Try(ns.Fn("UnitIsVisible"), u.unit) then
        zone = rec.zone
      end
      infos[#infos + 1] = {
        guid = guid, fullName = fullNameOf(u.unit), level = ns.Try(UnitLevel, u.unit),
        class = NonEmpty(select(2, ns.Try(UnitClass, u.unit))),
        race = NonEmpty(select(2, ns.Try(UnitRace, u.unit))),
        guild = NonEmpty(ns.Try(ns.Fn("GetGuildInfo"), u.unit)),
        zone = zone,
      }
    end
  end
  table.sort(members)

  local prev = S.groupMembers or {}
  local changed = #prev ~= #members
  if not changed then
    for i = 1, #members do
      if prev[i] ~= members[i] then
        changed = true
        break
      end
    end
  end
  if not changed then
    ns.DebugChanged("group", "group: %d others in the group, no change", #members)
    return
  end
  S.groupMembers = members

  -- With census or "record other players in my group" off, the `group` event still says that
  -- you joined, changed or left a group, but lists no members and no `seen` events are written.
  local kind = (#prev == 0 and "joined") or (#members == 0 and "left") or "changed"
  local others = ns.Setting("census") and ns.Setting("groupMembers")
  ns.Journal.Add("group", { kind = kind, members = others and members or nil })
  if not others then
    if #members > 0 then
      ns.Debug("%s: not recording the %d others in the group",
        ns.Setting("census") and "group members off" or "census off", #members)
    end
    return
  end
  local names = {}
  for _, info in ipairs(infos) do
    if S.Seen("group", info, true) then names[#names + 1] = info.fullName or info.guid end
  end
  if #names > 0 then ns.Debug("census: seen %d in your group: %s", #names, ns.ListText(names)) end
end

ns.On("GROUP_ROSTER_UPDATE", function() ns.Debounce("group", 1, S.ScanGroup) end)

-- Nearby: players whose nameplate appears, whom you target, or whom you mouse over. Each gives
-- the client's level, class, race and guild, and the observer's zone; the shared 10-minute
-- window applies (a sighting with a level is still recorded after one without). Players of both
-- factions are recorded, each with the `faction` the client gives (UnitFactionGroup's English
-- token, "Horde" or "Alliance"), which the API prefers over the observer's faction.
--
-- Debug: a city can bring dozens of nameplates in seconds, so nearby sightings print at most one
-- line per 5 seconds: the sighting itself when it is the only one, else a count (with the names
-- when there are 5 or fewer) and how many were skipped.
local SUMMARY_SECONDS = 5
local burst = { seen = 0, skipped = 0, names = {}, line = nil, printedAt = nil, pending = false }

local function flushBurst()
  burst.pending = false
  local total = burst.seen + burst.skipped
  if total == 0 then return end
  burst.printedAt = GetTime()
  if total == 1 then
    ns.Debug("%s", burst.line)
  else
    local line = string.format("census: %d nearby players seen", burst.seen)
    if burst.seen > 0 and burst.seen <= 5 then line = line .. ": " .. table.concat(burst.names, ", ") end
    if burst.skipped > 0 then
      line = line .. string.format(", %d skipped (already seen in the last 10 minutes)", burst.skipped)
    end
    ns.Debug("%s", line)
  end
  burst.seen, burst.skipped, burst.names, burst.line = 0, 0, {}, nil
end

local function noteNearby(line, name)
  if name then
    burst.seen = burst.seen + 1
    if #burst.names < 6 then burst.names[#burst.names + 1] = name end
  else
    burst.skipped = burst.skipped + 1
  end
  burst.line = line
  if burst.pending then return end
  local wait = burst.printedAt and (burst.printedAt + SUMMARY_SECONDS - GetTime()) or 0
  if wait <= 0 then
    flushBurst()
  else
    burst.pending = true
    ns.After(wait, "census nearby", flushBurst)
  end
end

local function levelOf(unit)
  local level = ns.Try(UnitLevel, unit)
  if type(level) ~= "number" or ns.IsSecret(level) or level <= 0 then return nil end
  return level
end

-- The unit's faction as the English token: "Horde" or "Alliance", else nil (a neutral or unknown).
local FACTIONS = { Horde = true, Alliance = true }
local function factionOf(unit)
  local faction = NonEmpty(ns.Try(UnitFactionGroup, unit))
  return faction and FACTIONS[faction] and faction or nil
end

local function seeUnit(unit, source)
  local rec = ns.rec
  if not rec then return end
  if not (ns.Setting("census") and ns.Setting("nearbyPlayers")) then
    if ns.Debugging() then
      ns.DebugChanged("nearby", "%s: not recording nearby players",
        ns.Setting("census") and "nearby players off" or "census off")
    end
    return
  end
  if not ns.Try(UnitIsPlayer, unit) or ns.Try(UnitIsUnit, unit, "player") then return end
  local visible = ns.Fn("UnitIsVisible")
  if visible and not ns.Try(visible, unit) then return end
  local guid = ns.Try(UnitGUID, unit)
  if ns.IsSecret(guid) or type(guid) ~= "string" or not guid:find("^Player%-") or guid == ns.playerGuid then
    return
  end
  local faction = factionOf(unit)

  local now = GetTime()
  local earlier = recent(guid, now)
  local level = (not (earlier and earlier.level)) and levelOf(unit) or nil
  if earlier and not level then
    if ns.Debugging() and noteSkip(guid, now) then
      noteNearby(string.format("census: %s already seen %s ago, skipped until 10 minutes have passed (%s)",
        tostring(NonEmpty(ns.Try(UnitName, unit)) or guid), ns.DurationText(now - earlier.at), source))
    end
    return
  end

  local info = {
    guid = guid, fullName = ns.Character.Names(unit).fullName, level = level,
    class = NonEmpty(select(2, ns.Try(UnitClass, unit))),
    race = NonEmpty(select(2, ns.Try(UnitRace, unit))),
    guild = NonEmpty(ns.Try(ns.Fn("GetGuildInfo"), unit)),
    zone = rec.zone, faction = faction,
  }
  if S.Seen(source, info, true) and ns.Debugging() then
    local name = tostring(info.fullName or guid)
    noteNearby(string.format("census: seen %s level %s %s %s nearby (%s)%s", name, tostring(level or "?"),
      tostring(faction or "?"), tostring(info.class or "?"), source, earlier and ", now with a level" or ""),
      name)
  end
end

-- Registered from the UI-ready login handler (Init.lua), so login itself stays cheap.
function S.RegisterNearby()
  if S.nearbyRegistered then return end
  S.nearbyRegistered = true
  ns.On("NAME_PLATE_UNIT_ADDED", function(_, unit)
    if type(unit) == "string" then seeUnit(unit, "nameplate") end
  end)
  ns.On("PLAYER_TARGET_CHANGED", function() seeUnit("target", "target") end)
  ns.On("UPDATE_MOUSEOVER_UNIT", function() seeUnit("mouseover", "mouseover") end)
end

-- Friendly nameplates: the game draws nameplates for enemies only unless the player turns friendly
-- ones on (Shift+V, the client variable nameplateShowFriends), and the nearby source sees only the
-- nameplates the game draws. The panel offers a button; the addon never changes it by itself.
local FRIENDLY_CVAR = "nameplateShowFriends"

local function cvarGet()
  return ns.Fn("C_CVar", "GetCVar") or ns.Fn("GetCVar")
end

-- True when friendly nameplates are on, false when off, nil when the client can't say.
function S.FriendlyNameplatesOn()
  local get = cvarGet()
  if not get then return nil end
  local v = ns.Try(get, FRIENDLY_CVAR)
  if v == nil or ns.IsSecret(v) then return nil end
  return tostring(v) == "1"
end

-- Sets nameplateShowFriends to 1, from the panel's button only. Returns true when it is on after.
function S.ShowFriendlyNameplates()
  local set = ns.Fn("C_CVar", "SetCVar") or ns.Fn("SetCVar")
  if not set then
    ns.Debug("friendly nameplates: this client has no SetCVar")
    return false
  end
  local was = S.FriendlyNameplatesOn()
  if was then
    ns.Debug("friendly nameplates: already on")
    return true
  end
  local ok, err = pcall(set, FRIENDLY_CVAR, "1")
  local now = S.FriendlyNameplatesOn()
  if ok and now ~= false then
    ns.Debug("friendly nameplates: turned on (%s 1, was %s)", FRIENDLY_CVAR, tostring(was))
    return true
  end
  ns.Debug("friendly nameplates: the client refused to set %s: %s", FRIENDLY_CVAR,
    ok and "still off afterwards" or tostring(err))
  return false
end
