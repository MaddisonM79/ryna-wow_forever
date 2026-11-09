# Ryna agent: notes for Claude

This repo is the on-machine half of Ryna for World of Warcraft: Forever. It holds the game addon
(a core plus mods), the CLI and the Tauri desktop app. The hosted API and portal are private and
live elsewhere. `README.md` covers installing; `docs/ryna-agent-cores-mods.md` covers the design.

## Layout

- `cores/wow/addon/Ryna`: the core addon (Lua 5.1), with its README for the mod API.
- `cores/wow/core`: the WoW core in Rust (parsing, the combat log, install discovery).
- `mods/wow/<id>/`: mods (`automation`, `chronicle`, `prices`, `settings`), each with `addon/`,
  `mod.json` and `test/*_test.lua`.
- `agent/cli`, `agent/host`: Rust crates. `agent/app`: the desktop app (Vue 3, pnpm, vitest, Tauri 2).
- `scripts/`: release helpers, the version check and the CLI smoke test.

## Before writing addon code

Read `docs/forever-client-api.md` first. Forever is Blizzard's own branch: the retail 12.1.5 engine
with Classic rules. Many Classic globals are nil there (`GetItemInfo`, `GetMerchantItemInfo`,
`GetTalentTabInfo`, `GetSkillLineInfo`), CLEU is refused to addons, and secret values are on. If a
probe in game contradicts the doc, fix the doc in the same change.

- Every client call goes through `ns.Try` / `ns.Fn`, with the modern API first and the classic one
  as a guarded fallback. Keep the fallbacks; the tests cover both shapes.
- Pass game values through `ns.IsSecret` before comparing them.
- Register optional events through `ns.On`, which skips events a client lacks.

## Checks (what CI runs)

Addon (`.github/workflows/addon.yml`, Lua 5.1):

```bash
luacheck cores/wow/addon/ mods/wow/ --config cores/wow/addon/.luacheckrc --no-color
```

```bash
node cores/wow/addon/tools/strings.mjs
```

```bash
for t in mods/wow/*/test/*_test.lua; do lua5.1 "$t" || exit 1; done
```

CI also parses every addon file with `luac5.1 -p`. This Mac may only have a newer `lua`; say so
when tests ran on it instead of 5.1.

Agent (`.github/workflows/agent.yml`): `cargo fmt --all --check`, `cargo clippy --all-targets
--locked -- -D warnings`, `cargo test --locked`, `node --test scripts/release.test.mjs` and
`node scripts/check-versions.mjs`. In `agent/app`: `pnpm typecheck` and `pnpm test`.

## Rules for this repo

- **Seen in game, richer on the portal.** Every feature that records, pools or does something has
  a whole view in the game (a section of the window, a tooltip line, the strip), and the addon
  never says "see ryna.io". A feature isn't done until that view exists; a design names it first.
  The portal is the more robust form of the same thing. `docs/ryna-agent-cores-mods.md` 7.5 has
  the rule, the audit of what is owed and the order. UI gets a mockup Maddison agrees to first.
- **The game install is read only.** Never write under `/Applications/World of Warcraft/`, and
  never run the game. Maddison runs the in-game probes.
- **Don't open `WTF/Account/*/SavedVariables`** or other personal files in the install.
- **Installed third-party addons are evidence, never a source.** Note their licence, and never
  copy their code.
- **Never record anything about another person** beyond what the docs already allow: nothing about
  sellers or bidders, and per-listing auction rows never leave the client.
- **A new translatable string** goes in its addon's `Locales/enUS.lua`. The strings check fails
  otherwise.
- **Versions must agree** across the TOCs, `mod.json` files, `core.json`, the app's
  `package.json`, Cargo and the tag. A release is a `v*` tag; see `release.yml` and
  `agent/README.md` ("Releasing").
- **Commit subjects** start with the area: `chronicle: ...`, `prices mod: ...`, `core: ...`,
  `app: ...`, `docs: ...`. Push straight to main unless the work came in on a PR branch.

<!-- global: v2, reviewed 2026-11-08 -->
