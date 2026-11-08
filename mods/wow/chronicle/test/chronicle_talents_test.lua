-- Chronicle's talent scan (Character.lua, C.ScanTalents) against a fake client (no game needed):
-- loads Chronicle's namespace (Mod.lua), its strings and Character.lua with a stand-in for the core's
-- mod API, then scans talents on a Forever-shaped client (trait groups, no GetTalentTabInfo) and on a
-- classic-shaped one (GetTalentTabInfo, both return shapes). From the repository root, with Lua 5.1
-- (what the game runs) or later:
--
--   lua5.1 mods/wow/chronicle/test/chronicle_talents_test.lua
--
-- This proves the logic; that Forever's trait API returns these shapes is from Blizzard's Forever UI
-- source and docs/forever-client-api.md, and the game has the final say.
-- luacheck: ignore 1

local ADDON = "mods/wow/chronicle/addon/Ryna_Chronicle/"

local failures, checks = 0, 0
local function check(cond, what)
  checks = checks + 1
  if not cond then
    failures = failures + 1
    print("FAIL: " .. what)
  end
end

local errors, chat = {}, {}
local function run(label, fn, ...)
  local ok, err = pcall(fn, ...)
  if not ok then errors[#errors + 1] = label .. ": " .. tostring(err) end
end

local SECRET = setmetatable({}, { __tostring = function() return "secret" end })

local function passResults(ok, ...)
  if ok then return ... end
  return nil
end
local function try(fn, ...)
  if type(fn) ~= "function" then return nil end
  return passResults(pcall(fn, ...))
end
local function fn(a, b)
  local v = _G[a]
  if b ~= nil then
    if type(v) ~= "table" then return nil end
    v = v[b]
  end
  return type(v) == "function" and v or nil
end

local handlers, debounced = {}, {}
_G.GetLocale = function() return "enUS" end
_G.DEFAULT_CHAT_FRAME = { AddMessage = function(_, msg) chat[#chat + 1] = msg end }
_G.Ryna = {
  API = 1, VERSION = "test", L = setmetatable({}, { __index = function(_, k) return k end }),
  Try = try, Fn = fn,
  IsSecret = function(v) return v == SECRET end,
  NonEmpty = function(s) if type(s) == "string" and s ~= "" then return s end end,
  Locale = function() return "enUS" end,
  Now = function() return 1800000000 end,
  Print = function(m) chat[#chat + 1] = "Ryna: " .. m end,
  Debug = function() end,
  Run = run,
  Debounce = function(key, _, f) debounced[key] = f end,
  ReportError = function(label, err) errors[#errors + 1] = label .. ": " .. tostring(err) end,
  On = function(event, f)
    handlers[event] = handlers[event] or {}
    table.insert(handlers[event], f)
    return true
  end,
  DB = function() return {} end,
  RegisterMod = function() return true end,
  Journal = { Add = function() end },
  Settings = { Get = function() return nil end, Set = function() end, AddDefaults = function() end },
}

local ns = {}
for _, file in ipairs({ "Mod.lua", "Locales/enUS.lua", "Character.lua" }) do
  local chunk = assert(loadfile(ADDON .. file))
  chunk("Ryna_Chronicle", ns)
end
check(not ns.off and ns.Character and type(ns.Character.ScanTalents) == "function", "Character.lua loads")

-- A Forever-shaped client: the class tree's trait groups are the three trees, points per group in
-- C_Traits.GetGroupCurrencyInfo. `trees` = { { groupID, name, spent } } in the client's order.
local function forever(trees, opts)
  opts = opts or {}
  _G.GetNumTalentTabs, _G.GetTalentTabInfo = nil, nil
  _G.C_ClassTalents = { GetActiveConfigID = function() return opts.configId ~= false and 4954925 or nil end }
  _G.C_Traits = {
    GetConfigInfo = function(configId)
      if configId == 4954925 then return { ID = configId, treeIDs = { 777 } } end
    end,
    GetGroupDisplayInfoByTreeID = function(treeId)
      if treeId ~= 777 then return {} end
      local out = {}
      for i, t in ipairs(trees) do
        out[i] = { groupID = t[1], treeID = 777, orderIndex = i, displayName = t[2], icon = 1 }
      end
      return out
    end,
    GetGroupCurrencyInfo = function(configId, groupIds)
      if configId ~= 4954925 then return {} end
      local out = {}
      for _, id in ipairs(groupIds) do
        for _, t in ipairs(trees) do
          if t[1] == id and t[3] ~= nil then
            local currency = { traitCurrencyID = 1, quantity = 0, maxQuantity = 51, spent = t[3], spentInTree = 99 }
            out[#out + 1] = { traitNodeGroupID = id, currencyInfos = { currency } }
          end
        end
      end
      return out
    end,
  }
end

-- A classic-shaped client: GetTalentTabInfo, name first (`shape` "name") or an id first ("id").
local function classic(trees, shape)
  _G.C_ClassTalents, _G.C_Traits = nil, nil
  _G.GetNumTalentTabs = function() return #trees end
  _G.GetTalentTabInfo = function(tab)
    local t = trees[tab]
    if shape == "id" then return 100 + tab, t[1], "description", 1, t[2] end
    return t[1], 1, t[2], "File"
  end
end

local function scan(rec)
  ns.rec = rec
  ns.Character.ScanTalents()
  return rec.talents
end

-- Forever. ---------------------------------------------------------------------------------------
forever({ { 11, "Assassination", 5 }, { 12, "Combat", 31 }, { 13, "Subtlety", 0 } })
check(scan({}) == "Assassination 5 / Combat 31 / Subtlety 0",
  "Forever: one '<tree> <points>' per trait group, in the client's order, points from `spent`")

forever({ { 21, "Discipline", 0 }, { 22, "Holy", 0 }, { 23, "Shadow", 0 } })
check(scan({}) == "Discipline 0 / Holy 0 / Shadow 0", "Forever, no points yet (a low level): all zeros")

forever({ { 31, "Arms", 3 }, { 32, "Fury", nil }, { 33, "Protection", 1 } })
check(scan({}) == "Arms 3 / Fury 0 / Protection 1", "Forever: a group missing from the currency info counts 0")

forever({ { 41, "Balance", 2 }, { 42, SECRET, 4 }, { 43, "Restoration", SECRET } })
check(scan({}) == "Balance 2", "Forever: a secret name or secret points are left out, never compared")

forever({ { 51, "Fire", 1 } }, { configId = false })
check(scan({ talents = "Fire 0" }) == "Fire 0",
  "Forever before the trait config loads (no config id): the last record stays")

ns.rec = nil
forever({ { 11, "Assassination", 5 } })
run("no record", ns.Character.ScanTalents)
check(#errors == 0, "no character record yet: nothing happens")

-- Classic clients. -------------------------------------------------------------------------------
classic({ { "Elemental", 5 }, { "Enhancement", 11 }, { "Restoration", 0 } }, "name")
check(scan({}) == "Elemental 5 / Enhancement 11 / Restoration 0", "classic: GetTalentTabInfo, name first")

classic({ { "Arcane", 0 }, { "Fire", 21 }, { "Frost", 30 } }, "id")
check(scan({}) == "Arcane 0 / Fire 21 / Frost 30", "classic: GetTalentTabInfo, an id first")

_G.C_ClassTalents, _G.C_Traits, _G.GetNumTalentTabs, _G.GetTalentTabInfo = nil, nil, nil, nil
check(scan({ talents = "Holy 1" }) == "Holy 1", "a client with neither API: the last record stays")

-- When it scans. ---------------------------------------------------------------------------------
for _, event in ipairs({ "CHARACTER_POINTS_CHANGED", "TRAIT_CONFIG_UPDATED", "TRAIT_CONFIG_LIST_UPDATED",
  "PLAYER_TALENT_UPDATE", "ACTIVE_TALENT_GROUP_CHANGED" }) do
  debounced.talents = nil
  for _, f in ipairs(handlers[event] or {}) do run(event, f, event) end
  check(debounced.talents == ns.Character.ScanTalents, event .. " schedules a talent scan")
end

check(#chat == 0, "nothing in chat")
check(#errors == 0, "no handler raised: " .. table.concat(errors, "; "))
print(string.format("chronicle talents: %d checks, %d failed", checks, failures))
os.exit(failures == 0 and 0 or 1)
