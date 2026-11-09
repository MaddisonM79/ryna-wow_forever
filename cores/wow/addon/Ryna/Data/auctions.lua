-- Data/auctions.lua: written by the Ryna companion app after a sync, not by hand (Data.lua says how
-- it's read). Everyone's auction house prices per item id, one row each: { today's agreed min,
-- median, listings, snapshots, observers, asOf (unix); the week's median, days, snapshots } (0: none).
-- As shipped: nothing synced yet.
RynaData_auctions = nil
