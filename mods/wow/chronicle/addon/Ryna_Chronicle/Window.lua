-- Window: Chronicle's sections of Ryna's window (the core's Ryna.Window, Window.lua there), under
-- Me. /chronicle, /ryna chronicle, a click on the session strip (Strip.lua) and the button on the
-- Recording tab open the window on Session.
--
--   Session     this session's numbers (Session.lua) as tiles and rows, then its kills by creature
--   Characters  Characters.lua
--   Items       Items.lua
--   Kills       a character's lifetime `kills`, most first; a row's tooltip lists that creature's
--               drops (from loot[*].from["c:<id>"] over kills[id].looted)
--   Loot        a character's lifetime `loot`, newest first, in the item's quality colour, with
--               where it came from
--   History     the journal's `level` events for a character, oldest first, with the time played
--               at each level (levels only for now)
--
-- Kills, Loot and History are one character's: the one the picker names, the one you're on when it
-- says All characters. Session is the one you're on. Rows are built light (ids and sort keys) and
-- their text is filled only for the rows on screen, so 2,000 creatures cost what 20 do. The window
-- redraws when Session.lua says something changed (at most every half second) and only while it is
-- shown; a section's rows are rebuilt only when its data's version changed.
--
-- With a core older than the window (0.3.9 and before have no Ryna.Window), none of this registers:
-- /chronicle says the core is too old, the strip's click does nothing, and the Recording tab's
-- button isn't there. Everything else in Chronicle works as before.

local _, ns = ...
if ns.off then return end
local L = ns.L

local Wn = {}
ns.Window = Wn

local RW = ns.API.Window
Wn.available = type(RW) == "table" and type(RW.AddSection) == "function"
Wn.keys = {} -- the sections Chronicle registered, by key

local MAX_DROPS = 12

local function num(v)
  if type(v) == "number" then return v end
  return tonumber(v) or 0
end

local function plain(s)
  return ns.Status.Plain(s) or s
end

local function when(t)
  if type(t) ~= "number" then return "" end
  return date("%Y-%m-%d %H:%M", t)
end

-- A character's record from the saved file (the one you're on is ns.rec, the same table).
function Wn.Record(guid)
  local db = ns.db
  local chars = db and db.characters
  local rec = type(chars) == "table" and guid ~= nil and chars[guid] or nil
  if type(rec) == "table" then return rec end
  return nil
end

local function recMap(rec, name)
  local m = type(rec) == "table" and rec[name]
  if type(m) == "table" then return m end
  return nil
end

-- A character's name as the window shows it.
function Wn.Name(rec)
  return plain(type(rec) == "table" and ns.NonEmpty(rec.fullName) or nil) or L["Unknown character"]
end

-- Where a character's own numbers come from, for a footer: live for the one you're on, else the
-- saved file as of its last logout.
function Wn.RecordFooter(ctx)
  local rec = Wn.Record(ctx.character)
  if not rec then return "" end
  if ctx.character == ctx.current then
    return string.format(L["Counted in game for %s, the character you're on."], Wn.Name(rec))
  end
  return string.format(L["From your saved file: %s as of %s, its last logout."], Wn.Name(rec),
    ns.Status.Ago(rec.lastSeen) or L["an unknown time"])
end

-- A loot source key as the Loot section names it: the creature's name, "object", "container", "fishing".
function Wn.SourceName(key, rec)
  if key == "f" then return L["fishing"] end
  local kind, id = tostring(key):match("^(%a):?(%d*)$")
  if kind == "c" then
    local kills = recMap(rec, "kills")
    local e = kills and kills[id]
    return (type(e) == "table" and ns.NonEmpty(e.name)) or string.format(L["creature %s"], id)
  elseif kind == "o" then
    return L["object"]
  elseif kind == "i" then
    return L["container"]
  end
  return tostring(key)
end

-- An entry's sources, most first: "Kobold Vermin, object".
local function sourcesText(from, max, rec)
  if type(from) ~= "table" then return "" end
  local best = {}
  for k, v in pairs(from) do
    local n = num(v)
    local pos = #best + 1
    while pos > 1 and num(from[best[pos - 1]]) < n do pos = pos - 1 end
    if pos <= max then
      table.insert(best, pos, k)
      best[max + 1] = nil
    end
  end
  for i, k in ipairs(best) do best[i] = plain(Wn.SourceName(k, rec)) end
  return table.concat(best, ", ")
end

-- Creature `id`'s drops from a record: { name, quality, n } most first, at most MAX_DROPS.
function Wn.Drops(id, rec)
  local list = {}
  local loot = recMap(rec, "loot")
  if not loot then return list end
  local key = "c:" .. tostring(id)
  for _, e in pairs(loot) do
    local n = type(e) == "table" and type(e.from) == "table" and num(e.from[key]) or 0
    if n > 0 then
      local pos = #list + 1
      while pos > 1 and list[pos - 1].n < n do pos = pos - 1 end
      if pos <= MAX_DROPS then
        table.insert(list, pos, { name = ns.NonEmpty(e.name) or "?", quality = e.quality, n = n })
        list[MAX_DROPS + 1] = nil
      end
    end
  end
  return list
end

-- What a creature's tooltip says: its numbers, then its drops with how often.
function Wn.CreatureTooltip(tip, id, rec)
  local kills = recMap(rec, "kills")
  local e = kills and kills[id]
  if type(e) ~= "table" then return end
  local S = ns.Session
  tip:AddLine(plain(ns.NonEmpty(e.name) or string.format(L["creature %s"], id)))
  tip:AddDoubleLine(L["Killed"], string.format(L["%s (%s yours)"], S.Number(e.n), S.Number(e.own)),
    0.8, 0.8, 0.8, 1, 1, 1)
  tip:AddDoubleLine(L["Experience"], S.Number(e.xp), 0.8, 0.8, 0.8, 1, 1, 1)
  if ns.NonEmpty(e.zone) then
    tip:AddDoubleLine(L["Last"], string.format(L["%s, %s"], plain(e.zone), ns.Status.Ago(e.last) or "?"),
      0.8, 0.8, 0.8, 1, 1, 1)
  end
  local looted = num(e.looted)
  tip:AddDoubleLine(L["Corpses looted"], S.Number(looted), 0.8, 0.8, 0.8, 1, 1, 1)
  if looted <= 0 then return end
  local drops = Wn.Drops(id, rec)
  if #drops == 0 then return end
  tip:AddLine(" ")
  tip:AddLine(L["Drops"])
  for _, d in ipairs(drops) do
    tip:AddDoubleLine(RW.QualityColor(d.quality) .. plain(d.name) .. "|r",
      string.format(L["%d of %d (%d%%)"], d.n, looted, math.floor(d.n * 100 / looted + 0.5)),
      1, 1, 1, 1, 1, 1)
  end
end

-- An item's sources under its own tooltip (the core drew the game's tooltip from the link).
function Wn.ItemTooltip(tip, id, rec, hadLink)
  local loot = recMap(rec, "loot")
  local e = loot and loot[id]
  if type(e) ~= "table" then return end
  if not hadLink then
    tip:AddLine(RW.QualityColor(e.quality) .. plain(ns.NonEmpty(e.name) or string.format(L["item %s"], id)) .. "|r")
  end
  tip:AddLine(" ")
  tip:AddLine(string.format(L["From: %s"], sourcesText(e.from, 8, rec)), 0.8, 0.8, 0.8, true)
end

-- The sections ------------------------------------------------------------------------------------

local function add(spec)
  if RW.AddSection(spec) then Wn.keys[spec.key] = true end
end

-- Session ------------------------------------------------------------------------------------------

local function sessionTiles()
  local s = ns.Session.Get()
  if not s then return nil end
  local S = ns.Session
  return {
    { label = L["Kills"], value = S.Number(s.kills), sub = string.format(L["%s/h"], S.Number(s.killsPerHour)) },
    { label = L["Experience"], value = S.Number(s.xp), sub = string.format(L["%s/h"], S.Number(s.xpPerHour)) },
    { label = L["Money"], value = S.Money(s.gold), sub = string.format(L["%s/h"], S.Money(s.goldPerHour)) },
    { label = L["Since"], value = date("%H:%M", s.startAt), sub = S.Duration(s.elapsed) },
  }
end

local function sessionRows()
  local rows = {}
  local s = ns.Session.Get()
  if not s then return rows end
  local S = ns.Session
  local function stat(label, value) rows[#rows + 1] = { cells = { label, value, "" } } end
  stat(L["Levels gained"], S.Number(s.levels))
  stat(L["Gained / spent"], string.format(L["%s / %s"], S.Money(s.goldGained), S.Money(s.goldSpent)))
  stat(L["Quests turned in"], S.Number(s.quests))
  stat(L["Items looted"], S.Number(s.items))
  stat(L["Deaths"], S.Number(s.deaths))
  rows[#rows + 1] = { header = true, cells = { L["Kills this session"], L["Kills"], L["Experience"] } }
  for _, k in ipairs(s.creatures) do
    rows[#rows + 1] = { ref = k.id or false, cells = { plain(k.name) or "?", S.Number(k.n), S.Number(k.xp) } }
  end
  return rows
end

-- Kills ----------------------------------------------------------------------------------------------

local killsTotals = {}

local function killsRows(ctx)
  local rows = {}
  local rec = Wn.Record(ctx.character)
  local kills = recMap(rec, "kills")
  killsTotals.rec, killsTotals.creatures, killsTotals.n, killsTotals.xp = rec, 0, 0, 0
  if not kills then return rows end
  for id, e in pairs(kills) do
    if type(e) == "table" and num(e.n) > 0 then
      killsTotals.creatures = killsTotals.creatures + 1
      killsTotals.n = killsTotals.n + num(e.n)
      killsTotals.xp = killsTotals.xp + num(e.xp)
      local name = ns.NonEmpty(e.name) or string.format(L["creature %s"], id)
      if RW.Match(name, ctx.search) then
        rows[#rows + 1] = { ref = id, rec = rec, e = e,
          sort = { name:lower(), num(e.n), num(e.xp), (ns.NonEmpty(e.zone) or ""):lower(), num(e.last) } }
      end
    end
  end
  table.sort(rows, function(a, b)
    if a.sort[2] ~= b.sort[2] then return a.sort[2] > b.sort[2] end
    return a.ref < b.ref
  end)
  return rows
end

local function killsFill(row)
  local e = row.e
  row.cells = {
    plain(ns.NonEmpty(e.name) or string.format(L["creature %s"], row.ref)), ns.Session.Number(e.n),
    ns.Session.Number(e.xp), plain(ns.NonEmpty(e.zone) or ""), ns.Status.Ago(e.last) or "",
  }
end

local function killsTiles(ctx)
  if killsTotals.rec ~= Wn.Record(ctx.character) then return nil end
  local S = ns.Session
  return {
    { label = L["Creatures"], value = S.Number(killsTotals.creatures) },
    { label = L["Kills"], value = S.Number(killsTotals.n) },
    { label = L["Experience"], value = S.Number(killsTotals.xp) },
  }
end

-- Loot ------------------------------------------------------------------------------------------------

local lootTotals = {}

local function lootRows(ctx)
  local rows = {}
  local rec = Wn.Record(ctx.character)
  local loot = recMap(rec, "loot")
  lootTotals.rec, lootTotals.items, lootTotals.n, lootTotals.qty = rec, 0, 0, 0
  if not loot then return rows end
  for id, e in pairs(loot) do
    if type(e) == "table" and num(e.n) > 0 then
      lootTotals.items = lootTotals.items + 1
      lootTotals.n = lootTotals.n + num(e.n)
      lootTotals.qty = lootTotals.qty + num(e.qty)
      local name = ns.NonEmpty(e.name) or string.format(L["item %s"], id)
      if RW.Match(name, ctx.search) then
        rows[#rows + 1] = { ref = id, rec = rec, e = e, quality = e.quality, link = ns.NonEmpty(e.link),
          sort = { name:lower(), num(e.n), num(e.qty), nil, num(e.last) } }
      end
    end
  end
  table.sort(rows, function(a, b)
    if a.sort[5] ~= b.sort[5] then return a.sort[5] > b.sort[5] end
    return a.ref < b.ref
  end)
  return rows
end

local function lootFill(row)
  local e = row.e
  row.icon = RW.ItemIcon(row.ref)
  row.cells = {
    plain(ns.NonEmpty(e.name) or string.format(L["item %s"], row.ref)), ns.Session.Number(e.n),
    ns.Session.Number(e.qty), sourcesText(e.from, 2, row.rec), ns.Status.Ago(e.last) or "",
  }
end

local function lootTiles(ctx)
  if lootTotals.rec ~= Wn.Record(ctx.character) then return nil end
  local S = ns.Session
  return {
    { label = L["Items"], value = S.Number(lootTotals.items) },
    { label = L["Looted"], value = S.Number(lootTotals.n) },
    { label = L["Count"], value = S.Number(lootTotals.qty) },
  }
end

-- History (levels) ------------------------------------------------------------------------------------

local function historyRows(ctx)
  local rows = {}
  local db, rec = ns.db, Wn.Record(ctx.character)
  if not db or not rec then return rows end
  local journal = type(db.journal) == "table" and db.journal or {}
  for i = 1, #journal do
    local e = journal[i]
    if type(e) == "table" and e.type == "level" and e.char == rec.guid and type(e.level) == "number" then
      rows[#rows + 1] = { level = e.level, at = e.at, playedTotal = num(e.playedTotal) }
    end
  end
  -- The time at each level: until the next level's played total; the current one from /played.
  for i = 1, #rows - 1 do
    local a, b = rows[i], rows[i + 1]
    if a.playedTotal > 0 and b.playedTotal > 0 then a.atLevel = b.playedTotal - a.playedTotal end
  end
  local atLevel = rec.playedLevel
  if rec == ns.rec then
    local _, now = ns.Character.PlayedNow()
    atLevel = now or atLevel
  end
  local n = #rows
  if n > 0 and rows[n].level == rec.level then
    rows[n].atLevel = atLevel
  elseif type(rec.level) == "number" then
    -- Reached before the journal's 30 days: the current level, with no date.
    rows[n + 1] = { level = rec.level, playedTotal = 0, atLevel = atLevel }
  end
  for _, r in ipairs(rows) do
    r.cells = {
      tostring(r.level), r.at and when(r.at) or L["before the journal"],
      r.atLevel and ns.Session.Duration(r.atLevel) or "",
      r.playedTotal > 0 and ns.Session.Duration(r.playedTotal) or "",
    }
    r.sort = { r.level, r.at or 0, r.atLevel or 0, r.playedTotal }
  end
  return rows
end

local function historyTiles(ctx)
  local rec = Wn.Record(ctx.character)
  if not rec then return nil end
  local total, atLevel = rec.playedTotal, rec.playedLevel
  if rec == ns.rec then
    local t, a = ns.Character.PlayedNow()
    total, atLevel = t or total, a or atLevel
  end
  return {
    { label = L["Level"], value = tostring(rec.level or "?") },
    { label = L["Played"], value = type(total) == "number" and ns.Session.Duration(total) or L["pending"] },
    { label = L["Time at the level"],
      value = type(atLevel) == "number" and ns.Session.Duration(atLevel) or L["pending"] },
  }
end

if Wn.available then
  add({
    key = "session", group = "me", order = 10, label = L["Session"], characters = "current",
    search = false, sortable = false,
    columns = { { title = L["This session"], width = 250 }, { title = "", width = 150, justify = "RIGHT" },
      { title = "", width = 120, justify = "RIGHT" } },
    version = function() return ns.Session.version end,
    tiles = sessionTiles,
    rows = sessionRows,
    tooltip = function(row, tip) if row.ref then Wn.CreatureTooltip(tip, row.ref, ns.rec) end end,
    footer = function() return L["Counted in game since you logged in; a /reload keeps it."] end,
  })
  add({
    key = "kills", group = "me", order = 40, label = L["Kills"], characters = "one",
    columns = { { title = L["Creature"], width = 210 }, { title = L["Kills"], width = 60, justify = "RIGHT" },
      { title = L["Experience"], width = 80, justify = "RIGHT" }, { title = L["Last zone"], width = 150 },
      { title = L["Last"], width = 110, justify = "RIGHT" } },
    version = function() return ns.Chronicle.killsVersion or 0 end,
    rows = killsRows, fill = killsFill, tiles = killsTiles,
    tooltip = function(row, tip) Wn.CreatureTooltip(tip, row.ref, row.rec) end,
    footer = Wn.RecordFooter,
  })
  add({
    key = "loot", group = "me", order = 50, label = L["Loot"], characters = "one", icons = true,
    columns = { { title = L["Item"], width = 190 }, { title = L["Looted"], width = 60, justify = "RIGHT" },
      { title = L["Count"], width = 60, justify = "RIGHT" }, { title = L["From"], width = 160, sort = false },
      { title = L["Last"], width = 110, justify = "RIGHT" } },
    version = function() return ns.Chronicle.lootVersion or 0 end,
    rows = lootRows, fill = lootFill, tiles = lootTiles,
    tooltip = function(row, tip, _, hadLink) Wn.ItemTooltip(tip, row.ref, row.rec, hadLink) end,
    footer = Wn.RecordFooter,
  })
  add({
    key = "history", group = "me", order = 60, label = L["History"], characters = "one", search = false,
    columns = { { title = L["Level"], width = 70 }, { title = L["Reached"], width = 160 },
      { title = L["Time at the level"], width = 150, justify = "RIGHT" },
      { title = L["Played when reached"], width = 160, justify = "RIGHT" } },
    version = function()
      local db = ns.db
      -- and the minute: the current level's time played counts on.
      return tostring(db and type(db.journal) == "table" and #db.journal or 0) .. ":" .. ns.Session.levelVersion
        .. ":" .. math.floor(GetTime() / 60)
    end,
    rows = historyRows, tiles = historyTiles,
    footer = function() return L["From the journal's level-ups (it keeps 30 days) and /played."] end,
  })
  -- Redrawn when Session.lua says something changed, only while the window is open.
  ns.Session.Watch(function() return RW.IsShown() end, function() RW.Refresh() end)
end

-- The window's sections whose data changed outside the session (a bag scan, a gear or reputation
-- scan): a redraw at most every half second, only while the window is open. Rows rebuild only if
-- the section's version moved.
function Wn.Changed()
  if not Wn.available or not RW.IsShown() then return end
  ns.Debounce("chronicle-window", 0.5, function() RW.Refresh() end)
end

-- Opening it -------------------------------------------------------------------------------------------

local function tooOld()
  ns.Print(L["the Ryna addon here is too old for its window. Update it from the Ryna companion app."])
end

-- Opens the window on section `key`; with none, on the section it was last on (the core remembers it
-- across sessions; Session the first time, the rail's first).
function Wn.Open(key)
  if not Wn.available then return tooOld() end
  RW.Open(key and Wn.keys[key] and key or nil)
end

-- The strip's click: opens the window where it was last, or closes it.
function Wn.Toggle()
  if not Wn.available then return tooOld() end
  if RW.IsShown() then RW.Hide() else RW.Open() end
end

-- /chronicle, and /ryna chronicle: opens the window where it was last, or closes it ("/chronicle
-- kills" opens a section; "levels" is History's old name).
local ALIASES = { levels = "history" }
local function command(arg)
  arg = tostring(arg or ""):lower():match("^%s*(%S*)")
  local key = ALIASES[arg] or arg
  if key ~= "" and Wn.keys[key] then
    Wn.Open(key)
  else
    Wn.Toggle()
  end
end
Wn.Command = command

SLASH_RYNACHRONICLE1 = "/chronicle"
if type(SlashCmdList) == "table" then
  SlashCmdList.RYNACHRONICLE = function(msg) ns.Run("/chronicle", command, msg) end
end
ns.API.Slash.Add({ word = "chronicle", order = 30, run = command,
  help = L["/ryna chronicle (or /chronicle): Ryna's window; /chronicle session, characters, items, "
    .. "kills, loot or history opens that section"] })
