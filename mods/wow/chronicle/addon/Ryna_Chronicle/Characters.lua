-- Characters: the Characters section of Ryna's window (under Me), every character on the account
-- from the saved file (RynaDB.characters, what Character.lua records; nothing new is recorded here).
--
-- All characters: one row each, the one you're on first: the name in its class colour, the level
-- (with the experience bar), gold, time played, professions with their ranks, and when it was last
-- seen; tiles for how many characters, their gold and their time played. A click on a row picks that
-- character. One character: its detail, as headed groups of rows: the character (class, guild, zone,
-- talents), its gear by slot (the game's own item tooltip on hover), its professions and its
-- reputations, each with a bar.
--
-- The character you're on is live (its record is the one Chronicle keeps up to date); the others are
-- as of their last logout, and the footer says so.

local _, ns = ...
if ns.off then return end
local L = ns.L
local N = ns.N

local Wn = ns.Window
if not (Wn and Wn.available) then return end
local RW = ns.API.Window

local Ch = {}
ns.Characters = Ch

local function num(v)
  if type(v) == "number" then return v end
  return tonumber(v)
end

local function plain(s)
  return ns.Status.Plain(s) or s
end

-- Equipment slots 0 to 19: the client's own name for each (its GlobalStrings), else English.
local SLOTS = {
  [0] = { "AMMOSLOT", N("Ammo") }, { "HEADSLOT", N("Head") }, { "NECKSLOT", N("Neck") },
  { "SHOULDERSLOT", N("Shoulder") }, { "SHIRTSLOT", N("Shirt") }, { "CHESTSLOT", N("Chest") },
  { "WAISTSLOT", N("Waist") }, { "LEGSSLOT", N("Legs") }, { "FEETSLOT", N("Feet") },
  { "WRISTSLOT", N("Wrist") }, { "HANDSSLOT", N("Hands") }, { "FINGER0SLOT", N("Finger") },
  { "FINGER1SLOT", N("Finger") }, { "TRINKET0SLOT", N("Trinket") }, { "TRINKET1SLOT", N("Trinket") },
  { "BACKSLOT", N("Back") }, { "MAINHANDSLOT", N("Main Hand") }, { "SECONDARYHANDSLOT", N("Off Hand") },
  { "RANGEDSLOT", N("Ranged") }, { "TABARDSLOT", N("Tabard") },
}

function Ch.SlotName(slot)
  local e = SLOTS[tonumber(slot)]
  if not e then return tostring(slot) end
  return ns.NonEmpty(_G[e[1]]) or L[e[2]]
end

-- A reputation standing's name in the client's language ("Friendly"), from its GlobalStrings.
function Ch.StandingName(standing)
  standing = tonumber(standing)
  if not standing then return "" end
  return ns.NonEmpty(_G["FACTION_STANDING_LABEL" .. standing]) or tostring(standing)
end

local function standingColor(standing)
  local colors = _G.FACTION_BAR_COLORS
  local c = type(colors) == "table" and colors[tonumber(standing)]
  if type(c) == "table" and type(c.r) == "number" then return c.r, c.g, c.b end
  return nil
end

-- An item link's name ("Linen Boots"), from its [brackets].
function Ch.LinkName(link)
  if type(link) ~= "string" or ns.IsSecret(link) then return nil end
  return link:match("|h%[(.-)%]|h")
end

-- "Mining 75, Tailoring 120".
function Ch.ProfessionsText(rec)
  local parts = {}
  for _, p in ipairs(type(rec.professions) == "table" and rec.professions or {}) do
    if type(p) == "table" and ns.NonEmpty(p.name) then
      parts[#parts + 1] = string.format(L["%s %s"], plain(p.name), tostring(num(p.rank) or "?"))
    end
  end
  return table.concat(parts, ", ")
end

-- Played totals: the one you're on counted on from its last /played.
local function played(rec)
  if rec == ns.rec then
    local total, atLevel = ns.Character.PlayedNow()
    if total then return total, atLevel end
  end
  return num(rec.playedTotal), num(rec.playedLevel)
end

local function lastSeen(rec, ctx)
  if rec.guid ~= nil and rec.guid == ctx.current then return L["now"] end
  return ns.Status.Ago(num(rec.lastSeen)) or L["never"]
end

-- All characters --------------------------------------------------------------------------------------

local ALL_COLUMNS = {
  { title = L["Character"], width = 170 }, { title = L["Level"], width = 64, justify = "RIGHT", kind = "bar" },
  { title = L["Gold"], width = 110, justify = "RIGHT" }, { title = L["Played"], width = 80, justify = "RIGHT" },
  { title = L["Professions"], width = 120 }, { title = L["Last seen"], width = 70, justify = "RIGHT" },
}

local DETAIL_COLUMNS = {
  { title = "", width = 230 }, { title = "", width = 150 }, { title = "", width = 170, kind = "bar" },
  { title = "", width = 60, justify = "RIGHT" },
}

function Ch.AllRows(ctx)
  local rows = {}
  for _, c in ipairs(RW.Characters()) do
    local rec = c.rec
    if RW.Match(c.name, ctx.search) then
      local total = played(rec)
      local xp, xpMax = num(rec.xp), num(rec.xpMax)
      rows[#rows + 1] = {
        ref = c.guid, rec = rec, color = RW.ClassColor(rec.class),
        cells = { c.name, tostring(num(rec.level) or "?"), RW.Money(rec.gold),
          total and ns.Session.Duration(total) or L["pending"], Ch.ProfessionsText(rec), lastSeen(rec, ctx) },
        bars = { [2] = xp and xpMax and xpMax > 0 and { value = xp, max = xpMax } or nil },
        sort = { c.name:lower(), num(rec.level) or 0, num(rec.gold) or 0, total or 0, nil,
          c.current and math.huge or (num(rec.lastSeen) or 0) },
      }
    end
  end
  return rows
end

function Ch.AllTiles()
  local n, gold, total = 0, 0, 0
  for _, c in ipairs(RW.Characters()) do
    n = n + 1
    gold = gold + (num(c.rec.gold) or 0)
    total = total + (played(c.rec) or 0)
  end
  return {
    { label = L["Characters"], value = tostring(n) },
    { label = L["Gold"], value = RW.Money(gold) },
    { label = L["Played"], value = ns.Session.Duration(total) },
  }
end

-- One character -----------------------------------------------------------------------------------------

function Ch.DetailRows(ctx)
  local rec = Wn.Record(ctx.character)
  local rows = {}
  if not rec then return rows end
  local groups = {}
  local function group(title)
    local g = { title = title, rows = {} }
    groups[#groups + 1] = g
    return g
  end
  local function line(g, row) g.rows[#g.rows + 1] = row end

  local about = group(L["Character"])
  line(about, { cells = { L["Class"], plain(RW.ClassName(rec.class) or "?") } })
  if type(rec.guild) == "table" and ns.NonEmpty(rec.guild.name) then
    line(about, { cells = { L["Guild"], plain(rec.guild.name) } })
  end
  if ns.NonEmpty(rec.zone) then
    local where = ns.NonEmpty(rec.subzone) and string.format(L["%s, %s"], plain(rec.zone), plain(rec.subzone))
      or plain(rec.zone)
    line(about, { cells = { L["Zone"], where } })
  end
  if ns.NonEmpty(rec.talents) then line(about, { cells = { L["Talents"], plain(rec.talents) } }) end

  local gear = group(L["Gear"])
  local items = {}
  for _, g in ipairs(type(rec.gear) == "table" and rec.gear or {}) do
    if type(g) == "table" then items[#items + 1] = g end
  end
  table.sort(items, function(a, b) return (num(a.slot) or 99) < (num(b.slot) or 99) end)
  for _, g in ipairs(items) do
    local link = ns.NonEmpty(g.link)
    line(gear, {
      icon = RW.ItemIcon(g.itemId), link = link, quality = g.quality,
      cells = { plain(Ch.LinkName(link) or string.format(L["item %s"], tostring(g.itemId))), Ch.SlotName(g.slot) },
    })
  end

  local profs = group(L["Professions"])
  for _, p in ipairs(type(rec.professions) == "table" and rec.professions or {}) do
    if type(p) == "table" and ns.NonEmpty(p.name) then
      local rank, max = num(p.rank) or 0, num(p.max) or 0
      line(profs, { cells = { plain(p.name), "", string.format(L["%s / %s"], rank, max) },
        bars = { [3] = { value = rank, max = max, r = 0.25, g = 0.5, b = 0.9 } } })
    end
  end

  local reps = group(L["Reputations"])
  for _, r in ipairs(type(rec.reputations) == "table" and rec.reputations or {}) do
    if type(r) == "table" and ns.NonEmpty(r.name) then
      local value, max = num(r.value), num(r.max)
      local red, green, blue = standingColor(r.standing)
      line(reps, { cells = { plain(r.name), Ch.StandingName(r.standing),
          value and max and string.format(L["%s / %s"], value, max) or "" },
        bars = { [3] = value and max and { value = value, max = max, r = red, g = green, b = blue } or nil } })
    end
  end

  for _, g in ipairs(groups) do
    local matched = {}
    for _, row in ipairs(g.rows) do
      if RW.Match(row.cells[1], ctx.search) or RW.Match(row.cells[2], ctx.search) then
        matched[#matched + 1] = row
      end
    end
    if #matched > 0 or ctx.search == "" then
      rows[#rows + 1] = { header = true, cells = { g.title } }
      if #matched == 0 then rows[#rows + 1] = { cells = { "|cff999999" .. L["None recorded yet."] .. "|r" } } end
      for _, row in ipairs(matched) do rows[#rows + 1] = row end
    end
  end
  return rows
end

function Ch.DetailTiles(ctx)
  local rec = Wn.Record(ctx.character)
  if not rec then return nil end
  local total, atLevel = played(rec)
  return {
    { label = L["Level"], value = tostring(num(rec.level) or "?"), sub = RW.ClassName(rec.class) },
    { label = L["Gold"], value = RW.Money(rec.gold) },
    { label = L["Played"], value = total and ns.Session.Duration(total) or L["pending"],
      sub = atLevel and string.format(L["%s at this level"], ns.Session.Duration(atLevel)) or nil },
    { label = L["Last seen"], value = lastSeen(rec, ctx) },
  }
end

-- A character row's tooltip: what the row says, in full.
function Ch.Tooltip(row, tip, ctx)
  local rec = row.rec
  if not rec or row.cells == nil or row.link then return end
  tip:AddLine(row.color .. Wn.Name(rec) .. "|r")
  tip:AddLine(string.format(L["Level %s %s"], tostring(num(rec.level) or "?"), plain(RW.ClassName(rec.class) or "")),
    1, 1, 1)
  if type(rec.guild) == "table" and ns.NonEmpty(rec.guild.name) then
    tip:AddDoubleLine(L["Guild"], plain(rec.guild.name), 0.8, 0.8, 0.8, 1, 1, 1)
  end
  if ns.NonEmpty(rec.zone) then tip:AddDoubleLine(L["Zone"], plain(rec.zone), 0.8, 0.8, 0.8, 1, 1, 1) end
  tip:AddDoubleLine(L["Gold"], RW.Money(rec.gold), 0.8, 0.8, 0.8, 1, 1, 1)
  local total = played(rec)
  tip:AddDoubleLine(L["Played"], total and ns.Session.Duration(total) or L["pending"], 0.8, 0.8, 0.8, 1, 1, 1)
  for _, p in ipairs(type(rec.professions) == "table" and rec.professions or {}) do
    if type(p) == "table" and ns.NonEmpty(p.name) then
      tip:AddDoubleLine(plain(p.name), string.format(L["%s / %s"], tostring(num(p.rank) or "?"),
        tostring(num(p.max) or "?")), 0.8, 0.8, 0.8, 1, 1, 1)
    end
  end
  tip:AddDoubleLine(L["Last seen"], lastSeen(rec, ctx), 0.8, 0.8, 0.8, 1, 1, 1)
  tip:AddLine(" ")
  tip:AddLine(L["Click for this character's gear, professions and reputations."], 0.6, 0.6, 0.6, true)
end

function Ch.Footer(ctx)
  if ctx.character == nil then
    return L["From your saved file: each character as of its last logout; the one you're on is live."]
  end
  return Wn.RecordFooter(ctx)
end

-- The section's version: the session's (gold, experience, levels) and the record's scans.
function Ch.Version()
  return tostring(ns.Session.version) .. ":" .. tostring(ns.Character.version or 0)
end

if RW.AddSection({
  key = "characters", group = "me", order = 20, label = L["Characters"], icons = true,
  columns = function(ctx) return ctx.character == nil and ALL_COLUMNS or DETAIL_COLUMNS end,
  sortable = true,
  rows = function(ctx)
    if ctx.character == nil then return Ch.AllRows(ctx) end
    return Ch.DetailRows(ctx)
  end,
  tiles = function(ctx)
    if ctx.character == nil then return Ch.AllTiles(ctx) end
    return Ch.DetailTiles(ctx)
  end,
  tooltip = Ch.Tooltip,
  click = function(row) if row.ref and row.rec then RW.Choose(row.ref) end end,
  footer = Ch.Footer,
  version = Ch.Version,
}) then
  Wn.keys.characters = true
end
