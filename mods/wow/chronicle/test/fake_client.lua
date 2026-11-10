-- A fake WoW client for the window tests (window_test.lua): frames, font strings, textures, sliders,
-- edit boxes and a tooltip that keep what they're told (text, shown, points, values, scripts), an
-- event bus, timers that run when the test says, and the tooltip data processor. Not a test itself
-- (only *_test.lua files run); dofile it from the repository root.
--
-- A frame answers the methods it needs to remember things; any other Set*, Enable*, Register*, ...
-- call does nothing, and every other unknown method is nil, so code that checks for an API (the
-- modern dropdown's SetupMenu, say) sees a client without it.
-- luacheck: ignore 1

local F = {}

local frames = {}
F.frames = frames
F.named = {}

local function noop() end

local QUIET = { "^Set", "^Enable", "^Register", "^Unregister", "^Clear", "^Start", "^Stop", "^Lock",
  "^Unlock", "^Hook", "^Disable" }
local MISSING = { SetupMenu = true, GenerateMenu = true, SetBackdrop = true, SetShown = true }

local methods = {}

local function new(kind, name, parent)
  local o = {
    _kind = kind, _name = name, _parent = parent, _shown = true, _scripts = {}, _points = {},
    _events = {}, _text = nil, _value = 0, _min = 0, _max = 0, _alpha = 1, _enabled = true,
    _width = 0, _height = 0,
  }
  setmetatable(o, {
    __index = function(_, k)
      local m = methods[k]
      if m then return m end
      if MISSING[k] then return nil end
      for _, p in ipairs(QUIET) do
        if tostring(k):find(p) then return noop end
      end
      return nil
    end,
  })
  frames[#frames + 1] = o
  if type(name) == "string" then
    F.named[name] = o
    _G[name] = o
  end
  return o
end
F.new = new

function methods.GetName(self) return self._name end
function methods.GetParent(self) return self._parent end
function methods.SetText(self, t)
  self._text = t
  if self._fs then self._fs:SetText(t) end
  if self._kind == "EditBox" and self._scripts.OnTextChanged then self._scripts.OnTextChanged(self, false) end
end
function methods.GetText(self) return self._text end
function methods.SetTextColor(self, r, g, b) self._color = { r, g, b } end
function methods.GetFont() return "Fonts\\FRIZQT__.TTF", 12 end
function methods.GetStringWidth(self) return #(tostring(self._text or "")) * 6 end
function methods.GetStringHeight() return 12 end
function methods.SetWidth(self, w) self._width = w end
function methods.GetWidth(self) return self._width end
function methods.SetHeight(self, h) self._height = h end
function methods.GetHeight(self) return self._height end
function methods.SetSize(self, w, h) self._width, self._height = w, h end
function methods.SetPoint(self, ...) self._points[#self._points + 1] = { ... } end
function methods.ClearAllPoints(self) self._points = {} end
function methods.GetPoint(self, i)
  local p = self._points[i or 1]
  if not p then return nil end
  return p[1], p[2], p[3], p[4], p[5]
end
function methods.CreateTexture(self, name) return new("Texture", name, self) end
function methods.CreateFontString(self, name) return new("FontString", name, self) end
function methods.SetScript(self, k, f) self._scripts[k] = f end
function methods.GetScript(self, k) return self._scripts[k] end
function methods.HookScript(self, k, f)
  local old = self._scripts[k]
  self._scripts[k] = function(...)
    if old then old(...) end
    f(...)
  end
end
function methods.RegisterEvent(self, e) self._events[e] = true end
function methods.UnregisterEvent(self, e) self._events[e] = nil end
function methods.IsEventRegistered(self, e) return self._events[e] == true end
function methods.Show(self)
  if self._shown then return end
  self._shown = true
  if self._scripts.OnShow then self._scripts.OnShow(self) end
end
function methods.Hide(self)
  if not self._shown then return end
  self._shown = false
  if self._scripts.OnHide then self._scripts.OnHide(self) end
end
function methods.IsShown(self) return self._shown end
function methods.IsVisible(self)
  local o = self
  while o do
    if o._shown == false then return false end
    o = o._parent
  end
  return true
end
function methods.SetAlpha(self, a) self._alpha = a end
function methods.GetAlpha(self) return self._alpha end
function methods.Enable(self) self._enabled = true end
function methods.Disable(self) self._enabled = false end
function methods.SetEnabled(self, on) self._enabled = on and true or false end
function methods.IsEnabled(self) return self._enabled end
function methods.LockHighlight(self) self._locked = true end
function methods.UnlockHighlight(self) self._locked = false end
function methods.GetFontString(self) return self._fs end
function methods.SetFontString(self, fs) self._fs = fs end
function methods.SetMinMaxValues(self, lo, hi) self._min, self._max = lo, hi end
function methods.GetMinMaxValues(self) return self._min, self._max end
function methods.SetValue(self, v)
  self._value = v
  if self._scripts.OnValueChanged then self._scripts.OnValueChanged(self, v) end
end
function methods.GetValue(self) return self._value end
function methods.SetTexture(self, t) self._texture = t end
function methods.GetTexture(self) return self._texture end
function methods.SetStatusBarColor(self, r, g, b) self._barColor = { r, g, b } end
function methods.SetChecked(self, on) self._checked = on and true or false end
function methods.GetChecked(self) return self._checked end
function methods.GetCenter() return 0, 0 end
function methods.GetNormalTexture(self) return self._normal end
function methods.GetHighlightTexture(self) return self._highlight end
function methods.GetEffectiveScale() return 1 end
function methods.GetNumPoints(self) return #self._points end

-- A click as the client delivers it.
function F.Click(f, button)
  if f._scripts.OnClick then f._scripts.OnClick(f, button or "LeftButton") end
end

-- CreateFrame: every template is accepted and gives a plain frame (so the code's fallbacks for
-- templates without their parts run, as on a trimmed client).
function F.CreateFrame(kind, name, parent)
  return new(kind, name, parent)
end

-- Events: OnEvent of every frame that registered the event.
function F.Fire(event, ...)
  for _, f in ipairs(frames) do
    if f._events[event] and f._scripts.OnEvent then f._scripts.OnEvent(f, event, ...) end
  end
end

-- Timers: queued by C_Timer.After, run by F.RunTimers (and those they queue, up to `rounds`).
local timers = {}
F.timers = timers
function F.RunTimers(rounds)
  for _ = 1, rounds or 5 do
    if #timers == 0 then return end
    local batch = {}
    for i, t in ipairs(timers) do batch[i] = t end
    for i = #timers, 1, -1 do timers[i] = nil end
    for _, t in ipairs(batch) do t() end
  end
end
function F.DropTimers()
  for i = #timers, 1, -1 do timers[i] = nil end
end

-- The tooltip: named, with <name>TextLeft<i> regions as the game's are. SetHyperlink puts a line for
-- the link and runs the item post calls, as the data processor does.
F.postCalls = {}
function F.NewTooltip(name)
  local t = new("GameTooltip", name, nil)
  t._shown = false
  t.lines = {}
  t.owner = nil
  for i = 1, 60 do
    _G[name .. "TextLeft" .. i] = { GetText = function() local l = t.lines[i]; return l and l.left end }
  end
  function t.SetOwner(self, owner) self.owner = owner end
  function t.ClearLines(self) self.lines = {} end
  function t.AddLine(self, text) self.lines[#self.lines + 1] = { left = text } end
  function t.AddDoubleLine(self, l, r) self.lines[#self.lines + 1] = { left = l, right = r } end
  function t.NumLines(self) return #self.lines end
  function t.SetHyperlink(self, link)
    self.lines[#self.lines + 1] = { left = "link " .. link }
    local id = tonumber(link:match("Hitem:(%d+)"))
    for _, pc in ipairs(F.postCalls) do
      if pc.kind == 0 then pc.fn(self, { id = id, hyperlink = link }) end
    end
  end
  function t.Texts(self)
    local out = {}
    for i, l in ipairs(self.lines) do out[i] = l.left end
    return out
  end
  return t
end

-- Colour codes off, for comparing what is drawn.
function F.Plain(s)
  if type(s) ~= "string" then return s end
  return (s:gsub("|c%x%x%x%x%x%x%x%x", ""):gsub("|r", ""))
end

-- The client's globals the core and Chronicle call directly (the rest they look up through _G).
function F.Install(opts)
  opts = opts or {}
  _G.CreateFrame = F.CreateFrame
  _G.UIParent = new("Frame", "UIParent")
  _G.Minimap = new("Frame", "Minimap")
  _G.Minimap._width = 140
  _G.UISpecialFrames = {}
  _G.GameTooltip = F.NewTooltip("GameTooltip")
  _G.ItemRefTooltip = F.NewTooltip("ItemRefTooltip")
  _G.DEFAULT_CHAT_FRAME = { AddMessage = function(_, msg) F.chat[#F.chat + 1] = msg end }
  F.chat = {}
  _G.SlashCmdList = {}
  _G.StaticPopupDialogs = {}
  _G.C_Timer = {
    After = function(_, fn) timers[#timers + 1] = fn end,
    NewTicker = function() return { Cancel = noop } end,
  }
  F.now = opts.now or 1800000000
  F.clock = opts.clock or 5000
  _G.time = function() return F.now end
  _G.GetServerTime = function() return F.now end
  _G.GetTime = function() return F.clock end
  _G.date = os.date
  _G.GetLocale = function() return "enUS" end
  _G.GetCursorPosition = function() return 0, 0 end
  _G.hooksecurefunc = noop
  _G.Enum = { TooltipDataType = { Item = 0, Unit = 2 } }
  _G.TooltipDataProcessor = {
    AddTooltipPostCall = function(kind, fn) F.postCalls[#F.postCalls + 1] = { kind = kind, fn = fn } end,
  }
  _G.ITEM_QUALITY_COLORS = {
    [0] = { hex = "|cff9d9d9d" }, [1] = { hex = "|cffffffff" }, [2] = { hex = "|cff1eff00" },
    [3] = { hex = "|cff0070dd" }, [4] = { hex = "|cffa335ee" },
  }
  _G.RAID_CLASS_COLORS = {
    PALADIN = { colorStr = "fff58cba" }, MAGE = { colorStr = "ff3fc7eb" }, WARRIOR = { r = 0.78, g = 0.61, b = 0.43 },
  }
  _G.LOCALIZED_CLASS_NAMES_MALE = { PALADIN = "Paladin", MAGE = "Mage", WARRIOR = "Warrior" }
  _G.HEADSLOT, _G.LEGSSLOT = "Head", "Legs"
  _G.FACTION_STANDING_LABEL4, _G.FACTION_STANDING_LABEL5 = "Neutral", "Friendly"
  _G.FACTION_BAR_COLORS = { [4] = { r = 0.9, g = 0.7, b = 0 }, [5] = { r = 0, g = 0.6, b = 0.1 } }
  _G.C_CVar = { SetCVar = noop, GetCVar = function() return "0" end, GetCVarBool = function() return false end }
  _G.C_Item = { GetItemIconByID = function(id) return 100000 + id end }
end

-- Loads an addon's files in its TOC's order with `ns` (the files the TOC names that exist; a locale
-- file loads, and does nothing outside its locale).
function F.LoadAddon(folder, name, ns, skip)
  local toc = assert(io.open(folder .. name .. ".toc")):read("*a")
  for line in toc:gmatch("[^\r\n]+") do
    if not line:match("^%s*#") and line:match("%.lua%s*$") then
      local file = line:gsub("\\", "/"):gsub("%s+$", "")
      if not (skip and skip[file]) then
        local chunk = assert(loadfile(folder .. file))
        chunk(name, ns)
      end
    end
  end
end

return F
