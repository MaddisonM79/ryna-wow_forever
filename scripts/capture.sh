#!/usr/bin/env bash
# capture.sh: turn a real Ryna.lua (or a combat log) into a fixture that is safe to commit.
#
#   scripts/capture.sh <path to WTF/Account/<ACCOUNT>/SavedVariables/Ryna.lua> [fixture-name]
#   scripts/capture.sh --combat <path to Logs/WoWCombatLog-*.txt> [fixture-name] [--from HH:MM:SS] [--until HH:MM:SS]
#
# Combat-log mode (docs/wow.md section 9.8) writes agent/fixtures/<fixture-name>.txt (default
# combat-world.txt), keeping the log's structure and every NPC's name but
#    - every player's and pet's name (the quoted name after a Player- or Pet- GUID, which covers
#      the names in deaths and kills) becomes a stable fake, its "Name" and "Realm" parts each from
#      the same fake lists as below, with the client's "-Normal-US" tail kept;
#    - the hex part of every Player- GUID, and the spawn part of every Creature-, Pet- and
#      Vehicle- GUID, becomes a stable fake of the same length;
#    - with --from/--until, only the lines in that window (by the line's clock) are kept, plus the
#      COMBAT_LOG_VERSION, ZONE_CHANGE and MAP_CHANGE lines before it, so a fixture can be a few
#      fights of a long night.
# The raw log is copied into captures/ first, and the same leak check runs on the result.
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

repo="$(cd "$(dirname "$0")/.." && pwd)"

if [ "${1:-}" = "--combat" ]; then
  shift
  src="${1:?usage: scripts/capture.sh --combat <WoWCombatLog-*.txt> [fixture-name] [--from HH:MM:SS] [--until HH:MM:SS]}"
  shift
  name="combat-world"
  if [ $# -gt 0 ] && [ "${1#--}" = "$1" ]; then name="$1"; shift; fi
  from=""; until=""
  while [ $# -gt 0 ]; do
    case "$1" in
      --from) from="${2:?--from needs HH:MM:SS}"; shift 2 ;;
      --until) until="${2:?--until needs HH:MM:SS}"; shift 2 ;;
      *) echo "unknown argument: $1" >&2; exit 1 ;;
    esac
  done
  [ -f "$src" ] || { echo "no such file: $src" >&2; exit 1; }
  mkdir -p "$repo/captures" "$repo/agent/fixtures"
  stamp="$(date -u +%Y%m%dT%H%M%SZ)"
  cp "$src" "$repo/captures/$stamp-$(basename "$src")"
  out="$repo/agent/fixtures/$name.txt"
  leaks="$repo/captures/$stamp-combat-scrubbed-values.txt"
  FROM="$from" UNTIL="$until" LEAKS="$leaks" perl -e '
use strict; use warnings;
my @first = qw(Aerin Thorbrand Kesra Brannoc Moraine Sorvyn Tavi Lunet Corwen Isolde Drevan Maelis Orrin Pyra Quill Rennick Saphe Tamsin Ulric Vesna);
my @last  = qw(Stormcall Ironhew Duskmantle Ashvale Thistlewood Galecrest Emberleaf Brightwater Coldmere Deepforge Fallowmere Greythorn Hollowell Kestrelwind Larkspur Marrowind Nettlefield Oakhart Ravenmoor Silverbrook);
my ($from, $until) = ($ENV{FROM} // "", $ENV{UNTIL} // "");
my (%fname, %lname, %hex, %spawn, %whole);
my ($nf, $nl, $nh, $ns) = (0, 0, 0, 0);
sub fake_first { my $w = shift; unless (exists $fname{$w}) { $fname{$w} = $first[$nf] // "First" . ($nf + 1); $nf++ } $fname{$w} }
sub fake_last  { my $w = shift; unless (exists $lname{$w}) { $lname{$w} = $last[$nl] // "Last" . ($nl + 1); $nl++ } $lname{$w} }
sub fake_hex   { my $h = shift; unless (exists $hex{$h}) { $nh++; $hex{$h} = substr(sprintf("%0*X", length($h), 0), 0, length($h) - 7) . sprintf("F%06X", $nh) } $hex{$h} }
sub fake_spawn { my $h = shift; unless (exists $spawn{$h}) { $ns++; $spawn{$h} = substr(sprintf("%0*X", length($h), 0), 0, length($h) - 7) . sprintf("E%06X", $ns) } $spawn{$h} }
sub fake_name {
  my $n = shift;
  return $whole{$n} if exists $whole{$n};
  my ($base, $tail) = ($n, "");
  if ($n =~ /^(.*?)(-[A-Za-z]+-[A-Z]{2})$/) { my ($b, $t) = ($1, $2); ($base, $tail) = ($b, $t) if $b =~ /-/ }
  my ($who, $realm) = split /-/, $base, 2;
  my $f = fake_first($who);
  $f .= "-" . join("-", map { fake_last($_) } split /-/, $realm) if defined $realm && length $realm;
  $whole{$n} = "$f$tail";
}
sub clock { my $l = shift; $l =~ /^\S+ (\d\d?:\d\d:\d\d)/ ? sprintf("%08s", $1) =~ s/ /0/gr : "" }
my $pad = sub { my $t = shift; length($t) == 7 ? "0$t" : $t };
($from, $until) = ($pad->($from), $pad->($until));
my @out;
while (my $line = <STDIN>) {
  my $t = $pad->(clock($line));
  my $structural = $line =~ /^\S+ \S+  (COMBAT_LOG_VERSION|ZONE_CHANGE|MAP_CHANGE),/;
  if (length $from && $t lt $from) { push @out, $line if $structural; next }
  last if length $until && $t gt $until;
  push @out, $line;
}
my $text = join("", @out);
# Names right after a player or pet GUID; hexes and spawn parts.
while ($text =~ /\b(?:Player-\d+-[0-9A-Fa-f]+|Pet-0(?:-\d+){4}-[0-9A-Fa-f]+),"([^"]+)"/g) { fake_name($1) }
while ($text =~ /\bPlayer-\d+-([0-9A-Fa-f]{6,})\b/g) { fake_hex($1) }
while ($text =~ /\b(?:Creature|Pet|Vehicle|GameObject)-0(?:-\d+){4}-([0-9A-Fa-f]{6,})\b/g) { fake_spawn($1) }
$text =~ s/\b(Player-\d+-[0-9A-Fa-f]+|Pet-0(?:-\d+){4}-[0-9A-Fa-f]+),"([^"]+)"/"$1,\"" . $whole{$2} . "\""/ge;
$text =~ s/\b(Player-\d+-)([0-9A-Fa-f]{6,})\b/$1 . $hex{$2}/ge;
$text =~ s/\b((?:Creature|Pet|Vehicle|GameObject)-0(?:-\d+){4}-)([0-9A-Fa-f]{6,})\b/$1 . $spawn{$2}/ge;
print $text;
my %fake = map { $_ => 1 } (values %fname, values %lname, values %hex, values %spawn);
open my $fh, ">", $ENV{LEAKS} or die;
print $fh "$_\n" for grep { length > 2 && !$fake{$_} } (keys %fname, keys %lname, keys %hex);
close $fh;
' < "$src" > "$out.tmp"
  if [ -s "$leaks" ] && grep -F -w -f "$leaks" "$out.tmp" >/dev/null 2>&1; then
    echo "refusing: the scrubbed copy still contains these real values:" >&2
    grep -o -F -w -f "$leaks" "$out.tmp" | sort -u >&2
    rm -f "$out.tmp"
    exit 1
  fi
  mv "$out.tmp" "$out"
  echo "raw copy:      captures/$stamp-$(basename "$src") (gitignored)"
  echo "scrubbed copy: agent/fixtures/$name.txt ($(wc -l < "$out" | tr -d ' ') lines; review before committing)"
  exit 0
fi

src="${1:?usage: scripts/capture.sh <Ryna.lua> [fixture-name]}"
name="${2:-captured}"
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
