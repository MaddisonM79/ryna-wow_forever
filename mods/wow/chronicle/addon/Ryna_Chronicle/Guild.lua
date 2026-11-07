-- Guild: the roster of the player's guild in RynaDB.guilds, keyed by "<name>-<realmId>" (no
-- guild GUID API is known on this client; docs/wow.md section 1 allows the name key). The
-- roster is requested on login and every 10 minutes; online members also become `seen` events
-- (source `roster`), at most once per refresh window.

local _, ns = ...
if ns.off then return end

local G = {}
ns.Guild = G

local NonEmpty = ns.NonEmpty
local REFRESH_SECONDS = 600
local ROSTER_SEEN_SECONDS = 540

function G.Key(name)
  local realmId = ns.rec and ns.rec.realmId
  return string.format("%s-%s", tostring(name), tostring(realmId or 0))
end

function G.Request()
  if not ns.Try(ns.Fn("IsInGuild")) then
    ns.Debug("guild roster: not in a guild, nothing to request")
    return
  end
  local modern = ns.Fn("C_GuildInfo", "GuildRoster")
  if modern then
    ns.Try(modern)
  else
    ns.Try(ns.Fn("GuildRoster"))
  end
  ns.Debug("guild roster: requested")
end

local function canViewOfficerNote()
  local f = ns.Fn("C_GuildInfo", "CanViewOfficerNote") or ns.Fn("CanViewOfficerNote")
  return f and ns.Try(f) and true or false
end

-- Approximate: the client reports years, months, days and hours since last online.
local function lastOnlineAt(index, now)
  local years, months, days, hours = ns.Try(ns.Fn("GetGuildRosterLastOnline"), index)
  if not (years or months or days or hours) then return nil end
  local h = ((((years or 0) * 12 + (months or 0)) * 30 + (days or 0)) * 24) + (hours or 0)
  return now - h * 3600
end

function G.ScanRoster()
  local rec = ns.rec
  if not rec then return end
  ns.Character.UpdateGuild()
  local guild = rec.guild
  if not guild then return end
  local total = ns.Try(ns.Fn("GetNumGuildMembers"))
  local info = ns.Fn("GetGuildRosterInfo")
  if type(total) ~= "number" or total < 1 or not info then
    ns.Debug("guild roster: not loaded yet")
    return
  end

  local now = ns.Now()
  local officer = canViewOfficerNote()
  local roster, online = {}, {}
  for i = 1, total do
    local name, rankName, rankIndex, level, _, zone, note, officerNote, isOnline, _, class,
      _, _, _, _, _, guid = ns.Try(info, i)
    name = NonEmpty(name)
    if name then
      guid = NonEmpty(guid)
      local row = {
        fullName = name, guid = guid, level = level, class = NonEmpty(class),
        rankIndex = rankIndex, rankName = NonEmpty(rankName), zone = NonEmpty(zone),
        note = NonEmpty(note), officerNote = officer and NonEmpty(officerNote) or nil,
        online = isOnline and true or false,
        lastOnlineAt = (not isOnline) and lastOnlineAt(i, now) or nil,
      }
      roster[#roster + 1] = row
      if row.online then online[#online + 1] = row end
    end
  end

  ns.db.guilds[guild.guid] = {
    guid = guild.guid, name = guild.name, realmId = rec.realmId, rosterAsOf = now, roster = roster,
  }

  ns.Debug("guild roster: read %d members, %d online (%s)", #roster, #online, guild.name)

  -- Online members become `seen` events only while the census setting is on.
  if not ns.Setting("census") then
    if #online > 0 then ns.Debug("census off: not recording the %d online guild members", #online) end
  elseif G.rosterSeenAt and now - G.rosterSeenAt < ROSTER_SEEN_SECONDS then
    ns.Debug("census: roster sightings skipped, last taken %s ago (at most every 9 minutes)",
      ns.DurationText(now - G.rosterSeenAt))
  else
    G.rosterSeenAt = now
    local names = {}
    for _, row in ipairs(online) do
      local race
      if row.guid then race = select(4, ns.Try(ns.Fn("GetPlayerInfoByGUID"), row.guid)) end
      local e = ns.Sightings.Seen("roster", {
        guid = row.guid, fullName = row.fullName, level = row.level, class = row.class,
        race = NonEmpty(race), guild = guild.name, zone = row.zone,
      }, true)
      if e then names[#names + 1] = row.fullName end
    end
    if #names > 0 then ns.Debug("census: seen %d online on the guild roster: %s", #names, ns.ListText(names)) end
  end
end

function G.StartTicker()
  if G.ticker or not (_G.C_Timer and _G.C_Timer.NewTicker) then return end
  G.ticker = _G.C_Timer.NewTicker(REFRESH_SECONDS, function() ns.Run("guild-ticker", G.Request) end)
end

ns.On("GUILD_ROSTER_UPDATE", function() ns.Debounce("roster", 2, G.ScanRoster) end)
