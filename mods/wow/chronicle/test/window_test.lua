-- Ryna's window (the core's Window.lua) and Chronicle's sections on it, against a fake client (no
-- game needed): loads the real core addon and Chronicle in their TOC order over fake_client.lua, logs
-- in a character with two more on the account in the saved file, then drives the window: section
-- registration and the rail, the virtual list, sorting, the character picker and the search context
-- (and when a section's rows are rebuilt), Session, Kills, Loot and History as the Chronicle window
-- had them, Characters and Items, the tooltip line naming the characters that hold an item (and its
-- guard against doubling), /chronicle, the strip's click, and the window's remembered place and
-- section. From the repository root, with Lua 5.1 (what the game runs) or later:
--
--   lua5.1 mods/wow/chronicle/test/window_test.lua
--
-- This proves the logic; how the frames look in the default UI and in ElvUI is the game's to show.
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

-- The character you're on, and two more on the account (the saved file).
local ME = "Player-91-0000000A"
local ALT = "Player-91-0000000B"
local ALT2 = "Player-91-0000000C"
_G.UnitGUID = function(unit) if unit == "player" then return ME end end
_G.UnitName = function(unit) if unit == "player" then return "Brennic" end end
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
_G.GetRealZoneText = function() return "Westfall" end
_G.GetSubZoneText = function() return "Sentinel Hill" end
_G.GetInventoryItemLink = function() return nil end

local function link(id, name, q)
  return "|cnIQ" .. (q or 1) .. ":|Hitem:" .. id .. "::::::::17:1487::::::|h[" .. name .. "]|h|r"
end

local DAY = 86400
_G.RynaDB = {
  schema = 1,
  settings = {},
  journal = {
    { type = "level", char = ME, level = 16, at = F.now - 2 * DAY, playedTotal = 30000 },
    { type = "level", char = ME, level = 17, at = F.now - DAY, playedTotal = 36000 },
  },
  characters = {
    [ALT] = {
      guid = ALT, fullName = "Alyra-Dawnmere", firstName = "Alyra", lastName = "Dawnmere", class = "MAGE",
      level = 22, xp = 100, xpMax = 1000, gold = 500000, playedTotal = 90000, playedLevel = 2000,
      lastSeen = F.now - 2 * DAY, zone = "Stormwind City", bankAsOf = F.now - 3 * DAY,
      professions = { { name = "Tailoring", rank = 120, max = 150 } },
      reputations = { { name = "Stormwind", standing = 5, value = 3000, max = 6000 } },
      gear = { { slot = 7, itemId = 2569, link = link(2569, "Linen Boots", 2), quality = 2 },
        { slot = 1, itemId = 1234, link = link(1234, "Linen Hood", 1), quality = 1 } },
      containers = {
        { container = "bank", itemId = 2589, count = 120, link = link(2589, "Linen Cloth"), quality = 1 },
      },
      kills = { ["6"] = { name = "Kobold Vermin", n = 50, own = 50, xp = 2000, last = F.now - 2 * DAY } },
      loot = {},
    },
    [ALT2] = {
      guid = ALT2, fullName = "Alyra-Stone", firstName = "Alyra", lastName = "Stone", class = "WARRIOR",
      level = 5, gold = 300, lastSeen = F.now - 10 * DAY,
      containers = {
        { container = "bag", itemId = 2589, count = 5, link = link(2589, "Linen Cloth"), quality = 1 },
        { container = "bank", itemId = 2589, count = 10, link = link(2589, "Linen Cloth"), quality = 1 },
        { container = "bag", itemId = 6948, count = 1, link = link(6948, "Hearthstone"), quality = 1 },
      },
    },
  },
}

-- The client's bags for the character you're on: 34 Linen Cloth and a sword.
local BAGS = { [0] = { [1] = { itemID = 2589, stackCount = 20, quality = 1, hyperlink = link(2589, "Linen Cloth") },
  [2] = { itemID = 2589, stackCount = 14, quality = 1, hyperlink = link(2589, "Linen Cloth") },
  [3] = { itemID = 2140, stackCount = 1, quality = 2, hyperlink = link(2140, "Carving Knife", 2) } } }
_G.C_Container = {
  GetContainerNumSlots = function(bag) return BAGS[bag] and 4 or 0 end,
  GetContainerItemInfo = function(bag, slot) return BAGS[bag] and BAGS[bag][slot] end,
}

-- What the client says each addon's TOC holds (## Version), the one place a version is written.
local function tocVersion(folder, name)
  return assert(io.open(folder .. name .. ".toc")):read("*a"):match("## Version:%s*(%S+)")
end
local TOC = { Ryna = tocVersion(CORE, "Ryna"), Ryna_Chronicle = tocVersion(CHRON, "Ryna_Chronicle") }
_G.C_AddOns = { GetAddOnMetadata = function(name, field) if field == "Version" then return TOC[name] end end }

-- Load the core, then Chronicle, and log in.
local core = {}
F.LoadAddon(CORE, "Ryna", core)
F.Fire("ADDON_LOADED", "Ryna")
local ns = {}
F.LoadAddon(CHRON, "Ryna_Chronicle", ns)
F.Fire("ADDON_LOADED", "Ryna_Chronicle")
F.Fire("PLAYER_LOGIN")
F.Fire("PLAYER_ENTERING_WORLD", true, false)
F.DropTimers() -- the deferred login scans: not this test's
ns.Containers.Scan()

local R = _G.Ryna
local Wn = core.Window
check(type(R.Window) == "table" and type(R.Window.AddSection) == "function", "the core exposes Ryna.Window")
check(type(R.Tooltip) == "table" and type(R.Tooltip.AddLine) == "function", "the core exposes Ryna.Tooltip")
check(ns.Window.available, "Chronicle sees the window")

-- Each addon's version is its TOC's, with no copy in Lua to fall behind it (the core's sat at 0.3.2
-- while the TOC said 0.3.9), and it is the number RynaDB hands the companion app.
check(type(TOC.Ryna) == "string" and TOC.Ryna:match("^%d+%.%d+%.%d+"), "the core's TOC has a version")
eq(core.VERSION, TOC.Ryna, "the core's version is its TOC's")
eq(R.VERSION, TOC.Ryna, "Ryna.VERSION is the TOC's")
eq(RynaDB.addon, TOC.Ryna, "RynaDB.addon is the TOC's")
eq(ns.VERSION, TOC.Ryna_Chronicle, "Chronicle's version is its TOC's")

-- Registration and the rail. ------------------------------------------------------------------------
local function rows() return {} end
local cols = { { title = "A", width = 100 } }
check(not R.Window.AddSection(nil), "no spec: refused")
check(not R.Window.AddSection({ group = "me", rows = rows, columns = cols }), "no key: refused")
check(not R.Window.AddSection({ key = "x", group = "elsewhere", rows = rows, columns = cols }),
  "unknown group: refused")
check(not R.Window.AddSection({ key = "x", group = "me", columns = cols }), "no rows: refused")
check(not R.Window.AddSection({ key = "x", group = "me", rows = rows }), "no columns: refused")
check(not R.Window.AddSection({ key = "x", group = "me", rows = rows, columns = cols, characters = "some" }),
  "an unknown picker mode: refused")
check(not R.Window.AddSection({ key = "x", group = "me", rows = rows, columns = cols, fill = 3 }),
  "a fill that isn't a function: refused")
check(not R.Window.AddSection({ key = "session", group = "me", rows = rows, columns = cols }), "a key taken: refused")
local probeBuilds = 0
check(R.Window.AddSection({ key = "probe", group = "combat", label = "Probe", order = 5, columns = cols,
  rows = function(ctx)
    probeBuilds = probeBuilds + 1
    return { { cells = { "probe for " .. tostring(ctx.character) } } }
  end }), "a section in Combat and gold: added")

local order = {}
for _, s in ipairs(Wn.List()) do order[#order + 1] = s.key end
eq(table.concat(order, " "), "session characters items kills loot history probe",
  "rail order: Me by order, then combat")

-- /chronicle opens the window on Session.
SlashCmdList.RYNACHRONICLE("")
local f = Wn.frame
check(f and f:IsShown(), "/chronicle opens the window")
eq(Wn.Selected(), "session", "on Session")
eq(f.title:GetText(), "Session", "the header's title")
local inSpecial = false
for _, name in ipairs(UISpecialFrames) do if name == "RynaWindow" then inSpecial = true end end
check(inSpecial, "Escape closes it (UISpecialFrames)")
check(f.groupHeads.me:IsShown() and f.groupHeads.combat:IsShown(), "the Me and Combat and gold headings are drawn")
check(not f.groupHeads.world:IsShown(), "World has no section: its heading isn't drawn")
eq(f.groupHeads.combat:GetText(), "COMBAT AND GOLD", "the group heading's text")
check(f.railButtons.session._locked and not f.railButtons.kills._locked, "the rail marks the section on screen")
check(not f.picker:IsEnabled(), "Session is the character you're on: the picker is greyed")
eq(f.search:GetAlpha(), 0.4, "Session has no search: the box is greyed")

-- Session. ----------------------------------------------------------------------------------------
local S = ns.Session
S.Kill("6", "Kobold Vermin", 40, 1)
S.Kill("6", "Kobold Vermin", 40, 1)
S.Kill(nil, "Defias Thug", 765, 1)
S.Settle("Defias Thug", "38")
_G.UnitXP = function() return 2045 end
F.Fire("PLAYER_XP_UPDATE")
check(#F.timers == 1, "a burst of changes: one redraw queued")
F.RunTimers()
eq(f.tiles[1].label:GetText(), "Kills", "tile 1 is kills")
eq(f.tiles[1].value:GetText(), "3", "three kills this session")
eq(f.tiles[2].value:GetText(), "845", "the session's experience")
eq(f.tiles[4].label:GetText(), "Since", "tile 4: since when")
eq(f.rows[1].cols[1]:GetText(), "Levels gained", "the first stat row")
eq(plain(f.rows[6].cols[1]:GetText()), "Kills this session", "the heading over the kills")
eq(f.rows[7].cols[1]:GetText() .. " " .. f.rows[7].cols[2]:GetText(), "Kobold Vermin 2", "the top creature")
eq(f.rows[8].cols[1]:GetText(), "Defias Thug", "the second, settled to its id")
check(not f.rows[9]:IsShown(), "no more rows")
eq(f.footer:GetText(), "Counted in game since you logged in; a /reload keeps it.", "Session's footer")

-- Kills: the lifetime record, a search, and the creature tooltip. ----------------------------------
local rec = ns.rec
rec.kills["6"] = { name = "Kobold Vermin", n = 2, own = 2, xp = 80, zone = "Elwynn Forest", last = F.now - 60,
  looted = 2 }
rec.kills["38"] = { name = "Defias Thug", n = 1, own = 1, xp = 765, zone = "Westfall", last = F.now - 30, looted = 1 }
rec.loot["2589"] = { name = "Linen Cloth", quality = 1, n = 2, qty = 3, last = F.now - 20,
  link = link(2589, "Linen Cloth"),
  from = { ["c:6"] = 2 } }
ns.Chronicle.killsVersion = ns.Chronicle.killsVersion + 1
ns.Chronicle.lootVersion = (ns.Chronicle.lootVersion or 0) + 1
F.Click(f.railButtons.kills)
eq(Wn.Selected(), "kills", "the rail opens Kills")
eq(f.rows[1].cols[1]:GetText() .. " " .. f.rows[1].cols[2]:GetText() .. " " .. f.rows[1].cols[3]:GetText(),
  "Kobold Vermin 2 80", "most kills first")
eq(f.tiles[1].value:GetText() .. "/" .. f.tiles[2].value:GetText(), "2/3", "tiles: creatures and kills")
check(f.picker:IsEnabled(), "Kills follows the picker")
eq(plain(f.picker:GetText()), "Brennic-Ashgrove", "All characters means the one you're on, for Kills")
local builds = Wn.views.kills.builds
Wn.Draw()
Wn.Draw()
eq(Wn.views.kills.builds, builds, "redraws with nothing changed don't rebuild the rows")
Wn.Search("  THUG ")
eq(Wn.views.kills.builds, builds + 1, "a search rebuilds them")
eq(#Wn.views.kills.sorted, 1, "one creature matches")
eq(f.rows[1].cols[1]:GetText(), "Defias Thug", "the match")
f.search:SetText("vermin")
Wn.Draw()
eq(f.search:GetText(), "vermin", "a redraw before the search runs leaves the typing alone")
F.RunTimers()
eq(f.rows[1].cols[1]:GetText(), "Kobold Vermin", "typing in the box searches (debounced)")
Wn.Search("")
f.rows[1]._scripts.OnEnter(f.rows[1])
local tips = GameTooltip:Texts()
eq(tips[1], "Kobold Vermin", "the creature tooltip's title")
local drops = false
for _, t in ipairs(tips) do if t == "Drops" then drops = true end end
check(drops, "the creature tooltip lists its drops")
f.rows[1]._scripts.OnLeave(f.rows[1])

-- The picker: another character's kills, from the saved file.
R.Window.Choose(ALT)
eq(f.rows[1].cols[1]:GetText() .. " " .. f.rows[1].cols[2]:GetText(), "Kobold Vermin 50", "Alyra-Dawnmere's kills")
check(f.footer:GetText():find("From your saved file: Alyra%-Dawnmere as of 2 days ago") ~= nil,
  "the footer says where and when")
eq(Wn.views.kills.builds, builds + 4, "a new character rebuilds the rows")
R.Window.Choose(nil)
eq(Wn.ctx.character, ME, "back to All: Kills shows the one you're on")

-- Sorting by a heading: its first direction, the other, then the section's own order.
Wn.SortBy(1)
eq(f.rows[1].cols[1]:GetText(), "Defias Thug", "by name, A to Z")
eq(f.headCells[1]:GetText(), "Creature ^", "the heading shows the sort")
Wn.SortBy(1)
eq(f.rows[1].cols[1]:GetText(), "Kobold Vermin", "by name, Z to A")
Wn.SortBy(1)
eq(f.headCells[1]:GetText(), "Creature", "the third click: the section's own order")
Wn.SortBy(3)
eq(f.rows[1].cols[1]:GetText(), "Defias Thug", "experience, most first")
Wn.SortBy(3)
Wn.SortBy(3)

-- The virtual list: 2,002 creatures, only the rows on screen filled.
for i = 1, 2000 do
  rec.kills["9" .. i] = { name = "Mob " .. i, n = 2 + i, own = 0, xp = i, zone = "Duskwood", last = F.now }
end
ns.Chronicle.killsVersion = ns.Chronicle.killsVersion + 1
R.Window.Refresh("kills")
eq(#Wn.views.kills.sorted, 2002, "2,002 creatures listed")
local filled = 0
for _ in pairs(Wn.views.kills.filled) do filled = filled + 1 end
eq(filled, Wn.visible, "only the rows on screen were filled")
Wn.Draw()
local again = 0
for _ in pairs(Wn.views.kills.filled) do again = again + 1 end
eq(again, Wn.visible, "a redraw in the same minute fills nothing again")
local fillsBefore = Wn.views.kills.filled
F.clock = F.clock + 60
Wn.Draw()
check(Wn.views.kills.filled ~= fillsBefore, "a minute later the rows on screen are filled afresh (ages move on)")
F.clock = F.clock - 60
check(Wn.visible >= 15 and Wn.visible <= 20, "15 to 20 rows on screen (" .. tostring(Wn.visible) .. ")")
eq(f.rows[1].cols[1]:GetText(), "Mob 2000", "most first")
f.list._scripts.OnMouseWheel(f.list, -1)
eq(f.rows[1].cols[1]:GetText(), "Mob 1997", "the wheel scrolls three rows")
f.bar:SetValue(5000)
eq(Wn.state.offset, 2002 - Wn.visible, "the bar's end clamps to the last screenful")
eq(f.rows[Wn.visible].cols[1]:GetText(), "Defias Thug", "fewest kills last")
for i = 1, 2000 do rec.kills["9" .. i] = nil end
ns.Chronicle.killsVersion = ns.Chronicle.killsVersion + 1

-- Loot: newest first, in the quality colour, with an icon; the game's tooltip and the sources.
SlashCmdList.RYNACHRONICLE("loot")
eq(Wn.Selected(), "loot", "/chronicle loot opens Loot")
eq(f.rows[1].cols[1]:GetText(), "|cffffffffLinen Cloth|r", "the item in its quality colour")
eq(f.rows[1].cols[2]:GetText() .. " " .. f.rows[1].cols[3]:GetText(), "2 3", "looted twice, three in all")
eq(f.rows[1].cols[4]:GetText(), "Kobold Vermin", "where it came from")
eq(f.rows[1].icon:GetTexture(), 102589, "its icon from C_Item.GetItemIconByID")
f.rows[1]._scripts.OnEnter(f.rows[1])
tips = GameTooltip:Texts()
eq(tips[1]:sub(1, 5), "link ", "the game's own item tooltip first")
eq(tips[#tips], "From: Kobold Vermin", "then where it came from")
Wn.SortBy(4)
check(Wn.state.sort.loot == nil, "From has no sort key: its heading doesn't sort")
check(not f.headButtons[4]:IsShown(), "and isn't clickable")
f.rows[1]._scripts.OnLeave(f.rows[1])

-- History (the Levels tab's levels): /chronicle levels still works.
SlashCmdList.RYNACHRONICLE("levels")
eq(Wn.Selected(), "history", "/chronicle levels opens History")
eq(f.rows[1].cols[1]:GetText() .. " " .. f.rows[1].cols[3]:GetText(), "16 1h 40m", "level 16, 6,000 s at it")
eq(f.rows[2].cols[1]:GetText(), "17", "the current level")
eq(f.tiles[1].value:GetText(), "17", "History's level tile")

-- Characters: every character on the account. ----------------------------------------------------
SlashCmdList.RYNACHRONICLE("characters")
eq(Wn.Selected(), "characters", "/chronicle characters")
eq(plain(f.picker:GetText()), "All characters", "Characters starts on All characters")
eq(#Wn.views.characters.sorted, 3, "three characters")
eq(plain(f.rows[1].cols[1]:GetText()), "Brennic-Ashgrove", "the one you're on first")
eq(f.rows[1].cols[1]:GetText():sub(1, 10), "|cfff58cba", "in its class colour")
local levelBar = f.rows[1].bars[2]
eq(levelBar.text:GetText() .. " " .. levelBar:GetValue() .. "/" .. select(2, levelBar:GetMinMaxValues()),
  "17 2045/4800", "the level over its experience bar")
eq(f.rows[1].cols[6]:GetText(), "now", "last seen: now")
eq(plain(f.rows[2].cols[1]:GetText()), "Alyra-Dawnmere", "then by name, hyphenated names whole")
eq(f.rows[2].cols[3]:GetText(), "50g", "its gold")
eq(f.rows[2].cols[4]:GetText(), "1d 1h", "its time played")
eq(f.rows[2].cols[5]:GetText(), "Tailoring 120", "its professions")
eq(f.rows[2].cols[6]:GetText(), "2 days ago", "when it was last seen")
eq(f.tiles[1].value:GetText(), "3", "tile: three characters")
eq(f.tiles[2].value:GetText(), "62g 37s 56c", "tile: their gold")
check(f.footer:GetText():find("saved file") ~= nil, "the footer names the saved file")
Wn.Search("stone")
eq(#Wn.views.characters.sorted, 1, "a search over the names")
Wn.Search("")
-- A click on a row picks that character: its detail.
F.Click(f.rows[2])
eq(Wn.ctx.character, ALT, "a click picks the character")
eq(plain(f.picker:GetText()), "Alyra-Dawnmere", "the picker says so")
local texts = {}
for _, r in ipairs(Wn.views.characters.sorted) do texts[#texts + 1] = plain((r.cells or {})[1]) end
eq(table.concat(texts, "|"),
  "Character|Class|Zone|Gear|Linen Hood|Linen Boots|Professions|Tailoring|Reputations|Stormwind",
  "the detail: character, gear by slot, professions, reputations")
local hood
for _, r in ipairs(Wn.views.characters.sorted) do if (r.cells or {})[1] == "Linen Hood" then hood = r end end
eq(hood and hood.cells[2], "Head", "the slot's name from the client")
local rep
for _, r in ipairs(Wn.views.characters.sorted) do if (r.cells or {})[1] == "Stormwind" then rep = r end end
eq(rep and rep.cells[2] .. " " .. rep.cells[3], "Friendly 3000 / 6000", "a reputation's standing and bar text")
eq(rep and rep.bars[3].r, 0, "in the standing's colour")
eq(f.tiles[1].value:GetText() .. " " .. f.tiles[1].sub:GetText(), "22 Mage", "the level tile")
Wn.Search("boots")
texts = {}
for _, r in ipairs(Wn.views.characters.sorted) do texts[#texts + 1] = plain(r.cells[1]) end
eq(table.concat(texts, "|"), "Gear|Linen Boots", "a search keeps only the matching rows and their heading")
Wn.Search("")

-- Items: every character's bags and bank. --------------------------------------------------------------
R.Window.Choose(nil)
SlashCmdList.RYNACHRONICLE("items")
eq(Wn.Selected(), "items", "/chronicle items")
local cloth
for _, r in ipairs(Wn.views.items.sorted) do if r.ref == "2589" then cloth = r end end
check(cloth ~= nil, "Linen Cloth is listed")
eq(cloth and (cloth.bags .. "/" .. cloth.bank), "39/130", "39 in bags (34 + 5), 130 in banks (120 + 10)")
eq(#Wn.views.items.sorted, 3, "three kinds of item across the account")
eq(f.tiles[1].value:GetText(), "3", "tile: kinds of item")
f.rows[1]._scripts.OnEnter(f.rows[1]) -- Carving Knife (sorted by name)
f.rows[1]._scripts.OnLeave(f.rows[1])
local clothRow
for r = 1, Wn.visible do if f.rows[r].data == cloth then clothRow = f.rows[r] end end
eq(clothRow and clothRow.cols[2]:GetText(), "169", "its total")
eq(clothRow and clothRow.cols[5]:GetText(), "Alyra-Dawnmere, Brennic, Alyra-Stone",
  "who holds it, most first; Brennic by first name, the two Alyras in full")
clothRow._scripts.OnEnter(clothRow)
local holderLines, heldBy = 0, false
for _, l in ipairs(GameTooltip.lines) do
  if l.right then holderLines = holderLines + 1 end
  if l.left and l.left:find("in the bank", 1, true) and not l.right then heldBy = true end
end
eq(holderLines, 3, "the tooltip lists each holder")
check(not heldBy, "inside Items, the holders line isn't added under the item too")
-- The client redraws a tooltip by itself once an item it hadn't loaded arrives, running the hooks again
-- outside the window's drawing: the tooltip's owner is still the window's row, so no holders line.
GameTooltip:ClearLines()
for _, pc in ipairs(F.postCalls) do if pc.kind == 0 then pc.fn(GameTooltip, { id = 2589 }) end end
local redrawn = false
for _, l in ipairs(GameTooltip.lines) do
  if l.left and l.left:find("in the bank", 1, true) then redrawn = true end
end
check(not redrawn, "the client's own redraw of Items' tooltip: still no holders line")
clothRow._scripts.OnLeave(clothRow)
GameTooltip.owner = nil
R.Window.Choose(ALT)
eq(#Wn.views.items.sorted, 1, "one character's items only")
eq(f.rows[1].cols[4]:GetText(), "120", "in its bank")
eq(f.tiles[4].value:GetText(), "3 days ago", "when its bank was read")
R.Window.Choose(nil)

-- The tooltip line: the other characters holding an item, on any item tooltip. ----------------------
local function itemTooltip(id)
  GameTooltip:ClearLines()
  GameTooltip:SetHyperlink(link(id, "Item " .. id))
  return GameTooltip:Texts()
end
local function holdersLine(lines)
  for _, l in ipairs(lines) do
    if l:find("in the bank", 1, true) or l:find("Alyra", 1, true) then return plain(l) end
  end
end
eq(holdersLine(itemTooltip(2589)), "Alyra-Dawnmere: 120 in the bank · Alyra-Stone: 15 (10 in the bank)",
  "the other characters, most first; full names where two share a first name")
local n = #GameTooltip.lines
for _, pc in ipairs(F.postCalls) do if pc.kind == 0 then pc.fn(GameTooltip, { id = 2589 }) end end
eq(#GameTooltip.lines, n, "the hook run again adds nothing twice")
eq(holdersLine(itemTooltip(2140)), nil, "an item only the character you're on holds: no line")
eq(holdersLine(itemTooltip(6948)), "Alyra-Stone: 1", "a bags-only holder: just the count")
RynaDB.settings.tooltipHolders = false
eq(holdersLine(itemTooltip(2589)), nil, "the line's own setting off: no line")
RynaDB.settings.tooltipHolders = true
RynaDB.settings.tooltips = false
eq(holdersLine(itemTooltip(2589)), nil, "Chronicle's tooltips off: no line")
RynaDB.settings.tooltips = true
eq(RynaDB.settings.tooltipHolders, true, "the line's setting defaults to on")

-- Rebuilds follow the version: a new bag scan changes Items.
builds = Wn.views.items.builds
Wn.Draw()
eq(Wn.views.items.builds, builds, "no scan: no rebuild")
BAGS[0][4] = { itemID = 4306, stackCount = 2, quality = 1, hyperlink = link(4306, "Silk Cloth") }
ns.Containers.Scan()
check(#F.timers == 1, "a bag scan while the window is open queues a redraw")
F.RunTimers()
eq(Wn.views.items.builds, builds + 1, "a scan rebuilds Items, without anything else happening")
eq(#Wn.views.items.sorted, 4, "the new item is there")

-- Characters redraws when a scan changes a record, with nothing else happening.
F.Click(f.railButtons.characters)
builds = Wn.views.characters.builds
ns.Character.ScanReputations()
check(#F.timers == 1, "a reputation scan queues a redraw")
F.RunTimers()
eq(Wn.views.characters.builds, builds + 1, "and Characters is rebuilt")

-- A section whose rows() fails is tried again on the next draw, not left empty.
local failOnce = true
check(R.Window.AddSection({ key = "flaky", group = "world", label = "Flaky", columns = cols,
  version = function() return 1 end,
  rows = function()
    if failOnce then
      failOnce = false
      error("not yet")
    end
    return { { cells = { "here now" } } }
  end }), "a flaky section: added")
local errorsBefore = core.errors.count
SlashCmdList.RYNACHRONICLE("") -- closes
R.Window.Open("flaky")
eq(core.errors.count, errorsBefore + 1, "its first build failed (caught)")
core.errors.count = errorsBefore
Wn.Draw()
eq(f.rows[1].cols[1]:GetText(), "here now", "the next draw builds it")
check(f.groupHeads.world:IsShown(), "World is drawn now that it has a section")
F.chat = {}

-- The probe section in Combat and gold: picker context passed through.
F.Click(f.railButtons.probe)
eq(f.rows[1].cols[1]:GetText(), "probe for nil", "an 'all' section gets character nil for All characters")
R.Window.Choose(ALT2)
eq(f.rows[1].cols[1]:GetText(), "probe for " .. ALT2, "and the picked character's GUID")
local p = probeBuilds
Wn.Draw()
eq(probeBuilds, p, "a section without a version: rebuilt only when the context changes")
R.Window.Choose(nil)

-- The plain picker's list (this fake client has no modern dropdown).
F.Click(f.picker)
check(f.picker.list:IsShown(), "the picker opens its list")
eq(plain(f.picker.list.buttons[1].text:GetText()), "> All characters", "All characters, chosen")
eq(plain(f.picker.list.buttons[2].text:GetText()), "  Brennic-Ashgrove", "then each character, yours first")
Wn.Draw()
check(f.picker.list:IsShown(), "a redraw (a kill, say) leaves the list open")
F.Click(f.picker.list.buttons[3])
eq(Wn.ctx.character, ALT, "a choice in the list picks the character")
check(not f.picker.list:IsShown(), "and closes it")
R.Window.Choose(nil)

-- Place and last section, remembered; closing and opening again.
f._scripts.OnDragStop(f)
f._points = { { "TOPLEFT", UIParent, "BOTTOMLEFT", 300.4, 600.6 } }
f._scripts.OnDragStop(f)
local saved = RynaDB.settings.window
eq(saved.point .. " " .. saved.x .. " " .. saved.y, "TOPLEFT 300 601", "where it was dragged to")
eq(saved.section, "probe", "the last section")
f:Hide()
check(not R.Window.IsShown(), "closed")
R.Window.Open()
eq(Wn.Selected(), "probe", "opened again on the section it was on")
eq(f._points[1][1] .. " " .. f._points[1][4], "TOPLEFT 300", "in its place")
Wn.state.section = nil
f:Hide()
R.Window.Open()
eq(Wn.Selected(), "probe", "a fresh session's window opens on the remembered section")

-- The strip's click opens the window on Session; a second closes it.
f:Hide()
F.Click(ns.Strip.frame, "LeftButton")
check(f:IsShown(), "the strip's click opens the window")
eq(Wn.Selected(), "session", "on Session")
F.Click(ns.Strip.frame, "LeftButton")
check(not f:IsShown(), "a second click closes it")
-- /ryna chronicle is the same command.
core.Slash("chronicle kills")
eq(Wn.Selected(), "kills", "/ryna chronicle kills")
RynaDB.settings = { minimap = RynaDB.settings.minimap, window = RynaDB.settings.window }
core.ApplySettingDefaults(RynaDB.settings)
eq(type(RynaDB.settings.window), "table", "the window's settings survive the defaults")

check(#F.chat == 0, "nothing in chat: " .. table.concat(F.chat, " / "))
check(core.errors.count == 0, "no errors caught: " .. tostring(core.errors.last))

-- A client that can't read the TOC: "unknown" (the app's own word), never nil in a format call.
-- Last, because it loads Core.lua a second time.
_G.C_AddOns = nil
local bare = {}
assert(loadfile(CORE .. "Core.lua"))("Ryna", bare)
eq(bare.VERSION, "unknown", "no metadata call: the version is unknown")
eq(core.Status.CompareVersions("9.9.9", bare.VERSION), nil, "unknown compares as no version, so no update line")
print(string.format("window: %d checks, %d failed", checks, failures))
os.exit(failures == 0 and 0 or 1)
