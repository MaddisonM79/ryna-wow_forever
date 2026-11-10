-- Chronicle beside a core from before Ryna's window (0.3.9 and earlier: no Ryna.Window, no
-- Ryna.Tooltip), against a fake client: Chronicle loads and records as before, registers no section,
-- builds its Recording tab without the window's button, keeps its item tooltip lines (with the line
-- naming the other characters that hold an item), and /chronicle or the strip's click says once, when
-- asked, that the core is too old. Nothing in chat unasked, no error. From the repository root:
--
--   lua5.1 mods/wow/chronicle/test/window_failsoft_test.lua
-- luacheck: ignore 1

local CORE = "cores/wow/addon/Ryna/"
local CHRON = "mods/wow/chronicle/addon/Ryna_Chronicle/"
local F = dofile("mods/wow/chronicle/test/fake_client.lua")

local failures, checks = 0, 0
local function check(cond, what)
  checks = checks + 1
  if not cond then
    failures = failures + 1
    print("FAIL: " .. what)
  end
end

F.Install()
local ME, ALT = "Player-91-0000000A", "Player-91-0000000B"
_G.UnitGUID = function(unit) if unit == "player" then return ME end end
_G.UnitName = function(unit) if unit == "player" then return "Brennic" end end
_G.UnitLevel = function() return 3 end
_G.GetMoney = function() return 0 end
_G.GetInventoryItemLink = function() return nil end
_G.RynaDB = { schema = 1, settings = {}, journal = {}, characters = {
  [ALT] = { guid = ALT, fullName = "Alyra-Dawnmere", firstName = "Alyra", containers = {
    { container = "bank", itemId = 2589, count = 120, link = "|cnIQ1:|Hitem:2589::|h[Linen Cloth]|h|r" } } },
} }

-- The core as it is, then without its window: what Chronicle sees beside an older core.
local core = {}
F.LoadAddon(CORE, "Ryna", core)
F.Fire("ADDON_LOADED", "Ryna")
_G.Ryna.Window, _G.Ryna.Tooltip = nil, nil

local ns = {}
F.LoadAddon(CHRON, "Ryna_Chronicle", ns)
F.Fire("PLAYER_LOGIN")
F.Fire("PLAYER_ENTERING_WORLD", true, false)
F.DropTimers()

check(not ns.off, "Chronicle loads")
check(ns.Window and ns.Window.available == false, "it knows the window isn't there")
check(next(ns.Window.keys) == nil, "no section registered")
check(ns.Characters == nil, "Characters stays out")
check(ns.Items and ns.Items.TooltipLine, "the Items index still loads (the tooltip line needs no window)")
check(#core.Window.List() == 0, "the core's window has no section of Chronicle's")
check(#F.chat == 0, "nothing in chat at login")

-- The tooltip line, without the core's guard.
GameTooltip:ClearLines()
GameTooltip:SetHyperlink("|cnIQ1:|Hitem:2589::|h[Linen Cloth]|h|r")
local found = false
for _, l in ipairs(GameTooltip.lines) do
  if F.Plain(l.left) == "Alyra: 120 in the bank" then found = true end
end
check(found, "the holders line, without Ryna.Tooltip")

-- Asked: one line each time, and nothing breaks.
SlashCmdList.RYNACHRONICLE("")
check(#F.chat == 1 and F.chat[1]:find("too old for its window", 1, true) ~= nil, "/chronicle says the core is too old")
F.Click(ns.Strip.frame, "LeftButton")
check(#F.chat == 2, "the strip's click says so too, and only when clicked")
check(core.Window.frame == nil, "no window was made")
check(core.errors.count == 0, "no errors caught: " .. tostring(core.errors.last))
print(string.format("window fail-soft: %d checks, %d failed", checks, failures))
os.exit(failures == 0 and 0 or 1)
