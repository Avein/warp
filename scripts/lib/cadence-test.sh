#!/usr/bin/env bash
#
# Pure unit tests for scripts/lib/cadence.sh. Run as
#   bash scripts/lib/cadence-test.sh
#
# Covers the four tier boundaries (1, 4, 12, 13 weeks) plus the eight
# adjacent off-by-one days (6, 8, 27, 29, 83, 85, 90, 92 days) plus
# the empty-input case. 13 assertions total — strictly above the
# acceptance criterion's floor of 8.

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=cadence.sh
source "$script_dir/cadence.sh"

pass=0
fail=0

# assert <label> <expected_tier> <today> <last>
assert_tier() {
  local label="$1" expected="$2" today="$3" last="$4"
  local got
  got=$(cadence_tier "$today" "$last")
  if [[ "$got" == "$expected" ]]; then
    pass=$((pass + 1))
    printf '  PASS  %s -> %s\n' "$label" "$got"
  else
    fail=$((fail + 1))
    printf '  FAIL  %s -> expected %s, got %s\n' "$label" "$expected" "$got"
  fi
}

# All assertions anchor on the same "today" so the day deltas are easy
# to read. 2026-04-01 picks a stable mid-year date with no leap-day
# weirdness in either direction.
TODAY="2026-04-01"

printf '\n--- empty input (no prior sync) ---\n'
assert_tier "no last (first sync ever)"  "Normal"  "$TODAY"  ""

printf '\n--- boundary cases: exactly 1, 4, 12, 13 weeks ---\n'
# 1 week  = 7 days  -> 2026-03-25
# 4 weeks = 28 days -> 2026-03-04
# 12 weeks = 84 days -> 2026-01-07
# 13 weeks = 91 days -> 2025-12-31
assert_tier "exactly  7 days (1 week)"    "Normal"              "$TODAY"  "2026-03-25"
assert_tier "exactly 28 days (4 weeks)"   "DoubleBudget"        "$TODAY"  "2026-03-04"
assert_tier "exactly 84 days (12 weeks)"  "IncrementalCatchup"  "$TODAY"  "2026-01-07"
assert_tier "exactly 91 days (13 weeks)"  "FreshStart"          "$TODAY"  "2025-12-31"

printf '\n--- off-by-one: 6/8 days flank the 1-week boundary ---\n'
assert_tier " 6 days  (-1 from 1wk)"  "Normal"  "$TODAY"  "2026-03-26"
assert_tier " 8 days  (+1 from 1wk)"  "Normal"  "$TODAY"  "2026-03-24"

printf '\n--- off-by-one: 27/29 days flank the 4-week boundary ---\n'
assert_tier "27 days  (-1 from 4wk)"  "DoubleBudget"  "$TODAY"  "2026-03-05"
assert_tier "29 days  (+1 from 4wk)"  "DoubleBudget"  "$TODAY"  "2026-03-03"

printf '\n--- off-by-one: 83/85 days flank the 12-week boundary ---\n'
assert_tier "83 days  (-1 from 12wk)"  "IncrementalCatchup"  "$TODAY"  "2026-01-08"
assert_tier "85 days  (+1 from 12wk)"  "IncrementalCatchup"  "$TODAY"  "2026-01-06"

printf '\n--- off-by-one: 90/92 days flank the 13-week boundary ---\n'
assert_tier "90 days  (-1 from 13wk)"  "IncrementalCatchup"  "$TODAY"  "2026-01-01"
assert_tier "92 days  (+1 from 13wk)"  "FreshStart"          "$TODAY"  "2025-12-30"

printf '\n--- summary ---\n'
printf '  %d passed, %d failed\n' "$pass" "$fail"
if (( fail > 0 )); then
  exit 1
fi
