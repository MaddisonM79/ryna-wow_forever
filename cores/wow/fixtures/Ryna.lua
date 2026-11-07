
-- A scrubbed, invented RynaDB (docs/wow.md section 1) in the client's SavedVariables
-- style. Every name, GUID and guild here is fake.
RynaDB = {
	["schema"] = 1,
	["addon"] = "0.1.0",
	["flavor"] = "wowf",
	["settings"] = {
		["combatLog"] = "instances",
	},
	["client"] = {
		["version"] = "1.60.0",
		["build"] = "12345",
		["date"] = "Oct  1 2026",
		["interface"] = 11600,
		["unitName"] = "Aerin-Stormcall",
		["unitFullName"] = "Aerin-Stormcall",
		["unitFullNameRealm"] = "Testrealm",
		["realm"] = "Testrealm",
		["realmName"] = "Testrealm",
	},
	["characters"] = {
		["Player-91-0F00A001"] = {
			["guid"] = "Player-91-0F00A001",
			["firstName"] = "Aerin",
			["lastName"] = "Stormcall",
			["fullName"] = "Aerin-Stormcall",
			["realm"] = "Testrealm",
			["realmId"] = 91,
			["class"] = "SHAMAN",
			["race"] = "Orc",
			["faction"] = "Horde",
			["gender"] = "female",
			["level"] = 24,
			["xp"] = 12345,
			["xpMax"] = 20000,
			["gold"] = 1222567,
			["playedTotal"] = 95000,
			["playedLevel"] = 4200,
			["zone"] = "The Barrens",
			["subzone"] = "The Crossroads",
			["lastSeen"] = 1762409000,
			["guild"] = {
				["guid"] = "Ember Wardens-91",
				["name"] = "Ember Wardens",
				["rankIndex"] = 3,
				["rankName"] = "Member",
			},
			["gear"] = {
				{
					["slot"] = 1,
					["itemId"] = 4724,
					["link"] = "|cff1eff00|Hitem:4724::::::::24:::::|h[Humbert's Helm]|h|r",
					["quality"] = 2,
					["enchant"] = 0,
				}, -- [1]
				{
					["slot"] = 5,
					["itemId"] = 6087,
					["link"] = "|cff1eff00|Hitem:6087::::::::24:::::|h[Chausses of Westfall]|h|r",
					["quality"] = 2,
					["enchant"] = 843,
				}, -- [2]
				{
					["slot"] = 7,
					["itemId"] = 6085,
					["link"] = "|cffffffff|Hitem:6085::::::::24:::::|h[Footpad's Pants]|h|r",
					["quality"] = 1,
					["enchant"] = 0,
				}, -- [3]
				{
					["slot"] = 16,
					["itemId"] = 1159,
					["link"] = "|cff1eff00|Hitem:1159::::::::24:::::|h[Militia Quarterstaff]|h|r",
					["quality"] = 2,
					["enchant"] = 241,
				}, -- [4]
				{
					["slot"] = 17,
					["itemId"] = 6320,
					["link"] = "|cff0070dd|Hitem:6320::::::::24:::::|h[Commander's Crest]|h|r",
					["quality"] = 3,
					["enchant"] = 0,
				}, -- [5]
				{
					["slot"] = 18,
					["itemId"] = 5976,
					["link"] = "|cffffffff|Hitem:5976::::::::24:::::|h[Guild Tabard]|h|r",
					["quality"] = 1,
					["enchant"] = 0,
				}, -- [6]
			},
			["talents"] = "Elemental 11 / Enhancement 5 / Restoration 0",
			["professions"] = {
				{
					["id"] = 333,
					["name"] = "Enchanting",
					["rank"] = 120,
					["max"] = 150,
					["recipes"] = {
						7418, -- [1]
						7420, -- [2]
						7421, -- [3]
						7443, -- [4]
						7457, -- [5]
						7748, -- [6]
					},
				}, -- [1]
				{
					["id"] = 197,
					["name"] = "Tailoring",
					["rank"] = 95,
					["max"] = 150,
					["recipes"] = {
						2385, -- [1]
						2386, -- [2]
						2392, -- [3]
						2393, -- [4]
						3915, -- [5]
					},
				}, -- [2]
				{
					["id"] = 129,
					["name"] = "First Aid",
					["rank"] = 80,
					["max"] = 150,
				}, -- [3]
			},
			["reputations"] = {
				{
					["factionId"] = 76,
					["name"] = "Orgrimmar",
					["standing"] = 5,
					["value"] = 3000,
					["max"] = 6000,
				}, -- [1]
				{
					["factionId"] = 530,
					["name"] = "Darkspear Trolls",
					["standing"] = 4,
					["value"] = 1200,
					["max"] = 3000,
				}, -- [2]
				{
					["factionId"] = 470,
					["name"] = "Ratchet",
					["standing"] = 4,
					["value"] = 250,
					["max"] = 3000,
				}, -- [3]
			},
			["quests"] = {
				["active"] = {
					{
						["id"] = 850,
						["title"] = "Kolkar Leaders",
						["level"] = 22,
						["acceptedAt"] = 1762402000,
						["objectives"] = {
							{
								["text"] = "Barak Kodobane's Head: 0/1",
								["done"] = false,
							}, -- [1]
						},
					}, -- [1]
					{
						["id"] = 855,
						["title"] = "Centaur Bracers",
						["level"] = 23,
						["acceptedAt"] = 1762403500,
						["objectives"] = {
							{
								["text"] = "Centaur Bracers: 7/15",
								["done"] = false,
							}, -- [1]
						},
					}, -- [2]
					{
						["id"] = 6541,
						["title"] = "Report to Kadrak",
						["level"] = 24,
						["acceptedAt"] = 1762405000,
						["objectives"] = {
						},
					}, -- [3]
				},
				["completed"] = {
					840, -- [1]
					842, -- [2]
					844, -- [3]
					845, -- [4]
					848, -- [5]
					849, -- [6]
					869, -- [7]
					870, -- [8]
					871, -- [9]
					872, -- [10]
					903, -- [11]
					6539, -- [12]
				},
			},
			["containers"] = {
				{
					["container"] = "bag",
					["bag"] = 0,
					["slot"] = 1,
					["itemId"] = 6948,
					["count"] = 1,
					["link"] = "|cffffffff|Hitem:6948::::::::24:::::|h[Hearthstone]|h|r",
					["quality"] = 1,
				}, -- [1]
				{
					["container"] = "bag",
					["bag"] = 0,
					["slot"] = 2,
					["itemId"] = 2589,
					["count"] = 20,
					["link"] = "|cffffffff|Hitem:2589::::::::24:::::|h[Linen Cloth]|h|r",
					["quality"] = 1,
				}, -- [2]
				{
					["container"] = "bag",
					["bag"] = 0,
					["slot"] = 3,
					["itemId"] = 2592,
					["count"] = 7,
					["link"] = "|cffffffff|Hitem:2592::::::::24:::::|h[Wool Cloth]|h|r",
					["quality"] = 1,
				}, -- [3]
				{
					["container"] = "bag",
					["bag"] = 0,
					["slot"] = 4,
					["itemId"] = 10940,
					["count"] = 14,
					["link"] = "|cffffffff|Hitem:10940::::::::24:::::|h[Strange Dust]|h|r",
					["quality"] = 1,
				}, -- [4]
				{
					["container"] = "bag",
					["bag"] = 1,
					["slot"] = 1,
					["itemId"] = 5075,
					["count"] = 3,
					["link"] = "|cff9d9d9d|Hitem:5075::::::::24:::::|h[Blood Shard]|h|r",
					["quality"] = 0,
				}, -- [5]
				{
					["container"] = "bag",
					["bag"] = 1,
					["slot"] = 2,
					["itemId"] = 1179,
					["count"] = 5,
					["link"] = "|cffffffff|Hitem:1179::::::::24:::::|h[Ice Cold Milk]|h|r",
					["quality"] = 1,
				}, -- [6]
				{
					["container"] = "bag",
					["bag"] = 1,
					["slot"] = 6,
					["itemId"] = 858,
					["count"] = 4,
					["link"] = "|cffffffff|Hitem:858::::::::24:::::|h[Lesser Healing Potion]|h|r",
					["quality"] = 1,
				}, -- [7]
				{
					["container"] = "keyring",
					["bag"] = -2,
					["slot"] = 1,
					["itemId"] = 5396,
					["count"] = 1,
					["link"] = "|cffffffff|Hitem:5396::::::::24:::::|h[Key to Searing Gorge]|h|r",
					["quality"] = 1,
				}, -- [8]
				{
					["container"] = "bank",
					["bag"] = -1,
					["slot"] = 1,
					["itemId"] = 10938,
					["count"] = 6,
					["link"] = "|cff1eff00|Hitem:10938::::::::24:::::|h[Lesser Magic Essence]|h|r",
					["quality"] = 2,
				}, -- [9]
				{
					["container"] = "bank",
					["bag"] = -1,
					["slot"] = 2,
					["itemId"] = 2996,
					["count"] = 12,
					["link"] = "|cffffffff|Hitem:2996::::::::24:::::|h[Bolt of Linen Cloth]|h|r",
					["quality"] = 1,
				}, -- [10]
				{
					["container"] = "bank",
					["bag"] = 5,
					["slot"] = 1,
					["itemId"] = 4306,
					["count"] = 9,
					["link"] = "|cffffffff|Hitem:4306::::::::24:::::|h[Silk Cloth]|h|r",
					["quality"] = 1,
				}, -- [11]
			},
			["bankAsOf"] = 1762404800,
		},
		["Player-91-0F00A002"] = {
			["guid"] = "Player-91-0F00A002",
			["firstName"] = "Thorbrand",
			["lastName"] = "Ironhew",
			["fullName"] = "Thorbrand-Ironhew",
			["realm"] = "Testrealm",
			["realmId"] = 91,
			["class"] = "WARRIOR",
			["race"] = "Tauren",
			["faction"] = "Horde",
			["gender"] = "male",
			["level"] = 9,
			["xp"] = 1800,
			["xpMax"] = 4500,
			["gold"] = 4210,
			["zone"] = "Mulgore",
			["subzone"] = "Bloodhoof Village",
			["lastSeen"] = 1762452400,
			["gear"] = {
				{
					["slot"] = 16,
					["itemId"] = 2361,
					["link"] = "|cffffffff|Hitem:2361::::::::9:::::|h[Battleworn Hammer]|h|r",
					["quality"] = 1,
					["enchant"] = 0,
				}, -- [1]
			},
			["professions"] = {
			},
			["reputations"] = {
				{
					["factionId"] = 81,
					["name"] = "Thunder Bluff",
					["standing"] = 4,
					["value"] = 600,
					["max"] = 3000,
				}, -- [1]
			},
			["quests"] = {
				["active"] = {
					{
						["id"] = 757,
						["title"] = "Rite of Strength",
						["level"] = 8,
						["acceptedAt"] = 1762450600,
						["objectives"] = {
							{
								["text"] = "Bristleback Quilboar slain: 4/8",
								["done"] = false,
							}, -- [1]
							{
								["text"] = "Rite completed",
								["done"] = true,
							}, -- [2]
						},
					}, -- [1]
				},
				["completed"] = {
				},
			},
			["containers"] = {
				{
					["container"] = "bag",
					["bag"] = 0,
					["slot"] = 1,
					["itemId"] = 6948,
					["count"] = 1,
					["link"] = "|cffffffff|Hitem:6948::::::::9:::::|h[Hearthstone]|h|r",
					["quality"] = 1,
				}, -- [1]
				{
					["container"] = "bag",
					["bag"] = 0,
					["slot"] = 2,
					["itemId"] = 4865,
					["count"] = 6,
					["link"] = "|cff9d9d9d|Hitem:4865::::::::9:::::|h[Ruined Pelt]|h|r",
					["quality"] = 0,
				}, -- [2]
			},
		},
	},
	["guilds"] = {
		["Ember Wardens-91"] = {
			["guid"] = "Ember Wardens-91",
			["name"] = "Ember Wardens",
			["realmId"] = 91,
			["rosterAsOf"] = 1762408000,
			["roster"] = {
				{
					["fullName"] = "Aerin-Stormcall",
					["guid"] = "Player-91-0F00A001",
					["level"] = 24,
					["class"] = "SHAMAN",
					["rankIndex"] = 3,
					["rankName"] = "Member",
					["zone"] = "The Barrens",
					["online"] = true,
				}, -- [1]
				{
					["fullName"] = "Kesra-Duskmantle",
					["guid"] = "Player-91-0F00B001",
					["level"] = 31,
					["class"] = "MAGE",
					["rankIndex"] = 1,
					["rankName"] = "Officer",
					["zone"] = "Orgrimmar",
					["note"] = "Raid lead \"Thursdays\"\nalt: Kesrin",
					["officerNote"] = "Bank access",
					["online"] = true,
				}, -- [2]
				{
					["fullName"] = "Brannoc-Ashvale",
					["guid"] = "Player-91-0F00B002",
					["level"] = 18,
					["class"] = "HUNTER",
					["rankIndex"] = 4,
					["rankName"] = "Initiate",
					["zone"] = "Stonetalon Mountains",
					["online"] = false,
					["lastOnlineAt"] = 1762310000,
				}, -- [3]
				{
					["fullName"] = "Moraine-Thistlewood",
					["level"] = 60,
					["class"] = "PRIEST",
					["rankIndex"] = 0,
					["rankName"] = "Guild Master",
					["online"] = false,
					["lastOnlineAt"] = 1761900000,
				}, -- [4]
			},
		},
	},
	["journal"] = {
		{
			["session"] = "1762400000-0F00A001",
			["seq"] = 1,
			["at"] = 1762400000,
			["char"] = "Player-91-0F00A001",
			["type"] = "session",
			["kind"] = "login",
			["clock"] = 1762400003,
			["zone"] = "Orgrimmar",
		}, -- [1]
		{
			["session"] = "1762400000-0F00A001",
			["seq"] = 2,
			["at"] = 1762401000,
			["char"] = "Player-91-0F00A001",
			["type"] = "zone",
			["zone"] = "The Barrens",
			["subzone"] = "The Crossroads",
		}, -- [2]
		{
			["session"] = "1762400000-0F00A001",
			["seq"] = 3,
			["at"] = 1762402000,
			["char"] = "Player-91-0F00A001",
			["type"] = "questAccept",
			["questId"] = 850,
			["title"] = "Kolkar Leaders",
			["level"] = 22,
			["zone"] = "The Barrens",
		}, -- [3]
		{
			["session"] = "1762400000-0F00A001",
			["seq"] = 4,
			["at"] = 1762403000,
			["char"] = "Player-91-0F00A001",
			["type"] = "questDone",
			["questId"] = 848,
			["xp"] = 1500,
			["gold"] = 2500,
			["zone"] = "The Barrens",
		}, -- [4]
		{
			["session"] = "1762400000-0F00A001",
			["seq"] = 5,
			["at"] = 1762404000,
			["char"] = "Player-91-0F00A001",
			["type"] = "level",
			["level"] = 24,
			["playedTotal"] = 90000,
			["playedLevel"] = 0,
			["zone"] = "The Barrens",
		}, -- [5]
		{
			["session"] = "1762400000-0F00A001",
			["seq"] = 6,
			["at"] = 1762404100,
			["char"] = "Player-91-0F00A001",
			["type"] = "death",
			["zone"] = "The Barrens",
		}, -- [6]
		{
			["session"] = "1762400000-0F00A001",
			["seq"] = 7,
			["at"] = 1762404200,
			["char"] = "Player-91-0F00A001",
			["type"] = "gold",
			["delta"] = -12000,
			["total"] = 1222567,
		}, -- [7]
		{
			["session"] = "1762400000-0F00A001",
			["seq"] = 8,
			["at"] = 1762404300,
			["char"] = "Player-91-0F00A001",
			["type"] = "group",
			["kind"] = "joined",
			["members"] = {
				"Player-91-0F00B001", -- [1]
				"Player-91-0F00B002", -- [2]
			},
		}, -- [8]
		{
			["session"] = "1762400000-0F00A001",
			["seq"] = 9,
			["at"] = 1762404300,
			["char"] = "Player-91-0F00A001",
			["type"] = "seen",
			["guid"] = "Player-91-0F00B001",
			["fullName"] = "Kesra-Duskmantle",
			["level"] = 31,
			["class"] = "MAGE",
			["race"] = "Troll",
			["guild"] = "Ember Wardens",
			["zone"] = "The Barrens",
			["source"] = "group",
		}, -- [9]
		{
			["session"] = "1762400000-0F00A001",
			["seq"] = 10,
			["at"] = 1762404300,
			["char"] = "Player-91-0F00A001",
			["type"] = "seen",
			["guid"] = "Player-91-0F00B002",
			["fullName"] = "Brannoc-Ashvale",
			["level"] = 18,
			["class"] = "HUNTER",
			["race"] = "Orc",
			["guild"] = "Ember Wardens",
			["zone"] = "The Barrens",
			["source"] = "group",
		}, -- [10]
		{
			["session"] = "1762400000-0F00A001",
			["seq"] = 11,
			["at"] = 1762404900,
			["char"] = "Player-91-0F00A001",
			["type"] = "seen",
			["guid"] = "Player-91-0F00B003",
			["fullName"] = "Sorvyn-Galecrest",
			["class"] = "ROGUE",
			["race"] = "Undead",
			["source"] = "chat",
		}, -- [11]
		{
			["session"] = "1762400000-0F00A001",
			["seq"] = 12,
			["at"] = 1762405000,
			["char"] = "Player-91-0F00A001",
			["type"] = "seen",
			["guid"] = "Player-91-0F00B001",
			["fullName"] = "Kesra-Duskmantle",
			["level"] = 31,
			["class"] = "MAGE",
			["zone"] = "Orgrimmar",
			["source"] = "roster",
		}, -- [12]
		{
			["session"] = "1762400000-0F00A001",
			["seq"] = 13,
			["at"] = 1762406000,
			["char"] = "Player-91-0F00A001",
			["type"] = "zone",
			["zone"] = "Wailing Caverns",
			["subzone"] = "",
			["instance"] = "party",
			["difficulty"] = 1,
		}, -- [13]
		{
			["session"] = "1762400000-0F00A001",
			["seq"] = 14,
			["at"] = 1762407000,
			["char"] = "Player-91-0F00A001",
			["type"] = "who",
			["query"] = "20-30",
			["results"] = 2,
		}, -- [14]
		{
			["session"] = "1762400000-0F00A001",
			["seq"] = 15,
			["at"] = 1762407000,
			["char"] = "Player-91-0F00A001",
			["type"] = "seen",
			["fullName"] = "Tavi-Emberleaf",
			["level"] = 27,
			["class"] = "DRUID",
			["race"] = "Tauren",
			["zone"] = "Wailing Caverns",
			["source"] = "who",
		}, -- [15]
		{
			["session"] = "1762400000-0F00A001",
			["seq"] = 16,
			["at"] = 1762407500,
			["char"] = "Player-91-0F00A001",
			["type"] = "group",
			["kind"] = "left",
			["members"] = {
			},
		}, -- [16]
		{
			["session"] = "1762400000-0F00A001",
			["seq"] = 17,
			["at"] = 1762408500,
			["char"] = "Player-91-0F00A001",
			["type"] = "mailbox",
			["future"] = {
				["nested"] = {
					1, -- [1]
					2, -- [2]
					3, -- [3]
				},
				["flag"] = true,
			},
		}, -- [17]
		{
			["session"] = "1762400000-0F00A001",
			["seq"] = 18,
			["at"] = 1762409000,
			["char"] = "Player-91-0F00A001",
			["type"] = "session",
			["kind"] = "logout",
			["clock"] = 1762409002,
			["playedTotal"] = 95000,
		}, -- [18]
		{
			["session"] = "1762450000-0F00A002",
			["seq"] = 1,
			["at"] = 1762450000,
			["char"] = "Player-91-0F00A002",
			["type"] = "session",
			["kind"] = "login",
			["clock"] = 1762450002,
			["zone"] = "Mulgore",
		}, -- [19]
		{
			["session"] = "1762450000-0F00A002",
			["seq"] = 2,
			["at"] = 1762450600,
			["char"] = "Player-91-0F00A002",
			["type"] = "questAccept",
			["questId"] = 757,
			["title"] = "Rite of Strength",
			["level"] = 8,
			["zone"] = "Mulgore",
		}, -- [20]
		{
			["session"] = "1762450000-0F00A002",
			["seq"] = 3,
			["at"] = 1762452400,
			["char"] = "Player-91-0F00A002",
			["type"] = "session",
			["kind"] = "logout",
			["clock"] = 1762452401,
		}, -- [21]
	},
}
