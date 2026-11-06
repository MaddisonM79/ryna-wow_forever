-- Panel: the options panel under Settings -> AddOns (the modern Settings API as a canvas
-- category), or Interface Options -> AddOns on clients without `Settings`. Three sections:
-- Status, Settings and Messages. Every frame template and UI call is tried and has a plainer
-- fallback, and registering runs under pcall from Init, so a trimmed client can't break login.

local _, ns = ...

local P = {}
ns.Panel = P

local St = ns.Status
local WIDTH = 580
local FONT = "Fonts\\FRIZQT__.TTF"

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

-- Lays children out top to bottom inside `content`.
local function stack(content)
  local s = { y = -12 }
  function s.add(region, height, indent, gap)
    region:ClearAllPoints()
    region:SetPoint("TOPLEFT", content, "TOPLEFT", 12 + (indent or 0), s.y)
    s.y = s.y - (height or region:GetHeight() or 14) - (gap or 6)
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
  local b = create("Button", parent, { "UIPanelButtonTemplate" })
  b:SetSize(300, 24)
  if not b:GetFontString() then
    local fs = text(b, "GameFontNormal")
    fs:SetAllPoints()
    fs:SetJustifyH("CENTER")
    b:SetFontString(fs)
  end
  b:SetScript("OnClick", function()
    local order, current = St.MODE_ORDER, ns.db.settings.combatLog
    for i, m in ipairs(order) do
      if m == current then return ns.Run("combat-log mode", setMode, order[i % #order + 1]) end
    end
    ns.Run("combat-log mode", setMode, order[1])
  end)
  function b.sync() b:SetText(St.MODE_LABELS[ns.db.settings.combatLog] .. " (click to change)") end
  return b
end

local LEVEL_COLOR = { info = "|cffffffff", warn = "|cffffd100", error = "|cffff4040" }

local function build()
  local panel = CreateFrame("Frame")
  panel.name = "Ryna"
  panel:Hide()

  -- Scrolls where a scroll template exists; otherwise the content sits in the panel directly.
  local content = panel
  local scroll = create("ScrollFrame", panel, { "ScrollFrameTemplate", "UIPanelScrollFrameTemplate" })
  if scroll and scroll.SetScrollChild then
    scroll:SetPoint("TOPLEFT", 0, -4)
    scroll:SetPoint("BOTTOMRIGHT", -26, 4)
    content = CreateFrame("Frame", nil, scroll)
    content:SetSize(WIDTH, 800)
    scroll:SetScrollChild(content)
  end

  local s = stack(content)
  local title = text(content, "GameFontNormalLarge", 16)
  title:SetText("Ryna " .. ns.VERSION)
  s.add(title, 20)
  local intro = text(content, "GameFontHighlightSmall", 11)
  intro:SetText("Ryna records your characters, bags, quests and guild for the Ryna companion app, which "
    .. "uploads them when you sign in there. Signing in happens in the app, not in the game.")
  s.add(intro, 28, 0, 12)

  local h1 = text(content, "GameFontNormal", 13)
  h1:SetText("Status")
  s.add(h1, 16)
  P.status = text(content, "GameFontHighlight")
  s.add(P.status, 150, 8, 12)
  P.statusAnchor = s.y

  local h2 = text(content, "GameFontNormal", 13)
  h2:SetText("Settings")
  P.h2 = h2

  local modeLabel = text(content, "GameFontHighlight")
  modeLabel:SetText("Turn on the game's combat log:")
  P.modeLabel = modeLabel
  P.mode = modeControl(content)
  P.modeHelp = text(content, "GameFontDisableSmall", 10)
  P.modeHelp:SetText("The combat log is how Ryna sees fights. \"In dungeons and raids only\" keeps the "
    .. "file small. Logging you start yourself with /combatlog is left alone.")

  P.chat = checkbox(content, "Record who I see in chat",
    "Only who spoke, never what they said or in which channel.")
  P.census = checkbox(content, "Take part in the census",
    "Records other players you see (in chat, your group and your guild). Off: Ryna records only your own characters.")
  P.group = checkbox(content, "Record other players in my group",
    "Who was in your groups.")
  P.acl = checkbox(content, "Advanced combat logging",
    "Sets the game's advanced combat logging on, so fights are logged with positions and health. Needed for Ryna's timelines and deaths.")
  P.minimap = checkbox(content, "Show the minimap button", "Drag it around the minimap to move it.")
  P.debug = checkbox(content, "Debug messages",
    "Prints what Ryna is doing to the chat window: scans, journal events, sightings. For checking that it works.")

  local function toggle(cb, key)
    cb:SetScript("OnClick", function(self)
      ns.Run("setting " .. key, function()
        ns.db.settings[key] = self:GetChecked() and true or false
        ns.Debug("setting %s: %s", key, ns.db.settings[key] and "on" or "off")
        P.Refresh()
      end)
    end)
  end
  toggle(P.chat, "chatSightings")
  toggle(P.census, "census")
  toggle(P.group, "groupMembers")
  P.acl:SetScript("OnClick", function(self)
    ns.Run("setting advancedCombatLog", function()
      ns.db.settings.advancedCombatLog = self:GetChecked() and true or false
      ns.Debug("setting advancedCombatLog: %s", ns.db.settings.advancedCombatLog and "on" or "off")
      ns.Zone.ApplyAdvancedLogging()
      P.Refresh()
    end)
  end)
  P.minimap:SetScript("OnClick", function(self)
    ns.Run("setting minimap", function()
      ns.db.settings.minimap.shown = self:GetChecked() and true or false
      ns.Debug("setting minimap button: %s", ns.db.settings.minimap.shown and "shown" or "hidden")
      if ns.Minimap then ns.Minimap.Update() end
      P.Refresh()
    end)
  end)
  P.debug:SetScript("OnClick", function(self)
    ns.Run("setting debug", function()
      ns.SetDebug(self:GetChecked() and true or false)
      P.Refresh()
    end)
  end)

  P.h3 = text(content, "GameFontNormal", 13)
  P.h3:SetText("Messages")
  P.messages = text(content, "GameFontHighlightSmall", 11)

  P.content, P.panel = content, panel
  panel:SetScript("OnShow", function() ns.Run("panel", P.Refresh) end)
  -- Settings and Interface Options call these; changes apply at once, so only Defaults does work.
  panel.OnCommit, panel.okay, panel.cancel = function() end, function() end, function() end
  panel.OnRefresh = function() ns.Run("panel", P.Refresh) end
  panel.refresh = panel.OnRefresh
  panel.OnDefault = function()
    ns.Run("panel defaults", function()
      local mm = ns.db.settings.minimap
      local wasDebug = ns.Debugging()
      ns.db.settings = { minimap = mm }
      ns.ApplySettingDefaults(ns.db.settings)
      mm.shown = true
      if wasDebug then ns.Print("settings back to their defaults; debug messages off.") end
      ns.Zone.SetMode(ns.db.settings.combatLog)
      if ns.Minimap then ns.Minimap.Update() end
      P.Refresh()
    end)
  end
  panel.default = panel.OnDefault
  return panel
end

local function statusText()
  local lines = {}
  local rec, c = ns.rec, St.Counts()
  if rec then
    lines[#lines + 1] = string.format("Character: %s, level %s", tostring(rec.fullName), tostring(rec.level))
  else
    lines[#lines + 1] = "Character: not loaded yet"
  end
  lines[#lines + 1] = string.format("Recorded: %d items in bags and bank, %d equipped, %d quests in the log, "
    .. "%d completed, %d guild members", c.items, c.gear, c.active, c.completed, c.roster)
  lines[#lines + 1] = string.format("Journal: %d events this session, %d kept in all (last 30 days)",
    c.session, c.journal)
  lines[#lines + 1] = "Session: " .. tostring(ns.Journal.session or "not started")
  lines[#lines + 1] = string.format("Combat log: %s right now. Mode: %s", St.LoggingText(),
    St.MODE_LABELS[ns.db.settings.combatLog] or "?")
  lines[#lines + 1] = " "
  lines[#lines + 1] = "Companion app: " .. St.SyncLine()
  local agent = St.Plain(St.Field(St.Agent(), "agent", "string"))
  if agent then lines[#lines + 1] = "Companion app version: " .. agent end
  for _, line in ipairs(St.UpdateLines()) do lines[#lines + 1] = "|cff40ff40" .. line .. "|r" end
  lines[#lines + 1] = "|cff999999(The app's status is as of your last login or /reload.)|r"
  if ns.errors.count > 0 then
    lines[#lines + 1] = "|cffff8080" .. ns.errors.count .. " errors caught; /ryna status shows the last.|r"
  end
  return table.concat(lines, "\n")
end

local function messagesText()
  local msgs = St.Messages(10)
  if #msgs == 0 then return "No messages" end
  local lines = {}
  for _, m in ipairs(msgs) do
    local when = m.at and (St.Ago(m.at) .. ": ") or ""
    lines[#lines + 1] = (LEVEL_COLOR[m.level] or "") .. when .. m.text .. "|r"
  end
  return table.concat(lines, "\n")
end

-- Fills every section and re-lays out the parts below the (variable-height) status text.
function P.Refresh()
  if not P.panel or not ns.db then return end
  local settings = ns.db.settings
  P.status:SetText(statusText())
  local s = stack(P.content)
  s.y = P.statusAnchor + 150 - (P.status:GetStringHeight() or 150)
  s.add(P.h2, 16)
  s.add(P.modeLabel, 14, 8, 4)
  s.add(P.mode, 26, 8, 2)
  s.add(P.modeHelp, P.modeHelp:GetStringHeight() or 24, 8, 10)
  P.mode.sync()
  P.chat:SetChecked(settings.chatSightings)
  P.census:SetChecked(settings.census)
  P.group:SetChecked(settings.groupMembers)
  P.acl:SetChecked(settings.advancedCombatLog)
  P.minimap:SetChecked(settings.minimap.shown)
  P.debug:SetChecked(settings.debug)
  P.chat.setActive(settings.census)
  P.group.setActive(settings.census)
  P.census.place(s, 4)
  P.chat.place(s, 30)
  P.group.place(s, 30)
  P.acl.place(s, 4)
  P.minimap.place(s, 4)
  P.debug.place(s, 4)
  s.y = s.y - 6
  s.add(P.h3, 16)
  P.messages:SetText(messagesText())
  s.add(P.messages, P.messages:GetStringHeight() or 14, 8)
  if P.content ~= P.panel then P.content:SetHeight(-s.y + 20) end
end

-- Called once from PLAYER_LOGIN (under ns.Run). Returns true when a panel is registered.
function P.Register()
  if P.panel then return true end
  local panel = build()
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
    ns.Print("the settings open after combat.")
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
  ns.Print("no settings panel on this client; /ryna status prints the status.")
end
