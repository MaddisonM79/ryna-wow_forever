-- Zone: the character's zone, and the `zone` and `death` journal events. (Turning the combat log on
-- in instances is the core addon's, its CombatLog.lua.)

local _, ns = ...
if ns.off then return end

local Z = {}
ns.Zone = Z

local NonEmpty = ns.NonEmpty

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
end

ns.On("ZONE_CHANGED_NEW_AREA", function() Z.Update() end)
ns.On("PLAYER_DEAD", function()
  if ns.rec then ns.Journal.Add("death", { zone = ns.rec.zone }) end
end)
