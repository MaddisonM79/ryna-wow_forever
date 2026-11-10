-- Data: the data layer's reader (docs/ryna-agent-cores-mods.md section 7). The companion app writes
-- versioned tables under Interface/AddOns/Ryna/Data/ (one file each, loaded by the TOC at login and
-- /reload), each as one global `RynaData_<name>` shaped
--
--   { version = 1, writtenAt = 1762464000, flavor = "wowf", items = { [itemId] = ... } }
--
-- and a mod reads one through Ryna.Data.Get(name, version): the table when it is there and carries
-- exactly the version the mod knows, else nil and why ("absent": no file or no table; "version":
-- another version, so the mod or the app is behind). The files ship as stubs (the global nil), so a
-- fresh install reads as absent. Everything is read defensively: the table may be anything.

local _, ns = ...

local D = {}
ns.Data = D

-- The global a table is read from: RynaData_vendors, RynaData_auctions.
function D.Global(name)
  return "RynaData_" .. tostring(name)
end

-- The table `name`, when it is one and its version is `version`; else nil and "absent" or "version".
function D.Get(name, version)
  local t = _G[D.Global(name)]
  if type(t) ~= "table" or type(t.items) ~= "table" then return nil, "absent" end
  if type(t.version) ~= "number" or t.version ~= version then return nil, "version" end
  return t
end

-- When a table was written (unix seconds), or nil.
function D.WrittenAt(t)
  if type(t) ~= "table" or type(t.writtenAt) ~= "number" then return nil end
  return t.writtenAt
end

-- How many items a table holds (counted once, on first ask).
function D.Count(t)
  if type(t) ~= "table" or type(t.items) ~= "table" then return 0 end
  if type(t.count) == "number" then return t.count end
  local n = 0
  for _ in pairs(t.items) do n = n + 1 end
  t.count = n
  return n
end
