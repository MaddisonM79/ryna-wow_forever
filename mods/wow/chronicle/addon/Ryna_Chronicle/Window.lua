-- Window: the Chronicle window (RynaChronicleWindow), a standalone frame with four tabs over this
-- character's numbers. /chronicle, /ryna chronicle, a click on the session strip (Strip.lua) and
-- the button on the Recording tab open it; Escape or its close button shut it.
--
--   Session   this session's numbers (Session.lua), then its kills by creature, most first
--   Kills     the lifetime `kills`, most first, with a search box; a row's tooltip lists that
--             creature's drops (from loot[*].from["c:<id>"] over kills[id].looted)
--   Loot      the lifetime `loot`, newest first, in the item's quality colour, with where it came
--             from and a search box
--   Levels    the journal's `level` events for this character, oldest first, with the time played
--             at each level
--
-- Every list is one fixed set of ROWS rows over a sorted view of the data, scrolled by the mouse
-- wheel or the bar beside it: only the rows on screen are filled, so a table of 2000 creatures costs
-- what 15 do. A view is rebuilt only when its data changed (a version number) or its search did.
-- The window redraws when Session.lua says something changed (at most every half second) and only
-- while it is shown.

local _, ns = ...
if ns.off then return end
local L = ns.L

local Wn = {}
ns.Window = Wn

local W = ns.API.Panel.Widgets
local WIDTH, HEIGHT = 600, 470
local ROWS, ROW_H = 15, 18
local LIST_TOP = -98    -- under the title, the tabs and the search box
local COLS = 5
local MAX_DROPS = 12

local function num(v)
  if type(v) == "number" then return v end
  return tonumber(v) or 0
end

local function plain(s)
  return ns.Status.Plain(s) or s
end

local function recMap(name)
  local m = ns.rec and ns.rec[name]
  if type(m) == "table" then return m end
  return nil
end

local function when(t)
  if type(t) ~= "number" then return "" end
  return date("%Y-%m-%d %H:%M", t)
end

-- "|cffRRGGBB" for an item quality, from the client's ITEM_QUALITY_COLORS (cached per quality).
local qualityHex = {}
function Wn.QualityColor(q)
  q = tonumber(q)
  if q == nil then return "|cffffffff" end
  if qualityHex[q] then return qualityHex[q] end
  local c = type(_G.ITEM_QUALITY_COLORS) == "table" and _G.ITEM_QUALITY_COLORS[q]
  local hex
  if type(c) == "table" then
    if type(c.hex) == "string" then
      hex = c.hex
    elseif type(c.r) == "number" then
      hex = string.format("|cff%02x%02x%02x", c.r * 255, c.g * 255, c.b * 255)
    end
  end
  qualityHex[q] = hex or "|cffffffff"
  return qualityHex[q]
end

-- A loot source key as the Loot tab names it: the creature's name, "object", "container", "fishing".
function Wn.SourceName(key)
  if key == "f" then return L["fishing"] end
  local kind, id = tostring(key):match("^(%a):?(%d*)$")
  if kind == "c" then
    local kills = recMap("kills")
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
local function sourcesText(from, max)
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
  for i, k in ipairs(best) do best[i] = plain(Wn.SourceName(k)) end
  return table.concat(best, ", ")
end

-- Creature `id`'s drops from the record: { name, quality, n } most first, at most MAX_DROPS.
function Wn.Drops(id)
  local list = {}
  local loot = recMap("loot")
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

-- The tabs' views ------------------------------------------------------------------------------
--
-- Each tab: { key, label, search, columns = { { title, width, justify } ... }, count(), fill(row, i),
-- tooltip(row, tip) or nil }. fill sets row.cols[1..COLS] and row.ref (what the tooltip reads).

local tabs, tabsByKey = {}, {}

local function filterText(tab)
  return tab.filter or ""
end

local function matches(name, filter)
  if filter == "" then return true end
  return type(name) == "string" and name:lower():find(filter, 1, true) ~= nil
end

-- Session --------------------------------------------------------------------------------------

local sessionView = { s = nil, stats = {}, nstats = 0 }

local function stat(i, label, value, rate)
  local r = sessionView.stats[i]
  if not r then
    r = {}
    sessionView.stats[i] = r
  end
  r.label, r.value, r.rate = label, value, rate
end

local function buildSession()
  local s = ns.Session.Get()
  local v = sessionView
  v.s = s
  v.nstats = 0
  if not s then return end
  local S = ns.Session
  local i = 0
  local function add(label, value, rate)
    i = i + 1
    stat(i, label, value, rate)
  end
  add(L["Since"], string.format("%s (%s)", date("%H:%M", s.startAt), S.Duration(s.elapsed)), "")
  add(L["Kills"], S.Number(s.kills), string.format(L["%s/h"], S.Number(s.killsPerHour)))
  add(L["Experience"], S.Number(s.xp), string.format(L["%s/h"], S.Number(s.xpPerHour)))
  add(L["Levels gained"], S.Number(s.levels), "")
  add(L["Money"], S.Money(s.gold), string.format(L["%s/h"], S.Money(s.goldPerHour)))
  add(L["Gained / spent"], string.format(L["%s / %s"], S.Money(s.goldGained), S.Money(s.goldSpent)), "")
  add(L["Quests turned in"], S.Number(s.quests), "")
  add(L["Items looted"], S.Number(s.items), "")
  add(L["Deaths"], S.Number(s.deaths), "")
  v.nstats = i
end

local sessionTab = {
  key = "session", label = L["Session"],
  columns = {
    { L["This session"], 250, "LEFT" }, { "", 150, "RIGHT" }, { "", 120, "RIGHT" }, { "", 0 }, { "", 0 },
  },
  rebuild = buildSession,
  count = function()
    local v = sessionView
    if not v.s then return 0 end
    return v.nstats + 1 + #v.s.creatures
  end,
  fill = function(row, i)
    local v = sessionView
    local c = row.cols
    row.ref = false
    if i <= v.nstats then
      local r = v.stats[i]
      c[1]:SetText(r.label)
      c[2]:SetText(r.value)
      c[3]:SetText(r.rate)
    elseif i == v.nstats + 1 then
      c[1]:SetText("|cffffd100" .. L["Kills this session"] .. "|r")
      c[2]:SetText("|cffffd100" .. L["Kills"] .. "|r")
      c[3]:SetText("|cffffd100" .. L["Experience"] .. "|r")
    else
      local k = v.s.creatures[i - v.nstats - 1]
      c[1]:SetText(plain(k.name) or "?")
      c[2]:SetText(ns.Session.Number(k.n))
      c[3]:SetText(ns.Session.Number(k.xp))
      if k.id then row.ref = k.id end
    end
    c[4]:SetText("")
    c[5]:SetText("")
  end,
}

-- Kills ----------------------------------------------------------------------------------------

local killsView = { ids = {}, version = -1, rec = nil, filter = nil }

local function byKillsN(a, b)
  local kills = ns.rec.kills
  local ea, eb = kills[a], kills[b]
  local na, nb = num(ea.n), num(eb.n)
  if na ~= nb then return na > nb end
  return a < b
end

local function buildKills(force)
  local v = killsView
  local filter = filterText(tabsByKey.kills)
  local version = ns.Chronicle.killsVersion or 0
  if not force and v.version == version and v.rec == ns.rec and v.filter == filter then return end
  v.version, v.rec, v.filter = version, ns.rec, filter
  local ids = v.ids
  for i = #ids, 1, -1 do ids[i] = nil end
  local kills = recMap("kills")
  if not kills then return end
  for id, e in pairs(kills) do
    if type(e) == "table" and num(e.n) > 0 and matches(e.name, filter) then ids[#ids + 1] = id end
  end
  table.sort(ids, byKillsN)
end

-- What a creature's tooltip says: its numbers, then its drops with how often.
function Wn.CreatureTooltip(tip, id)
  local kills = recMap("kills")
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
  local drops = Wn.Drops(id)
  if #drops == 0 then return end
  tip:AddLine(" ")
  tip:AddLine(L["Drops"])
  for _, d in ipairs(drops) do
    tip:AddDoubleLine(Wn.QualityColor(d.quality) .. plain(d.name) .. "|r",
      string.format(L["%d of %d (%d%%)"], d.n, looted, math.floor(d.n * 100 / looted + 0.5)),
      1, 1, 1, 1, 1, 1)
  end
end

local killsTab = {
  key = "kills", label = L["Kills"], search = true,
  columns = {
    { L["Creature"], 200, "LEFT" }, { L["Kills"], 60, "RIGHT" }, { L["Experience"], 80, "RIGHT" },
    { L["Last zone"], 130, "LEFT" }, { L["Last"], 90, "RIGHT" },
  },
  rebuild = buildKills,
  count = function() return #killsView.ids end,
  fill = function(row, i)
    local id = killsView.ids[i]
    local e = ns.rec.kills[id]
    local c = row.cols
    row.ref = id
    c[1]:SetText(plain(ns.NonEmpty(e.name) or string.format(L["creature %s"], id)))
    c[2]:SetText(ns.Session.Number(e.n))
    c[3]:SetText(ns.Session.Number(e.xp))
    c[4]:SetText(plain(ns.NonEmpty(e.zone) or ""))
    c[5]:SetText(ns.Status.Ago(e.last) or "")
  end,
}

-- Loot -----------------------------------------------------------------------------------------

local lootView = { ids = {}, version = -1, rec = nil, filter = nil }

local function byLast(a, b)
  local loot = ns.rec.loot
  local la, lb = num(loot[a].last), num(loot[b].last)
  if la ~= lb then return la > lb end
  return a < b
end

local function buildLoot(force)
  local v = lootView
  local filter = filterText(tabsByKey.loot)
  local version = ns.Chronicle.lootVersion or 0
  if not force and v.version == version and v.rec == ns.rec and v.filter == filter then return end
  v.version, v.rec, v.filter = version, ns.rec, filter
  local ids = v.ids
  for i = #ids, 1, -1 do ids[i] = nil end
  local loot = recMap("loot")
  if not loot then return end
  for id, e in pairs(loot) do
    if type(e) == "table" and num(e.n) > 0 and matches(e.name, filter) then ids[#ids + 1] = id end
  end
  table.sort(ids, byLast)
end

local lootTab = {
  key = "loot", label = L["Loot"], search = true,
  columns = {
    { L["Item"], 190, "LEFT" }, { L["Looted"], 60, "RIGHT" }, { L["Count"], 60, "RIGHT" },
    { L["From"], 150, "LEFT" }, { L["Last"], 90, "RIGHT" },
  },
  rebuild = buildLoot,
  count = function() return #lootView.ids end,
  fill = function(row, i)
    local id = lootView.ids[i]
    local e = ns.rec.loot[id]
    local c = row.cols
    row.ref = id
    c[1]:SetText(Wn.QualityColor(e.quality) .. plain(ns.NonEmpty(e.name) or string.format(L["item %s"], id)) .. "|r")
    c[2]:SetText(ns.Session.Number(e.n))
    c[3]:SetText(ns.Session.Number(e.qty))
    c[4]:SetText(sourcesText(e.from, 2))
    c[5]:SetText(ns.Status.Ago(e.last) or "")
  end,
}

function Wn.ItemTooltip(tip, id)
  local loot = recMap("loot")
  local e = loot and loot[id]
  if type(e) ~= "table" then return end
  local link = ns.NonEmpty(e.link)
  if not (link and pcall(tip.SetHyperlink, tip, link)) then
    tip:AddLine(Wn.QualityColor(e.quality) .. plain(ns.NonEmpty(e.name) or string.format(L["item %s"], id)) .. "|r")
  end
  tip:AddLine(" ")
  tip:AddLine(string.format(L["From: %s"], sourcesText(e.from, 8)), 0.8, 0.8, 0.8, true)
end

-- Levels ---------------------------------------------------------------------------------------

local levelsView = { rows = {}, n = 0, key = nil }

local function buildLevels(force)
  local v = levelsView
  local db, rec = ns.db, ns.rec
  if not db or not rec then
    v.n = 0
    return
  end
  local key = tostring(rec.guid) .. ":" .. #db.journal .. ":" .. ns.Session.levelVersion
  if not force and v.key == key then return end
  v.key = key
  local rows, n = v.rows, 0
  for i = 1, #db.journal do
    local e = db.journal[i]
    if type(e) == "table" and e.type == "level" and e.char == rec.guid and type(e.level) == "number" then
      n = n + 1
      local r = rows[n]
      if not r then
        r = {}
        rows[n] = r
      end
      r.level, r.at, r.playedTotal, r.atLevel = e.level, e.at, num(e.playedTotal), nil
    end
  end
  -- The time at each level: until the next level's played total; the current one from /played.
  for i = 1, n - 1 do
    local a, b = rows[i], rows[i + 1]
    if a.playedTotal > 0 and b.playedTotal > 0 then a.atLevel = b.playedTotal - a.playedTotal end
  end
  local _, atLevel = ns.Character.PlayedNow()
  atLevel = atLevel or rec.playedLevel
  if n > 0 and rows[n].level == rec.level then
    rows[n].atLevel = atLevel
  elseif type(rec.level) == "number" then
    -- Reached before the journal's 30 days: the current level, with no date.
    n = n + 1
    local r = rows[n] or {}
    rows[n] = r
    r.level, r.at, r.playedTotal, r.atLevel = rec.level, nil, 0, atLevel
  end
  for i = n + 1, #rows do rows[i] = nil end
  v.n = n
end

local levelsTab = {
  key = "levels", label = L["Levels"],
  columns = {
    { L["Level"], 70, "LEFT" }, { L["Reached"], 160, "LEFT" }, { L["Time at the level"], 150, "RIGHT" },
    { L["Played when reached"], 160, "RIGHT" }, { "", 0 },
  },
  rebuild = buildLevels,
  count = function() return levelsView.n end,
  fill = function(row, i)
    local r = levelsView.rows[i]
    local c = row.cols
    row.ref = false
    c[1]:SetText(tostring(r.level))
    c[2]:SetText(r.at and when(r.at) or L["before the journal"])
    c[3]:SetText(r.atLevel and ns.Session.Duration(r.atLevel) or "")
    c[4]:SetText(r.playedTotal > 0 and ns.Session.Duration(r.playedTotal) or "")
    c[5]:SetText("")
  end,
}

sessionTab.tooltip = function(row, tip) if row.ref then Wn.CreatureTooltip(tip, row.ref) end end
killsTab.tooltip = function(row, tip) Wn.CreatureTooltip(tip, row.ref) end
lootTab.tooltip = function(row, tip) Wn.ItemTooltip(tip, row.ref) end

for _, t in ipairs({ sessionTab, killsTab, lootTab, levelsTab }) do
  tabs[#tabs + 1] = t
  tabsByKey[t.key] = t
end
Wn.tabs = tabsByKey

-- The frame ------------------------------------------------------------------------------------

local function fontString(parent, fontObject, layer)
  local ok, fs = pcall(parent.CreateFontString, parent, nil, layer or "OVERLAY", fontObject)
  if not ok or not fs then fs = parent:CreateFontString(nil, layer or "OVERLAY") end
  if not fs:GetFont() then fs:SetFont(_G.STANDARD_TEXT_FONT or "Fonts\\FRIZQT__.TTF", 11) end
  if fs.SetWordWrap then fs:SetWordWrap(false) end
  return fs
end

local function shown()
  return Wn.frame ~= nil and Wn.frame:IsShown() and true or false
end

-- Places a row's (or the header's) columns for the current tab.
local function layoutColumns(cols, tab)
  local x = 6
  for i = 1, COLS do
    local spec = tab.columns[i]
    local fs = cols[i]
    local w = spec and spec[2] or 0
    fs:ClearAllPoints()
    if w > 0 then
      fs:SetPoint("LEFT", fs.row, "LEFT", x, 0)
      fs:SetWidth(w - 8)
      fs:SetJustifyH(spec[3] or "LEFT")
      fs:Show()
    else
      fs:Hide()
    end
    x = x + w
  end
end

local function rowTooltip(row)
  local tab = tabsByKey[Wn.selected]
  local tip = _G.GameTooltip
  if not tip or not tab or not tab.tooltip or not row.ref then return end
  tip:SetOwner(row, "ANCHOR_RIGHT")
  tip:ClearLines()
  tab.tooltip(row, tip)
  tip:Show()
end

-- Fills the rows on screen from the selected tab's view, from the scroll offset.
function Wn.DrawRows()
  local f = Wn.frame
  local tab = tabsByKey[Wn.selected]
  if not f or not tab then return end
  local count = tab.count()
  local maxOffset = math.max(0, count - ROWS)
  if Wn.offset > maxOffset then Wn.offset = maxOffset end
  if Wn.offset < 0 then Wn.offset = 0 end
  for r = 1, ROWS do
    local row = f.rows[r]
    local i = Wn.offset + r
    if i <= count then
      tab.fill(row, i)
      row:Show()
    else
      row.ref = false
      row:Hide()
    end
  end
  f.empty:SetText(count == 0 and (tab.search and filterText(tab) ~= "" and L["Nothing matches."]
    or L["Nothing recorded yet."]) or "")
  local bar = f.bar
  bar.syncing = true
  bar:SetMinMaxValues(0, maxOffset)
  bar:SetValue(Wn.offset)
  bar.syncing = false
  if maxOffset > 0 then bar:Show() else bar:Hide() end
  local over = f.hoverRow
  if over and over:IsShown() then rowTooltip(over) end
end

-- Rebuilds the selected tab's view (when its data changed, or always with `force`) and redraws.
function Wn.Redraw(force)
  if not shown() then return end
  local tab = tabsByKey[Wn.selected]
  if not tab then return end
  tab.rebuild(force)
  Wn.DrawRows()
end

function Wn.Select(key)
  local tab = tabsByKey[key]
  if not tab then return end
  Wn.selected = key
  Wn.offset = 0
  local f = Wn.frame
  if not f then return end
  for _, t in ipairs(tabs) do
    local b = f.tabButtons[t.key]
    if b then
      if t.key == key then b:LockHighlight() else b:UnlockHighlight() end
      local fs = b.GetFontString and b:GetFontString()
      if fs then
        if t.key == key then fs:SetTextColor(1, 1, 1) else fs:SetTextColor(1, 0.82, 0) end
      end
    end
  end
  layoutColumns(f.header, tab)
  for c = 1, COLS do
    local spec = tab.columns[c]
    f.header[c]:SetText(spec and spec[1] or "")
  end
  for r = 1, ROWS do layoutColumns(f.rows[r].cols, tab) end
  if tab.search then
    f.search.filling = true
    f.search:SetText(tab.filter or "")
    f.search.filling = false
    f.search:Show()
  else
    f.search:Hide()
  end
  Wn.Redraw(true)
end

local function searchBox(parent)
  for _, template in ipairs({ "SearchBoxTemplate", "InputBoxTemplate" }) do
    local ok, box = pcall(CreateFrame, "EditBox", "RynaChronicleWindowSearch", parent, template)
    if ok and box then return box end
  end
  local box = CreateFrame("EditBox", "RynaChronicleWindowSearch", parent)
  local bg = box:CreateTexture(nil, "BACKGROUND")
  bg:SetAllPoints()
  bg:SetColorTexture(1, 1, 1, 0.08)
  box:SetFontObject(_G.ChatFontNormal or "GameFontHighlightSmall")
  box:SetTextInsets(4, 4, 0, 0)
  return box
end

local function onSearch()
  local f = Wn.frame
  local tab = tabsByKey[Wn.selected]
  if not f or not tab or not tab.search then return end
  local text = f.search:GetText()
  tab.filter = type(text) == "string" and text:lower():gsub("^%s+", ""):gsub("%s+$", "") or ""
  Wn.offset = 0
  Wn.Redraw()
end

function Wn.Create()
  if Wn.frame then return Wn.frame end
  local f = CreateFrame("Frame", "RynaChronicleWindow", _G.UIParent)
  Wn.frame = f
  f:SetSize(WIDTH, HEIGHT)
  f:SetPoint("CENTER", _G.UIParent, "CENTER", 0, 40)
  f:SetFrameStrata("DIALOG")
  if f.SetToplevel then f:SetToplevel(true) end
  f:SetClampedToScreen(true)
  f:SetMovable(true)
  f:EnableMouse(true)
  f:RegisterForDrag("LeftButton")
  f:SetScript("OnDragStart", function(self) self:StartMoving() end)
  f:SetScript("OnDragStop", function(self)
    self:StopMovingOrSizing()
    if self.SetUserPlaced then pcall(self.SetUserPlaced, self, false) end
  end)
  local bg = f:CreateTexture(nil, "BACKGROUND")
  bg:SetAllPoints()
  bg:SetColorTexture(0.05, 0.05, 0.06, 0.92)
  f:Hide()
  -- Escape closes it, like the game's own windows.
  if type(_G.UISpecialFrames) == "table" then table.insert(_G.UISpecialFrames, "RynaChronicleWindow") end

  local title = fontString(f, "GameFontNormalLarge")
  title:SetPoint("TOPLEFT", f, "TOPLEFT", 12, -10)
  title:SetText(L["Chronicle"])
  f.who = fontString(f, "GameFontDisableSmall")
  f.who:SetPoint("LEFT", title, "RIGHT", 10, -1)

  local okClose, close = pcall(CreateFrame, "Button", nil, f, "UIPanelCloseButton")
  if not okClose or not close then
    close = W.button(f, "x", 22, 22)
  end
  if close then
    close:SetPoint("TOPRIGHT", f, "TOPRIGHT", -2, -2)
    close:SetScript("OnClick", function() f:Hide() end)
  end

  f.tabButtons = {}
  local prev
  for _, t in ipairs(tabs) do
    local b = W.button(f, t.label, 96, 22)
    if b then
      if prev then b:SetPoint("LEFT", prev, "RIGHT", 4, 0) else b:SetPoint("TOPLEFT", f, "TOPLEFT", 10, -36) end
      local key = t.key
      b:SetScript("OnClick", function() ns.Run("chronicle window", Wn.Select, key) end)
      f.tabButtons[key] = b
      prev = b
    end
  end

  local search = searchBox(f)
  search:SetSize(200, 20)
  search:SetPoint("TOPRIGHT", f, "TOPRIGHT", -14, -38)
  search:SetAutoFocus(false)
  if search.SetMaxLetters then search:SetMaxLetters(40) end
  search:SetScript("OnTextChanged", function(self)
    if self.filling then return end
    ns.Debounce("chronicle-search", 0.2, onSearch)
  end)
  search:SetScript("OnEscapePressed", function(self) self:ClearFocus() end)
  search:SetScript("OnEnterPressed", function(self) self:ClearFocus() end)
  search.filling = false
  f.search = search

  -- The list: a header, ROWS rows, and the bar.
  local list = CreateFrame("Frame", nil, f)
  list:SetPoint("TOPLEFT", f, "TOPLEFT", 10, LIST_TOP)
  list:SetSize(WIDTH - 40, ROWS * ROW_H)
  list:EnableMouseWheel(true)
  f.list = list

  local header = CreateFrame("Frame", nil, f)
  header:SetPoint("BOTTOMLEFT", list, "TOPLEFT", 0, 2)
  header:SetSize(WIDTH - 40, ROW_H)
  f.header = {}
  for c = 1, COLS do
    local fs = fontString(header, "GameFontNormalSmall")
    fs.row = header
    f.header[c] = fs
  end

  f.rows = {}
  for r = 1, ROWS do
    local row = CreateFrame("Button", nil, list)
    row:SetSize(WIDTH - 40, ROW_H)
    row:SetPoint("TOPLEFT", list, "TOPLEFT", 0, -(r - 1) * ROW_H)
    local stripe = row:CreateTexture(nil, "BACKGROUND")
    stripe:SetAllPoints()
    stripe:SetColorTexture(1, 1, 1, r % 2 == 0 and 0.03 or 0)
    local hl = row:CreateTexture(nil, "HIGHLIGHT")
    hl:SetAllPoints()
    hl:SetColorTexture(1, 1, 1, 0.08)
    row.cols = {}
    for c = 1, COLS do
      local fs = fontString(row, "GameFontHighlightSmall")
      fs.row = row
      row.cols[c] = fs
    end
    row:SetScript("OnEnter", function(self)
      f.hoverRow = self
      ns.Run("chronicle window", rowTooltip, self)
    end)
    row:SetScript("OnLeave", function()
      f.hoverRow = false
      local tip = _G.GameTooltip
      if tip then tip:Hide() end
    end)
    row.ref = false
    row:Hide()
    f.rows[r] = row
  end

  f.empty = fontString(list, "GameFontDisable")
  f.empty:SetPoint("TOP", list, "TOP", 0, -40)

  f.hoverRow = false
  local bar = CreateFrame("Slider", nil, f)
  bar.syncing = false
  bar:SetOrientation("VERTICAL")
  bar:SetPoint("TOPLEFT", list, "TOPRIGHT", 6, 0)
  bar:SetSize(10, ROWS * ROW_H)
  local track = bar:CreateTexture(nil, "BACKGROUND")
  track:SetAllPoints()
  track:SetColorTexture(1, 1, 1, 0.05)
  local thumb = bar:CreateTexture(nil, "OVERLAY")
  thumb:SetColorTexture(1, 1, 1, 0.35)
  thumb:SetSize(10, 30)
  bar:SetThumbTexture(thumb)
  bar:SetValueStep(1)
  if bar.SetObeyStepOnDrag then bar:SetObeyStepOnDrag(true) end
  bar:SetMinMaxValues(0, 0)
  bar:SetValue(0)
  bar:SetScript("OnValueChanged", function(self, value)
    if self.syncing then return end
    local offset = math.floor((tonumber(value) or 0) + 0.5)
    if offset ~= Wn.offset then
      Wn.offset = offset
      ns.Run("chronicle window", Wn.DrawRows)
    end
  end)
  f.bar = bar
  list:SetScript("OnMouseWheel", function(_, delta)
    Wn.offset = Wn.offset - (tonumber(delta) or 0) * 3
    ns.Run("chronicle window", Wn.DrawRows)
  end)

  f.foot = fontString(f, "GameFontDisableSmall")
  f.foot:SetPoint("BOTTOMLEFT", f, "BOTTOMLEFT", 12, 10)
  f.foot:SetText(L["Your own numbers for this character. Hover a row for more."])

  f:SetScript("OnShow", function()
    ns.Run("chronicle window", function()
      local rec = ns.rec
      f.who:SetText(rec and plain(rec.fullName) or "")
      Wn.Select(Wn.selected or "session")
    end)
  end)
  f:SetScript("OnHide", function()
    local tip = _G.GameTooltip
    if f.hoverRow and tip then tip:Hide() end
    f.hoverRow = false
  end)
  Wn.selected = Wn.selected or "session"
  Wn.offset = 0
  return f
end

function Wn.Open(key)
  local f = Wn.Create()
  if key and tabsByKey[key] then Wn.selected = key end
  if f:IsShown() then
    Wn.Select(Wn.selected)
  else
    f:Show()
  end
end

function Wn.Toggle()
  local f = Wn.Create()
  if f:IsShown() then f:Hide() else Wn.Open() end
end

ns.Session.Watch(shown, function() Wn.Redraw(false) end)

-- /chronicle, and /ryna chronicle: opens or closes the window ("/chronicle kills" opens a tab).
local function command(arg)
  arg = tostring(arg or ""):lower():match("^%s*(%S*)")
  if arg ~= "" and tabsByKey[arg] then
    Wn.Open(arg)
  else
    Wn.Toggle()
  end
end

SLASH_RYNACHRONICLE1 = "/chronicle"
if type(SlashCmdList) == "table" then
  SlashCmdList.RYNACHRONICLE = function(msg) ns.Run("/chronicle", command, msg) end
end
ns.API.Slash.Add({ word = "chronicle", order = 30, run = command,
  help = L["/ryna chronicle (or /chronicle): the Chronicle window; "
    .. "/chronicle kills, loot, levels or session opens that tab"] })
