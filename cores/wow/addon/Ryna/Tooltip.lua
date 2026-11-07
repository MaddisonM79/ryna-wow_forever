-- Tooltip: the player's own Chronicle numbers on creature and item tooltips (docs/wow.md section
-- 12.2), with the `tooltips` setting on. Through TooltipDataProcessor where the client has it,
-- GameTooltip's OnTooltipSetUnit / OnTooltipSetItem scripts otherwise. Only this character's
-- numbers; nothing here writes to the record.

local _, ns = ...
local L = ns.L

local T = {}
ns.Tooltip = T

local COLOR = "|cff999999"
local MAX_DROPS = 3

local function plain(s)
  return ns.Status.Plain(s) or s
end

local function recMap(name)
  local m = ns.rec and ns.rec[name]
  if type(m) == "table" then return m end
  return nil
end

local function num(v)
  if type(v) == "number" then return v end
  return tonumber(v) or 0
end

-- What a loot source key reads as: the creature's or item's name where the record has it.
function T.SourceName(key)
  if key == "f" then return L["fishing"] end
  if key == "i" then return L["a container"] end
  local kind, id = tostring(key):match("^(%a):(%d+)$")
  if kind == "c" then
    local kills = recMap("kills")
    local e = kills and kills[id]
    return (type(e) == "table" and ns.NonEmpty(e.name)) or string.format(L["creature %s"], id)
  elseif kind == "o" then
    return string.format(L["object %s"], id)
  elseif kind == "i" then
    local loot = recMap("loot")
    local e = loot and loot[id]
    local name = type(e) == "table" and ns.NonEmpty(e.name)
    if not name then name = ns.NonEmpty(ns.Try(ns.Fn("GetItemInfo"), tonumber(id))) end
    return name or string.format(L["item %s"], id)
  end
  return tostring(key)
end

-- The top drops of creature `id` from the record's loot, cached until loot changes.
local dropCache = { rec = nil, id = nil, version = nil, list = {} }

function T.Drops(id)
  local c = dropCache
  local version = ns.Chronicle.lootVersion or 0
  if c.rec == ns.rec and c.id == id and c.version == version then return c.list end
  c.rec, c.id, c.version = ns.rec, id, version
  local list = {}
  c.list = list
  local loot = recMap("loot")
  if not loot then return list end
  local key = "c:" .. id
  for _, e in pairs(loot) do
    local from = type(e) == "table" and e.from
    local n = type(from) == "table" and num(from[key]) or 0
    if n > 0 then
      local pos = #list + 1
      while pos > 1 and list[pos - 1].n < n do pos = pos - 1 end
      if pos <= MAX_DROPS then
        table.insert(list, pos, { name = ns.NonEmpty(e.name) or "?", n = n })
        list[MAX_DROPS + 1] = nil
      end
    end
  end
  return list
end

function T.UnitLines(guid)
  local id = ns.Chronicle.CreatureId(guid)
  if not id then return nil end
  local kills = recMap("kills")
  local e = kills and kills[id]
  if type(e) ~= "table" or num(e.n) <= 0 and num(e.looted) <= 0 then return nil end
  local lines = {}
  local ago = ns.Status.Ago(e.last) or "?"
  local zone = ns.NonEmpty(e.zone)
  if zone then
    lines[1] = string.format(L["Ryna: killed %d (%d yours), last %s %s"], num(e.n), num(e.own), plain(zone), ago)
  else
    lines[1] = string.format(L["Ryna: killed %d (%d yours), last %s"], num(e.n), num(e.own), ago)
  end
  local looted = num(e.looted)
  if looted > 0 then
    for _, d in ipairs(T.Drops(id)) do
      lines[#lines + 1] = string.format(L["%s: %d of %d corpses (%d%%)"], plain(d.name), d.n, looted,
        math.floor(d.n * 100 / looted + 0.5))
    end
  end
  return lines
end

function T.ItemLines(itemId)
  if itemId == nil then return nil end
  local loot = recMap("loot")
  local e = loot and loot[tostring(itemId)]
  if type(e) ~= "table" or num(e.n) <= 0 then return nil end
  local bestKey, best
  if type(e.from) == "table" then
    for k, v in pairs(e.from) do
      if not best or num(v) > best then bestKey, best = k, num(v) end
    end
  end
  local times = ns.Plural(num(e.n), L["%d time"], L["%d times"])
  if bestKey ~= nil then
    return { string.format(L["Ryna: looted %s (%d), mostly from %s (%d)"], times, num(e.qty),
      plain(T.SourceName(bestKey)), best) }
  end
  return { string.format(L["Ryna: looted %s (%d)"], times, num(e.qty)) }
end

local function addLines(tooltip, lines, show)
  if not lines or type(tooltip) ~= "table" or type(tooltip.AddLine) ~= "function" then return end
  for _, line in ipairs(lines) do tooltip:AddLine(COLOR .. line .. "|r") end
  if show and type(tooltip.Show) == "function" then tooltip:Show() end
end

local function usable(v)
  return v ~= nil and not ns.IsSecret(v)
end

-- The unit's GUID: the tooltip data's, else the tooltip's unit, else the mouseover.
local function unitGuid(tooltip, data)
  local guid = type(data) == "table" and data.guid
  if usable(guid) and type(guid) == "string" then return guid end
  local unit
  if type(tooltip) == "table" and type(tooltip.GetUnit) == "function" then
    unit = select(2, ns.Try(tooltip.GetUnit, tooltip))
  end
  if not usable(unit) then unit = "mouseover" end
  guid = ns.Try(UnitGUID, unit)
  if usable(guid) and type(guid) == "string" then return guid end
  return nil
end

local function itemId(tooltip, data)
  local id = type(data) == "table" and data.id
  if usable(id) and type(id) == "number" then return tostring(id) end
  if type(tooltip) == "table" and type(tooltip.GetItem) == "function" then
    local _, link = ns.Try(tooltip.GetItem, tooltip)
    if usable(link) then return ns.Chronicle.ItemIdFromLink(link) end
  end
  return nil
end

local function onUnit(tooltip, data, show)
  ns.Run("tooltip unit", function()
    if not ns.rec or not ns.Setting("tooltips") then return end
    local guid = unitGuid(tooltip, data)
    if guid then addLines(tooltip, T.UnitLines(guid), show) end
  end)
end

local function onItem(tooltip, data, show)
  ns.Run("tooltip item", function()
    if not ns.rec or not ns.Setting("tooltips") then return end
    addLines(tooltip, T.ItemLines(itemId(tooltip, data)), show)
  end)
end

-- Once, at login. Returns how the hooks went in: "processor", "scripts" or nil.
function T.Register()
  if T.mode then return T.mode end
  local enum = _G.Enum
  local types = type(enum) == "table" and enum.TooltipDataType
  local postCall = ns.Fn("TooltipDataProcessor", "AddTooltipPostCall")
  if postCall and type(types) == "table" and types.Unit ~= nil and types.Item ~= nil then
    local okUnit = ns.Try(function()
      postCall(types.Unit, function(tooltip, data) onUnit(tooltip, data, false) end)
      return true
    end)
    local okItem = ns.Try(function()
      postCall(types.Item, function(tooltip, data) onItem(tooltip, data, false) end)
      return true
    end)
    if okUnit or okItem then T.mode = "processor" end
  end
  if not T.mode then
    local hooked = false
    for _, tip in ipairs({ _G.GameTooltip, _G.ItemRefTooltip }) do
      if type(tip) == "table" and type(tip.HookScript) == "function" then
        if tip == _G.GameTooltip and ns.Try(function()
          tip:HookScript("OnTooltipSetUnit", function(self) onUnit(self, nil, true) end)
          return true
        end) then hooked = true end
        if ns.Try(function()
          tip:HookScript("OnTooltipSetItem", function(self) onItem(self, nil, true) end)
          return true
        end) then hooked = true end
      end
    end
    if hooked then T.mode = "scripts" end
  end
  ns.Debug("tooltips: %s", T.mode or "no hook on this client")
  return T.mode
end

ns.On("PLAYER_LOGIN", function()
  ns.Run("tooltips", T.Register)
end)
