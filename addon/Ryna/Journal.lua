-- Journal: the append-only event table, docs/wow.md section 2 "journal".
-- Every event is { session, seq, at, char, type, ...fields }. A session is
-- "<login unix>-<GUID hex>", and seq restarts at 1 per session.

local _, ns = ...

local J = {}
ns.Journal = J

local KEEP_SECONDS = 30 * 86400
local CAP = 100000 -- launch week: the addon may run days before the agent first uploads
local SLACK = 1000 -- compact only when this far over the cap, so appends stay cheap

local function keepNewest(list, n)
  if #list <= n then return list end
  local out = {}
  for i = #list - n + 1, #list do out[#out + 1] = list[i] end
  return out
end

-- On login: drop events older than 30 days (and anything malformed), then apply the cap.
function J.Prune()
  local cutoff = ns.Now() - KEEP_SECONDS
  local out = {}
  local old = ns.db.journal
  for i = 1, #old do
    local e = old[i]
    if type(e) == "table" and type(e.at) == "number" and e.at >= cutoff then out[#out + 1] = e end
  end
  ns.db.journal = keepNewest(out, CAP)
end

function J.StartSession(guid)
  local hex = (type(guid) == "string" and guid:match("%-(%x+)$")) or "0"
  J.loginAt = ns.Now()
  J.session = string.format("%d-%s", J.loginAt, hex)
  J.seq = 0
  J.char = guid
end

function J.Add(eventType, fields)
  if not J.session then return nil end
  J.seq = J.seq + 1
  local e = { session = J.session, seq = J.seq, at = ns.Now(), char = J.char, type = eventType }
  if fields then
    for k, v in pairs(fields) do e[k] = v end
  end
  local journal = ns.db.journal
  journal[#journal + 1] = e
  if #journal > CAP + SLACK then ns.db.journal = keepNewest(journal, CAP) end
  return e
end

function J.Count()
  return ns.db and #ns.db.journal or 0
end
