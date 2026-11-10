-- Panel: the options panel under Settings -> AddOns (the modern Settings API as a canvas
-- category), or Interface Options -> AddOns on clients without `Settings`. One frame, registered
-- once, with a row of tabs under the title: the core's Status, Messages, Mods and Advanced, and any
-- a mod adds (Ryna.Panel.AddTab: Chronicle's Recording and Census sit between Status and Messages).
-- Each tab's content is its own child frame of the scroll child; only the selected one is shown and
-- laid out, and the choice is kept in RynaDB.settings.panelTab. Every frame template and UI call is tried
-- and has a plainer fallback, and registering runs under pcall from Init, so a trimmed client can't
-- break login.

local _, ns = ...
local L = ns.L

local P = {}
ns.Panel = P

local St = ns.Status
local CL = ns.CombatLog
local WIDTH = 580
local FONT = "Fonts\\FRIZQT__.TTF"
local TABS_Y = -36 -- the tab row, under the title
local BODY_Y = -72 -- the scrolling content, under the tabs

-- The tabs, by `order` (the lowest is the default): the core's four, and a mod's through
-- Ryna.Panel.AddTab before the panel is built at login. Each is { key, label, order, build(frame),
-- layout(stack, settings) }.
local TABS = {}
-- The Status tab's lines: { order, fn, seq }, fn() -> lines (Ryna.Panel.AddStatusLines).
P.statusSources = {}

-- CreateFrame with the first template this client has (nil, last, means no template).
local function create(kind, parent, templates)
  for _, template in ipairs(templates or {}) do
    local ok, f = pcall(CreateFrame, kind, nil, parent, template)
    if ok and f then return f end
  end
  local ok, f = pcall(CreateFrame, kind, nil, parent)
  if ok then return f end
  return nil
end

local function text(parent, fontObject, size)
  local ok, fs = pcall(parent.CreateFontString, parent, nil, "ARTWORK", fontObject)
  if not ok or not fs then fs = parent:CreateFontString(nil, "ARTWORK") end
  if not fs:GetFont() then fs:SetFont(_G.STANDARD_TEXT_FONT or FONT, size or 12) end
  fs:SetJustifyH("LEFT")
  fs:SetJustifyV("TOP")
  if fs.SetWordWrap then fs:SetWordWrap(true) end
  fs:SetWidth(WIDTH - 30)
  return fs
end

-- A push button, with its own label when the template gives it none.
local function button(parent, label, w, h)
  local b = create("Button", parent, { "UIPanelButtonTemplate" })
  if not b then return nil end
  b:SetSize(w, h)
  if not b:GetFontString() then
    local fs = text(b, "GameFontNormal")
    fs:SetAllPoints()
    fs:SetJustifyH("CENTER")
    b:SetFontString(fs)
  end
  if label then b:SetText(label) end
  return b
end

-- A text-only button that reads like a link ("Try again").
local function link(parent, label)
  local b = create("Button", parent)
  if not b then return nil end
  local fs = text(b, "GameFontHighlightSmall", 11)
  fs:SetText("|cff7fb2ff" .. label .. "|r")
  local w = (fs.GetStringWidth and fs:GetStringWidth()) or 60
  fs:SetWidth(w + 2)
  fs:SetPoint("LEFT", b, "LEFT", 0, 0)
  b:SetSize(w + 4, 18)
  if b.SetHighlightFontObject then pcall(b.SetHighlightFontObject, b, "GameFontNormalSmall") end
  return b
end

-- Lays children out top to bottom inside `content`.
local function stack(content)
  local s = { y = -12 }
  function s.add(region, height, indent, gap)
    region:ClearAllPoints()
    region:SetPoint("TOPLEFT", content, "TOPLEFT", 12 + (indent or 0), s.y)
    s.y = s.y - (height or region:GetHeight() or 14) - (gap or 6)
  end
  -- A font string, as tall as its text.
  function s.text(fs, indent, gap)
    s.add(fs, fs:GetStringHeight() or 14, indent, gap)
  end
  return s
end

local function checkbox(parent, label, help)
  local cb = create("CheckButton", parent, { "UICheckButtonTemplate", "InterfaceOptionsCheckButtonTemplate" })
  if not cb:GetNormalTexture() then
    cb:SetNormalTexture("Interface\\Buttons\\UI-CheckBox-Up")
    cb:SetPushedTexture("Interface\\Buttons\\UI-CheckBox-Down")
    cb:SetHighlightTexture("Interface\\Buttons\\UI-CheckBox-Highlight", "ADD")
    cb:SetCheckedTexture("Interface\\Buttons\\UI-CheckBox-Check")
    cb:SetDisabledCheckedTexture("Interface\\Buttons\\UI-CheckBox-Check-Disabled")
  end
  cb:SetSize(26, 26)
  cb.label = text(parent, "GameFontHighlight")
  cb.label:SetWidth(WIDTH - 70)
  cb.label:SetPoint("LEFT", cb, "RIGHT", 4, 1)
  cb.label:SetText(label)
  cb.help = text(parent, "GameFontDisableSmall", 10)
  cb.help:SetWidth(WIDTH - 70)
  cb.help:SetText(help)
  function cb.place(s, indent)
    s.add(cb, 24, indent, 0)
    cb.help:ClearAllPoints()
    cb.help:SetPoint("TOPLEFT", cb.label, "BOTTOMLEFT", 0, -2)
    s.y = s.y - (cb.help:GetStringHeight() or 12) - 10
  end
  function cb.setActive(active)
    if active then cb:Enable() else cb:Disable() end
    cb.label:SetAlpha(active and 1 or 0.5)
    cb.help:SetAlpha(active and 1 or 0.5)
  end
  return cb
end

local function setMode(mode)
  if not ns.COMBAT_LOG_MODES[mode] then return end
  ns.CombatLog.SetMode(mode)
  P.Refresh()
end

-- The combat-log mode: the modern dropdown (11.x Menu API), else a button that cycles the modes.
local function modeControl(parent)
  local dd = create("DropdownButton", parent, { "WowStyle1DropdownTemplate" })
  if dd and dd.SetupMenu then
    local ok = pcall(dd.SetupMenu, dd, function(_, root)
      for _, mode in ipairs(CL.MODE_ORDER) do
        root:CreateRadio(CL.MODE_LABELS[mode], function(m) return ns.db.settings.combatLog == m end,
          function(m) ns.Run("combat-log mode", setMode, m) end, mode)
      end
    end)
    if ok then
      dd:SetWidth(240)
      function dd.sync() if dd.GenerateMenu then pcall(dd.GenerateMenu, dd) end end
      return dd
    end
    dd:Hide()
  end
  local b = button(parent, nil, 300, 24)
  b:SetScript("OnClick", function()
    local order, current = CL.MODE_ORDER, ns.db.settings.combatLog
    for i, m in ipairs(order) do
      if m == current then return ns.Run("combat-log mode", setMode, order[i % #order + 1]) end
    end
    ns.Run("combat-log mode", setMode, order[1])
  end)
  function b.sync() b:SetText(string.format(L["%s (click to change)"], CL.MODE_LABELS[ns.db.settings.combatLog])) end
  return b
end

-- Tab buttons: WoW's tab templates where this client has them (and the functions that mark one
-- selected), else plain buttons with the selected one's highlight locked on.
local TAB_TEMPLATES = { "PanelTopTabButtonTemplate", "PanelTabButtonTemplate" }

local function templateExists(name)
  local info = ns.Fn("C_XMLUtil", "GetTemplateInfo")
  if not info then return true end -- can't ask: CreateFrame under pcall decides
  local ok, found = pcall(info, name)
  return ok and found ~= nil
end

local function tabButton(parent, i, label)
  local selectTab = ns.Fn("PanelTemplates_SelectTab")
  local deselectTab = ns.Fn("PanelTemplates_DeselectTab")
  local resize = ns.Fn("PanelTemplates_TabResize")
  if selectTab and deselectTab then
    for _, template in ipairs(TAB_TEMPLATES) do
      if templateExists(template) then
        -- Named, because the older tab functions find a tab's parts by its name.
        local ok, b = pcall(CreateFrame, "Button", "RynaPanelTab" .. i, parent, template)
        if ok and b and b.SetText then
          b:SetText(label)
          if b.SetID then b:SetID(i) end
          if resize then pcall(resize, b, 0) end
          b.select, b.deselect = selectTab, deselectTab
          return b
        end
      end
    end
  end
  return button(parent, label, 104, 24)
end

local function markTab(b, selected)
  if b.select and pcall(selected and b.select or b.deselect, b) then return end
  b.select, b.deselect = nil, nil -- the template's functions failed once: plain from now on
  b:Enable()
  if selected then b:LockHighlight() else b:UnlockHighlight() end
  local fs = b:GetFontString()
  if fs then
    if selected then fs:SetTextColor(1, 1, 1) else fs:SetTextColor(1, 0.82, 0) end
  end
end

local LEVEL_COLOR = { info = "|cffffffff", warn = "|cffffd100", error = "|cffff4040" }

local function when(t)
  return date("%Y-%m-%d %H:%M", t)
end

local function statusText()
  return table.concat(ns.OrderedLines(P.statusSources), "\n")
end

-- The core's own Status tab lines: the journal, then the companion app and the errors.
P.statusSources[#P.statusSources + 1] = { order = 30, seq = 0, fn = function()
  local journal = ns.db and #ns.db.journal or 0
  local session = ns.Journal.seq or 0
  return {
    string.format(L["Journal: %s, %d kept in all (last 30 days)"],
      ns.Plural(session, L["%d event this session"], L["%d events this session"]), journal),
    string.format(L["Session: %s"], tostring(ns.Journal.session or L["not started"])),
  }
end }

-- The combat log (35: after the journal, before a mod's lines at 40).
P.statusSources[#P.statusSources + 1] = { order = 35, seq = 0, fn = function()
  return { string.format(L["Combat log: %s right now. Mode: %s"], CL.LoggingText(),
    CL.MODE_LABELS[ns.db.settings.combatLog] or "?") }
end }

P.statusSources[#P.statusSources + 1] = { order = 50, seq = 0, fn = function()
  local lines = { " ", string.format(L["Companion app: %s"], St.SyncLine()) }
  local agent = St.Plain(St.Field(St.Agent(), "agent", "string"))
  if agent then lines[#lines + 1] = string.format(L["Companion app version: %s"], agent) end
  for _, line in ipairs(St.UpdateLines()) do lines[#lines + 1] = "|cff40ff40" .. line .. "|r" end
  lines[#lines + 1] = "|cff999999" .. L["(The app's status is as of your last login or /reload.)"] .. "|r"
  if ns.errors.count > 0 then
    lines[#lines + 1] = "|cffff8080" .. ns.Plural(ns.errors.count, L["%d error caught; /ryna status shows the last."],
      L["%d errors caught; /ryna status shows the last."]) .. "|r"
  end
  return lines
end }

local function messagesText()
  local msgs = St.Messages(10)
  if #msgs == 0 then return L["No messages"] end
  local lines = {}
  for _, m in ipairs(msgs) do
    local at = m.at and string.format(L["%s: "], St.Ago(m.at)) or ""
    lines[#lines + 1] = (LEVEL_COLOR[m.level] or "") .. at .. m.text .. "|r"
  end
  return table.concat(lines, "\n")
end

local function versionsText()
  local lines = { string.format(L["Ryna addon version: %s"], ns.VERSION) }
  local t = St.FileTime()
  if t then
    lines[2] = string.format(L["Companion app status file: as of %s (%s)"], when(t), St.Ago(t) or "?")
  else
    lines[2] = L["Companion app status file: nothing in it yet"]
  end
  for _, c in ipairs(ns.CREDITS or {}) do
    lines[#lines + 1] = ""
    lines[#lines + 1] = string.format(L["Thanks to %s: %s"], c.name, c.line)
  end
  return table.concat(lines, "\n")
end

-- A setting's checkbox: writes RynaDB.settings[key], then `after` (if any), then refreshes.
local function toggle(cb, key, after)
  cb:SetScript("OnClick", function(self)
    ns.Run("setting " .. key, function()
      ns.db.settings[key] = self:GetChecked() and true or false
      ns.Debug("setting %s: %s", key, ns.db.settings[key] and "on" or "off")
      if after then after() end
      P.Refresh()
    end)
  end)
end

-- Each tab: build(frame) makes its controls once; layout(s, settings) fills and places them.
local build, layout = {}, {}

function build.status(f)
  P.intro = text(f, "GameFontHighlightSmall", 11)
  P.intro:SetText(L["Ryna records your characters, bags, quests and guild for the Ryna companion app, which "
    .. "uploads them when you sign in there. Signing in happens in the app, not in the game."])
  P.status = text(f, "GameFontHighlight")
end

function layout.status(s)
  s.text(P.intro, 0, 12)
  P.status:SetText(statusText())
  s.text(P.status, 8, 6)
end

function build.messages(f)
  P.messagesIntro = text(f, "GameFontDisableSmall", 10)
  P.messagesIntro:SetText(L["From the companion app, newest first, as of your last login or /reload."])
  P.messages = text(f, "GameFontHighlightSmall", 11)
end

function layout.messages(s)
  s.text(P.messagesIntro, 0, 10)
  P.messages:SetText(messagesText())
  s.text(P.messages, 8, 6)
end

function build.advanced(f)
  -- The combat log: the core's, so the companion app gets fights whatever mods are on.
  P.logHeader = text(f, "GameFontNormal")
  P.logHeader:SetText(L["Combat log"])
  P.modeLabel = text(f, "GameFontHighlight")
  P.modeLabel:SetText(L["Turn on the game's combat log:"])
  P.mode = modeControl(f)
  P.modeHelp = text(f, "GameFontDisableSmall", 10)
  P.modeHelp:SetText(L["The combat log is how Ryna sees fights. \"In dungeons and raids only\" keeps the "
    .. "file small. Logging you start yourself with /combatlog is left alone."])
  P.acl = checkbox(f, L["Advanced combat logging"],
    L["Sets the game's advanced combat logging on, so fights are logged with positions and health. "
    .. "Needed for Ryna's timelines and deaths."])
  toggle(P.acl, "advancedCombatLog", function() CL.ApplyAdvancedLogging() end)
  P.otherHeader = text(f, "GameFontNormal")
  P.otherHeader:SetText(L["Other settings"])
  P.notices = checkbox(f, L["Chat notices"],
    L["A line in chat when you log in, when the companion app hasn't synced, and when Ryna turns "
    .. "combat logging on or off. Off, Ryna only answers /ryna."])
  toggle(P.notices, "chatNotices")
  P.debug = checkbox(f, L["Debug messages"],
    L["Prints what Ryna is doing to the chat window: scans, journal events, sightings. "
    .. "For checking that it works."])
  P.debug:SetScript("OnClick", function(self)
    ns.Run("setting debug", function()
      ns.SetDebug(self:GetChecked() and true or false)
      P.Refresh()
    end)
  end)
  P.minimap = checkbox(f, L["Show the minimap button"], L["Drag it around the minimap to move it."])
  P.minimap:SetScript("OnClick", function(self)
    ns.Run("setting minimap", function()
      ns.db.settings.minimap.shown = self:GetChecked() and true or false
      ns.Debug("setting minimap button: %s", ns.db.settings.minimap.shown and "shown" or "hidden")
      if ns.Minimap then ns.Minimap.Update() end
      P.Refresh()
    end)
  end)
  P.defaults = button(f, L["Defaults"], 140, 22)
  if P.defaults then
    P.defaults:SetScript("OnClick", function() P.panel.OnDefault() end)
    P.defaultsText = text(f, "GameFontDisableSmall", 10)
    P.defaultsText:SetWidth(WIDTH - 230)
    P.defaultsText:SetPoint("LEFT", P.defaults, "RIGHT", 8, 0)
    P.defaultsText:SetText(L["Every setting back to its default. The minimap button keeps its place."])
  end
  P.versions = text(f, "GameFontHighlightSmall", 11)
end

function layout.advanced(s, settings)
  s.text(P.logHeader, 0, 6)
  s.add(P.modeLabel, 14, 8, 4)
  s.add(P.mode, 26, 8, 2)
  s.text(P.modeHelp, 8, 10)
  P.mode.sync()
  P.acl:SetChecked(settings.advancedCombatLog)
  P.acl.place(s, 4)
  s.text(P.otherHeader, 0, 6)
  P.notices:SetChecked(settings.chatNotices)
  P.debug:SetChecked(settings.debug)
  P.minimap:SetChecked(settings.minimap.shown)
  P.notices.place(s, 4)
  P.debug.place(s, 4)
  P.minimap.place(s, 4)
  if P.defaults then s.add(P.defaults, 22, 8, 16) end
  P.versions:SetText(versionsText())
  s.text(P.versions, 8, 6)
end

-- The Mods tab: every Ryna_* addon in the client's list, each with a box that enables or disables
-- it in that list (C_AddOns.EnableAddOn / DisableAddOn). The change applies at the next load, so a
-- change offers a reload. The companion app installs and removes the folders (its Game tab); a mod
-- the app hasn't installed isn't in the client's list, so it isn't here.
local function addonCall(name)
  return ns.Fn("C_AddOns", name) or ns.Fn(name)
end

-- The Ryna_* addons in the client's list: { name, title, notes }.
function P.ModAddons()
  local out = {}
  local count = ns.Try(addonCall("GetNumAddOns"))
  local info = addonCall("GetAddOnInfo")
  if type(count) ~= "number" or not info then return out end
  for i = 1, count do
    local name, title, notes = ns.Try(info, i)
    if type(name) == "string" and name:find("^Ryna_") then
      out[#out + 1] = { name = name, title = ns.NonEmpty(title) or name, notes = ns.NonEmpty(notes) }
    end
  end
  table.sort(out, function(a, b) return a.name < b.name end)
  return out
end

-- Whether the client's list has the addon enabled for this character (what loads next time).
function P.ModEnabled(name)
  local me = ns.Try(UnitName, "player")
  local state = ns.Try(ns.Fn("C_AddOns", "GetAddOnEnableState"), name, me)
  if state == nil then state = ns.Try(ns.Fn("GetAddOnEnableState"), me, name) end
  if type(state) == "number" then return state > 0 end
  return ns.Try(addonCall("IsAddOnLoaded"), name) and true or false
end

function P.ModLoaded(name)
  return ns.Try(addonCall("IsAddOnLoaded"), name) and true or false
end

-- Asks to reload now (the game's own dialog), else says how.
local function offerReload()
  local dialogs = StaticPopupDialogs
  local show = ns.Fn("StaticPopup_Show")
  local reload = ns.Fn("C_UI", "Reload") or ns.Fn("ReloadUI")
  if type(dialogs) == "table" and show and reload then
    if not dialogs.RYNA_RELOAD then
      dialogs.RYNA_RELOAD = {
        text = L["Reload the interface now, so the change to Ryna's mods takes effect?"],
        button1 = _G.RELOADUI or L["Reload"], button2 = _G.CANCEL or L["Later"],
        OnAccept = function() reload() end,
        timeout = 0, whileDead = true, hideOnEscape = true, preferredIndex = 3,
      }
    end
    if ns.Try(show, "RYNA_RELOAD") then return end
  end
  ns.Print(L["type /reload to apply the change."])
end

function P.SetModEnabled(name, on)
  local f = addonCall(on and "EnableAddOn" or "DisableAddOn")
  if not f or not pcall(f, name) then
    ns.Print(L["the game didn't let Ryna change its addon list; use the AddOns button on the character screen."])
    return
  end
  ns.Debug("mods: %s %s in the game's addon list", name, on and "enabled" or "disabled")
  offerReload()
end

function build.mods(f)
  P.modsIntro = text(f, "GameFontHighlightSmall", 11)
  P.modsIntro:SetText(L["Mods are optional addons beside Ryna that the companion app installs. Turning one "
    .. "off here turns it off in the game's AddOns list from the next reload; the app's Game tab removes it."])
  P.modsNone = text(f, "GameFontDisableSmall", 10)
  P.modsNone:SetText(L["No mods are installed. The companion app's Game tab installs them."])
  P.modBoxes = {}
  for _, m in ipairs(P.ModAddons()) do
    local cb = checkbox(f, m.title, "")
    cb.addon = m
    cb:SetScript("OnClick", function(self)
      ns.Run("mods " .. m.name, function()
        P.SetModEnabled(m.name, self:GetChecked() and true or false)
        P.Refresh()
      end)
    end)
    P.modBoxes[#P.modBoxes + 1] = cb
  end
end

local function modHelp(m)
  local enabled, loaded = P.ModEnabled(m.name), P.ModLoaded(m.name)
  local version = ns.NonEmpty(ns.Try(addonCall("GetAddOnMetadata"), m.name, "Version"))
  local state
  if loaded and enabled then
    state = L["On."]
  elseif loaded then
    state = L["On until the next reload, then off."]
  elseif enabled then
    state = L["Off until the next reload, then on."]
  else
    state = L["Off."]
  end
  local parts = { version and string.format(L["Version %s."], version) or nil, state, m.notes }
  local out = {}
  for _, p in ipairs(parts) do if p then out[#out + 1] = p end end
  return table.concat(out, " ")
end

function layout.mods(s)
  s.text(P.modsIntro, 0, 12)
  if #P.modBoxes == 0 then
    P.modsNone:Show()
    s.text(P.modsNone, 8, 6)
    return
  end
  P.modsNone:Hide()
  for _, cb in ipairs(P.modBoxes) do
    cb:SetChecked(P.ModEnabled(cb.addon.name))
    cb.help:SetText(modHelp(cb.addon))
    cb.place(s, 4)
  end
end

-- A tab: { key, label, order, build(frame), layout(stack, settings) }. Only before the panel is built
-- (at PLAYER_LOGIN), so a mod adds its tabs while it loads. Returns true when added.
function P.AddTab(spec)
  if P.built or type(spec) ~= "table" or type(spec.key) ~= "string" or type(spec.build) ~= "function"
    or type(spec.layout) ~= "function" then
    return false
  end
  for _, t in ipairs(TABS) do
    if t.key == spec.key then return false end
  end
  TABS[#TABS + 1] = {
    key = spec.key, label = tostring(spec.label or spec.key), order = tonumber(spec.order) or 100,
    build = spec.build, layout = spec.layout, seq = #TABS + 1,
  }
  return true
end

P.AddTab({ key = "status", label = L["Status"], order = 10, build = build.status, layout = layout.status })
P.AddTab({ key = "messages", label = L["Messages"], order = 40, build = build.messages, layout = layout.messages })
P.AddTab({ key = "mods", label = L["Mods"], order = 50, build = build.mods, layout = layout.mods })
P.AddTab({ key = "advanced", label = L["Advanced"], order = 60, build = build.advanced, layout = layout.advanced })

-- Refreshes the panel only while it is on screen (for a mod whose state moved on by itself).
function P.RefreshIfShown()
  if not (P.panel and P.Refresh) then return end
  local ok, shown = pcall(P.panel.IsVisible, P.panel)
  if ok and shown then ns.Run("panel", P.Refresh) end
end

-- What a mod's tabs build with: the same widgets as the core's (Ryna.Panel.Widgets).
P.Widgets = {
  WIDTH = WIDTH, create = create, text = text, button = button, link = link, checkbox = checkbox,
  toggle = function(cb, key, after) return toggle(cb, key, after) end,
}

local function buildPanel()
  local panel = CreateFrame("Frame")
  panel.name = "Ryna"
  panel:Hide()

  local title = text(panel, "GameFontNormalLarge", 16)
  title:SetText("Ryna " .. ns.VERSION)
  title:SetPoint("TOPLEFT", panel, "TOPLEFT", 12, -12)

  -- The scroll child holds every tab's frame. Without a scroll template it sits in the panel.
  local content = CreateFrame("Frame", nil, panel)
  content:SetSize(WIDTH, 800)
  local scroll = create("ScrollFrame", panel, { "ScrollFrameTemplate", "UIPanelScrollFrameTemplate" })
  if scroll and scroll.SetScrollChild then
    scroll:SetPoint("TOPLEFT", 0, BODY_Y)
    scroll:SetPoint("BOTTOMRIGHT", -26, 4)
    content:SetParent(scroll)
    scroll:SetScrollChild(content)
    P.scroll = scroll
  else
    content:SetPoint("TOPLEFT", panel, "TOPLEFT", 0, BODY_Y)
  end
  P.content, P.panel = content, panel

  P.tabs, P.tabsByKey = {}, {}
  P.built = true
  table.sort(TABS, function(a, b)
    if a.order ~= b.order then return a.order < b.order end
    return a.seq < b.seq
  end)
  local prev
  for _, def in ipairs(TABS) do
    local f = CreateFrame("Frame", nil, content)
    f:SetPoint("TOPLEFT", content, "TOPLEFT", 0, 0)
    f:SetSize(WIDTH, 100)
    f:Hide()
    -- A tab that fails to build (a mod's, say) is left out; the others still work.
    local ok, err = pcall(def.build, f)
    if ok then
      local i = #P.tabs + 1
      local b = tabButton(panel, i, def.label)
      if prev then
        b:SetPoint("LEFT", prev, "RIGHT", b.select and 2 or 4, 0)
      else
        b:SetPoint("TOPLEFT", panel, "TOPLEFT", 10, TABS_Y)
      end
      local key = def.key
      b:SetScript("OnClick", function() ns.Run("panel tab", P.Select, key) end)
      prev = b
      local tab = { key = key, frame = f, button = b, layout = def.layout }
      P.tabs[i], P.tabsByKey[key] = tab, tab
    else
      ns.ReportError("panel tab " .. def.key, err)
    end
  end

  panel:SetScript("OnShow", function() ns.Run("panel", P.Refresh) end)
  -- Settings and Interface Options call these; changes apply at once, so only Defaults does work.
  panel.OnCommit, panel.okay, panel.cancel = function() end, function() end, function() end
  panel.OnRefresh = function() ns.Run("panel", P.Refresh) end
  panel.refresh = panel.OnRefresh
  panel.OnDefault = function()
    ns.Run("panel defaults", function()
      local mm, tab, window = ns.db.settings.minimap, ns.db.settings.panelTab, ns.db.settings.window
      local wasDebug = ns.Debugging()
      ns.db.settings = { minimap = mm, panelTab = tab, window = window }
      ns.ApplySettingDefaults(ns.db.settings)
      mm.shown = true
      if wasDebug then ns.Print(L["settings back to their defaults; debug messages off."]) end
      CL.SetMode(ns.db.settings.combatLog)
      ns.CallMods("onDefaults")
      if ns.Minimap then ns.Minimap.Update() end
      P.Refresh()
    end)
  end
  panel.default = panel.OnDefault
  return panel
end

-- The selected tab's key: RynaDB.settings.panelTab when it names a tab, else the first.
function P.SelectedTab()
  local key = ns.db and ns.db.settings.panelTab
  if P.tabsByKey and P.tabsByKey[key] then return key end
  return P.tabs and P.tabs[1] and P.tabs[1].key or "status"
end

-- Shows another tab, remembers it, and scrolls back to the top.
function P.Select(key)
  if not (P.tabsByKey and P.tabsByKey[key] and ns.db) then return end
  ns.db.settings.panelTab = key
  if P.scroll and P.scroll.SetVerticalScroll then pcall(P.scroll.SetVerticalScroll, P.scroll, 0) end
  P.Refresh()
end

-- Marks the selected tab, shows only its frame, fills and lays it out, and sizes the scroll child
-- to it. The other tabs are filled when they're selected.
function P.Refresh()
  if not P.panel or not ns.db then return end
  local key = P.SelectedTab()
  for _, tab in ipairs(P.tabs) do
    local on = tab.key == key
    markTab(tab.button, on)
    if on then tab.frame:Show() else tab.frame:Hide() end
  end
  local tab = P.tabsByKey[key]
  if not tab then return end
  local s = stack(tab.frame)
  tab.layout(s, ns.db.settings)
  local height = -s.y + 20
  tab.frame:SetHeight(height)
  P.content:SetHeight(height)
  if P.scroll and P.scroll.UpdateScrollChildRect then pcall(P.scroll.UpdateScrollChildRect, P.scroll) end
end

-- Called once from PLAYER_LOGIN (under ns.Run). Returns true when a panel is registered.
function P.Register()
  if P.panel then return true end
  local panel = buildPanel()
  local S = _G.Settings
  if type(S) == "table" and type(S.RegisterCanvasLayoutCategory) == "function"
    and type(S.RegisterAddOnCategory) == "function" then
    local category = S.RegisterCanvasLayoutCategory(panel, "Ryna")
    S.RegisterAddOnCategory(category)
    P.category = category
    return true
  end
  local add = ns.Fn("InterfaceOptions_AddCategory")
  if add then
    add(panel)
    P.legacy = true
    return true
  end
  P.panel = nil
  return false
end

-- /ryna and the minimap button. Settings can't open in combat on modern clients.
function P.Open()
  if ns.Try(ns.Fn("InCombatLockdown")) then
    ns.Print(L["the settings open after combat."])
    return
  end
  local S = _G.Settings
  if P.category and type(S) == "table" and type(S.OpenToCategory) == "function" then
    local id = (P.category.GetID and P.category:GetID()) or P.category.ID
    if pcall(S.OpenToCategory, id) then return end
  end
  local open = ns.Fn("InterfaceOptionsFrame_OpenToCategory")
  if P.legacy and open then
    -- The old frame often needs two calls the first time it opens.
    ns.Try(open, P.panel)
    ns.Try(open, P.panel)
    return
  end
  ns.Print(L["no settings panel on this client; /ryna status prints the status."])
end
