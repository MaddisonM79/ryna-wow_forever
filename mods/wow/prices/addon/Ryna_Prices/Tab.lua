-- Tab: Prices' tab in the core's panel (Ryna.Panel.AddTab), after Automation and before Messages,
-- built with the core's widgets (Ryna.Panel.Widgets): the sell line's checkbox, and under it the
-- stack total and "not while a merchant window is open", greyed while the line is off; then the
-- auction line and its stack total, and the auction snapshot.

local _, ns = ...
if ns.off then return end
local L = ns.L
local S = ns.Settings

local W = ns.API.Panel.Widgets
local text, checkbox, toggle = W.text, W.checkbox, W.toggle

local P = {}

-- What each checkbox says when hovered, by setting.
local HELP = {
  priceSellLine = L["Adds the sell price to every item tooltip: in your bags and bank, in chat links, in loot "
    .. "and at merchants. From the game's own item data, so it works away from vendors. Items vendors won't "
    .. "buy get no line, and where the game shows its own price, this one stays away."],
  priceStackTotal = L["After the price, what the stack sells for, wherever the game says how many there are: "
    .. "bags, bank, loot, mail, trade, quest rewards and merchants. A chat link shows one item's price."],
  priceOnlyAwayFromMerchant = L["The line already stays away where the game shows its own price. This hides "
    .. "it for every item while a merchant window is open."],
  priceAuctionLine = L["Adds what the item goes for at the auction house: the median price each from your "
    .. "last snapshot, how many were listed and how long ago. From your own visits only. Not shown while "
    .. "the auction house window shows the item's price itself."],
  priceAuctionStackTotal = L["After the auction price, what the stack would go for, wherever the game says "
    .. "how many there are."],
  priceAuctionSnapshot = L["When you open the auction house, at most once every 15 minutes, reads the whole "
    .. "house over a few seconds and keeps each item's prices for the line above. The prices per item (never "
    .. "who posted them) go to your Ryna journal, so the companion app can pool them. Off: no snapshot and "
    .. "nothing sent; searches still update the line."],
}

local function build(f)
  P.intro = text(f, "GameFontHighlightSmall", 11)
  P.intro:SetText(L["Prices puts what a vendor pays, and what the auction house asks, on item tooltips. It "
    .. "only reads the game's item data and the auction house you open: it does nothing for you and says "
    .. "nothing in chat."])
  P.boxes = {}
  for _, feature in ipairs(S.FEATURES) do
    local cb = checkbox(f, feature.label, HELP[feature.key])
    toggle(cb, feature.key, function() ns.API.Panel.RefreshIfShown() end)
    P.boxes[#P.boxes + 1] = { cb = cb, key = feature.key, under = feature.under }
  end
end

local function layout(s, settings)
  s.text(P.intro, 0, 12)
  for _, b in ipairs(P.boxes) do
    b.cb:SetChecked(settings[b.key] and true or false)
    if b.under then
      b.cb.setActive(settings[b.under] and true or false)
      b.cb.place(s, 30)
    else
      b.cb.place(s, 4)
    end
  end
end

ns.API.Panel.AddTab({ key = "prices", label = L["Prices"], order = 38, build = build, layout = layout })
