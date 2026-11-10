-- Strip: the session strip, one line of this session's numbers on screen (Session.lua):
--
--   Kills 42 · 3,120 XP/h · 1g 20s/h · 2 quests · 1h 12m
--
-- A plain frame (RynaChronicleStrip): the game's own small font on a translucent black backdrop,
-- nothing else, so it sits as well in the default UI as in a dark one like ElvUI's. Hovering shows
-- the session's top kills and the rest of its numbers; a click opens Ryna's window
-- (Window.lua); a right-click locks or unlocks it; Shift-drag moves it while it is unlocked.
--
-- Settings (RynaDB.settings): sessionStrip (shown, default on), sessionStripLocked (default off),
-- and sessionStripPos { point, relativePoint, x, y } (its place; none means the default place, at
-- the top of the screen). It redraws when Session.lua says something changed (at most every half
-- second) and every 5 seconds for the clock, only while it is shown.

local _, ns = ...
if ns.off then return end
local L = ns.L

local St = {}
ns.Strip = St

local SEP = " · "
local TICK_SECONDS = 5
local TOP_KILLS = 8
local DEFAULT_POS = { point = "TOP", relativePoint = "TOP", x = 0, y = -8 }

ns.API.Settings.AddDefaults({ sessionStrip = true, sessionStripLocked = false })

local function settings()
  return ns.API.Settings.All()
end

local function locked()
  return ns.Setting("sessionStripLocked") and true or false
end

-- The strip's line from a session table (S.Get()).
function St.Line(s)
  if not s then return "" end
  local S = ns.Session
  return string.format(L["Kills %s"], S.Number(s.kills)) .. SEP
    .. string.format(L["%s XP/h"], S.Number(s.xpPerHour)) .. SEP
    .. string.format(L["%s/h"], S.Money(s.goldPerHour)) .. SEP
    .. ns.Plural(s.quests, L["%d quest"], L["%d quests"]) .. SEP
    .. S.Duration(s.elapsed)
end

-- The hover tooltip's lines: { left, right } pairs, a "" left for a gap.
function St.TooltipLines(s)
  local S = ns.Session
  local out = {}
  local function add(left, right) out[#out + 1] = { left, right } end
  add(string.format(L["Since %s (%s)"], date("%H:%M", s.startAt), S.Duration(s.elapsed)))
  add(L["Kills"], string.format(L["%s (%s/h)"], S.Number(s.kills), S.Number(s.killsPerHour)))
  add(L["Experience"], string.format(L["%s (%s/h)"], S.Number(s.xp), S.Number(s.xpPerHour)))
  if s.levels > 0 then add(L["Levels gained"], S.Number(s.levels)) end
  add(L["Money"], string.format(L["%s (%s/h)"], S.Money(s.gold), S.Money(s.goldPerHour)))
  add(L["Gained / spent"], string.format(L["%s / %s"], S.Money(s.goldGained), S.Money(s.goldSpent)))
  add(L["Quests turned in"], S.Number(s.quests))
  add(L["Items looted"], S.Number(s.items))
  add(L["Deaths"], S.Number(s.deaths))
  if #s.creatures > 0 then
    add("")
    add(L["Top kills this session"])
    for i = 1, math.min(TOP_KILLS, #s.creatures) do
      local c = s.creatures[i]
      add(ns.Status.Plain(c.name) or "?", string.format(L["%s (%s XP)"], S.Number(c.n), S.Number(c.xp)))
    end
  end
  add("")
  add(L["Click: Ryna's window. Right-click: lock or unlock."])
  if not locked() then add(L["Shift-drag to move."]) end
  return out
end

-- The frame ---------------------------------------------------------------------------------

local function shown()
  return St.frame ~= nil and St.frame:IsShown() and true or false
end

function St.Redraw()
  local f = St.frame
  if not f or not f:IsShown() then return end
  local line = St.Line(ns.Session.Get())
  if line ~= f.line then
    f.line = line
    f.text:SetText(line)
    f:SetWidth((f.text:GetStringWidth() or 200) + 16)
  end
  if f.hovered then St.ShowTooltip(f) end
end

function St.ShowTooltip(f)
  local tip = _G.GameTooltip
  local s = ns.Session.Get()
  if not tip or not s then return end
  tip:SetOwner(f, "ANCHOR_BOTTOM")
  tip:ClearLines()
  tip:AddLine(L["Chronicle: this session"])
  for _, line in ipairs(St.TooltipLines(s)) do
    if line[2] then
      tip:AddDoubleLine(line[1], line[2], 0.8, 0.8, 0.8, 1, 1, 1)
    elseif line[1] ~= "" then
      tip:AddLine(line[1], 0.6, 0.6, 0.6, true)
    else
      tip:AddLine(" ")
    end
  end
  tip:Show()
end

local function place(f)
  local pos = settings() and settings().sessionStripPos
  if type(pos) ~= "table" or type(pos.x) ~= "number" or type(pos.y) ~= "number" then pos = DEFAULT_POS end
  f:ClearAllPoints()
  local point = pos.point or "TOP"
  local ok = pcall(f.SetPoint, f, point, _G.UIParent, pos.relativePoint or point, pos.x, pos.y)
  if not ok then f:SetPoint("TOP", _G.UIParent, "TOP", DEFAULT_POS.x, DEFAULT_POS.y) end
end

local function savePosition(f)
  local point, _, relativePoint, x, y = f:GetPoint(1)
  if type(point) ~= "string" or type(x) ~= "number" or type(y) ~= "number" then return end
  St.pos = { point = point, relativePoint = relativePoint or point, x = math.floor(x + 0.5), y = math.floor(y + 0.5) }
  ns.API.Settings.Set("sessionStripPos", St.pos)
end

local ticker

local function startTicker()
  if ticker or not (_G.C_Timer and _G.C_Timer.NewTicker) then return end
  ticker = ns.Try(_G.C_Timer.NewTicker, TICK_SECONDS, function() ns.Run("session strip", St.Redraw) end)
end

local function stopTicker()
  if ticker and ticker.Cancel then pcall(ticker.Cancel, ticker) end
  ticker = nil
end

-- Once, at login (Init.lua).
function St.Create()
  if St.frame then return St.Apply() end
  local f = CreateFrame("Button", "RynaChronicleStrip", _G.UIParent)
  St.frame = f
  f:SetSize(240, 20)
  f:SetFrameStrata("MEDIUM")
  f:SetClampedToScreen(true)
  f:SetMovable(true)
  f:EnableMouse(true)
  f:RegisterForClicks("LeftButtonUp", "RightButtonUp")
  f:RegisterForDrag("LeftButton")
  local bg = f:CreateTexture(nil, "BACKGROUND")
  bg:SetAllPoints()
  bg:SetColorTexture(0, 0, 0, 0.45)
  local ok, text = pcall(f.CreateFontString, f, nil, "OVERLAY", "GameFontHighlightSmall")
  if not ok or not text then text = f:CreateFontString(nil, "OVERLAY") end
  if not text:GetFont() then text:SetFont(_G.STANDARD_TEXT_FONT or "Fonts\\FRIZQT__.TTF", 11) end
  text:SetPoint("CENTER", f, "CENTER", 0, 0)
  if text.SetWordWrap then text:SetWordWrap(false) end
  f.text = text
  f.line, f.moving, f.dragged, f.hovered = false, false, false, false

  f:SetScript("OnDragStart", function(self)
    if locked() or not ns.Try(ns.Fn("IsShiftKeyDown")) then return end
    self.moving = true
    self:StartMoving()
  end)
  f:SetScript("OnDragStop", function(self)
    if not self.moving then return end
    self.moving = false
    self.dragged = true
    self:StopMovingOrSizing()
    if self.SetUserPlaced then pcall(self.SetUserPlaced, self, false) end -- the place is Ryna's to keep
    ns.Run("session strip", savePosition, self)
  end)
  f:SetScript("OnClick", function(self, button)
    if self.dragged then
      self.dragged = false
      return
    end
    if button == "RightButton" then
      ns.Run("session strip", St.ToggleLock)
    elseif ns.Window then
      ns.Run("chronicle window", ns.Window.Toggle)
    end
  end)
  f:SetScript("OnEnter", function(self)
    self.hovered = true
    ns.Run("session strip", St.ShowTooltip, self)
  end)
  f:SetScript("OnLeave", function(self)
    self.hovered = false
    local tip = _G.GameTooltip
    if tip then tip:Hide() end
  end)
  f:SetScript("OnShow", function()
    startTicker()
    ns.Run("session strip", St.Redraw)
  end)
  f:SetScript("OnHide", stopTicker)

  local saved = settings() and settings().sessionStripPos
  if type(saved) == "table" then St.pos = saved end
  f:Hide()
  St.Apply()
end

-- Shows or hides it and puts it in its place, from the settings.
function St.Apply()
  local f = St.frame
  if not f then return end
  place(f)
  if ns.Setting("sessionStrip") and ns.rec then
    f:Show()
    f.line = false
    St.Redraw()
  else
    f:Hide()
  end
  ns.API.Panel.RefreshIfShown()
end

function St.ToggleLock()
  local on = not locked()
  ns.API.Settings.Set("sessionStripLocked", on)
  ns.Print(on and L["the session strip is locked. Right-click it to unlock."]
    or L["the session strip is unlocked: Shift-drag to move it."])
  ns.API.Panel.RefreshIfShown()
end

function St.ResetPosition()
  St.pos = nil
  ns.API.Settings.Set("sessionStripPos", nil)
  St.Apply()
end

-- After the panel's Defaults (which replaced RynaDB.settings): back in the place it had.
function St.OnDefaults()
  if St.pos then ns.API.Settings.Set("sessionStripPos", St.pos) end
  St.Apply()
end

ns.Session.Watch(shown, St.Redraw)
