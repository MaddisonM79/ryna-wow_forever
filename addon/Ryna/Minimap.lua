-- Minimap: a round button on the minimap's edge. Left-click opens the panel, dragging moves it
-- around the ring, the tooltip shows the one-line status. Its angle and whether it's shown live
-- in RynaDB.settings.minimap. No library; nothing happens on a client without a Minimap frame.

local _, ns = ...
local L = ns.L

local M = {}
ns.Minimap = M

local ICON = "Interface\\Icons\\INV_Misc_Note_01"

local atan2 = math.atan2 or function(y, x) return math.atan(y, x) end

local function place(button, angle)
  local map = _G.Minimap
  local radius = (map:GetWidth() or 140) / 2 + 5
  local rad = math.rad(angle)
  button:ClearAllPoints()
  button:SetPoint("CENTER", map, "CENTER", math.cos(rad) * radius, math.sin(rad) * radius)
end

local function cursorAngle()
  local map = _G.Minimap
  local mx, my = map:GetCenter()
  local x, y = GetCursorPosition()
  local scale = map:GetEffectiveScale()
  return math.deg(atan2(y / scale - my, x / scale - mx)) % 360
end

local function onDragUpdate(self)
  local angle = cursorAngle()
  ns.db.settings.minimap.angle = angle
  place(self, angle)
end

local function tooltip(self)
  local tip = _G.GameTooltip
  if not tip then return end
  tip:SetOwner(self, "ANCHOR_LEFT")
  tip:SetText("Ryna " .. ns.VERSION)
  tip:AddLine(ns.Status.SyncLine(), 1, 1, 1, true)
  if #ns.Status.UpdateLines() > 0 then tip:AddLine(L["Update available"], 0.25, 1, 0.25) end
  tip:AddLine(L["Click: open Ryna. Drag: move this button."], 0.6, 0.6, 0.6, true)
  tip:Show()
end

function M.Create()
  if M.button or not _G.Minimap then return end
  local b = CreateFrame("Button", "RynaMinimapButton", _G.Minimap)
  b:SetSize(31, 31)
  b:SetFrameStrata("MEDIUM")
  b:SetFrameLevel(8)
  b:RegisterForClicks("LeftButtonUp")
  b:RegisterForDrag("LeftButton")
  b:SetHighlightTexture("Interface\\Minimap\\UI-Minimap-ZoomButton-Highlight")

  local bg = b:CreateTexture(nil, "BACKGROUND")
  bg:SetTexture("Interface\\Minimap\\UI-Minimap-Background")
  bg:SetSize(20, 20)
  bg:SetPoint("CENTER")
  local icon = b:CreateTexture(nil, "ARTWORK")
  icon:SetTexture(ICON)
  icon:SetSize(18, 18)
  icon:SetPoint("CENTER")
  icon:SetTexCoord(0.08, 0.92, 0.08, 0.92)
  local border = b:CreateTexture(nil, "OVERLAY")
  border:SetTexture("Interface\\Minimap\\MiniMap-TrackingBorder")
  border:SetSize(53, 53)
  border:SetPoint("TOPLEFT")

  b:SetScript("OnClick", function() ns.Run("minimap click", ns.Panel.Open) end)
  b:SetScript("OnDragStart", function(self)
    self:SetScript("OnUpdate", function(s) ns.Run("minimap drag", onDragUpdate, s) end)
  end)
  b:SetScript("OnDragStop", function(self) self:SetScript("OnUpdate", nil) end)
  b:SetScript("OnEnter", function(self) ns.Run("minimap tooltip", tooltip, self) end)
  b:SetScript("OnLeave", function()
    if _G.GameTooltip then _G.GameTooltip:Hide() end
  end)

  M.button = b
  M.Update()
end

-- Shows or hides the button and puts it at its saved angle.
function M.Update()
  local b = M.button
  if not b then return end
  local mm = ns.db.settings.minimap
  place(b, mm.angle)
  if mm.shown then b:Show() else b:Hide() end
end
