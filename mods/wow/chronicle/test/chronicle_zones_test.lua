-- Chronicle by zone (docs/ryna-agent-cores-mods.md 7.6) against a fake client (no game needed): loads
-- the real core addon and Chronicle over fake_client.lua, logs in, then counts kills, loot and objects
-- by zone (A, B, A gives { A = 2, B = 1 }; a kill whose name came late counts where it died; loot where
-- the window opened; the eight-zone cap keeps the biggest; nothing with the `chronicle` setting off),
-- the lifetime `places` map (time on zone changes and at logout, quests and deaths, seeded once from the
-- journal), the record carried over at login, the tooltip line, and the window: the zone picker on
-- Kills and Loot with its tiles, Earlier travels, and the Zones section. From the repository root,
-- with Lua 5.1 (what the game runs) or later:
--
--   lua5.1 mods/wow/chronicle/test/chronicle_zones_test.lua
-- luacheck: ignore 1

local CORE = "cores/wow/addon/Ryna/"
local CHRON = "mods/wow/chronicle/addon/Ryna_Chronicle/"
local F = dofile("mods/wow/chronicle/test/fake_client.lua")
local plain = F.Plain

local failures, checks = 0, 0
local function check(cond, what)
  checks = checks + 1
  if not cond then
    failures = failures + 1
    print("FAIL: " .. what)
  end
end
local function eq(got, want, what)
  check(got == want, what .. ": got " .. tostring(got) .. ", want " .. tostring(want))
end

F.Install()

local ME = "Player-91-0000000A"
local HOUR = 3600
local DAY = 86400

-- The world: the zone the client reports, the target, the loot window.
local zone = "Elwynn Forest"
local target = nil -- { guid, name, dead }
local lootSlots = {} -- { { link, name, qty, source } }

_G.UnitGUID = function(unit)
  if unit == "player" then return ME end
  if unit == "target" and target then return target.guid end
end
_G.UnitName = function(unit)
  if unit == "player" then return "Brennic" end
  if unit == "target" and target then return target.name end
end
_G.UnitIsDead = function(unit) return unit == "target" and target and target.dead or false end
_G.UnitIsTapDenied = function() return false end
_G.UnitFullName = function(unit) if unit == "player" then return "Brennic", "Ashgrove" end end
_G.GetNormalizedRealmName = function() return "91" end
_G.UnitClass = function() return "Paladin", "PALADIN" end
_G.UnitRace = function() return "Human", "Human" end
_G.UnitFactionGroup = function() return "Alliance" end
_G.UnitSex = function() return 3 end
_G.UnitLevel = function() return 17 end
_G.UnitXP = function() return 1200 end
_G.UnitXPMax = function() return 4800 end
_G.GetMoney = function() return 123456 end
_G.GetRealZoneText = function() return zone end
_G.GetSubZoneText = function() return "" end
_G.IsInInstance = function() return false, "none" end
_G.GetInventoryItemLink = function() return nil end
_G.GetNumGroupMembers = function() return 0 end
_G.COMBATLOG_XPGAIN_FIRSTPERSON = "%s dies, you gain %d experience."
_G.GetNumLootItems = function() return #lootSlots end
_G.GetLootSlotType = function() return 1 end
_G.GetLootSlotLink = function(i) return lootSlots[i] and lootSlots[i].link end
_G.GetLootSlotInfo = function(i)
  local s = lootSlots[i]
  if s then return nil, s.name, s.qty, nil, 1 end
end
_G.GetLootSourceInfo = function(i)
  local s = lootSlots[i]
  if s then return s.source, s.qty end
end

local function link(id, name)
  return "|cnIQ1:|Hitem:" .. id .. "::::::::17:1487::::::|h[" .. name .. "]|h|r"
end
local function creature(id, spawn) return "Creature-0-1-0-1-" .. id .. "-" .. (spawn or "0000AAAA") end

-- The saved file: last time's sessions in the journal (for the places seed), and the record's
-- backfill stamp from the agent, which the login must keep.
local S1, S2 = "1799000000-0000000A", "1799100000-0000000A"
_G.RynaDB = {
  schema = 1,
  settings = {},
  journal = {
    { session = S1, seq = 1, at = F.now - 2 * DAY, char = ME, type = "session", kind = "login", zone = "Westfall" },
    { session = S1, seq = 2, at = F.now - 2 * DAY, char = ME, type = "zone", zone = "Westfall" },
    { session = S1, seq = 3, at = F.now - 2 * DAY + 600, char = ME, type = "questDone", questId = 1,
      zone = "Westfall" },
    { session = S1, seq = 4, at = F.now - 2 * DAY + HOUR, char = ME, type = "zone", zone = "Duskwood" },
    { session = S1, seq = 5, at = F.now - 2 * DAY + HOUR + 60, char = ME, type = "death", zone = "Duskwood" },
    { session = S1, seq = 6, at = F.now - 2 * DAY + 2 * HOUR, char = ME, type = "session", kind = "logout" },
    -- A session the game closed without a logout: it ends at its last event.
    { session = S2, seq = 1, at = F.now - DAY, char = ME, type = "session", kind = "login", zone = "Westfall" },
    { session = S2, seq = 2, at = F.now - DAY + 1800, char = ME, type = "questDone", questId = 2,
      zone = "Westfall" },
    -- Another character's: not this one's places.
    { session = "x", seq = 1, at = F.now - DAY, char = "Player-91-0000000B", type = "zone",
      zone = "Stranglethorn Vale" },
  },
  characters = {
    [ME] = { guid = ME, zonesBackfill = { at = F.now - HOUR, files = 3, placed = 12 } },
  },
}

local TOC = {}
_G.C_AddOns = { GetAddOnMetadata = function(name, field) if field == "Version" then return TOC[name] end end }

local core = {}
F.LoadAddon(CORE, "Ryna", core)
F.Fire("ADDON_LOADED", "Ryna")
local ns = {}
F.LoadAddon(CHRON, "Ryna_Chronicle", ns)
F.Fire("ADDON_LOADED", "Ryna_Chronicle")
F.Fire("PLAYER_LOGIN")
F.Fire("PLAYER_ENTERING_WORLD", true, false)
F.DropTimers()

local Ch = ns.Chronicle
local rec = ns.rec
check(rec ~= nil, "logged in")

-- The record at login: the agent's backfill stamp is kept. -------------------------------------------
eq(type(rec.zonesBackfill) == "table" and rec.zonesBackfill.placed, 12, "the login keeps the agent's zonesBackfill")
eq(rec.zonesSince, nil, "no zone counted yet: no zonesSince")

-- Places seeded from the journal. ------------------------------------------------------------------
local places = rec.places
eq(rec.placesSince, F.now - 2 * DAY, "placesSince is the oldest journal event read")
eq(places.Westfall and places.Westfall.seconds, HOUR + 1800, "Westfall: an hour, then half an hour to the last event")
eq(places.Westfall and places.Westfall.quests, 2, "two quests turned in in Westfall")
eq(places.Duskwood and places.Duskwood.seconds, HOUR, "Duskwood: until the logout")
eq(places.Duskwood and places.Duskwood.deaths, 1, "a death in Duskwood")
eq(places.Westfall and places.Westfall.first, F.now - 2 * DAY, "Westfall first seen")
eq(places.Westfall and places.Westfall.last, F.now - DAY + 1800, "Westfall last seen")
eq(places["Stranglethorn Vale"], nil, "another character's zones are not this one's")
check(places["Elwynn Forest"] ~= nil, "the zone of this login is a place too")
ns.Places.Seed(rec)
eq(places.Westfall.seconds, HOUR + 1800, "the seed runs once (placesSince set)")

-- Kills: A, B, A gives { A = 2, B = 1 }. --------------------------------------------------------------
local function xpKill(name, xp)
  F.clock = F.clock + 10 -- apart from the last signal's window
  F.Fire("CHAT_MSG_COMBAT_XP_GAIN", name .. " dies, you gain " .. xp .. " experience.")
end
local function moveTo(z)
  zone = z
  F.now = F.now + 600
  F.Fire("ZONE_CHANGED_NEW_AREA")
end
target = { guid = creature(6), name = "Kobold Vermin" }
F.Fire("PLAYER_TARGET_CHANGED") -- learns its id
target = nil
xpKill("Kobold Vermin", 40)
eq(rec.zonesSince, F.now, "the first zone counted stamps zonesSince")
local since = rec.zonesSince
moveTo("Westfall")
xpKill("Kobold Vermin", 40)
moveTo("Elwynn Forest")
xpKill("Kobold Vermin", 40)
local e = rec.kills["6"]
eq(e.n, 3, "three kills")
eq(e.zones["Elwynn Forest"], 2, "A, B, A: two in Elwynn Forest")
eq(e.zones.Westfall, 1, "and one in Westfall")
eq(e.zone, "Elwynn Forest", "zone is still the latest")
eq(rec.zonesSince, since, "zonesSince is stamped once")

-- A kill waiting for its name counts where it died, even after the player moves on. -----------------
xpKill("Defias Thug", 100)
check(rec.kills["38"] == nil, "not counted yet: its id isn't known")
moveTo("Westfall")
target = { guid = creature(38), name = "Defias Thug" }
F.Fire("PLAYER_TARGET_CHANGED")
target = nil
local thug = rec.kills["38"]
eq(thug and thug.n, 1, "counted once its id is seen")
eq(thug and thug.zones["Elwynn Forest"], 1, "in Elwynn Forest, where it died")
eq(thug and thug.zones.Westfall, nil, "not in Westfall, where its name was learned")
eq(thug and thug.zone, "Elwynn Forest", "its latest zone is where it died")

-- The target's death counts in the zone the player is in. -----------------------------------------
target = { guid = creature(38, "0000BBBB"), name = "Defias Thug" }
F.Fire("PLAYER_TARGET_CHANGED")
target.dead = true
F.clock = F.clock + 10
F.Fire("UNIT_HEALTH", "target")
target = nil
eq(thug.zones.Westfall, 1, "a target's death in Westfall")

-- Loot: the zone when the window opened. -------------------------------------------------------------
lootSlots = {
  { link = link(2589, "Linen Cloth"), name = "Linen Cloth", qty = 2, source = creature(38, "0000CCCC") },
  { link = link(2589, "Linen Cloth"), name = "Linen Cloth", qty = 3, source = "GameObject-0-1-0-1-1731-0000DDDD" },
}
F.Fire("LOOT_OPENED")
moveTo("Duskwood")
F.Fire("LOOT_SLOT_CLEARED", 1)
F.Fire("LOOT_SLOT_CLEARED", 2)
F.Fire("LOOT_CLOSED")
local cloth = rec.loot["2589"]
eq(cloth.n, 2, "looted twice")
eq(cloth.zones.Westfall and cloth.zones.Westfall.n, 2, "both in Westfall, where the window opened")
eq(cloth.zones.Westfall and cloth.zones.Westfall.qty, 5, "five pieces there")
eq(cloth.zones.Duskwood, nil, "nothing in Duskwood, where the slots were taken")
eq(rec.objects["1731"].zones.Westfall, 1, "the object opened counts in Westfall too")

-- The cap: eight zones, keeping the biggest. -----------------------------------------------------------
local capped = { n = 20 }
for i = 1, 8 do Ch.AddZone(capped, "Zone " .. i, i == 1 and 1 or 2) end
check(Ch.Capped(capped), "eight zones: at the cap")
Ch.AddZone(capped, "Zone 9", 1)
local count = 0
for _ in pairs(capped.zones) do count = count + 1 end
eq(count, 8, "a ninth zone: still eight")
eq(capped.zones["Zone 1"], nil, "the smallest went")
eq(capped.zones["Zone 9"], 1, "the new one stays")
Ch.AddZone(capped, "Zone 9", 1)
eq(capped.zones["Zone 9"], 2, "an existing zone just counts")
eq(Ch.Earlier(capped), 20 - 16, "what no zone holds is the earlier part (the dropped zone's kill with it)")
local lootCap = { n = 10, qty = 30 }
for i = 1, 9 do Ch.AddZone(lootCap, "Zone " .. i, 1, 3) end
count = 0
for _ in pairs(lootCap.zones) do count = count + 1 end
eq(count, 8, "loot keeps eight zones too")
local earlierN, earlierQty = Ch.Earlier(lootCap)
eq(earlierN .. "/" .. earlierQty, "2/6", "loot's earlier part: drops and pieces")

-- Reading by zone: zones and logZones. -------------------------------------------------------------------
local old = { n = 43, zones = { Westfall = 12, ["Elwynn Forest"] = 20 }, logZones = { Westfall = 4, Duskwood = 3 } }
local n, _, fromLogs = Ch.InZone(old, "Westfall")
eq(n .. "/" .. fromLogs, "16/4", "a zone's count is live plus placed from logs")
eq(Ch.Zones(old).Duskwood, 3, "a zone only the logs placed is a zone")
eq(Ch.Earlier(old), 43 - 39, "earlier: n less zones and logZones")

-- The setting off: nothing counts. ---------------------------------------------------------------------
RynaDB.settings.chronicle = false
local before = { n = e.n, ez = e.zones["Elwynn Forest"], places = places.Duskwood.seconds }
xpKill("Kobold Vermin", 40)
lootSlots = { { link = link(2589, "Linen Cloth"), name = "Linen Cloth", qty = 1, source = creature(6, "0000EEEE") } }
F.Fire("LOOT_OPENED")
F.Fire("LOOT_SLOT_CLEARED", 1)
F.Fire("LOOT_CLOSED")
F.Fire("QUEST_TURNED_IN", 5, 100, 10)
moveTo("Westfall")
eq(e.n, before.n, "setting off: no kill")
eq(e.zones["Elwynn Forest"], before.ez, "setting off: no zone")
eq(cloth.n, 2, "setting off: no loot")
eq(places.Duskwood.seconds, before.places, "setting off: no time")
RynaDB.settings.chronicle = nil

-- Places live: time on each zone change and at logout, quests and deaths where they happened. ----------
local wfBefore = places.Westfall.seconds
moveTo("Redridge Mountains") -- 600 seconds in Westfall
eq(places.Westfall.seconds, wfBefore + 600, "ten minutes in Westfall, added on leaving")
eq(places["Redridge Mountains"].first, F.now, "a new zone: first seen now")
F.Fire("QUEST_TURNED_IN", 6, 100, 10)
F.Fire("PLAYER_DEAD")
eq(places["Redridge Mountains"].quests, 1, "a quest turned in in Redridge")
eq(places["Redridge Mountains"].deaths, 1, "and a death")
eq((ns.Places.Current()), "Redridge Mountains", "the stay under way")
F.now = F.now + 300
local _, extra = ns.Places.Current()
eq(extra, 300, "five minutes not yet added")
F.Fire("PLAYER_LOGOUT")
eq(places["Redridge Mountains"].seconds, 300, "the logout adds the stay")
eq(places["Redridge Mountains"].last, F.now, "last seen at the logout")

-- The tooltip line: "12 here (Westfall) · 43 in all", only when it says something. ---------------------
local T = ns.Tooltip
zone = "Westfall"
rec.zone = "Westfall"
eq(T.ZoneLine(e, e.n), "1 here (Westfall) · 3 in all", "two zones: the line")
rec.zone = "Elwynn Forest"
local single = { n = 5, zones = { ["Elwynn Forest"] = 5 } }
eq(T.ZoneLine(single, 5), nil, "one zone, and you're in it: no line")
rec.zone = "Westfall"
eq(T.ZoneLine(single, 5), "0 here (Westfall) · 5 in all", "one zone, and you're elsewhere: the line")
eq(T.ZoneLine({ n = 5 }, 5), nil, "no zones at all: no line")
local lines = T.UnitLines(creature(6))
eq(lines[#lines], "1 here (Westfall) · 3 in all", "the creature's tooltip carries it")
lines = T.ItemLines("2589")
eq(#lines, 1, "an item looted only here (one zone, and you're in it): no zone line")
rec.zone = "Elwynn Forest"
lines = T.ItemLines("2589")
eq(lines[#lines], "0 here (Elwynn Forest) · 2 in all", "an item looted elsewhere than here")

-- The window: the zone picker on Kills. -----------------------------------------------------------------
local R = _G.Ryna
local Wn = core.Window
rec.zone = "Westfall"
-- An older kill, counted before zones were kept, partly placed from the logs by the agent.
rec.kills["40"] = { name = "Gnoll", n = 10, own = 10, xp = 500, last = F.now - DAY, zone = "Westfall",
  zones = { Westfall = 2 }, logZones = { Westfall = 3, Redridge = 1 } }
Ch.killsVersion = Ch.killsVersion + 1
R.Window.Open("kills")
local f = Wn.frame
check(f.filter:IsShown(), "Kills shows the zone picker")
local choices = Wn.FilterChoices(Wn.sections.kills, Wn.ctx)
eq(choices[1].label, "All zones", "first: All zones")
eq(choices[1].value, nil, "the default")
eq(choices[2].label, "This zone (Westfall)", "then This zone")
eq(choices[#choices].label, "Earlier travels", "Earlier travels last")
local labels = {}
for _, c in ipairs(choices) do labels[#labels + 1] = c.label end
eq(table.concat(labels, ", "), "All zones, This zone (Westfall), Westfall, Elwynn Forest, Redridge, Earlier travels",
  "the zones with kills, most first")
eq(f.tiles[2].value:GetText(), "15", "All zones: every kill (3 + 2 + 10)")

R.Window.SetFilter("kills", "Westfall")
eq(Wn.ctx.filter, "Westfall", "the picked zone is in the context")
eq(plain(f.filter:GetText()), "Westfall", "the picker reads it")
eq(f.rows[1].cols[1]:GetText() .. " " .. f.rows[1].cols[2]:GetText() .. " " .. f.rows[1].cols[3]:GetText(),
  "Gnoll 5 10", "Westfall's kills (2 live, 3 from logs), and in all")
eq(f.headCells[3]:GetText(), "In all", "the third column is the kills in all zones")
eq(f.tiles[1].value:GetText() .. "/" .. f.tiles[2].value:GetText(), "3/7", "tiles follow: 3 creatures, 7 kills")
eq(f.tiles[3].label:GetText(), "From your combat logs", "and the log-placed part")
eq(f.tiles[3].value:GetText(), "3", "three from the logs")
check(f.footer:GetText():find("Counted by zone since", 1, true) == 1, "the footer says since when")

R.Window.SetFilter("kills", ns.Window.HERE)
eq(#Wn.views.kills.sorted, 3, "This zone: Westfall's three creatures")
rec.zone = "Elwynn Forest"
Ch.killsVersion = Ch.killsVersion + 1
R.Window.Refresh()
eq(#Wn.views.kills.sorted, 2, "This zone follows the player: Elwynn Forest's two")

R.Window.SetFilter("kills", ns.Window.EARLIER)
eq(f.rows[1].cols[1]:GetText() .. " " .. f.rows[1].cols[2]:GetText(), "Gnoll 4", "Earlier travels: what no zone holds")
eq(#Wn.views.kills.sorted, 1, "only creatures with an earlier part")

-- The creature tooltip's zones, and Earlier travels explained.
f.rows[1]._scripts.OnEnter(f.rows[1])
local tips = table.concat(GameTooltip:Texts(), "\n")
check(tips:find("By zone", 1, true) ~= nil, "the tooltip lists the zones")
check(tips:find("Earlier travels", 1, true) ~= nil, "and Earlier travels")
check(tips:find("counted before Ryna kept zones", 1, true) ~= nil, "explained")
f.rows[1]._scripts.OnLeave(f.rows[1])

R.Window.SetFilter("kills", nil)
eq(f.headCells[3]:GetText(), "Experience", "All zones: Experience again")
eq(f.tiles[3].label:GetText(), "Experience", "and its tile")

-- Loot follows its own picker.
R.Window.Open("loot")
check(f.filter:IsShown(), "Loot shows the zone picker")
R.Window.SetFilter("loot", "Westfall")
eq(f.rows[1].cols[2]:GetText() .. " " .. f.rows[1].cols[3]:GetText(), "2 5", "Westfall's drops and pieces")
R.Window.SetFilter("loot", "Duskwood")
eq(#Wn.views.loot.sorted, 0, "nothing looted in Duskwood")
R.Window.SetFilter("loot", nil)

-- The Zones section. ---------------------------------------------------------------------------------------
R.Window.Open("zones")
check(not f.filter:IsShown(), "Zones has no zone picker")
local byZone = {}
for _, row in ipairs(Wn.views.zones.sorted) do byZone[row.ref] = row end
local wf = byZone.Westfall
check(wf ~= nil, "a Westfall row")
eq(table.concat(wf.cells, "|"), "Westfall|7|3|2|5|2|0|" .. ns.Session.Duration(places.Westfall.seconds),
  "Westfall: kills, creatures, drops, pieces, quests, deaths, time")
eq(byZone.Duskwood.cells[7], "1", "Duskwood's death")
eq(byZone.Redridge.cells[2], "1", "a zone only the logs placed kills in")
local earlierRow = byZone[ns.Window.EARLIER]
check(earlierRow ~= nil, "an Earlier travels row")
eq(Wn.views.zones.sorted[#Wn.views.zones.sorted], earlierRow, "last")
eq(earlierRow.cells[1] .. " " .. earlierRow.cells[2], "Earlier travels 4", "with the kills no zone holds")
eq(f.tiles[1].label:GetText(), "Zones", "tiles: zones")
check(f.footer:GetText():find("Time, quests and deaths since", 1, true) == 1, "the footer says since when")

-- A row opens Kills on its zone.
local wfIndex
for i, row in ipairs(Wn.views.zones.sorted) do if row == wf then wfIndex = i end end
f.rows[wfIndex]._scripts.OnEnter(f.rows[wfIndex])
tips = table.concat(GameTooltip:Texts(), "\n")
check(tips:find("Click to see its kills.", 1, true) ~= nil, "the row says what a click does")
f.rows[wfIndex]._scripts.OnLeave(f.rows[wfIndex])
F.Click(f.rows[wfIndex])
eq(Wn.Selected(), "kills", "a click opens Kills")
eq(Wn.ctx.filter, "Westfall", "on that zone")

-- The picker's own list on this client (no modern menu): it lists the choices and picks one.
F.Click(f.filter)
check(f.filter.list:IsShown(), "the zone picker's list opens")
F.Click(f.filter.list.buttons[1])
eq(Wn.ctx.filter, nil, "All zones picked from the list")

check(#F.chat == 0, "nothing in chat: " .. table.concat(F.chat, " / "))
check(core.errors.count == 0, "no errors caught: " .. tostring(core.errors.last))

print(string.format("chronicle_zones: %d checks, %d failed", checks, failures))
os.exit(failures == 0 and 0 or 1)
