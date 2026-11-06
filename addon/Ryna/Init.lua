-- Init: the login sequence. Loaded last, so every module's handlers exist.
--
-- PLAYER_LOGIN does only the cheap part (identity, level, gold, the session event). The heavier
-- scans (gear, professions, reputations, talents, bags, quests, the guild roster) and the played
-- time request are deferred a few seconds past the first PLAYER_ENTERING_WORLD.

local addonName, ns = ...

ns.On("ADDON_LOADED", function(_, name)
  if name == addonName then ns.InitDB() end
end)

ns.On("PLAYER_LOGIN", function()
  if not ns.db then ns.InitDB() end
  ns.Journal.Prune()
  local rec = ns.Character.Login()
  ns.Journal.StartSession(rec.guid)
  ns.Journal.Add("session", { kind = "login", clock = time(), zone = rec.zone })
  ns.PrintRaw("|cff7fb2ffRyna " .. ns.VERSION .. "|r: recording. /ryna for settings.")
  ns.Debug("login: record built for %s, level %s %s, in %s", rec.fullName, rec.level, rec.class, rec.zone)
  if ns.Status.IsStale() then ns.Print("the companion app hasn't synced recently.") end
  ns.Debug("companion app file: %s", ns.Status.DebugLine())
  ns.Run("advanced combat logging", ns.Zone.ApplyAdvancedLogging)
end)

-- The UI is a second PLAYER_LOGIN handler, after the one above, with each part under its own
-- pcall: a client without the Settings API or a minimap still records, and a recording error
-- above doesn't take the panel with it.
ns.On("PLAYER_LOGIN", function()
  if not ns.db then ns.InitDB() end
  ns.Run("panel", ns.Panel.Register)
  ns.Run("minimap", ns.Minimap.Create)
end)

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

ns.On("PLAYER_LOGOUT", function()
  ns.Character.Logout()
end)
