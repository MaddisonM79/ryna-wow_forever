-- Sightings: `seen` journal events (other players this character saw) and `group` events.
-- Gated by RynaDB.settings: `census` (off: no `seen` events at all), `chatSightings` (chat) and
-- `groupMembers` (group members, and the `group` event's member list).
--
-- Privacy: a chat message yields only who sent it: the sender's GUID and name, plus class and
-- race where the client's GUID cache has them. The message text, the channel and the recipient
-- are never read into a variable, stored, buffered or printed; the chat handler takes only the
-- sender (argument 2) and the GUID (argument 12) and discards the rest.

local _, ns = ...

local S = {}
ns.Sightings = S

local NonEmpty = ns.NonEmpty
local CHAT_WINDOW = 600 -- seconds: one chat sighting per player per 10 minutes

-- info: { guid, fullName, level, class, race, guild, zone }
-- Nothing is written while the census setting is off, whatever the source (chat, group, roster).
-- quiet: no debug line per sighting (group and roster print one summary). Returns the event.
function S.Seen(source, info, quiet)
  if not info then return nil end
  if not ns.Setting("census") then
    ns.Debug("census off: not recording %s", tostring(NonEmpty(info.fullName) or info.guid))
    return nil
  end
  local guid = NonEmpty(info.guid)
  if guid and guid == ns.playerGuid then return nil end
  if not guid and not NonEmpty(info.fullName) then return nil end
  return ns.Journal.Add("seen", {
    guid = guid, fullName = NonEmpty(info.fullName), level = info.level, class = info.class,
    race = info.race, guild = info.guild, zone = info.zone, source = source,
  }, quiet)
end

local function guidInfo(guid)
  local _, class, _, race = ns.Try(ns.Fn("GetPlayerInfoByGUID"), guid)
  return NonEmpty(class), NonEmpty(race)
end

-- Chat: last-seen times per GUID in memory only (never saved), pruned as it grows.
local lastChat, lastChatCount = {}, 0

-- Debug only: a skipped chat sighting is noted once per sender per window, not per message.
local skipNoted, skipNotedCount = {}, 0
local function noteSkip(guid, now)
  local t = skipNoted[guid]
  if t and now - t < CHAT_WINDOW then return false end
  if skipNotedCount > 2000 then skipNoted, skipNotedCount = {}, 0 end
  if not t then skipNotedCount = skipNotedCount + 1 end
  skipNoted[guid] = now
  return true
end

local function onChat(_, ...)
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
  local last = lastChat[guid]
  if last and now - last < CHAT_WINDOW then
    if ns.Debugging() and noteSkip(guid, now) then
      ns.Debug("census: %s already seen in chat %s, skipped until 10 minutes have passed",
        tostring(NonEmpty(sender) or guid), ns.DurationText(now - last) .. " ago")
    end
    return
  end
  if not last then lastChatCount = lastChatCount + 1 end
  lastChat[guid] = now
  if lastChatCount > 2000 then
    local kept, n = {}, 0
    for g, t in pairs(lastChat) do
      if now - t < CHAT_WINDOW then
        kept[g] = t
        n = n + 1
      end
    end
    lastChat, lastChatCount = kept, n
  end
  local class, race = guidInfo(guid)
  S.Seen("chat", { guid = guid, fullName = NonEmpty(sender), class = class, race = race })
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
