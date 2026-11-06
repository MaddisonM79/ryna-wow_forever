-- Zone: the character's zone, `zone` and `death` journal events, and the combat-log setting.
-- RynaDB.settings.combatLog is "instances" (default), "always" or "never". Settings can be lost
-- with the rest of RynaDB if the client doesn't restore it; the default then applies.

local _, ns = ...

local Z = {}
ns.Zone = Z

local NonEmpty = ns.NonEmpty

local function loggingState()
  local fn = ns.Fn("LoggingCombat")
  if not fn then return nil, nil end
  return ns.Try(fn) and true or false, fn
end

-- Turns logging on or off for the current place and setting. The addon only turns logging off
-- when it is the reason it's on (it turned it on, or the player just left an instance in
-- "instances" mode), so a /combatlog typed in the open world is left alone.
function Z.ApplyCombatLog(inInstance)
  local on, fn = loggingState()
  if not fn then return end
  local mode = ns.db.settings.combatLog
  local wanted = mode == "always" or (mode == "instances" and inInstance)
  if wanted then
    if not on and pcall(fn, true) then
      Z.addonTurnedOn = true
      ns.Print("combat logging on" .. (inInstance and " (instance)" or ""))
    end
  elseif on and (Z.addonTurnedOn or (mode == "instances" and Z.wasInInstance)) then
    if pcall(fn, false) then
      Z.addonTurnedOn = false
      ns.Print("combat logging off")
    end
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
    ns.Journal.Add("zone", { zone = zone, subzone = subzone, instance = instance, difficulty = difficulty })
  end
  Z.ApplyCombatLog(inInstance)
end

function Z.SetMode(mode)
  ns.db.settings.combatLog = mode
  Z.ApplyCombatLog(Z.wasInInstance and true or false)
end

function Z.IsLogging()
  return (loggingState())
end

ns.On("ZONE_CHANGED_NEW_AREA", function() Z.Update() end)
ns.On("PLAYER_DEAD", function()
  if ns.rec then ns.Journal.Add("death", { zone = ns.rec.zone }) end
end)
