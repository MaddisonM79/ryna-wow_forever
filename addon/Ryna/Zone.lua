-- Zone: the character's zone, `zone` and `death` journal events, and the combat-log setting.
-- RynaDB.settings.combatLog is "instances" (default), "always" or "never". Settings can be lost
-- with the rest of RynaDB if the client doesn't restore it; the default then applies.

local _, ns = ...
local L = ns.L

local Z = {}
ns.Zone = Z

local NonEmpty = ns.NonEmpty

-- What the addon knows about logging. It never asks the client: on this client
-- `LoggingCombat()` with no argument reported "off" while a log file was being written
-- (2026-11-06), and a query that might also be a `LoggingCombat(false)` isn't worth the risk.
-- So the state is what the addon last set, or nil when it set nothing.
local function loggingState()
  local fn = ns.Fn("LoggingCombat")
  if not fn then return nil, nil end
  return Z.addonTurnedOn and true or false, fn
end

-- Turns logging on or off for the current place and setting. The addon only turns logging off
-- when it is the reason it's on (it turned it on, or the player just left an instance in
-- "instances" mode), so a /combatlog typed in the open world is left alone. Turning it on when
-- it already is costs nothing, so "on" is asked for every time it's wanted.
function Z.ApplyCombatLog(inInstance)
  local on, fn = loggingState()
  if not fn then
    ns.Debug("combat log: this client has no LoggingCombat, nothing to do")
    return
  end
  local mode = ns.db.settings.combatLog
  local where = inInstance and "in an instance" or "outside instances"
  local wanted = mode == "always" or (mode == "instances" and inInstance)
  if wanted then
    if pcall(fn, true) then
      if not on then ns.Print(inInstance and L["combat logging on (instance)"] or L["combat logging on"]) end
      Z.addonTurnedOn = true
      ns.Debug("combat log: mode %s, %s: %s", mode, where, on and "kept on" or "turned on")
    else
      ns.Debug("combat log: mode %s, %s: the client refused to turn it on", mode, where)
    end
  elseif on and (Z.addonTurnedOn or (mode == "instances" and Z.wasInInstance)) then
    if pcall(fn, false) then
      Z.addonTurnedOn = false
      ns.Print(L["combat logging off"])
      ns.Debug("combat log: mode %s, %s: turned off", mode, where)
    end
  else
    ns.Debug("combat log: mode %s, %s: not wanted here, left alone", mode, where)
  end
  Z.wasInInstance = inInstance
end

function Z.Update()
  local rec = ns.rec
  if not rec then return end
  local zone = NonEmpty(ns.Try(ns.Fn("GetRealZoneText"))) or NonEmpty(ns.Try(ns.Fn("GetZoneText")))
  local subzone = NonEmpty(ns.Try(ns.Fn("GetSubZoneText")))
  local inInstance, instanceType = ns.Try(ns.Fn("IsInInstance"))
  inInstance = (inInstance and instanceType ~= "none") and true or false
  local instance, difficulty
  if inInstance then
    local name, _, _, difficultyName = ns.Try(ns.Fn("GetInstanceInfo"))
    instance, difficulty = NonEmpty(name), NonEmpty(difficultyName)
  end
  rec.zone, rec.subzone = zone, subzone
  rec.lastSeen = ns.Now()
  if zone ~= Z.lastZone or instance ~= Z.lastInstance then
    Z.lastZone, Z.lastInstance = zone, instance
    local best = ns.Fn("C_Map", "GetBestMapForUnit")
    local mapId = best and ns.Try(best, "player") or nil
    if type(mapId) ~= "number" then mapId = nil end
    ns.Journal.Add("zone", {
      zone = zone, subzone = subzone, instance = instance, difficulty = difficulty, mapId = mapId,
    })
  end
  Z.ApplyCombatLog(inInstance)
end

function Z.SetMode(mode)
  ns.db.settings.combatLog = mode
  ns.Debug("setting combatLog: %s", mode)
  Z.ApplyCombatLog(Z.wasInInstance and true or false)
end

-- Advanced combat logging is a client variable the addon may set. On, the log carries positions
-- and health (what the companion app's fight timelines and deaths want); the setting defaults to on
-- and the variable is set at login and whenever the setting changes.
function Z.ApplyAdvancedLogging()
  local get = ns.Fn("GetCVar") or (C_CVar and C_CVar.GetCVar)
  local set = ns.Fn("SetCVar") or (C_CVar and C_CVar.SetCVar)
  if not set then
    ns.Debug("advanced combat logging: this client has no SetCVar")
    return
  end
  local wanted = ns.Setting("advancedCombatLog") and "1" or "0"
  local current = get and ns.Try(get, "advancedCombatLogging")
  if current ~= nil then current = tostring(current) end
  if current == wanted then
    ns.Debug("advanced combat logging: already %s", wanted == "1" and "on" or "off")
    return
  end
  if pcall(set, "advancedCombatLogging", wanted) then
    ns.Debug("advanced combat logging: set %s (was %s)", wanted == "1" and "on" or "off", tostring(current))
  else
    ns.Debug("advanced combat logging: the client refused to set it")
  end
end

function Z.IsLogging()
  return (loggingState())
end

ns.On("ZONE_CHANGED_NEW_AREA", function() Z.Update() end)
ns.On("PLAYER_DEAD", function()
  if ns.rec then ns.Journal.Add("death", { zone = ns.rec.zone }) end
end)
