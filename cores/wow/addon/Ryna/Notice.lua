-- Notice: when the API has said this version of Ryna is no longer supported (RynaAgentStatus.support,
-- written by the companion app; docs/ryna-agent-cores-mods.md 9.1), a small frame at login with the
-- sentence, how to update and the policy's note, closed with a button. Once per login (and /reload),
-- never in chat, and nothing at all for any other band: reduced is the Status tab's first line.
-- Every frame call is tried, with a plainer fallback, and Init runs it under ns.Run.

local _, ns = ...
local L = ns.L

local N = {}
ns.Notice = N

local WIDTH = 420

-- CreateFrame with the first template this client has (none, last).
local function create(kind, name, parent, templates)
  for _, template in ipairs(templates or {}) do
    local ok, f = pcall(CreateFrame, kind, name, parent, template)
    if ok and f then return f end
  end
  local ok, f = pcall(CreateFrame, kind, name, parent)
  if ok then return f end
  return nil
end

local function text(parent, fontObject, size)
  local ok, fs = pcall(parent.CreateFontString, parent, nil, "OVERLAY", fontObject)
  if not ok or not fs then fs = parent:CreateFontString(nil, "OVERLAY") end
  if not fs:GetFont() then fs:SetFont(_G.STANDARD_TEXT_FONT or "Fonts\\FRIZQT__.TTF", size or 12) end
  fs:SetJustifyH("LEFT")
  fs:SetJustifyV("TOP")
  if fs.SetWordWrap then fs:SetWordWrap(true) end
  fs:SetWidth(WIDTH - 32)
  return fs
end

local function backdrop(f)
  if f.SetBackdrop and pcall(f.SetBackdrop, f, {
    bgFile = "Interface\\Tooltips\\UI-Tooltip-Background",
    edgeFile = "Interface\\Tooltips\\UI-Tooltip-Border",
    tile = true, tileSize = 16, edgeSize = 16,
    insets = { left = 4, right = 4, top = 4, bottom = 4 },
  }) then
    pcall(f.SetBackdropColor, f, 0.04, 0.04, 0.05, 0.96)
    pcall(f.SetBackdropBorderColor, f, 0.9, 0.25, 0.25, 1)
    return
  end
  local bg = f:CreateTexture(nil, "BACKGROUND")
  bg:SetAllPoints()
  bg:SetColorTexture(0.04, 0.04, 0.05, 0.96)
end

-- What the notice says: { title, body, note } for an unsupported version, else nil.
function N.Words()
  local sup = ns.Status.Support()
  if not sup or sup.band ~= "unsupported" then return nil end
  return {
    title = L["Ryna needs updating"],
    body = L[sup.sentence] .. "\n\n" .. L["Open the Ryna app to update."],
    note = sup.note,
  }
end

function N.Create()
  if N.frame then return N.frame end
  local f = create("Frame", "RynaVersionNotice", _G.UIParent, { "BackdropTemplate" })
  if not f then return nil end
  N.frame = f
  f:SetWidth(WIDTH)
  f:SetPoint("TOP", _G.UIParent, "TOP", 0, -140)
  f:SetFrameStrata("DIALOG")
  if f.SetToplevel then f:SetToplevel(true) end
  f:SetClampedToScreen(true)
  f:EnableMouse(true)
  backdrop(f)
  -- Escape closes it, like the game's own windows.
  if type(_G.UISpecialFrames) == "table" then table.insert(_G.UISpecialFrames, "RynaVersionNotice") end

  f.title = text(f, "GameFontNormalLarge", 14)
  f.title:SetPoint("TOPLEFT", f, "TOPLEFT", 16, -14)
  f.body = text(f, "GameFontHighlight", 12)
  f.body:SetPoint("TOPLEFT", f.title, "BOTTOMLEFT", 0, -8)
  f.note = text(f, "GameFontDisable", 11)
  f.note:SetPoint("TOPLEFT", f.body, "BOTTOMLEFT", 0, -8)

  local close = create("Button", nil, f, { "UIPanelButtonTemplate" })
  close:SetSize(96, 22)
  if not close:GetFontString() then
    local fs = text(close, "GameFontNormal")
    fs:SetAllPoints()
    fs:SetJustifyH("CENTER")
    close:SetFontString(fs)
  end
  close:SetText(L["Close"])
  close:SetPoint("BOTTOMRIGHT", f, "BOTTOMRIGHT", -12, 12)
  close:SetScript("OnClick", function() f:Hide() end)
  f.close = close
  f:Hide()
  return f
end

-- At login: the notice, when this version is unsupported, once per login.
function N.ShowAtLogin()
  if N.shown then return end
  local words = N.Words()
  if not words then return end
  local f = N.Create()
  if not f then return end
  N.shown = true
  f.title:SetText("|cffff4040" .. words.title .. "|r")
  f.body:SetText(words.body)
  f.note:SetText(words.note or "")
  local height = 14 + (f.title:GetStringHeight() or 16) + 8 + (f.body:GetStringHeight() or 40)
  if words.note then height = height + 8 + (f.note:GetStringHeight() or 14) end
  f:SetHeight(height + 50)
  f:Show()
end
