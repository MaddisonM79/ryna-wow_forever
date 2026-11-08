-- Tab: Prices' tab in the core's panel (Ryna.Panel.AddTab), after Automation and before Messages,
-- built with the core's widgets (Ryna.Panel.Widgets): the sell line's checkbox, and under it the
-- stack total and "not while a merchant window is open", greyed while the line is off.

local _, ns = ...
if ns.off then return end
local L = ns.L
local S = ns.Settings

local W = ns.API.Panel.Widgets
local text, checkbox, toggle = W.text, W.checkbox, W.toggle

local P = {}

-- What each checkbox says when hovered, by setting.
local HELP = {
  priceSellLine = L["Adds \"Sells for ...\" to every item tooltip: in your bags and bank, in chat links, in loot "
    .. "and at merchants. From the game's own item data, so it works away from vendors. Items vendors won't "
    .. "buy get no line."],
  priceStackTotal = L["After the price, what the stack sells for: the stack in that bag slot, or a full stack "
    .. "of an item you're not holding."],
  priceOnlyAwayFromMerchant = L["At a merchant the game shows its own sell price, so the line can stay away "
    .. "while the merchant window is open."],
}

local function build(f)
  P.intro = text(f, "GameFontHighlightSmall", 11)
  P.intro:SetText(L["Prices puts what a vendor pays on item tooltips. It only reads the game's item data: "
    .. "it does nothing for you and says nothing in chat."])
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
