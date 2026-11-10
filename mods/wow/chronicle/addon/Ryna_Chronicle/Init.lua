-- Init: Chronicle registers with the core (Ryna.RegisterMod). Loaded last, so every module's
-- handlers exist.
--
-- onLogin, which the core runs at PLAYER_LOGIN once RynaDB is set up and the journal session has
-- started, does only the cheap part (identity, level, gold, the session event). The heavier scans
-- (gear, professions, reputations, talents, bags, quests, the guild roster) and the played time
-- request are deferred a few seconds past the first PLAYER_ENTERING_WORLD.

local _, ns = ...
if ns.off then return end

local function onLogin()
  -- RynaDB.census held the retired /who sweep's state (its pace, mode and last summary); nothing
  -- reads it now, so it goes.
  if ns.db then ns.db.census = nil end
  local rec = ns.Character.Login()
  ns.Run("places seed", ns.Places.Seed, rec)
  ns.Journal.Add("session", { kind = "login", clock = time(), zone = rec.zone })
  ns.Debug("login: record built for %s, level %s %s, in %s", rec.fullName, rec.level, rec.class, rec.zone)
  ns.Run("nearby sightings", ns.Sightings.RegisterNearby)
  ns.Run("merchant fingerprints", ns.Merchants.Prune)
  ns.Session.Start(rec.guid)
  ns.Run("session strip", ns.Strip.Create)
end

ns.API.RegisterMod({
  id = "chronicle",
  addon = ns.NAME,
  name = "Chronicle",
  version = ns.VERSION,
  onLogin = onLogin,
  onLogout = function()
    ns.Places.Accrue()
    ns.Character.Logout()
    ns.Session.Save()
  end,
  -- The panel's Defaults: the strip shows and unlocks again, in the place it had.
  onDefaults = function() ns.Strip.OnDefaults() end,
})

local deferred = false
ns.On("PLAYER_ENTERING_WORLD", function()
  if not ns.rec then return end
  ns.Zone.Update()
  if deferred then return end
  deferred = true
  ns.Debug("login: scans start in 2 to 8 seconds")
  ns.After(2, "played", function() ns.Character.RequestPlayed() end)
  ns.After(3, "character-scan", function()
    local C = ns.Character
    C.ScanGear()
    C.ScanTalents()
    C.ScanProfessions()
    C.ScanReputations()
    C.UpdateGuild()
  end)
  ns.After(4, "containers-scan", ns.Containers.Scan)
  ns.After(5, "quests-scan", function()
    ns.Quests.ScanLog()
    ns.Quests.RefreshCompleted()
  end)
  ns.After(6, "group-scan", ns.Sightings.ScanGroup)
  ns.After(8, "roster-request", function()
    ns.Guild.Request()
    ns.Guild.StartTicker()
  end)
end)
