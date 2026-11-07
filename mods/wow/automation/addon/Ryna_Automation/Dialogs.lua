-- Dialogs: the confirmations the game asks for again and again, and cinematics. Two toggles
-- (Settings.lua), both off by default.
--
-- autoConfirmDialogs answers "Yes" for the player, a frame later and only while the game's own dialog
-- is still up: looting a bind-on-pickup item (LOOT_BIND_CONFIRM -> ConfirmLootSlot), equipping a
-- bind-on-equip one (EQUIP_BIND_CONFIRM -> EquipPendingItem), using a bind-on-use one
-- (USE_BIND_CONFIRM -> ConfirmBindOnUse) and a bind-on-pickup roll (CONFIRM_LOOT_ROLL ->
-- ConfirmLootRoll), then hides that dialog. Deleting a good item keeps its dialog: only the typing goes
-- (the word DELETE is filled in and the button enabled, as Leatrix Plus does), so it still takes a click.
--
-- autoSkipCinematics cancels an in-game cinematic (CINEMATIC_START) and stops a pre-rendered movie
-- (PLAY_MOVIE). Holding Shift when one starts means "let me watch it".

local _, ns = ...
if ns.off then return end
local L = ns.L
local S = ns.Settings

local D = {}
ns.Dialogs = D

-- Bind, use and roll confirmations ---------------------------------------------------------------

-- event -> the dialog it shows, the client function that answers it, and that function's arguments.
local CONFIRMS = {
  LOOT_BIND_CONFIRM = { popup = "LOOT_BIND", fn = "ConfirmLootSlot", args = function(slot) return slot end },
  EQUIP_BIND_CONFIRM = { popup = "EQUIP_BIND", fn = "EquipPendingItem", args = function(slot) return slot end },
  USE_BIND_CONFIRM = { popup = "USE_BIND", fn = "ConfirmBindOnUse", args = function() end },
  CONFIRM_LOOT_ROLL = { popup = "CONFIRM_LOOT_ROLL", fn = "ConfirmLootRoll",
    args = function(rollID, rollType) return rollID, rollType end },
}

for event, c in pairs(CONFIRMS) do
  ns.On(event, function(_, ...)
    if not S.On("autoConfirmDialogs") then return end
    if not ns.Need("confirmDialogs", c.fn) then return end
    local n, a, b = select("#", ...), ...
    -- Keyed on the event and what it's for, so a re-fired event is answered once.
    ns.Soon(event .. ":" .. tostring(a) .. ":" .. tostring(b), function() return ns.PopupShown(c.popup) end,
      function()
        if n > 0 then ns.G(c.fn, c.args(a, b)) else ns.G(c.fn) end
        ns.HidePopups(c.popup)
        ns.Debug("automation: answered %s", c.popup)
      end)
  end)
end

-- Deleting a good item ---------------------------------------------------------------------------

local DELETE_POPUPS = { "DELETE_GOOD_ITEM", "DELETE_GOOD_QUEST_ITEM" }

-- Types the confirmation word into a delete dialog and enables its button. Nothing when it's done
-- already (the dialog's own OnShow and the event both come here).
function D.FillDelete(popup)
  local word = _G.DELETE_ITEM_CONFIRM_STRING
  if type(word) ~= "string" or word == "" then return ns.Missing("confirmDialogs", "DELETE_ITEM_CONFIRM_STRING") end
  local box = ns.PopupPart(popup, "EditBox")
  if not box or type(box.SetText) ~= "function" then return ns.Missing("confirmDialogs", "a delete dialog edit box") end
  if type(box.GetText) == "function" and ns.Try(box.GetText, box) == word then return end
  ns.Try(box.SetText, box, word)
  local button = ns.PopupPart(popup, "Button1")
  if button and type(button.Enable) == "function" then ns.Try(button.Enable, button) end
  ns.Debug("automation: filled in the delete confirmation")
end

local function fillVisible()
  for _, which in ipairs(DELETE_POPUPS) do
    local popup = ns.PopupFrame(which)
    if popup then return D.FillDelete(popup) end
  end
end

-- The dialog's own OnShow, hooked (never replaced) where the client has it.
local hook = ns.Fn("hooksecurefunc")
local dialogs = _G.StaticPopupDialogs
for _, which in ipairs(DELETE_POPUPS) do
  local d = type(dialogs) == "table" and dialogs[which] or nil
  if hook and type(d) == "table" and type(d.OnShow) == "function" then
    ns.Try(hook, d, "OnShow", function(popup)
      if not S.On("autoConfirmDialogs") then return end
      ns.Soon("delete", function()
        return ns.Shown(popup) and (popup.which == nil or popup.which == which)
      end, function() D.FillDelete(popup) end)
    end)
  end
end

ns.On("DELETE_ITEM_CONFIRM", function()
  if not S.On("autoConfirmDialogs") then return end
  ns.Soon("delete", nil, fillVisible)
end)

-- Cinematics -------------------------------------------------------------------------------------

ns.On("CINEMATIC_START", function(_, canBeCancelled)
  if not S.On("autoSkipCinematics") then return end
  if canBeCancelled == false then
    ns.Debug("automation: this cinematic can't be skipped")
    return
  end
  local cancel = ns.Fn("CinematicFrame_CancelCinematic") or ns.Fn("StopCinematic")
  if not cancel then return ns.Missing("skipCinematics", "CinematicFrame_CancelCinematic") end
  ns.Soon("cinematic", function() return ns.Shown("CinematicFrame") end, function()
    ns.Try(cancel)
    ns.Report(L["Skipped a cinematic. Hold Shift when one starts to watch it."])
  end)
end)

ns.On("PLAY_MOVIE", function()
  if not S.On("autoSkipCinematics") then return end
  ns.Soon("movie", function() return ns.Shown("MovieFrame") end, function()
    local movie = _G.MovieFrame
    if type(movie) == "table" and type(movie.StopMovie) == "function" then
      ns.Try(movie.StopMovie, movie)
    elseif ns.Fn("GameMovieFinished") then
      ns.G("GameMovieFinished")
    elseif type(movie) == "table" and type(movie.Hide) == "function" then
      ns.Try(movie.Hide, movie)
    else
      return ns.Missing("skipCinematics", "MovieFrame")
    end
    ns.Report(L["Skipped a cinematic. Hold Shift when one starts to watch it."])
  end)
end)
