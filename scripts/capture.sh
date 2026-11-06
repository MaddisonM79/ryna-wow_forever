#!/usr/bin/env bash
# capture.sh: turn a real Ryna.lua into a fixture that is safe to commit.
#
#   scripts/capture.sh <path to WTF/Account/<ACCOUNT>/SavedVariables/Ryna.lua> [fixture-name]
#
# 1. Copies the file as it is into captures/ (gitignored: it never enters the tree).
# 2. Writes a scrubbed copy to agent/fixtures/<fixture-name>.lua (default: captured.lua), where
#    - every character name ("First-Last", from fullName/firstName/lastName and roster/seen
#      entries) becomes a stable fake (the first two characters become Aerin-Stormcall and
#      Thorbrand-Ironhew, then Kesra-Duskmantle, ...), word by word, everywhere in the file;
#    - the hex part of every Player-/Pet-/Creature- GUID becomes a stable fake (0F00A001, ...),
#      which also covers the session ids ("<login>-<hex>");
#    - guild names (guilds' and characters' guild.name, seen events' guild, and the guild keys
#      "<name>-<realmId>") become "Guild One", "Guild Two", ...;
#    - roster note and officerNote become "note";
#    - the account folder's name (from the path) is replaced wherever it appears.
#    "Stable" means the same real value gets the same fake within one run, in order of first
#    appearance, so the fixture keeps its structure (who grouped with whom, which roster row is
#    which character).
# 3. Checks the scrubbed copy contains none of the real names, hexes, guilds or account name, and
#    refuses to leave it behind if it does.
#
# It reads the client's SavedVariables layout (one `["key"] = value,` per line), which is what
# the game writes. Review the result before committing it anyway.
set -euo pipefail

src="${1:?usage: scripts/capture.sh <Ryna.lua> [fixture-name]}"
name="${2:-captured}"
repo="$(cd "$(dirname "$0")/.." && pwd)"
[ -f "$src" ] || { echo "no such file: $src" >&2; exit 1; }

account="$(basename "$(dirname "$(dirname "$src")")")"
[ "$(basename "$(dirname "$src")")" = "SavedVariables" ] || account=""

mkdir -p "$repo/captures" "$repo/agent/fixtures"
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
cp "$src" "$repo/captures/$stamp-Ryna.lua"
out="$repo/agent/fixtures/$name.lua"
leaks="$repo/captures/$stamp-scrubbed-values.txt"

ACCOUNT="$account" LEAKS="$leaks" perl -e '
use strict; use warnings;
local $/; my $text = <STDIN>;
my @first = qw(Aerin Thorbrand Kesra Brannoc Moraine Sorvyn Tavi Lunet Corwen Isolde Drevan Maelis Orrin Pyra Quill Rennick Saphe Tamsin Ulric Vesna);
my @last  = qw(Stormcall Ironhew Duskmantle Ashvale Thistlewood Galecrest Emberleaf Brightwater Coldmere Deepforge Fallowmere Greythorn Hollowell Kestrelwind Larkspur Marrowind Nettlefield Oakhart Ravenmoor Silverbrook);
my @guildfake = ("Guild One", "Guild Two", "Guild Three", "Guild Four", "Guild Five");
my (%fname, %lname, %hex, %guild, @order);
my ($nf, $nl, $nh, $ng) = (0, 0, 0, 0);
sub fake_first { my $w = shift; $fname{$w} //= ($first[$nf] // "First" . ($nf + 1)), $nf++ unless exists $fname{$w}; $fname{$w} }
sub fake_last  { my $w = shift; $lname{$w} //= ($last[$nl] // "Last" . ($nl + 1)), $nl++ unless exists $lname{$w}; $lname{$w} }
sub fake_hex   { my $h = shift; unless (exists $hex{$h}) { $nh++; $hex{$h} = sprintf("0F00%04X", $nh) } $hex{$h} }
sub fake_guild { my $g = shift; unless (exists $guild{$g}) { $guild{$g} = $guildfake[$ng] // "Guild " . ($ng + 1); $ng++ } $guild{$g} }

# Pass 1: collect, in order of appearance.
my @lines = split /\n/, $text, -1;
my @path;
for my $line (@lines) {
  if ($line =~ /^\s*\["(fullName|unitName|unitFullName)"\]\s*=\s*"([^"\\]+)-([^"\\]+)"/) { fake_first($2); fake_last($3) }
  elsif ($line =~ /^\s*\["firstName"\]\s*=\s*"([^"\\]+)"/) { fake_first($1) }
  elsif ($line =~ /^\s*\["lastName"\]\s*=\s*"([^"\\]+)"/) { fake_last($1) }
  while ($line =~ /\b(?:Player|Pet|Creature|Vehicle)-\d+-([0-9A-Fa-f]{6,})\b/g) { fake_hex($1) }
  if ($line =~ /^\s*\[(?:"([^"]*)"|\d+)\]\s*=\s*\{\s*$/ || $line =~ /^\s*\{\s*$/) { push @path, ($1 // "#") }
  elsif ($line =~ /^\s*\}/) { pop @path }
  my $p = join(".", @path);
  if ($p =~ /(^|\.)guilds\.[^.]+$/ && $line =~ /^\s*\["name"\]\s*=\s*"([^"]*)"/) { fake_guild($1) }
  if ($p =~ /\.guild$/ && $line =~ /^\s*\["name"\]\s*=\s*"([^"]*)"/) { fake_guild($1) }
  if ($line =~ /^\s*\["guild"\]\s*=\s*"([^"]*)"/) { fake_guild($1) }
}
# Pass 2: replace. Guild keys "<name>-<realmId>" first, then names word by word, then hexes.
for my $g (sort { length($b) <=> length($a) } keys %guild) {
  my $f = $guild{$g};
  $text =~ s/"\Q$g\E-(\d+)"/"$f-$1"/g;
  $text =~ s/"\Q$g\E"/"$f"/g;
}
$text =~ s/(\["(?:note|officerNote)"\]\s*=\s*)"(?:[^"\\]|\\.)*"/$1"note"/g;
for my $w (sort { length($b) <=> length($a) } keys %fname) { my $f = $fname{$w}; $text =~ s/\b\Q$w\E\b/$f/g }
for my $w (sort { length($b) <=> length($a) } keys %lname) { my $f = $lname{$w}; $text =~ s/\b\Q$w\E\b/$f/g }
for my $h (keys %hex) { my $f = $hex{$h}; $text =~ s/\b\Q$h\E\b/$f/gi }
my $acct = $ENV{ACCOUNT} // "";
$text =~ s/\Q$acct\E/FAKEACCOUNT1/g if length $acct;
print $text;
# What was replaced, for the leak check (kept in captures/, never committed).
# A real value that is also one of the fakes used (a capture that already holds fake names) is
# expected in the output, so it is left out of the check.
my %fake = map { $_ => 1 } (values %fname, values %lname, values %hex, values %guild);
open my $fh, ">", $ENV{LEAKS} or die;
print $fh "$_\n" for grep { length > 2 && !$fake{$_} } (keys %fname, keys %lname, keys %hex, keys %guild, ($acct ne "" ? $acct : ()));
close $fh;
' < "$src" > "$out.tmp"

# The leak check: none of the real values may survive.
if grep -F -w -f "$leaks" "$out.tmp" >/dev/null 2>&1; then
  echo "refusing: the scrubbed copy still contains these real values:" >&2
  grep -o -F -w -f "$leaks" "$out.tmp" | sort -u >&2
  rm -f "$out.tmp"
  exit 1
fi
mv "$out.tmp" "$out"
echo "raw copy:      captures/$stamp-Ryna.lua (gitignored)"
echo "scrubbed copy: agent/fixtures/$name.lua (review before committing)"
