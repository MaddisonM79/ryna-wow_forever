-- Containers: bags and keyring always; bank containers only while the bank is open. Bank rows
-- are kept between visits (from this session, or from a restored RynaDB) with `bankAsOf`, the
-- time they were read; closing the bank clears nothing.

local _, ns = ...
if ns.off then return end

local B = {}
ns.Containers = B
B.version = 0 -- every scan (Items.lua rebuilds its index when it moves)

local NonEmpty = ns.NonEmpty

local function numSlots(bag)
  local CC = _G.C_Container
  local n
  if type(CC) == "table" and CC.GetContainerNumSlots then
    n = ns.Try(CC.GetContainerNumSlots, bag)
  else
    n = ns.Try(ns.Fn("GetContainerNumSlots"), bag)
  end
  return type(n) == "number" and n or 0
end

-- itemId, count, link, quality of one slot, or nil when it's empty.
local function slotInfo(bag, slot)
  local CC = _G.C_Container
  if type(CC) == "table" and CC.GetContainerItemInfo then
    local ok, info = pcall(CC.GetContainerItemInfo, bag, slot)
    if ok then
      if type(info) ~= "table" then return nil end
      local link = NonEmpty(info.hyperlink)
      return info.itemID or (link and tonumber(link:match("item:(%d+)"))), info.stackCount, link, info.quality
    end
  end
  local legacy = ns.Fn("GetContainerItemInfo")
  if legacy then
    local _, count, _, quality, _, _, link, _, _, itemId = ns.Try(legacy, bag, slot)
    link = NonEmpty(link)
    if link then return itemId or tonumber(link:match("item:(%d+)")), count, link, quality end
  end
  return nil
end

local function scan(kind, bags, out)
  for _, bag in ipairs(bags) do
    for slot = 1, numSlots(bag) do
      local itemId, count, link, quality = slotInfo(bag, slot)
      if itemId or link then
        out[#out + 1] = {
          container = kind, bag = bag, slot = slot, itemId = itemId, count = count or 1,
          link = link, quality = quality,
        }
      end
    end
  end
end

local function bagCount()
  return _G.NUM_TOTAL_EQUIPPED_BAG_SLOTS or _G.NUM_BAG_SLOTS or 4
end

local function carriedBags()
  local bags = {}
  for bag = 0, bagCount() do bags[#bags + 1] = bag end
  return bags
end

local function bankBags()
  local seen, bags = {}, {}
  local function add(bag)
    if type(bag) == "number" and not seen[bag] then
      seen[bag] = true
      bags[#bags + 1] = bag
    end
  end
  add(_G.BANK_CONTAINER or -1)
  local first = bagCount() + 1
  for bag = first, first + (_G.NUM_BANKBAGSLOTS or 6) - 1 do add(bag) end
  -- Newer clients number bank tabs separately; harmless where they don't exist.
  local index = type(_G.Enum) == "table" and _G.Enum.BagIndex
  if type(index) == "table" then
    for i = 1, 6 do add(index["CharacterBankTab_" .. i]) end
  end
  return bags
end

function B.Scan()
  local rec = ns.rec
  if not rec then return end
  local rows = {}
  scan("bag", carriedBags(), rows)
  scan("keyring", { _G.KEYRING_CONTAINER or -2 }, rows)
  if B.bankOpen then
    scan("bank", bankBags(), rows)
    rec.bankAsOf = ns.Now()
  else
    for _, row in ipairs(rec.containers or {}) do
      if type(row) == "table" and row.container == "bank" then rows[#rows + 1] = row end
    end
  end
  rec.containers = rows
  B.version = B.version + 1
  if ns.Window and ns.Window.Changed then ns.Window.Changed() end
  if ns.Debugging() then
    local items, slots = { bag = 0, keyring = 0, bank = 0 }, { bag = 0, keyring = 0, bank = 0 }
    for _, row in ipairs(rows) do
      local kind = row.container
      if items[kind] then
        items[kind] = items[kind] + (tonumber(row.count) or 1)
        slots[kind] = slots[kind] + 1
      end
    end
    local bank
    if B.bankOpen then
      bank = "read now"
    elseif rec.bankAsOf then
      bank = "kept from " .. date("%Y-%m-%d %H:%M", rec.bankAsOf)
    else
      bank = "never read (open your bank once)"
    end
    ns.Debug("bags: %d items in %d slots%s; bank: %d items in %d slots, %s", items.bag + items.keyring,
      slots.bag + slots.keyring, slots.keyring > 0 and " (keyring included)" or "", items.bank, slots.bank, bank)
  end
end

ns.On("BAG_UPDATE_DELAYED", function() ns.Debounce("containers", 1, B.Scan) end)
ns.On("BANKFRAME_OPENED", function()
  B.bankOpen = true
  ns.Debounce("containers", 0.5, B.Scan)
end)
ns.On("BANKFRAME_CLOSED", function() B.bankOpen = false end)
ns.On("PLAYERBANKSLOTS_CHANGED", function()
  if B.bankOpen then ns.Debounce("containers", 1, B.Scan) end
end)
