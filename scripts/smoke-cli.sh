#!/usr/bin/env bash
# smoke-cli.sh: run a built ryna-wow against the fixtures, the way a person's machine would.
#
#   scripts/smoke-cli.sh <path to ryna-wow or ryna-wow.exe>
#
# --version; parse agent/fixtures/Ryna.lua (stdout must be JSON); combat
# agent/fixtures/combat-world.txt (must find a fight); and discovery: a temporary
# `Program Files (x86)/World of Warcraft/_forever_` tree with a .flavor.info and one account's
# Ryna.lua, which `status --install <that>` must list. CI runs it on every OS (Windows is the
# case it's for). The sign-in provider is a made-up one, so the keychain lookup finds nothing
# and never touches a real sign-in; the state folder is temporary too.
set -euo pipefail

bin="${1:?usage: smoke-cli.sh <ryna-wow binary>}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d)"
# Git Bash on Windows: hand the native exe Windows paths (C:\...), not /tmp/...
native() { if command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else printf '%s' "$1"; fi; }
trap 'rm -rf "$tmp"' EXIT
export RYNA_WOW_STATE_DIR="$(native "$tmp/state")"
export RYNA_WOW_ISSUER="https://smoke.invalid"
unset RYNA_WOW_INSTALL

step() { printf '\n== %s\n' "$*"; }
fail() { printf '::error::%s\n' "$*" >&2; exit 1; }

step "version"
"$bin" --version | tee "$tmp/version.txt"
grep -Eq '^ryna-wow [0-9]+\.[0-9]+\.[0-9]+' "$tmp/version.txt" || fail "--version printed no version"

step "parse"
"$bin" parse "$(native "$root/agent/fixtures/Ryna.lua")" > "$tmp/parse.json"
node -e '
  const s = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
  if (!Array.isArray(s.characters) || s.characters.length === 0) throw new Error("no characters in the snapshot");
  console.log(`snapshot: schema ${s.schema}, ${s.characters.length} character(s)`);
' "$(native "$tmp/parse.json")" || fail "parse didn't print a snapshot as JSON"

step "combat"
"$bin" combat "$(native "$root/agent/fixtures/combat-world.txt")" > "$tmp/combat.json"
node -e '
  const c = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
  if (!Array.isArray(c.fights) || c.fights.length === 0) throw new Error("no fights");
  console.log(`combat: ${c.fights.length} fight(s)`);
' "$(native "$tmp/combat.json")" || fail "combat found no fights in the fixture"

step "discovery"
game="$tmp/Program Files (x86)/World of Warcraft"
product="$game/_forever_"
mkdir -p "$product/WTF/Account/SMOKE/SavedVariables"
printf 'Product Flavor!STRING:0\nwowf\n' > "$product/.flavor.info"
cp "$root/agent/fixtures/Ryna.lua" "$product/WTF/Account/SMOKE/SavedVariables/Ryna.lua"
"$bin" status --install "$(native "$game")" | tee "$tmp/status.txt"
grep -q '_forever_  flavor wowf' "$tmp/status.txt" || fail "status didn't list the _forever_ install"
grep -q '^Accounts:' "$tmp/status.txt" || fail "status didn't list the account"
grep -q 'SavedVariables.Ryna\.lua' "$tmp/status.txt" || fail "status didn't list the account's Ryna.lua"

printf '\nsmoke: ok\n'
