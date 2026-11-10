-- Window: Ryna's one window (RynaWindow), where every mod shows what it records. The core owns the
-- frame and everything that is the same on every section; a mod registers sections and supplies
-- their rows (Ryna.Window.AddSection, below; docs/ryna-agent-cores-mods.md 7.4 and 7.5).
--
--   +--------------+---------------------------------------------------------+
--   | Ryna         | Title              [ All characters v ] [ Search      ] |
--   |              |  +--------+ +--------+ +--------+ +--------+            |
--   | ME           |  | tile   | | tile   | | tile   | | tile   |  (optional)|
--   |  Session     |  +--------+ +--------+ +--------+ +--------+            |
--   |  Characters  |  Column   Column            Column              Column  |
--   | WORLD        |  [ic] row in quality colour  [=====bar=====]       1,204 |
--   |  ...         |  ...                                                  |  |
--   |              |  Where these numbers came from, and how old they are     |
--   +--------------+---------------------------------------------------------+
--
-- The rail lists the registered sections in three groups (Me, World, Combat and gold); a group
-- with no section isn't drawn, and nothing is greyed out or "coming soon". The header is the same
-- on every section: its title, the character picker ("All characters" and every character in
-- RynaDB.characters) and the search box. The body is optional summary tiles, then rows: only the
-- rows on screen are filled (a fixed set of row frames over the section's list), so 2,000 rows cost
-- what 20 do. A row may carry an icon, a name in the game's quality (or class) colour, a progress
-- bar and right-aligned numbers, and shows the game's own tooltip on hover (an item link) with the
-- section's lines under it. The footer says where the numbers came from and how old they are.
-- Escape closes it; its place and last section are kept in RynaDB.settings.window.
--
-- The look is the game's own: its fonts, the tooltip backdrop and border, its close button, search
-- box, dropdown and status-bar textures. No art of Ryna's, so it sits in the default UI and in a
-- dark replacement like ElvUI alike. Every template is tried under pcall with a plainer fallback.
--
-- A section (Ryna.Window.AddSection(spec) -> true, or false when the spec is refused):
--
--   key        unique string
--   group      "me", "world" or "combat"
--   label      the rail's text; title (optional) the header's, else the label
--   order      within its group, lowest first
--   characters "all" (the default: the picker offers All characters and each character),
--              "one" (needs one character: All means the character you're on, and the picker
--              offers characters only) or "current" (only the character you're on; picker greyed)
--   search     false greys the search box (default: on)
--   sortable   false: the column headings don't sort (rows with headings in them; default: on)
--   icons      true leaves room for an icon at the start of every row
--   columns    { { title, width, justify = "LEFT"|"RIGHT", kind = "bar", sort = false }, ... } (at
--              most 8; sort = false: its heading doesn't sort, for a column with no row.sort key whose
--              text only fill() makes), or function(ctx) -> that list
--   rows       function(ctx) -> the section's rows, in its own order (see "A row" below)
--   fill       optional function(row, ctx): fills row.cells (and icon, link, ...) for a row about to
--              be drawn, once per row and again each minute (ages move on): rows(ctx) may return
--              light rows and leave the strings here
--   tiles      optional function(ctx) -> { { label, value, sub }, ... } (at most 4), every draw
--   tooltip    optional function(row, tip, ctx, hadLink): adds lines under the row's own tooltip
--              (the game's item tooltip when row.link is set; hadLink says whether it took)
--   click      optional function(row, ctx): a click on a row
--   footer     optional function(ctx) -> the footer line, every draw
--   version    optional function() -> anything comparable with ==: rows(ctx) runs when the section
--              is first shown and again only when this or the context changes
--   empty      optional text for a list with nothing in it (default "Nothing recorded yet.")
--   filter     optional function(ctx) -> { { value, label }, ... }: a second picker, left of the
--              character picker, on this section only (Chronicle's zones). The first choice is the
--              default and has value nil; the others' values are strings. Ryna.Window.SetFilter(key,
--              value) picks one from outside (a Zones row opening Kills on its zone)
--
-- ctx = { section, character (a GUID, or nil for all characters), current (the GUID you're on),
-- all (character == nil), search (lowercased, trimmed; "" for none), filter (the section's filter
-- value, nil for its default) }. A section filters its own
-- rows by ctx.search (Ryna.Window.Match helps).
--
-- A row = { cells = { "text", ... } (by column), sort = { value, ... } (what a column sorts by;
-- default its cell's text), icon (a texture), link (an item link: the game's own tooltip),
-- quality (cell 1 in the item quality's colour), color ("|cffRRGGBB" for cell 1), bars = { [col] =
-- { value, max, r, g, b } } (drawn in that column, its cell's text over the bar), header (a gold
-- heading row: no hover, no click), and anything the section's tooltip, fill or click want }.

local _, ns = ...
local L = ns.L

local Wn = {}
ns.Window = Wn

local WIDTH, HEIGHT = 820, 520
local RAIL_W = 160
local BODY_X = RAIL_W + 16
local BODY_W = WIDTH - BODY_X - 28 -- room for the scroll bar
local ROW_H = 20
local MAX_ROWS = 20
local MAX_COLS = 8
local MAX_TILES = 4
local ICON = 16
local TOP = -46      -- under the header
local TILES_H = 50
local COLHEAD_H = 18
local BOTTOM = 34    -- above the footer
local BAR_TEXTURE = "Interface\\TargetingFrame\\UI-StatusBar"
local HIGHLIGHT = "Interface\\QuestFrame\\UI-QuestTitleHighlight"
local GOLD = "|cffffd100"

Wn.GROUPS = {
  { key = "me", label = L["Me"] },
  { key = "world", label = L["World"] },
  { key = "combat", label = L["Combat and gold"] },
}
local GROUP_INDEX = { me = 1, world = 2, combat = 3 }
local CHARACTER_MODES = { all = true, one = true, current = true }

local sections, byKey = {}, {}
Wn.sections = byKey

-- What the window shows: the section, the picker's choice (nil: all characters), each section's
-- search and sort, and the scroll offset.
Wn.state = { section = nil, character = nil, search = {}, sort = {}, filter = {}, offset = 0 }
-- Each section's built rows: { rows, sorted, columns, version, character, search, builds }.
local views = {}
Wn.views = views

-- Text --------------------------------------------------------------------------------------------

-- Colour codes, textures and link wrappers out of a string, for matching and sorting.
local function stripCodes(s)
  s = s:gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|cn[^:]*:", ""):gsub("|r", "")
  s = s:gsub("|T.-|t", ""):gsub("|H.-|h(.-)|h", "%1")
  return s
end
Wn.Strip = stripCodes

-- Whether `text` contains `search` (lowercased already, as ctx.search is), ignoring colour codes.
function Wn.Match(text, search)
  if search == nil or search == "" then return true end
  if type(text) ~= "string" or ns.IsSecret(text) then return false end
  return stripCodes(text):lower():find(search, 1, true) ~= nil
end

local function normalize(text)
  if type(text) ~= "string" then return "" end
  return (text:lower():gsub("^%s+", ""):gsub("%s+$", ""))
end

-- "|cffRRGGBB" for 0..1 colour parts (whole numbers for %x, as every Lua takes them).
local function rgbHex(r, g, b)
  local function part(v) return math.max(0, math.min(255, math.floor((tonumber(v) or 0) * 255 + 0.5))) end
  return string.format("|cff%02x%02x%02x", part(r), part(g), part(b))
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
      hex = rgbHex(c.r, c.g, c.b)
    end
  end
  qualityHex[q] = hex or "|cffffffff"
  return qualityHex[q]
end

-- "|cffRRGGBB" for a class token ("PALADIN"), from the client's RAID_CLASS_COLORS.
function Wn.ClassColor(class)
  local colors = _G.RAID_CLASS_COLORS
  local c = type(class) == "string" and type(colors) == "table" and colors[class]
  if type(c) == "table" then
    if type(c.colorStr) == "string" then return "|c" .. c.colorStr end
    if type(c.r) == "number" then return rgbHex(c.r, c.g, c.b) end
  end
  return "|cffffffff"
end

-- The class's name in the client's language ("Paladin"), else the token as it is.
function Wn.ClassName(class)
  if type(class) ~= "string" then return nil end
  local names = _G.LOCALIZED_CLASS_NAMES_MALE
  local name = type(names) == "table" and ns.NonEmpty(names[class])
  return name or (class:sub(1, 1) .. class:sub(2):lower())
end

-- Money as the client draws it (coin icons), else "1g 20s 5c".
function Wn.Money(copper)
  copper = tonumber(copper)
  if not copper then return "" end
  copper = math.floor(copper)
  local abs = math.abs(copper)
  local text = ns.Try(ns.Fn("C_CurrencyInfo", "GetCoinTextureString"), abs)
  if not ns.NonEmpty(text) then text = ns.Try(ns.Fn("GetCoinTextureString"), abs) end
  if ns.NonEmpty(text) then return (copper < 0 and "-" or "") .. text end
  return ns.MoneyText(copper)
end

-- An item's icon by id (the client knows it without loading the item), or nil.
function Wn.ItemIcon(id)
  id = tonumber(id)
  if not id then return nil end
  local icon = ns.Try(ns.Fn("C_Item", "GetItemIconByID"), id)
  if icon == nil then icon = ns.Try(ns.Fn("GetItemIcon"), id) end
  if icon == nil or ns.IsSecret(icon) then return nil end
  return icon
end

-- The characters -----------------------------------------------------------------------------------

local function currentGuid()
  local g = ns.Try(UnitGUID, "player")
  if type(g) ~= "string" or ns.IsSecret(g) then return nil end
  return g
end
Wn.CurrentGuid = currentGuid

-- Every character on the account (RynaDB.characters): { guid, name, class, level, current, rec },
-- the one you're on first, then by name.
function Wn.Characters()
  local out = {}
  local db = ns.db
  if not db or type(db.characters) ~= "table" then return out end
  local current = currentGuid()
  for guid, rec in pairs(db.characters) do
    if type(guid) == "string" and type(rec) == "table" then
      out[#out + 1] = {
        guid = guid, rec = rec, class = rec.class, level = rec.level, current = guid == current,
        name = ns.Status.Plain(ns.NonEmpty(rec.fullName)) or L["Unknown character"],
      }
    end
  end
  table.sort(out, function(a, b)
    if a.current ~= b.current then return a.current end
    local na, nb = a.name:lower(), b.name:lower()
    if na ~= nb then return na < nb end
    return a.guid < b.guid
  end)
  return out
end

-- The context a section is drawn for (see the top of the file).
function Wn.Context(section)
  local db = ns.db
  local chosen = Wn.state.character
  if chosen and not (db and type(db.characters) == "table" and type(db.characters[chosen]) == "table") then
    chosen = nil
    Wn.state.character = nil
  end
  local current = currentGuid()
  local character = chosen
  if section.characters == "current" then
    character = current
  elseif section.characters == "one" and character == nil then
    character = current
  end
  local search = ""
  if section.search then search = Wn.state.search[section.key] or "" end
  local filter
  if section.filter then filter = Wn.state.filter[section.key] end
  return { section = section.key, character = character, current = current, all = character == nil,
    search = search, filter = filter }
end

-- The picker's choices for a section: { guid (nil: All characters), label }.
function Wn.Choices(section)
  local out = {}
  if section.characters == "all" then out[1] = { guid = nil, label = L["All characters"] } end
  for _, c in ipairs(Wn.Characters()) do
    out[#out + 1] = { guid = c.guid, label = Wn.ClassColor(c.class) .. c.name .. "|r" }
  end
  return out
end

-- What the picker reads for a context.
function Wn.PickerText(ctx)
  if ctx.character == nil then return L["All characters"] end
  local db = ns.db
  local rec = db and type(db.characters) == "table" and db.characters[ctx.character]
  if type(rec) ~= "table" then return L["Unknown character"] end
  return Wn.ClassColor(rec.class) .. (ns.Status.Plain(ns.NonEmpty(rec.fullName)) or L["Unknown character"]) .. "|r"
end

-- Sections -----------------------------------------------------------------------------------------

local function sortSections()
  table.sort(sections, function(a, b)
    local ga, gb = GROUP_INDEX[a.group], GROUP_INDEX[b.group]
    if ga ~= gb then return ga < gb end
    if a.order ~= b.order then return a.order < b.order end
    return a.seq < b.seq
  end)
end

function Wn.AddSection(spec)
  if type(spec) ~= "table" or type(spec.key) ~= "string" or spec.key == "" or byKey[spec.key]
    or not GROUP_INDEX[spec.group] or type(spec.rows) ~= "function"
    or (type(spec.columns) ~= "table" and type(spec.columns) ~= "function") then
    return false
  end
  local function optional(name)
    local v = spec[name]
    if v ~= nil and type(v) ~= "function" then return false end
    return true
  end
  for _, name in ipairs({ "fill", "tiles", "tooltip", "click", "footer", "version", "filter" }) do
    if not optional(name) then return false end
  end
  local mode = spec.characters or "all"
  if not CHARACTER_MODES[mode] then return false end
  local s = {
    key = spec.key, group = spec.group, label = tostring(spec.label or spec.key),
    title = spec.title and tostring(spec.title) or nil, order = tonumber(spec.order) or 100,
    characters = mode, search = spec.search ~= false, sortable = spec.sortable ~= false,
    icons = spec.icons and true or false, columns = spec.columns, rows = spec.rows, fill = spec.fill,
    tiles = spec.tiles, tooltip = spec.tooltip, click = spec.click, footer = spec.footer,
    version = spec.version, empty = type(spec.empty) == "string" and spec.empty or nil,
    filter = spec.filter, seq = #sections + 1,
  }
  sections[#sections + 1] = s
  byKey[s.key] = s
  sortSections()
  if Wn.frame then ns.Run("ryna window", Wn.LayoutRail) end
  return true
end

-- The sections in rail order: { key, group, label }.
function Wn.List()
  local out = {}
  for i, s in ipairs(sections) do out[i] = { key = s.key, group = s.group, label = s.label } end
  return out
end

local function settings()
  local db = ns.db
  if not db or type(db.settings) ~= "table" then return nil end
  if type(db.settings.window) ~= "table" then db.settings.window = {} end
  return db.settings.window
end

local function defaultSection()
  local saved = settings()
  if saved and byKey[saved.section] then return saved.section end
  return sections[1] and sections[1].key or nil
end

-- Building and sorting a section's rows --------------------------------------------------------------

local function columnsFor(section, ctx)
  local c = section.columns
  if type(c) == "function" then c = c(ctx) end
  return type(c) == "table" and c or {}
end

-- Builds the section's rows when it was never built, or its version or the context changed.
-- Returns the view and whether it was rebuilt.
function Wn.View(section, ctx)
  local v = views[section.key]
  if not v then
    v = { rows = {}, sorted = {}, builds = 0 }
    views[section.key] = v
  end
  local version
  if section.version then version = section.version() end
  if v.built and v.version == version and v.character == ctx.character and v.search == ctx.search
    and v.filter == ctx.filter then
    return v, false
  end
  local rows = section.rows(ctx)
  -- Marked built only once rows() returned, so a section that failed is tried again next draw.
  v.built, v.version, v.character, v.search, v.filter = true, version, ctx.character, ctx.search, ctx.filter
  v.rows = type(rows) == "table" and rows or {}
  v.columns = columnsFor(section, ctx)
  v.builds = v.builds + 1
  v.sortedFrom = nil
  v.filled = {} -- the rows section.fill has filled, by row (emptied each minute, so ages move on)
  v.filledMinute = math.floor((tonumber(GetTime()) or 0) / 60)
  return v, true
end

local function sortKey(row, col)
  local s = type(row.sort) == "table" and row.sort[col] or nil
  if s == nil and type(row.cells) == "table" then s = row.cells[col] end
  if type(s) == "string" then
    if ns.IsSecret(s) then return nil end
    s = stripCodes(s):lower()
  elseif type(s) ~= "number" then
    s = nil
  end
  return s
end

-- Numbers before text; nothing (nil) last whichever way the column sorts.
local TYPE_RANK = { number = 1, string = 2 }
local function before(a, b, desc)
  if a == nil or b == nil then
    if a == nil and b == nil then return nil end
    return b == nil
  end
  local ta, tb = type(a), type(b)
  if ta ~= tb then return TYPE_RANK[ta] < TYPE_RANK[tb] end
  if a == b then return nil end
  if desc then return a > b end
  return a < b
end

-- The view's rows in the chosen column's order, or the section's own order.
local function applySort(v, sort)
  local tag = sort and (sort.col .. (sort.desc and "d" or "a")) or ""
  if v.sortedFrom == v.rows and v.sortTag == tag then return end
  v.sortedFrom, v.sortTag = v.rows, tag
  if not sort then
    v.sorted = v.rows
    return
  end
  local list, keys, index = {}, {}, {}
  for i, r in ipairs(v.rows) do
    list[i] = r
    keys[r] = sortKey(r, sort.col)
    index[r] = i
  end
  table.sort(list, function(a, b)
    local first = before(keys[a], keys[b], sort.desc)
    if first ~= nil then return first end
    return index[a] < index[b]
  end)
  v.sorted = list
end

-- A heading click: this column's first direction (numbers, right-aligned, most first), then the
-- other, then the section's own order again.
function Wn.SortBy(col)
  local section = byKey[Wn.state.section]
  if not section or not section.sortable then return end
  local v = views[section.key]
  local spec = v and v.columns and v.columns[col]
  if not spec or spec.sort == false then return end
  local cur = Wn.state.sort[section.key]
  local firstDesc = spec.justify == "RIGHT"
  if not cur or cur.col ~= col then
    Wn.state.sort[section.key] = { col = col, desc = firstDesc }
  elseif cur.desc == firstDesc then
    Wn.state.sort[section.key] = { col = col, desc = not firstDesc }
  else
    Wn.state.sort[section.key] = nil
  end
  Wn.state.offset = 0
  Wn.Draw()
end

-- The frame ------------------------------------------------------------------------------------------

local function create(kind, name, parent, templates)
  for _, template in ipairs(templates or {}) do
    local ok, f = pcall(CreateFrame, kind, name, parent, template)
    if ok and f then return f end
  end
  local ok, f = pcall(CreateFrame, kind, name, parent)
  if ok then return f end
  return nil
end

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

local function backdrop(f, alpha)
  if f.SetBackdrop and pcall(f.SetBackdrop, f, {
    bgFile = "Interface\\Tooltips\\UI-Tooltip-Background",
    edgeFile = "Interface\\Tooltips\\UI-Tooltip-Border",
    tile = true, tileSize = 16, edgeSize = 16,
    insets = { left = 4, right = 4, top = 4, bottom = 4 },
  }) then
    pcall(f.SetBackdropColor, f, 0.04, 0.04, 0.05, alpha)
    pcall(f.SetBackdropBorderColor, f, 0.55, 0.55, 0.55, 1)
    return
  end
  local bg = f:CreateTexture(nil, "BACKGROUND")
  bg:SetAllPoints()
  bg:SetColorTexture(0.04, 0.04, 0.05, alpha)
end

local function textButton(parent, fontObject, w, h)
  local b = CreateFrame("Button", nil, parent)
  b:SetSize(w, h)
  local fs = fontString(b, fontObject)
  fs:SetPoint("LEFT", b, "LEFT", 6, 0)
  fs:SetWidth(w - 10)
  fs:SetJustifyH("LEFT")
  b.text = fs
  local hl = b:CreateTexture(nil, "HIGHLIGHT")
  hl:SetAllPoints()
  pcall(hl.SetTexture, hl, HIGHLIGHT)
  if hl.SetBlendMode then pcall(hl.SetBlendMode, hl, "ADD") end
  b.hl = hl
  return b
end

local function pushButton(parent, label, w, h)
  local b = create("Button", nil, parent, { "UIPanelButtonTemplate" })
  b:SetSize(w, h)
  if not b:GetFontString() then
    local fs = fontString(b, "GameFontNormalSmall")
    fs:SetAllPoints()
    fs:SetJustifyH("CENTER")
    b:SetFontString(fs)
  end
  b:SetText(label or "")
  return b
end

-- The rail ---------------------------------------------------------------------------------------------

local function markRail()
  local f = Wn.frame
  if not f then return end
  for key, b in pairs(f.railButtons) do
    local on = key == Wn.state.section
    if on then b:LockHighlight() else b:UnlockHighlight() end
    if on then b.text:SetTextColor(1, 1, 1) else b.text:SetTextColor(1, 0.82, 0) end
  end
end

-- Places the group headings and the section buttons: a group with no section isn't drawn.
function Wn.LayoutRail()
  local f = Wn.frame
  if not f then return end
  local rail = f.rail
  local y = -38
  for _, g in ipairs(Wn.GROUPS) do
    local head = f.groupHeads[g.key]
    local any = false
    for _, s in ipairs(sections) do
      if s.group == g.key then
        if not any then
          head:ClearAllPoints()
          head:SetPoint("TOPLEFT", rail, "TOPLEFT", 10, y)
          head:Show()
          y = y - 18
          any = true
        end
        local b = f.railButtons[s.key]
        if not b then
          b = textButton(rail, "GameFontNormal", RAIL_W - 12, 20)
          local key = s.key
          b:SetScript("OnClick", function() ns.Run("ryna window", Wn.Select, key) end)
          f.railButtons[key] = b
        end
        b.text:SetText(s.label)
        b:ClearAllPoints()
        b:SetPoint("TOPLEFT", rail, "TOPLEFT", 6, y)
        b:Show()
        y = y - 22
      end
    end
    if any then y = y - 8 else head:Hide() end
  end
  markRail()
end

-- The pickers ---------------------------------------------------------------------------------------------
--
-- Two of them, built the same way: the character picker (every section) and the section's own filter
-- (spec.filter, a zone on Chronicle's Kills and Loot), shown only on a section that has one. A source
-- says what a picker lists and does: choices(section, ctx) -> { { value, label } }, chosen(value),
-- choose(value), text(section, ctx) and enabled(section). A nil value travels as NONE.

local NONE = "\001none"

local characterSource = {
  choices = function(section)
    local out = {}
    for i, c in ipairs(Wn.Choices(section)) do out[i] = { value = c.guid, label = c.label } end
    return out
  end,
  chosen = function(value)
    local section = byKey[Wn.state.section]
    if not section then return false end
    return Wn.Context(section).character == value
  end,
  choose = function(value) Wn.Choose(value) end,
  text = function(_, ctx) return Wn.PickerText(ctx) end,
  enabled = function(section) return section.characters ~= "current" end,
}

local filterSource = {
  choices = function(section, ctx) return Wn.FilterChoices(section, ctx) end,
  chosen = function(value)
    local key = Wn.state.section
    return byKey[key] ~= nil and Wn.state.filter[key] == value
  end,
  choose = function(value) Wn.SetFilter(Wn.state.section, value) end,
  text = function(section, ctx) return Wn.FilterText(section, ctx) end,
  enabled = function() return true end,
}

local function encode(value)
  if value == nil then return NONE end
  return value
end
local function decode(data)
  if data == NONE then return nil end
  return data
end

-- The game's own dropdown (the 11.x menu) where the client has it.
local function modernPicker(f, src)
  local dd = create("DropdownButton", nil, f, { "WowStyle1DropdownTemplate" })
  if not dd or not dd.SetupMenu then
    if dd then dd:Hide() end
    return nil
  end
  local ok = pcall(dd.SetupMenu, dd, function(_, root)
    local section = byKey[Wn.state.section]
    if not section then return end
    for _, c in ipairs(src.choices(section, Wn.Context(section))) do
      root:CreateRadio(c.label, function(data) return src.chosen(decode(data)) end, function(data)
        ns.Run("ryna window picker", src.choose, decode(data))
      end, encode(c.value))
    end
  end)
  if not ok then
    dd:Hide()
    return nil
  end
  function dd.sync(section)
    if dd.GenerateMenu then pcall(dd.GenerateMenu, dd) end
    if dd.SetEnabled then pcall(dd.SetEnabled, dd, src.enabled(section)) end
  end
  return dd
end

-- Else a button that opens a list of its own under it (at most PICKER_MAX choices).
local PICKER_MAX = 24
local function plainPicker(f, src, width)
  local b = pushButton(f, "", width, 22)
  local list = create("Frame", nil, f, { "BackdropTemplate" })
  backdrop(list, 0.97)
  list:SetFrameStrata("FULLSCREEN_DIALOG")
  list:SetPoint("TOPLEFT", b, "BOTTOMLEFT", 0, -2)
  list:SetWidth(width + 10)
  list:Hide()
  list.buttons = {}
  b.list = list
  function b.open()
    local section = byKey[Wn.state.section]
    if not section then return end
    local choices = src.choices(section, Wn.Context(section))
    local n = math.min(#choices, PICKER_MAX)
    for i = 1, n do
      local c = choices[i]
      local item = list.buttons[i]
      if not item then
        item = textButton(list, "GameFontHighlightSmall", width - 2, 18)
        list.buttons[i] = item
      end
      item:ClearAllPoints()
      item:SetPoint("TOPLEFT", list, "TOPLEFT", 6, -6 - (i - 1) * 18)
      item.text:SetText((src.chosen(c.value) and "> " or "  ") .. c.label)
      local value = c.value
      item:SetScript("OnClick", function()
        list:Hide()
        ns.Run("ryna window picker", src.choose, value)
      end)
      item:Show()
    end
    for i = n + 1, #list.buttons do list.buttons[i]:Hide() end
    list:SetHeight(12 + n * 18)
    list:Show()
  end
  b:SetScript("OnClick", function()
    if list:IsShown() then list:Hide() else ns.Run("ryna window picker", b.open) end
  end)
  function b.sync(section, ctx)
    b:SetText(src.text(section, ctx))
    if src.enabled(section) then b:Enable() else b:Disable() end
  end
  return b
end

local function picker(f, src, width)
  local p = modernPicker(f, src) or plainPicker(f, src, width)
  p:SetWidth(width)
  return p
end

-- Picks a character (nil: all characters) for every section that follows the picker.
function Wn.Choose(guid)
  Wn.state.character = guid
  Wn.state.offset = 0
  Wn.Draw()
end

-- A section's filter choices: { { value, label } }, the first (value nil) its default.
function Wn.FilterChoices(section, ctx)
  if type(section.filter) ~= "function" then return {} end
  local list = section.filter(ctx)
  return type(list) == "table" and list or {}
end

-- What the filter picker reads: the chosen choice's label (the first's when none is).
function Wn.FilterText(section, ctx)
  local first
  for _, c in ipairs(Wn.FilterChoices(section, ctx)) do
    first = first or c.label
    if c.value == ctx.filter then return c.label end
  end
  return first or ""
end

-- Sets section `key`'s filter (nil: its default) and redraws it if it is on screen.
function Wn.SetFilter(key, value)
  local section = byKey[key]
  if not section or not section.filter then return false end
  if value ~= nil and type(value) ~= "string" then return false end
  Wn.state.filter[key] = value
  if Wn.state.section == key then
    Wn.state.offset = 0
    Wn.Draw()
  end
  return true
end

-- The search box -------------------------------------------------------------------------------------------

local function searchBox(parent)
  for _, template in ipairs({ "SearchBoxTemplate", "InputBoxTemplate" }) do
    local ok, box = pcall(CreateFrame, "EditBox", "RynaWindowSearch", parent, template)
    if ok and box then return box end
  end
  local box = CreateFrame("EditBox", "RynaWindowSearch", parent)
  local bg = box:CreateTexture(nil, "BACKGROUND")
  bg:SetAllPoints()
  bg:SetColorTexture(1, 1, 1, 0.08)
  box:SetFontObject(_G.ChatFontNormal or "GameFontHighlightSmall")
  box:SetTextInsets(4, 4, 0, 0)
  return box
end

-- The search for the section on screen (`fromBox`: typed in the box, which already shows it).
function Wn.Search(text, fromBox)
  local section = byKey[Wn.state.section]
  if not section or not section.search then return end
  Wn.state.search[section.key] = normalize(text)
  Wn.state.offset = 0
  local f = Wn.frame
  if f and not fromBox then
    f.search.filling = true
    f.search:SetText(type(text) == "string" and text or "")
    f.search.filling = false
  end
  Wn.Draw()
end

local function onSearchChanged()
  local f = Wn.frame
  if not f then return end
  Wn.Search(f.search:GetText(), true)
end

-- Rows -------------------------------------------------------------------------------------------------------

local function layoutColumns(f, columns, icons)
  local function place(row, cols, bars)
    local x = 4 + (icons and (ICON + 4) or 0)
    for c = 1, MAX_COLS do
      local spec = columns[c]
      local w = spec and tonumber(spec.width) or 0
      local fs = cols[c]
      fs:ClearAllPoints()
      local bar = bars and bars[c]
      if bar then bar:ClearAllPoints() end
      if w > 0 then
        fs:SetPoint("LEFT", row, "LEFT", x, 0)
        fs:SetWidth(w - 8)
        fs:SetJustifyH(spec.justify == "RIGHT" and "RIGHT" or "LEFT")
        fs:Show()
        if bar then
          bar:SetPoint("LEFT", row, "LEFT", x, 0)
          bar:SetWidth(w - 8)
        end
      else
        fs:Hide()
        if bar then bar:Hide() end
      end
      x = x + w
    end
  end
  place(f.head, f.headCells, nil)
  for r = 1, MAX_ROWS do
    local row = f.rows[r]
    place(row, row.cols, row.bars)
    if icons then row.icon:Show() else row.icon:Hide() end
  end
  f.columnsFor, f.iconsFor = columns, icons
end

local function fillRow(row, data, columns, icons)
  row.data = data
  local header = data.header and true or false
  if icons then
    if data.icon and not header then
      pcall(row.icon.SetTexture, row.icon, data.icon)
      row.icon:Show()
    else
      row.icon:Hide()
    end
  end
  if header then row.stripe:Hide() else row.stripe:Show() end
  for c = 1, MAX_COLS do
    local spec = columns[c]
    local fs, bar = row.cols[c], row.bars[c]
    if spec then
      local text = type(data.cells) == "table" and data.cells[c] or nil
      if type(text) ~= "string" then text = text ~= nil and tostring(text) or "" end
      if header then
        text = GOLD .. text .. "|r"
      elseif c == 1 and data.quality ~= nil then
        text = Wn.QualityColor(data.quality) .. text .. "|r"
      elseif c == 1 and type(data.color) == "string" then
        text = data.color .. text .. "|r"
      end
      local b = spec.kind == "bar" and not header and type(data.bars) == "table" and data.bars[c] or nil
      if type(b) == "table" and tonumber(b.max) and tonumber(b.max) > 0 then
        local max = tonumber(b.max)
        local value = math.max(0, math.min(tonumber(b.value) or 0, max))
        bar:SetMinMaxValues(0, max)
        bar:SetValue(value)
        bar:SetStatusBarColor(b.r or 0.26, b.g or 0.52, b.b or 0.9)
        bar.text:SetText(text)
        bar:Show()
        fs:SetText("")
      else
        bar:Hide()
        fs:SetText(text)
      end
    else
      fs:SetText("")
      bar:Hide()
    end
  end
end

-- The row tooltip: the game's own for a row's item link, then the section's lines.
Wn.tooltipSection = nil

local function rowTooltip(row)
  local data = row.data
  local section = byKey[Wn.state.section]
  local tip = _G.GameTooltip
  if not tip or not data or data.header or not section then return end
  if not data.link and not section.tooltip then return end
  tip:SetOwner(row, "ANCHOR_RIGHT")
  tip:ClearLines()
  Wn.tooltipSection = section.key
  local hadLink = false
  if type(data.link) == "string" then hadLink = pcall(tip.SetHyperlink, tip, data.link) and true or false end
  local ok, err = true, nil
  if section.tooltip then ok, err = pcall(section.tooltip, data, tip, Wn.ctx, hadLink) end
  Wn.tooltipSection = nil
  if not ok then ns.ReportError("window tooltip " .. section.key, err) end
  local lines = tip.NumLines and ns.Try(tip.NumLines, tip)
  if lines == nil or lines > 0 then tip:Show() else tip:Hide() end
end

-- The key of the section whose row tooltip is being drawn right now, or nil (a mod's tooltip hook
-- can leave out a line the section already shows). With `tooltip`, also when that tooltip belongs to
-- one of the window's rows: the client redraws a tooltip by itself once an item it hadn't loaded
-- arrives, and runs the hooks again then, after the window's own drawing is over.
function Wn.TooltipSection(tooltip)
  if Wn.tooltipSection then return Wn.tooltipSection end
  if type(tooltip) == "table" and type(tooltip.GetOwner) == "function" and shown() then
    local owner = ns.Try(tooltip.GetOwner, tooltip)
    if type(owner) == "table" and rawget(owner, "rynaWindowRow") then return Wn.state.section end
  end
  return nil
end

-- Drawing -----------------------------------------------------------------------------------------------------

local function drawTiles(f, section, ctx)
  local tiles = section.tiles and section.tiles(ctx) or nil
  local n = type(tiles) == "table" and math.min(#tiles, MAX_TILES) or 0
  local w = n > 0 and math.floor((BODY_W - (n - 1) * 8) / n) or 0
  for i = 1, MAX_TILES do
    local tile = f.tiles[i]
    local t = i <= n and tiles[i] or nil
    if type(t) == "table" then
      tile:ClearAllPoints()
      tile:SetPoint("TOPLEFT", f, "TOPLEFT", BODY_X + (i - 1) * (w + 8), TOP)
      tile:SetSize(w, TILES_H - 8)
      tile.label:SetText(tostring(t.label or ""))
      tile.value:SetText(tostring(t.value or ""))
      tile.sub:SetText(t.sub and tostring(t.sub) or "")
      tile:Show()
    else
      tile:Hide()
    end
  end
  return n
end

local function sortArrow(section, col)
  local sort = section.sortable and Wn.state.sort[section.key] or nil
  if not sort or sort.col ~= col then return "" end
  return sort.desc and " v" or " ^"
end

-- Draws the section on screen: header, tiles, the rows from the scroll offset, the footer.
function Wn.Draw()
  if not shown() then return end
  local f = Wn.frame
  local section = byKey[Wn.state.section]
  if not section then return end
  local ctx = Wn.Context(section)
  Wn.ctx = ctx
  local v = Wn.View(section, ctx)

  f.title:SetText(section.title or section.label)
  -- The picker and the search box follow the section and the choice, not every redraw: a kill
  -- redraws the window, and must neither close the picker's list nor undo what is being typed.
  local pickerFor = section.key .. ":" .. tostring(ctx.character)
  if f.picker and f.picker.sync and f.pickerFor ~= pickerFor then
    f.picker.sync(section, ctx)
    f.pickerFor = pickerFor
  end
  local filterFor = section.filter and (pickerFor .. ":" .. tostring(ctx.filter)) or false
  if f.filter and f.filterFor ~= filterFor then
    if filterFor then
      f.filter:Show()
      if f.filter.sync then f.filter.sync(section, ctx) end
    else
      f.filter:Hide()
      if f.filter.list then f.filter.list:Hide() end
    end
    f.filterFor = filterFor
  end
  if f.searchFor ~= section.key then
    local box = f.search
    box.filling = true
    box:SetText(section.search and (Wn.state.search[section.key] or "") or "")
    box.filling = false
    if box.SetEnabled then pcall(box.SetEnabled, box, section.search) end
    box:SetAlpha(section.search and 1 or 0.4)
    if not section.search and box.ClearFocus then box:ClearFocus() end
    f.searchFor = section.key
  end

  local nTiles = drawTiles(f, section, ctx)
  local top = TOP - (nTiles > 0 and TILES_H or 0)
  local visible = math.min(MAX_ROWS, math.floor((HEIGHT + top - COLHEAD_H - BOTTOM) / ROW_H))
  if f.listTop ~= top then
    f.head:ClearAllPoints()
    f.head:SetPoint("TOPLEFT", f, "TOPLEFT", BODY_X, top)
    f.list:ClearAllPoints()
    f.list:SetPoint("TOPLEFT", f, "TOPLEFT", BODY_X, top - COLHEAD_H)
    f.list:SetHeight(visible * ROW_H)
    f.bar:SetHeight(visible * ROW_H)
    f.listTop = top
  end
  Wn.visible = visible

  local columns = v.columns
  if f.columnsFor ~= columns or f.iconsFor ~= section.icons then layoutColumns(f, columns, section.icons) end
  for c = 1, MAX_COLS do
    local spec = columns[c]
    f.headCells[c]:SetText(spec and ((spec.title or "") .. sortArrow(section, c)) or "")
    if spec ~= nil and section.sortable and spec.sort ~= false and (spec.title or "") ~= "" then
      f.headButtons[c]:Show()
    else
      f.headButtons[c]:Hide()
    end
  end

  local sort = section.sortable and Wn.state.sort[section.key] or nil
  local sortSpec = sort and columns[sort.col]
  if sort and not (sortSpec and sortSpec.sort ~= false and (sortSpec.title or "") ~= "") then sort = nil end
  applySort(v, sort)
  local list = v.sorted
  local count = #list
  local maxOffset = math.max(0, count - visible)
  local offset = math.max(0, math.min(Wn.state.offset, maxOffset))
  Wn.state.offset = offset
  for r = 1, MAX_ROWS do
    local row = f.rows[r]
    local i = offset + r
    local data = r <= visible and list[i] or nil
    if type(data) == "table" then
      local minute = math.floor((tonumber(GetTime()) or 0) / 60)
      if v.filledMinute ~= minute then
        v.filled, v.filledMinute = {}, minute
      end
      if section.fill and not v.filled[data] then
        section.fill(data, ctx)
        v.filled[data] = true
      end
      fillRow(row, data, columns, section.icons)
      row:Show()
    else
      row.data = nil
      row:Hide()
    end
  end
  f.empty:SetText(count == 0 and (ctx.search ~= "" and L["Nothing matches."]
    or section.empty or L["Nothing recorded yet."]) or "")
  f.footer:SetText(section.footer and tostring(section.footer(ctx) or "") or "")

  local bar = f.bar
  bar.syncing = true
  bar:SetMinMaxValues(0, maxOffset)
  bar:SetValue(offset)
  bar.syncing = false
  if maxOffset > 0 then bar:Show() else bar:Hide() end
  local over = f.hoverRow
  if over and over:IsShown() then rowTooltip(over) end
end

-- Showing a section ------------------------------------------------------------------------------------------

function Wn.Select(key)
  if not byKey[key] then return end
  if Wn.state.section ~= key then
    Wn.state.offset = 0
    for _, name in ipairs({ "picker", "filter" }) do
      local list = Wn.frame and Wn.frame[name] and Wn.frame[name].list
      if list then list:Hide() end
    end
  end
  Wn.state.section = key
  local saved = settings()
  if saved then saved.section = key end
  markRail()
  Wn.Draw()
end

local function place(f)
  local pos = settings()
  f:ClearAllPoints()
  if pos and type(pos.x) == "number" and type(pos.y) == "number" and type(pos.point) == "string"
    and pcall(f.SetPoint, f, pos.point, _G.UIParent, pos.relativePoint or pos.point, pos.x, pos.y) then
    return
  end
  f:SetPoint("CENTER", _G.UIParent, "CENTER", 0, 40)
end

local function savePosition(f)
  local point, _, relativePoint, x, y = f:GetPoint(1)
  local pos = settings()
  if not pos or type(point) ~= "string" or type(x) ~= "number" or type(y) ~= "number" then return end
  pos.point, pos.relativePoint = point, relativePoint or point
  pos.x, pos.y = math.floor(x + 0.5), math.floor(y + 0.5)
end

function Wn.Create()
  if Wn.frame then return Wn.frame end
  local f = create("Frame", "RynaWindow", _G.UIParent, { "BackdropTemplate" })
  Wn.frame = f
  f:SetSize(WIDTH, HEIGHT)
  f:SetFrameStrata("DIALOG")
  if f.SetToplevel then f:SetToplevel(true) end
  f:SetClampedToScreen(true)
  f:SetMovable(true)
  f:EnableMouse(true)
  f:RegisterForDrag("LeftButton")
  f:SetScript("OnDragStart", function(self) self:StartMoving() end)
  f:SetScript("OnDragStop", function(self)
    self:StopMovingOrSizing()
    if self.SetUserPlaced then pcall(self.SetUserPlaced, self, false) end -- the place is Ryna's to keep
    ns.Run("ryna window", savePosition, self)
  end)
  backdrop(f, 0.94)
  f:Hide()
  -- Escape closes it, like the game's own windows.
  if type(_G.UISpecialFrames) == "table" then table.insert(_G.UISpecialFrames, "RynaWindow") end

  local close = create("Button", nil, f, { "UIPanelCloseButton" })
  if close and not close:GetNormalTexture() then
    close:Hide()
    close = pushButton(f, "x", 22, 22)
  end
  close:SetPoint("TOPRIGHT", f, "TOPRIGHT", -2, -2)
  close:SetScript("OnClick", function() f:Hide() end)

  -- The rail.
  local rail = CreateFrame("Frame", nil, f)
  rail:SetPoint("TOPLEFT", f, "TOPLEFT", 6, -6)
  rail:SetPoint("BOTTOMLEFT", f, "BOTTOMLEFT", 6, 6)
  rail:SetWidth(RAIL_W)
  local railBg = rail:CreateTexture(nil, "BACKGROUND")
  railBg:SetAllPoints()
  railBg:SetColorTexture(1, 1, 1, 0.03)
  f.rail = rail
  local name = fontString(rail, "GameFontNormalLarge")
  name:SetPoint("TOPLEFT", rail, "TOPLEFT", 10, -10)
  name:SetText("Ryna")
  f.groupHeads, f.railButtons = {}, {}
  for _, g in ipairs(Wn.GROUPS) do
    local head = fontString(rail, "GameFontDisableSmall")
    head:SetText(g.label:upper())
    head:Hide()
    f.groupHeads[g.key] = head
  end

  -- The header: title, picker, search.
  f.title = fontString(f, "GameFontNormalLarge")
  f.title:SetPoint("TOPLEFT", f, "TOPLEFT", BODY_X, -16)
  local search = searchBox(f)
  search:SetSize(170, 20)
  search:SetPoint("TOPRIGHT", f, "TOPRIGHT", -34, -14)
  search:SetAutoFocus(false)
  if search.SetMaxLetters then search:SetMaxLetters(40) end
  -- Hooked, so the search box template keeps its own handler (its "Search" hint and clear button).
  local function onTextChanged(self)
    if self.filling then return end
    ns.Debounce("ryna-window-search", 0.2, onSearchChanged)
  end
  if search.GetScript and search:GetScript("OnTextChanged") and search.HookScript then
    search:HookScript("OnTextChanged", onTextChanged)
  else
    search:SetScript("OnTextChanged", onTextChanged)
  end
  search:SetScript("OnEscapePressed", function(self) self:ClearFocus() end)
  search:SetScript("OnEnterPressed", function(self) self:ClearFocus() end)
  search.filling = false
  f.search = search
  f.picker = picker(f, characterSource, 190)
  f.picker:SetPoint("RIGHT", search, "LEFT", -12, 0)
  f.filter = picker(f, filterSource, 150)
  f.filter:SetPoint("RIGHT", f.picker, "LEFT", -10, 0)
  f.filter:Hide()

  -- Tiles.
  f.tiles = {}
  for i = 1, MAX_TILES do
    local tile = CreateFrame("Frame", nil, f)
    local bg = tile:CreateTexture(nil, "BACKGROUND")
    bg:SetAllPoints()
    bg:SetColorTexture(1, 1, 1, 0.05)
    tile.label = fontString(tile, "GameFontDisableSmall")
    tile.label:SetPoint("TOPLEFT", tile, "TOPLEFT", 8, -5)
    tile.value = fontString(tile, "GameFontHighlightLarge")
    tile.value:SetPoint("TOPLEFT", tile.label, "BOTTOMLEFT", 0, -3)
    tile.sub = fontString(tile, "GameFontDisableSmall")
    tile.sub:SetPoint("LEFT", tile.value, "RIGHT", 6, 0)
    tile:Hide()
    f.tiles[i] = tile
  end

  -- Column headings (click to sort), the rows and the scroll bar.
  local head = CreateFrame("Frame", nil, f)
  head:SetSize(BODY_W, COLHEAD_H)
  f.head = head
  f.headCells, f.headButtons = {}, {}
  for c = 1, MAX_COLS do
    local fs = fontString(head, "GameFontNormalSmall")
    f.headCells[c] = fs
    local b = CreateFrame("Button", nil, head)
    b:SetAllPoints(fs)
    local col = c
    b:SetScript("OnClick", function() ns.Run("ryna window sort", Wn.SortBy, col) end)
    f.headButtons[c] = b
  end

  local list = CreateFrame("Frame", nil, f)
  list:SetWidth(BODY_W)
  list:EnableMouseWheel(true)
  f.list = list
  f.rows = {}
  for r = 1, MAX_ROWS do
    local row = CreateFrame("Button", nil, list)
    row:SetSize(BODY_W, ROW_H)
    row:SetPoint("TOPLEFT", list, "TOPLEFT", 0, -(r - 1) * ROW_H)
    local stripe = row:CreateTexture(nil, "BACKGROUND")
    stripe:SetAllPoints()
    stripe:SetColorTexture(1, 1, 1, r % 2 == 0 and 0.03 or 0)
    row.stripe = stripe
    local hl = row:CreateTexture(nil, "HIGHLIGHT")
    hl:SetAllPoints()
    hl:SetColorTexture(1, 1, 1, 0.08)
    row.icon = row:CreateTexture(nil, "ARTWORK")
    row.icon:SetSize(ICON, ICON)
    row.icon:SetPoint("LEFT", row, "LEFT", 4, 0)
    row.icon:Hide()
    row.rynaWindowRow = true
    row.cols, row.bars = {}, {}
    for c = 1, MAX_COLS do
      row.cols[c] = fontString(row, "GameFontHighlightSmall")
      local bar = CreateFrame("StatusBar", nil, row)
      bar:SetHeight(ROW_H - 6)
      pcall(bar.SetStatusBarTexture, bar, BAR_TEXTURE)
      local track = bar:CreateTexture(nil, "BACKGROUND")
      track:SetAllPoints()
      track:SetColorTexture(0, 0, 0, 0.5)
      bar.text = fontString(bar, "GameFontHighlightSmall")
      bar.text:SetPoint("CENTER", bar, "CENTER", 0, 0)
      bar:Hide()
      row.bars[c] = bar
    end
    row:SetScript("OnEnter", function(self)
      f.hoverRow = self
      ns.Run("ryna window", rowTooltip, self)
    end)
    row:SetScript("OnLeave", function()
      f.hoverRow = false
      local tip = _G.GameTooltip
      if tip then tip:Hide() end
    end)
    row:SetScript("OnClick", function(self)
      local section = byKey[Wn.state.section]
      local data = self.data
      if section and section.click and data and not data.header then
        ns.Run("ryna window click", section.click, data, Wn.ctx)
      end
    end)
    row.data = nil
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
  bar:SetWidth(10)
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
    if offset ~= Wn.state.offset then
      Wn.state.offset = offset
      ns.Run("ryna window", Wn.Draw)
    end
  end)
  f.bar = bar
  list:SetScript("OnMouseWheel", function(_, delta)
    Wn.state.offset = Wn.state.offset - (tonumber(delta) or 0) * 3
    ns.Run("ryna window", Wn.Draw)
  end)

  f.footer = fontString(f, "GameFontDisableSmall")
  f.footer:SetPoint("BOTTOMLEFT", f, "BOTTOMLEFT", BODY_X, 12)
  f.footer:SetWidth(BODY_W)
  f.footer:SetJustifyH("LEFT")

  f:SetScript("OnShow", function()
    ns.Run("ryna window", function()
      place(f)
      Wn.LayoutRail()
      Wn.Select(Wn.state.section or defaultSection())
    end)
  end)
  f:SetScript("OnHide", function()
    local tip = _G.GameTooltip
    if f.hoverRow and tip then tip:Hide() end
    f.hoverRow = false
    if f.picker and f.picker.list then f.picker.list:Hide() end
    if f.filter and f.filter.list then f.filter.list:Hide() end
  end)
  return f
end

-- Opens the window, on section `key` when given (else the one it was on, or the last remembered).
-- Returns false when no section is registered.
function Wn.Open(key)
  if #sections == 0 then return false end
  local f = Wn.Create()
  local want = (key and byKey[key] and key) or Wn.state.section or defaultSection()
  if f:IsShown() then
    Wn.Select(want)
  else
    Wn.state.section = want
    f:Show()
  end
  return true
end

function Wn.Toggle(key)
  if shown() then
    Wn.frame:Hide()
    return true
  end
  return Wn.Open(key)
end

function Wn.Hide()
  if shown() then Wn.frame:Hide() end
end

function Wn.IsShown()
  return shown()
end

function Wn.Selected()
  return Wn.state.section
end

-- A mod's data changed: redraws when the window shows that section (any section when key is nil).
-- Rows are rebuilt only if the section's version moved.
function Wn.Refresh(key)
  if not shown() then return end
  if key ~= nil and key ~= Wn.state.section then return end
  Wn.Draw()
end

-- Tooltip lines -------------------------------------------------------------------------------------------------
--
-- The guard every Ryna mod uses before adding a line to a tooltip: a tooltip the client runs the
-- hook on twice, or a mod that refreshes a tooltip by hand, never shows a line twice, and a line the
-- game or another mod already drew (the same text, whatever its colour) isn't drawn again.

local TL = {}
ns.TooltipLines = TL

function TL.Has(tooltip, text)
  if type(tooltip) ~= "table" or type(text) ~= "string" then return false end
  if type(tooltip.NumLines) ~= "function" or type(tooltip.GetName) ~= "function" then return false end
  local n, name = ns.Try(tooltip.NumLines, tooltip), ns.Try(tooltip.GetName, tooltip)
  if type(n) ~= "number" or type(name) ~= "string" then return false end
  local want = stripCodes(text)
  for i = 1, n do
    local region = _G[name .. "TextLeft" .. i]
    local got = type(region) == "table" and type(region.GetText) == "function" and ns.Try(region.GetText, region)
    if type(got) == "string" and not ns.IsSecret(got) and (got == text or stripCodes(got) == want) then
      return true
    end
  end
  return false
end

-- Adds the line unless the tooltip already has it. Returns whether it added it.
function TL.Add(tooltip, text, r, g, b, wrap)
  if type(tooltip) ~= "table" or type(tooltip.AddLine) ~= "function" or type(text) ~= "string" then return false end
  if TL.Has(tooltip, text) then return false end
  tooltip:AddLine(text, r, g, b, wrap)
  return true
end
