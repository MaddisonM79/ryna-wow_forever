-- Panel: the options panel under Settings -> AddOns (the modern Settings API as a canvas
-- category), or Interface Options -> AddOns on clients without `Settings`. One frame, registered
-- once, with a row of tabs under the title: Status, Recording, Census, Messages and Advanced. Each
-- tab's content is its own child frame of the scroll child; only the selected one is shown and laid
-- out, and the choice is kept in RynaDB.settings.panelTab. Every frame template and UI call is tried
-- and has a plainer fallback, and registering runs under pcall from Init, so a trimmed client can't
-- break login.

local _, ns = ...
local L = ns.L

local P = {}
ns.Panel = P

local St = ns.Status
local WIDTH = 580
local FONT = "Fonts\\FRIZQT__.TTF"
local TABS_Y = -36 -- the tab row, under the title
local BODY_Y = -72 -- the scrolling content, under the tabs

-- The tabs, in order. The first is the default.
local TABS = {
  { key = "status", label = L["Status"] },
  { key = "recording", label = L["Recording"] },
  { key = "census", label = L["Census"] },
  { key = "messages", label = L["Messages"] },
  { key = "advanced", label = L["Advanced"] },
}

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
  ns.Zone.SetMode(mode)
  P.Refresh()
end

-- The combat-log mode: the modern dropdown (11.x Menu API), else a button that cycles the modes.
local function modeControl(parent)
  local dd = create("DropdownButton", parent, { "WowStyle1DropdownTemplate" })
  if dd and dd.SetupMenu then
    local ok = pcall(dd.SetupMenu, dd, function(_, root)
      for _, mode in ipairs(St.MODE_ORDER) do
        root:CreateRadio(St.MODE_LABELS[mode], function(m) return ns.db.settings.combatLog == m end,
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
    local order, current = St.MODE_ORDER, ns.db.settings.combatLog
    for i, m in ipairs(order) do
      if m == current then return ns.Run("combat-log mode", setMode, order[i % #order + 1]) end
    end
    ns.Run("combat-log mode", setMode, order[1])
  end)
  function b.sync() b:SetText(string.format(L["%s (click to change)"], St.MODE_LABELS[ns.db.settings.combatLog])) end
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

-- The status tab's census line: off, the running sweep, or the last one.
local function censusLine()
  local C = ns.Census
  if not C then return nil end
  if not ns.db.settings.census then return L["Census: off"] end
  if C.Running() or C.Mode() == "blocked" then return string.format(L["Census: %s"], C.StatusLine()) end
  local l = C.LastSweep()
  if not l then return L["Last census sweep: none yet"] end
  return string.format(L["Last census sweep: %s, %s"],
    ns.Plural(l.players or 0, L["%d player"], L["%d players"]), St.Ago(l.at) or "?")
end

local function statusText()
  local lines = {}
  local rec, c = ns.rec, St.Counts()
  if rec then
    lines[#lines + 1] = string.format(L["Character: %s, level %s"], tostring(rec.fullName), tostring(rec.level))
  else
    lines[#lines + 1] = L["Character: not loaded yet"]
  end
  lines[#lines + 1] = string.format(L["Recorded: %s, %d equipped, %s, %d completed, %s"],
    ns.Plural(c.items, L["%d item in bags and bank"], L["%d items in bags and bank"]), c.gear,
    ns.Plural(c.active, L["%d quest in the log"], L["%d quests in the log"]), c.completed,
    ns.Plural(c.roster, L["%d guild member"], L["%d guild members"]))
  lines[#lines + 1] = string.format(L["Journal: %s, %d kept in all (last 30 days)"],
    ns.Plural(c.session, L["%d event this session"], L["%d events this session"]), c.journal)
  lines[#lines + 1] = string.format(L["Session: %s"], tostring(ns.Journal.session or L["not started"]))
  lines[#lines + 1] = string.format(L["Combat log: %s right now. Mode: %s"], St.LoggingText(),
    St.MODE_LABELS[ns.db.settings.combatLog] or "?")
  local census = censusLine()
  if census then lines[#lines + 1] = census end
  lines[#lines + 1] = " "
  lines[#lines + 1] = string.format(L["Companion app: %s"], St.SyncLine())
  local agent = St.Plain(St.Field(St.Agent(), "agent", "string"))
  if agent then lines[#lines + 1] = string.format(L["Companion app version: %s"], agent) end
  for _, line in ipairs(St.UpdateLines()) do lines[#lines + 1] = "|cff40ff40" .. line .. "|r" end
  lines[#lines + 1] = "|cff999999" .. L["(The app's status is as of your last login or /reload.)"] .. "|r"
  if ns.errors.count > 0 then
    lines[#lines + 1] = "|cffff8080" .. ns.Plural(ns.errors.count, L["%d error caught; /ryna status shows the last."],
      L["%d errors caught; /ryna status shows the last."]) .. "|r"
  end
  return table.concat(lines, "\n")
end

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

function build.recording(f)
  P.recordingIntro = text(f, "GameFontHighlightSmall", 11)
  P.recordingIntro:SetText(L["About your own characters, Ryna records their gear, bags and bank, quests, "
    .. "professions, reputations, gold and played time, your guild's roster, and a 30-day journal of what "
    .. "you do. Other players are the Census tab's."])
  P.modeLabel = text(f, "GameFontHighlight")
  P.modeLabel:SetText(L["Turn on the game's combat log:"])
  P.mode = modeControl(f)
  P.modeHelp = text(f, "GameFontDisableSmall", 10)
  P.modeHelp:SetText(L["The combat log is how Ryna sees fights. \"In dungeons and raids only\" keeps the "
    .. "file small. Logging you start yourself with /combatlog is left alone."])
  P.acl = checkbox(f, L["Advanced combat logging"],
    L["Sets the game's advanced combat logging on, so fights are logged with positions and health. "
    .. "Needed for Ryna's timelines and deaths."])
  toggle(P.acl, "advancedCombatLog", function() ns.Zone.ApplyAdvancedLogging() end)
end

function layout.recording(s, settings)
  s.text(P.recordingIntro, 0, 12)
  s.add(P.modeLabel, 14, 8, 4)
  s.add(P.mode, 26, 8, 2)
  s.text(P.modeHelp, 8, 10)
  P.mode.sync()
  P.acl:SetChecked(settings.advancedCombatLog)
  P.acl.place(s, 4)
end

function build.census(f)
  P.census = checkbox(f, L["Take part in the census"],
    L["Records other players you see (nearby, in chat, your group and your guild). "
    .. "Off: Ryna records only your own characters."])
  P.chat = checkbox(f, L["Record who I see in chat"],
    L["Only who spoke, never what they said or in which channel."])
  P.group = checkbox(f, L["Record other players in my group"],
    L["Who was in your groups."])
  P.nearby = checkbox(f, L["Record players I see nearby"],
    L["Players whose nameplates, target or mouseover you see: their level, class and guild, and where."])
  P.nameplateHint = text(f, "GameFontDisableSmall", 10)
  P.nameplateHint:SetWidth(WIDTH - 100)
  P.nameplateHint:SetText(L["The game shows nameplates for enemies only unless friendly nameplates are on "
    .. "(Shift+V). Ryna only sees nameplates the game draws."])
  -- Sets the game's nameplateShowFriends only when clicked; the addon never changes it by itself.
  P.friendly = button(f, L["Turn friendly nameplates on"], 240, 22)
  if P.friendly then
    P.friendly:SetScript("OnClick", function()
      ns.Run("friendly nameplates", function()
        ns.Sightings.ShowFriendlyNameplates()
        P.Refresh()
      end)
    end)
  end
  P.sweepHelp = L["Every 30 minutes while you're parked in a capital, Ryna asks the server who is online (/who), "
    .. "a few queries at a time, to count the realm's players by level, class and zone. "
    .. "Nothing about you is sent."]
  P.sweep = checkbox(f, L["Run a realm census while I'm in a city"], P.sweepHelp)
  toggle(P.census, "census")
  toggle(P.chat, "chatSightings")
  toggle(P.group, "groupMembers")
  toggle(P.nearby, "nearbyPlayers")
  toggle(P.sweep, "censusSweep")
  -- The click itself sends the query (C.Request runs synchronously inside OnClick): some clients
  -- accept /who only from a hardware event.
  P.sweepButton = button(f, L["Run census now"], 220, 22)
  if P.sweepButton then
    P.sweepButton:SetScript("OnClick", function()
      ns.Run("census sweep", function()
        ns.Census.Request()
        P.Refresh()
      end)
    end)
    P.sweepRetry = link(f, L["Try again"])
    if P.sweepRetry then
      P.sweepRetry:SetPoint("LEFT", P.sweepButton, "RIGHT", 8, 0)
      P.sweepRetry:SetScript("OnClick", function()
        ns.Run("census try again", function()
          ns.Census.TryAgain()
          P.Refresh()
        end)
      end)
    end
    P.sweepText = text(f, "GameFontHighlightSmall", 11)
    P.sweepText:SetWidth(WIDTH - 320)
    -- ">" rather than an arrow: the game's font has no arrow glyph.
    P.keyHint = text(f, "GameFontDisableSmall", 10)
    P.keyHint:SetWidth(WIDTH - 100)
    P.keyHint:SetText(L["Bind a key to Next census query in Key Bindings > AddOns > Ryna to run a sweep "
      .. "with taps instead of clicks. /ryna census does the same from chat."])
  end
  P.privacy = text(f, "GameFontDisableSmall", 10)
  P.privacy:SetText(L["The census keeps who was seen, never what they said. "
    .. "Its data is never sold or used for advertising."])
end

function layout.census(s, settings)
  P.census:SetChecked(settings.census)
  P.chat:SetChecked(settings.chatSightings)
  P.group:SetChecked(settings.groupMembers)
  P.nearby:SetChecked(settings.nearbyPlayers)
  P.sweep:SetChecked(settings.censusSweep)
  local C = ns.Census
  local mode = C and C.Mode()
  -- Automatic sweeps send from a timer, which a click-only or blocked client refuses.
  local clickOnly = mode == "click" or mode == "blocked"
  P.sweep.help:SetText(clickOnly and L["The game only allows /who from a click on this client."] or P.sweepHelp)
  P.census.place(s, 4)
  for _, cb in ipairs({ P.chat, P.group, P.nearby, P.sweep }) do
    cb.setActive(settings.census and not (cb == P.sweep and clickOnly))
    if cb == P.sweep and clickOnly and settings.census then cb.help:SetAlpha(1) end
    cb.place(s, 30)
    if cb == P.nearby then
      P.nameplateHint:SetAlpha(settings.census and 1 or 0.5)
      s.text(P.nameplateHint, 60, 4)
      if P.friendly then
        local on = ns.Sightings.FriendlyNameplatesOn()
        P.friendly:SetText(on and L["Friendly nameplates are on"] or L["Turn friendly nameplates on"])
        if settings.census and not on then P.friendly:Enable() else P.friendly:Disable() end
        s.add(P.friendly, 22, 60, 12)
      end
    end
  end
  if P.sweepButton then
    local label, usable = L["Run census now"], false
    if C then
      usable = settings.census and C.Available() and true or false
      if mode == "blocked" then
        label, usable = L["Census: not allowed by the game"], false
      elseif mode == "click" or C.Stepping() or (C.Running() and C.sweep.byClicks) then
        label = string.format(L["Next census query (%d of %d)"], C.NextStep())
        usable = usable and (not C.Running() or C.Stepping())
      else
        usable = usable and not C.Running()
      end
    end
    P.sweepButton:SetText(label)
    if usable then P.sweepButton:Enable() else P.sweepButton:Disable() end
    local retry = P.sweepRetry and mode == "blocked"
    if P.sweepRetry then
      if retry then P.sweepRetry:Show() else P.sweepRetry:Hide() end
    end
    P.sweepText:ClearAllPoints()
    P.sweepText:SetPoint("LEFT", retry and P.sweepRetry or P.sweepButton, "RIGHT", 8, 0)
    P.sweepText:SetText(C and C.StatusLine() or "")
    s.add(P.sweepButton, 22, 34, 6)
    s.text(P.keyHint, 34, 14)
  end
  s.text(P.privacy, 4, 6)
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
  P.debug:SetChecked(settings.debug)
  P.minimap:SetChecked(settings.minimap.shown)
  P.debug.place(s, 4)
  P.minimap.place(s, 4)
  if P.defaults then s.add(P.defaults, 22, 8, 16) end
  P.versions:SetText(versionsText())
  s.text(P.versions, 8, 6)
end

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
  local prev
  for i, def in ipairs(TABS) do
    local f = CreateFrame("Frame", nil, content)
    f:SetPoint("TOPLEFT", content, "TOPLEFT", 0, 0)
    f:SetSize(WIDTH, 100)
    f:Hide()
    build[def.key](f)
    local b = tabButton(panel, i, def.label)
    if prev then
      b:SetPoint("LEFT", prev, "RIGHT", b.select and 2 or 4, 0)
    else
      b:SetPoint("TOPLEFT", panel, "TOPLEFT", 10, TABS_Y)
    end
    local key = def.key
    b:SetScript("OnClick", function() ns.Run("panel tab", P.Select, key) end)
    prev = b
    local tab = { key = key, frame = f, button = b, layout = layout[key] }
    P.tabs[i], P.tabsByKey[key] = tab, tab
  end

  panel:SetScript("OnShow", function() ns.Run("panel", P.Refresh) end)
  -- Settings and Interface Options call these; changes apply at once, so only Defaults does work.
  panel.OnCommit, panel.okay, panel.cancel = function() end, function() end, function() end
  panel.OnRefresh = function() ns.Run("panel", P.Refresh) end
  panel.refresh = panel.OnRefresh
  panel.OnDefault = function()
    ns.Run("panel defaults", function()
      local mm, tab = ns.db.settings.minimap, ns.db.settings.panelTab
      local wasDebug = ns.Debugging()
      ns.db.settings = { minimap = mm, panelTab = tab }
      ns.ApplySettingDefaults(ns.db.settings)
      mm.shown = true
      if wasDebug then ns.Print(L["settings back to their defaults; debug messages off."]) end
      ns.Zone.SetMode(ns.db.settings.combatLog)
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
  return TABS[1].key
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
