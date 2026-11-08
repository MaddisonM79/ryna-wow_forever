-- Trainer: at a trainer, learn everything available under a gold cap. A toggle (autoTrainAll) and the
-- cap (autoTrainCap, in copper; 10 gold until it's set with /ryna auto traincap <gold>, and 0 means
-- train nothing), off by default.
--
-- A frame after TRAINER_SHOW, while the trainer's window is up, it buys the available services in
-- the trainer's order, one every 0.2 seconds, while the total stays within the cap and the player's
-- gold. The list is read again before every purchase, since indices move as services are learnt, and
-- nothing is bought twice in a visit. One chat line at the end: what was learnt and what it cost, and
-- how many were left.
--
-- Forever's trainer (checked in game 2026-11-07): GetNumTrainerServices() answers 0 to addons while the
-- window lists its spells, but GetTrainerServiceInfo(i), GetTrainerServiceCost(i) and
-- BuyTrainerService(i) work by index. So the list is walked by index until the first empty one (the
-- count, when the client gives one, only raises the floor), and the type filter isn't touched.

local _, ns = ...
if ns.off then return end
local L = ns.L
local S = ns.Settings

local T = {}
ns.Trainer = T

local INTERVAL = 0.2
-- How far to walk by index when the client's count is 0 or short (Forever's is 0).
T.WALK_LIMIT = 200
local visit, open, started = 0, false, nil

local function trainerOpen(v)
  return open and v == visit and ns.Shown("ClassTrainerFrame")
end

local function money()
  return tonumber((ns.G("GetMoney"))) or 0
end

local KINDS = { available = true, unavailable = true, used = true }

-- A service's name, sub text ("Rank 2") and kind ("available", "unavailable", "used"), whichever way
-- this client orders GetTrainerServiceInfo's returns: the classic order is (name, subText, kind,
-- texture); Forever's (checked in game 2026-11-07) is (name, kind, texture, requiredLevel, subText).
function T.ServiceInfo(i)
  local a, b, c, _, e = ns.G("GetTrainerServiceInfo", i)
  if type(b) == "string" and KINDS[b] then return a, e, b end
  return a, b, c
end

-- The first available service not yet bought this visit that fits: its index, label and cost; and
-- how many available ones don't fit.
function T.Next(bought, total, cap, startMoney)
  local pick, label, price, left = nil, nil, nil, 0
  local count = tonumber((ns.G("GetNumTrainerServices"))) or 0
  for i = 1, math.max(count, T.WALK_LIMIT) do
    local name, subText, kind = T.ServiceInfo(i)
    -- Past the count the client admits to, the first index with no service is the end of the list.
    if not ns.NonEmpty(name) and i > count then break end
    if kind == "available" and ns.NonEmpty(name) then
      local text = ns.NonEmpty(subText) and string.format("%s (%s)", name, subText) or name
      if not bought[text] then
        local cost = tonumber((ns.G("GetTrainerServiceCost", i))) or 0
        if not pick and total + cost <= cap and total + cost <= startMoney and cost <= money() then
          pick, label, price = i, text, cost
        else
          left = left + 1
        end
      end
    end
  end
  return pick, label, price, left
end

local function finish(learnt, total, left, cap)
  if #learnt > 0 then
    ns.Report(string.format(L["Learnt %s for %s."], table.concat(learnt, ", "), ns.MoneyText(total)))
  end
  if left > 0 then
    ns.Report(string.format(left == 1 and L["%d more is over your %s cap or your gold."]
      or L["%d more are over your %s cap or your gold."], left, ns.MoneyText(cap)))
  end
end

local function step(v, cap, startMoney, bought, learnt, total)
  if not trainerOpen(v) or ns.InCombat() then return finish(learnt, total, 0, cap) end
  local i, label, cost, left = T.Next(bought, total, cap, startMoney)
  if not i then return finish(learnt, total, left, cap) end
  bought[label] = true
  ns.G("BuyTrainerService", i)
  learnt[#learnt + 1] = label
  total = total + cost
  ns.After(INTERVAL, "automation train", function() step(v, cap, startMoney, bought, learnt, total) end)
end

ns.On("TRAINER_SHOW", function()
  -- A trainer that shows its window again (a re-fired event) is the same visit.
  if not open then visit = visit + 1 end
  open = true
  if not S.On("autoTrainAll") then return end
  local cap = S.TrainCap()
  if cap <= 0 then return ns.Debug("automation: trainer: the cap is 0, so nothing") end
  for _, name in ipairs({ "GetNumTrainerServices", "GetTrainerServiceInfo", "BuyTrainerService" }) do
    if not ns.Need("trainAll", name) then return end
  end
  ns.Soon("trainer", function() return trainerOpen(visit) end, function()
    local v = visit
    if started == v then return end
    started = v
    step(v, cap, money(), {}, {}, 0)
  end)
end)

ns.On("TRAINER_CLOSED", function()
  open = false
  visit = visit + 1
end)
