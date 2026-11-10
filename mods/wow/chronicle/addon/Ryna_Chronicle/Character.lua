-- Character: the character record (docs/wow.md section 2, `characters[]`), keyed by GUID.
-- Identity, level, xp, gold, played time, gear, talents, professions (with known recipes),
-- reputations and the guild membership. Bags and bank are Containers.lua, quests Quests.lua,
-- the zone Zone.lua, kills and loot Chronicle.lua.

local _, ns = ...
if ns.off then return end

local C = {}
ns.Character = C
C.version = 0 -- every change to the record after login (the Characters section of the window)

local function changed()
  C.version = C.version + 1
  if ns.Window and ns.Window.Changed then ns.Window.Changed() end
end

local NonEmpty = ns.NonEmpty
local GENDER = { [2] = "male", [3] = "female" }

-- Classic professions by English name, for clients without GetProfessions (skill-line ids).
local PROFESSION_IDS = {
  Alchemy = 171, Blacksmithing = 164, Enchanting = 333, Engineering = 202, Herbalism = 182,
  Leatherworking = 165, Mining = 186, Skinning = 393, Tailoring = 197,
  Cooking = 185, ["First Aid"] = 129, Fishing = 356,
}

function C.ParseGuid(guid)
  if type(guid) ~= "string" then return nil, nil end
  local realmId, hex = guid:match("^Player%-(%d+)%-(%x+)$")
  return tonumber(realmId), hex
end

-- "First-Last" splits on the first hyphen; a name with no hyphen is all first name.
function C.SplitName(fullName)
  if type(fullName) ~= "string" then return nil, nil end
  local first, last = fullName:match("^([^%-]+)%-(.+)$")
  if first then return first, last end
  return fullName, nil
end

local function squash(s)
  return (s:gsub("[%s%-']", ""):lower())
end

-- The player's names. fullName is what the client returns for the name; when UnitFullName hands
-- back a second part that isn't the realm (Forever may return the last name there), it is
-- joined with a hyphen, so the record always reads "First-Last".
function C.Names(unit)
  unit = unit or "player"
  local unitName = NonEmpty(ns.Try(UnitName, unit))
  local fullFirst, fullSecond = ns.Try(ns.Fn("UnitFullName"), unit)
  fullFirst, fullSecond = NonEmpty(fullFirst), NonEmpty(fullSecond)
  local realm = NonEmpty(ns.Try(ns.Fn("GetNormalizedRealmName")))
  local realmName = NonEmpty(ns.Try(ns.Fn("GetRealmName")))

  local base = fullFirst or unitName
  local fullName = base
  if base and fullSecond and not base:find("-", 1, true) then
    local isRealm = (realm and squash(fullSecond) == squash(realm))
      or (realmName and squash(fullSecond) == squash(realmName))
    if not isRealm then fullName = base .. "-" .. fullSecond end
  end
  local firstName, lastName = C.SplitName(fullName)
  return {
    fullName = fullName, firstName = firstName, lastName = lastName,
    realm = realm or (realmName and realmName:gsub("[%s%-]", "")) or nil,
    -- the raw parts, for ns.RecordClient
    unitName = unitName, unitFullName = fullFirst, unitFullNameRealm = fullSecond, realmName = realmName,
  }
end

function C.Current()
  return ns.rec
end

-- What the client reports about itself and the player's name parts, for checking the TOC's
-- Interface number and how Forever splits "First-Last" (RynaDB.client, overwritten every login).
function ns.RecordClient(nameParts)
  local version, build, buildDate, interface = ns.Try(ns.Fn("GetBuildInfo"))
  ns.db.client = {
    version = version, build = build, date = buildDate, interface = interface,
    unitName = nameParts and nameParts.unitName,
    unitFullName = nameParts and nameParts.unitFullName,
    unitFullNameRealm = nameParts and nameParts.unitFullNameRealm,
    realm = nameParts and nameParts.realm,
    realmName = nameParts and nameParts.realmName,
  }
end

-- Builds this session's record. Identity and the cheap numbers are read now; lists the record
-- had from a restored RynaDB are kept only until their scans (deferred) replace them.
function C.Login()
  local guid = UnitGUID("player")
  local prev = ns.db.characters[guid]
  if type(prev) ~= "table" then prev = {} end
  local names = C.Names("player")
  ns.RecordClient(names)

  local rec = {
    guid = guid,
    firstName = names.firstName,
    lastName = names.lastName,
    fullName = names.fullName,
    realm = names.realm,
    realmId = C.ParseGuid(guid),
    class = select(2, ns.Try(UnitClass, "player")),
    race = select(2, ns.Try(UnitRace, "player")),
    faction = NonEmpty(ns.Try(UnitFactionGroup, "player")),
    gender = GENDER[ns.Try(UnitSex, "player")],
    level = ns.Try(UnitLevel, "player"),
    xp = ns.Try(UnitXP, "player"),
    xpMax = ns.Try(UnitXPMax, "player"),
    gold = ns.Try(GetMoney),
    playedTotal = nil, -- from TIME_PLAYED_MSG; absent until it arrives
    playedLevel = nil,
    zone = NonEmpty(ns.Try(ns.Fn("GetRealZoneText"))),
    subzone = NonEmpty(ns.Try(ns.Fn("GetSubZoneText"))),
    lastSeen = ns.Now(),
    guild = nil,
    gear = type(prev.gear) == "table" and prev.gear or {},
    talents = prev.talents,
    professions = type(prev.professions) == "table" and prev.professions or {},
    reputations = type(prev.reputations) == "table" and prev.reputations or {},
    quests = type(prev.quests) == "table" and prev.quests or { active = {}, completed = {} },
    containers = type(prev.containers) == "table" and prev.containers or {},
    bankAsOf = prev.bankAsOf,
    -- Chronicle (Chronicle.lua): lifetime tallies, string-keyed so they're never read as arrays.
    kills = type(prev.kills) == "table" and prev.kills or {},
    loot = type(prev.loot) == "table" and prev.loot or {},
    objects = type(prev.objects) == "table" and prev.objects or {},
    -- 7.6: when zones were first counted (the agent's combat-log backfill reads it), the agent's
    -- record of that backfill, and the lifetime by zone (Places.lua) with when it starts.
    zonesSince = prev.zonesSince,
    zonesBackfill = type(prev.zonesBackfill) == "table" and prev.zonesBackfill or nil,
    places = type(prev.places) == "table" and prev.places or {},
    placesSince = prev.placesSince,
  }
  if type(rec.quests.active) ~= "table" then rec.quests.active = {} end
  if type(rec.quests.completed) ~= "table" then rec.quests.completed = {} end

  -- Known recipes are only readable while a profession window is open, so keep what we had.
  C.recipes = {}
  for _, p in ipairs(rec.professions) do
    if type(p) == "table" and p.name and type(p.recipes) == "table" then C.recipes[p.name] = p.recipes end
  end

  ns.db.characters[guid] = rec
  ns.rec = rec
  ns.playerGuid = guid
  C.goldAnchor = rec.gold
  C.UpdateGuild()
  return rec
end

function C.Touch()
  if ns.rec then ns.rec.lastSeen = ns.Now() end
end

-- Guild membership on the record (the roster itself is Guild.lua).
function C.UpdateGuild()
  local rec = ns.rec
  if not rec then return end
  local inGuild = ns.Try(ns.Fn("IsInGuild"))
  if not inGuild then
    rec.guild = nil
    return
  end
  local name, rankName, rankIndex = ns.Try(ns.Fn("GetGuildInfo"), "player")
  name = NonEmpty(name)
  if not name then return end -- not loaded yet; PLAYER_GUILD_UPDATE comes back
  rec.guild = { guid = ns.Guild.Key(name), name = name, rankIndex = rankIndex, rankName = NonEmpty(rankName) }
  changed()
end

-- Gear: slots 0 (ammo) to 19 (tabard).
function C.ScanGear()
  local rec = ns.rec
  if not rec then return end
  local gear = {}
  for slot = 0, 19 do
    local link = NonEmpty(ns.Try(GetInventoryItemLink, "player", slot))
    if link then
      local itemId = ns.Try(ns.Fn("GetInventoryItemID"), "player", slot) or tonumber(link:match("item:(%d+)"))
      local enchant = tonumber(link:match("item:%-?%d+:(%-?%d*)") or "") or 0
      gear[#gear + 1] = {
        slot = slot, itemId = itemId, link = link,
        quality = ns.Try(ns.Fn("GetInventoryItemQuality"), "player", slot),
        enchant = enchant,
      }
    end
  end
  rec.gear = gear
  changed()
  ns.Debug("gear: %d items equipped", #gear)
end

-- Talents: "Elemental 5 / Enhancement 11 / Restoration 0", one "<tree> <points>" per tree.
--
-- Forever (checked in game 2026-11-08): GetNumTalentTabs and GetTalentTabInfo are nil. Its talent
-- frame is the trait system: the class tree's trait groups are the three trees, and each group's
-- points are C_Traits.GetGroupCurrencyInfo's currencyInfos[1].spent, which is what Blizzard's own
-- Forever tree headers show (Blizzard_PlayerSpells/Camelot/ClassTalents/Blizzard_ClassTalentsFrame.lua,
-- docs/forever-client-api.md "Talents and specs"). Trees come in the order the client lists them.
local function traitTalentParts()
  local configId = ns.Try(ns.Fn("C_ClassTalents", "GetActiveConfigID"))
  local getConfig = ns.Fn("C_Traits", "GetConfigInfo")
  local getGroups = ns.Fn("C_Traits", "GetGroupDisplayInfoByTreeID")
  local getCurrency = ns.Fn("C_Traits", "GetGroupCurrencyInfo")
  if type(configId) ~= "number" or not (getConfig and getGroups and getCurrency) then return nil end
  local config = ns.Try(getConfig, configId)
  local treeId = type(config) == "table" and type(config.treeIDs) == "table" and config.treeIDs[1]
  if type(treeId) ~= "number" then return nil end
  local groups = ns.Try(getGroups, treeId)
  if type(groups) ~= "table" or #groups == 0 then return nil end
  local ids = {}
  for _, group in ipairs(groups) do
    if type(group) == "table" and type(group.groupID) == "number" then ids[#ids + 1] = group.groupID end
  end
  local spent = {}
  local infos = ns.Try(getCurrency, configId, ids)
  if type(infos) == "table" then
    for _, info in ipairs(infos) do
      local currency = type(info) == "table" and type(info.currencyInfos) == "table" and info.currencyInfos[1]
      if type(currency) == "table" and info.traitNodeGroupID ~= nil then
        spent[info.traitNodeGroupID] = currency.spent
      end
    end
  end
  local parts = {}
  for _, group in ipairs(groups) do
    local name = type(group) == "table" and group.displayName
    local points = type(group) == "table" and spent[group.groupID]
    if type(name) == "string" and not ns.IsSecret(name) and not ns.IsSecret(points) then
      parts[#parts + 1] = name .. " " .. tostring(tonumber(points) or 0)
    end
  end
  return #parts > 0 and parts or nil
end

-- Classic clients keep GetTalentTabInfo, whose returns differ by client generation (name first, or
-- an id first), so both shapes are read.
local function tabTalentParts()
  local numTabs = ns.Try(ns.Fn("GetNumTalentTabs"))
  local info = ns.Fn("GetTalentTabInfo")
  if type(numTabs) ~= "number" or numTabs < 1 or not info then return nil end
  local parts = {}
  for tab = 1, numTabs do
    local r1, r2, r3, _, r5 = ns.Try(info, tab)
    local name, points
    if type(r1) == "string" then -- name, icon, pointsSpent, fileName
      name, points = r1, r3
    elseif type(r1) == "number" then -- id, name, description, icon, pointsSpent, ...
      name, points = r2, r5
    end
    if type(name) == "string" then
      parts[#parts + 1] = name .. " " .. tostring(tonumber(points) or 0)
    end
  end
  return #parts > 0 and parts or nil
end

function C.ScanTalents()
  local rec = ns.rec
  if not rec then return end
  local parts = traitTalentParts() or tabTalentParts()
  if parts then rec.talents = table.concat(parts, " / ") end
  changed()
  ns.Debug("talents: %s", parts and rec.talents or "none readable")
end

local function professionRow(id, name, rank, max)
  return { id = id, name = name, rank = rank, max = max, recipes = C.recipes and C.recipes[name] or nil }
end

-- Professions: GetProfessions/GetProfessionInfo where the client has them (ids included),
-- otherwise the classic skill list, where the id comes from the English name. Forever has
-- GetProfessions and no global GetSkillLineInfo (docs/forever-client-api.md), so it takes the first.
function C.ScanProfessions()
  local rec = ns.rec
  if not rec then return end
  local rows = {}
  local getProfessions, getInfo = ns.Fn("GetProfessions"), ns.Fn("GetProfessionInfo")
  if getProfessions and getInfo then
    local indexes = { ns.Try(getProfessions) }
    for i = 1, 6 do
      local index = indexes[i]
      if index then
        local name, _, rank, max, _, _, skillLine = ns.Try(getInfo, index)
        name = NonEmpty(name)
        if name then rows[#rows + 1] = professionRow(skillLine or PROFESSION_IDS[name], name, rank, max) end
      end
    end
  end
  if #rows == 0 then
    local numLines = ns.Try(ns.Fn("GetNumSkillLines"))
    local lineInfo = ns.Fn("GetSkillLineInfo")
    if type(numLines) == "number" and lineInfo then
      local tradeHeaders = {
        [_G.TRADE_SKILLS or "Professions"] = true,
        [_G.SECONDARY_SKILLS or "Secondary Skills"] = true,
      }
      local header
      for i = 1, numLines do
        local name, isHeader, _, rank, _, _, max = ns.Try(lineInfo, i)
        name = NonEmpty(name)
        if name then
          if isHeader then
            header = name
          elseif PROFESSION_IDS[name] or (header and tradeHeaders[header]) then
            rows[#rows + 1] = professionRow(PROFESSION_IDS[name], name, rank, max)
          end
        end
      end
    end
  end
  if #rows > 0 then rec.professions = rows end
  changed()
  if ns.Debugging() then
    local names = {}
    for _, p in ipairs(rows) do
      names[#names + 1] = string.format("%s %s/%s", p.name, tostring(p.rank), tostring(p.max))
    end
    -- Weapon skill-ups also fire SKILL_LINES_CHANGED, so an unchanged line isn't repeated.
    ns.DebugChanged("professions", "professions: %d%s", #rows, #rows > 0 and (" (" .. ns.ListText(names) .. ")") or "")
  end
end

local function sortedIds(set)
  local list = {}
  for id in pairs(set) do list[#list + 1] = id end
  table.sort(list)
  return list
end

local function recipeIdFromLink(link)
  if type(link) ~= "string" then return nil end
  return tonumber(link:match("enchant:(%d+)")) or tonumber(link:match("spell:(%d+)"))
end

-- Known recipes of the open profession window (TRADE_SKILL_* or the classic CRAFT_* window).
function C.ScanRecipes(window)
  if not ns.rec then return end
  local name
  local ids = {}
  local TS = _G.C_TradeSkillUI
  if window == "trade" and type(TS) == "table" and TS.GetAllRecipeIDs and TS.GetRecipeInfo then
    local all = ns.Try(TS.GetAllRecipeIDs)
    if type(all) == "table" then
      for _, id in ipairs(all) do
        local info = ns.Try(TS.GetRecipeInfo, id)
        if type(info) == "table" and info.learned then ids[id] = true end
      end
    end
    local base = TS.GetBaseProfessionInfo and ns.Try(TS.GetBaseProfessionInfo)
    name = type(base) == "table" and NonEmpty(base.professionName) or nil
  end
  if window == "trade" and not next(ids) then
    name = name or NonEmpty(ns.Try(ns.Fn("GetTradeSkillLine")))
    local n = ns.Try(ns.Fn("GetNumTradeSkills"))
    local infoFn, linkFn = ns.Fn("GetTradeSkillInfo"), ns.Fn("GetTradeSkillRecipeLink")
    if type(n) == "number" and linkFn then
      for i = 1, n do
        local _, kind = ns.Try(infoFn, i)
        if kind ~= "header" and kind ~= "subheader" then
          local id = recipeIdFromLink(ns.Try(linkFn, i))
          if id then ids[id] = true end
        end
      end
    end
  elseif window == "craft" then
    name = NonEmpty(ns.Try(ns.Fn("GetCraftDisplaySkillLine")))
    local n = ns.Try(ns.Fn("GetNumCrafts"))
    local infoFn = ns.Fn("GetCraftInfo")
    local linkFn = ns.Fn("GetCraftRecipeLink") or ns.Fn("GetCraftItemLink")
    if type(n) == "number" and linkFn then
      for i = 1, n do
        local _, _, kind = ns.Try(infoFn, i)
        if kind ~= "header" then
          local id = recipeIdFromLink(ns.Try(linkFn, i))
          if id then ids[id] = true end
        end
      end
    end
  end
  if not name or not next(ids) then
    ns.DebugChanged("recipes-" .. tostring(window), "recipes: nothing readable in the %s window yet", tostring(window))
    return
  end
  C.recipes[name] = sortedIds(ids)
  ns.DebugChanged("recipes-" .. tostring(window), "recipes: %d known for %s", #C.recipes[name], name)
  for _, p in ipairs(ns.rec.professions) do
    if p.name == name then p.recipes = C.recipes[name] end
  end
end

-- Reputations: C_Reputation where the client has it, otherwise GetFactionInfo. value and max are
-- within the current standing (3000 of 6000 into Friendly). Collapsed headers hide their
-- factions; the addon doesn't expand them, so what's listed is what the reputation pane shows.
function C.ScanReputations()
  local rec = ns.rec
  if not rec then return end
  local rows = {}
  local R = _G.C_Reputation
  if type(R) == "table" and R.GetNumFactions and R.GetFactionDataByIndex then
    local n = ns.Try(R.GetNumFactions)
    for i = 1, (type(n) == "number" and n or 0) do
      local d = ns.Try(R.GetFactionDataByIndex, i)
      if type(d) == "table" and (not d.isHeader or d.isHeaderWithRep) and d.factionID then
        local low = d.currentReactionThreshold or 0
        rows[#rows + 1] = {
          factionId = d.factionID, name = NonEmpty(d.name), standing = d.reaction,
          value = d.currentStanding and (d.currentStanding - low) or nil,
          max = d.nextReactionThreshold and (d.nextReactionThreshold - low) or nil,
        }
      end
    end
  end
  if #rows == 0 then
    local n = ns.Try(ns.Fn("GetNumFactions"))
    local infoFn = ns.Fn("GetFactionInfo")
    for i = 1, (type(n) == "number" and infoFn and n or 0) do
      local name, _, standing, barMin, barMax, barValue, _, _, isHeader, _, hasRep, _, _, factionId =
        ns.Try(infoFn, i)
      if NonEmpty(name) and (not isHeader or hasRep) then
        rows[#rows + 1] = {
          factionId = factionId, name = name, standing = standing,
          value = (barValue and barMin) and (barValue - barMin) or nil,
          max = (barMax and barMin) and (barMax - barMin) or nil,
        }
      end
    end
  end
  if #rows > 0 then rec.reputations = rows end
  changed()
  -- UPDATE_FACTION comes with every reputation gain, so an unchanged line isn't repeated.
  ns.DebugChanged("reputations", "reputations: %d factions", #rows)
end

function C.UpdateXp()
  local rec = ns.rec
  if not rec then return end
  rec.xp = ns.Try(UnitXP, "player")
  rec.xpMax = ns.Try(UnitXPMax, "player")
  rec.level = ns.Try(UnitLevel, "player") or rec.level
end

-- Gold: the record always has the current amount; a `gold` event is written once the change
-- since the last gold event (or login) reaches 1 gold either way.
local GOLD_EVENT_COPPER = 10000
function C.UpdateMoney()
  local rec = ns.rec
  if not rec then return end
  local money = ns.Try(GetMoney)
  if type(money) ~= "number" then return end
  rec.gold = money
  local anchor = C.goldAnchor or money
  local delta = money - anchor
  if delta >= GOLD_EVENT_COPPER or delta <= -GOLD_EVENT_COPPER then
    ns.Journal.Add("gold", { delta = delta, total = money })
    C.goldAnchor = money
  elseif C.goldAnchor == nil then
    C.goldAnchor = money
  end
end

-- Played time. RequestTimePlayed answers with TIME_PLAYED_MSG, which the chat frames would also
-- print; for the addon's own request their registration is taken away and given back after.
local muted
local function mutePlayedChat()
  if muted then return end
  muted = {}
  for i = 1, (_G.NUM_CHAT_WINDOWS or 10) do
    local f = _G["ChatFrame" .. i]
    if f and f.IsEventRegistered and f:IsEventRegistered("TIME_PLAYED_MSG") then
      f:UnregisterEvent("TIME_PLAYED_MSG")
      muted[#muted + 1] = f
    end
  end
end

local function restorePlayedChat()
  local list = muted
  muted = nil
  if not list then return end
  for _, f in ipairs(list) do pcall(f.RegisterEvent, f, "TIME_PLAYED_MSG") end
end

C.playedToken = 0

local function writeLevelEvent()
  local rec = ns.rec
  local level = C.pendingLevel
  C.pendingLevel = nil
  if not level or not rec then return end
  ns.Journal.Add("level", {
    level = level, playedTotal = rec.playedTotal, playedLevel = rec.playedLevel, zone = rec.zone,
  })
end

function C.RequestPlayed()
  local request = ns.Fn("RequestTimePlayed")
  if not request then
    ns.Debug("played time: this client can't request it")
    return false
  end
  mutePlayedChat()
  if not pcall(request) then
    restorePlayedChat()
    ns.Debug("played time: the request failed")
    return false
  end
  C.playedToken = C.playedToken + 1
  local token = C.playedToken
  C.playedPending = true
  ns.Debug("played time: requested")
  ns.After(10, "played-timeout", function()
    if C.playedPending and token == C.playedToken then
      C.playedPending = false
      ns.Debug("played time: no answer after 10 seconds")
      restorePlayedChat()
      writeLevelEvent() -- without played totals: the answer never came
    end
  end)
  return true
end

function C.OnTimePlayed(total, atLevel)
  local rec = ns.rec
  if rec and type(total) == "number" then
    rec.playedTotal = total
    rec.playedLevel = atLevel
    C.playedAt = GetTime()
    changed()
    ns.Debug("played time: received, %s in all, %s at this level", ns.DurationText(total), ns.DurationText(atLevel))
  end
  if C.playedPending then
    C.playedPending = false
    -- Give the chat frames their event back after this dispatch, so this one isn't printed.
    ns.After(0, "played-restore", restorePlayedChat)
  end
  writeLevelEvent()
end

function C.OnLevelUp(level)
  local rec = ns.rec
  if not rec then return end
  rec.level = tonumber(level) or rec.level
  C.pendingLevel = rec.level
  if not C.RequestPlayed() then writeLevelEvent() end
end

-- Played totals advanced by the time since the last answer, for the logout record.
function C.PlayedNow()
  local rec = ns.rec
  if not rec or type(rec.playedTotal) ~= "number" or not C.playedAt then return nil, nil end
  local elapsed = math.floor(GetTime() - C.playedAt)
  return rec.playedTotal + elapsed, (rec.playedLevel or 0) + elapsed
end

function C.Logout()
  local rec = ns.rec
  if not rec then return end
  local total, atLevel = C.PlayedNow()
  if total then
    rec.playedTotal, rec.playedLevel = total, atLevel
    C.playedAt = GetTime()
  end
  rec.gold = ns.Try(GetMoney) or rec.gold
  rec.lastSeen = ns.Now()
  ns.Journal.Add("session", { kind = "logout", clock = time(), playedTotal = rec.playedTotal })
  ns.Debug("logout: record updated (played %s, gold %s); the game writes RynaDB now",
    ns.DurationText(rec.playedTotal), ns.MoneyText(rec.gold))
end

ns.On("PLAYER_MONEY", function() C.UpdateMoney() end)
ns.On("PLAYER_XP_UPDATE", function() C.UpdateXp() end)
ns.On("PLAYER_LEVEL_UP", function(_, level) C.OnLevelUp(level) end)
ns.On("TIME_PLAYED_MSG", function(_, total, atLevel) C.OnTimePlayed(total, atLevel) end)
ns.On("PLAYER_GUILD_UPDATE", function() C.UpdateGuild() end)
ns.On("PLAYER_EQUIPMENT_CHANGED", function() ns.Debounce("gear", 1, C.ScanGear) end)
ns.On("SKILL_LINES_CHANGED", function() ns.Debounce("professions", 2, C.ScanProfessions) end)
ns.On("UPDATE_FACTION", function() ns.Debounce("reputations", 2, C.ScanReputations) end)
-- Talents change through the trait system on Forever (spending, a spec swap, the config loading
-- after login) and through CHARACTER_POINTS_CHANGED on classic clients. A client without one of
-- these events just skips it (ns.On).
for _, event in ipairs({ "CHARACTER_POINTS_CHANGED", "TRAIT_CONFIG_UPDATED", "TRAIT_CONFIG_LIST_UPDATED",
  "PLAYER_TALENT_UPDATE", "ACTIVE_TALENT_GROUP_CHANGED" }) do
  ns.On(event, function() ns.Debounce("talents", 1, C.ScanTalents) end)
end
ns.On("TRADE_SKILL_SHOW", function() ns.Debounce("recipes-trade", 1, function() C.ScanRecipes("trade") end) end)
ns.On("TRADE_SKILL_UPDATE", function() ns.Debounce("recipes-trade", 1, function() C.ScanRecipes("trade") end) end)
ns.On("CRAFT_SHOW", function() ns.Debounce("recipes-craft", 1, function() C.ScanRecipes("craft") end) end)
ns.On("CRAFT_UPDATE", function() ns.Debounce("recipes-craft", 1, function() C.ScanRecipes("craft") end) end)
