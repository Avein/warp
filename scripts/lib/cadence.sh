#!/usr/bin/env bash
# shellcheck shell=bash
#
# Pure cadence-tier classification — see docs/fork-strategy.md §4 and
# docs/issues/fork-strategy-03-cadence-tier.md.
#
# `cadence_tier <today_iso> <last_post_iso>` prints one of:
#   Normal              (0-1 weeks since last sync)
#   DoubleBudget        (2-4 weeks)
#   IncrementalCatchup  (5-12 weeks)
#   FreshStart          (13+ weeks)
#
# The function is intentionally pure: it does not read tags, does not
# run git, does not read environment, does not print warnings. Those
# are the caller's responsibility.
#
# Both arguments are YYYY-MM-DD. If <last_post_iso> is empty (no prior
# successful sync), the function returns Normal — the first sync ever
# is by definition not behind.
#
# Date parsing handles both BSD (`date -j -f`) and GNU (`date -d`)
# syntax, so the same module works on macOS (local fallback) and on
# Linux (GitHub Actions runner).

cadence_tier() {
  local today="$1"
  local last="${2:-}"

  if [[ -z "$last" ]]; then
    printf 'Normal\n'
    return 0
  fi

  local today_secs last_secs
  today_secs=$(_cadence_iso_to_secs "$today") || return 2
  last_secs=$(_cadence_iso_to_secs "$last") || return 2

  if (( last_secs > today_secs )); then
    printf 'cadence_tier: last (%s) is after today (%s)\n' "$last" "$today" >&2
    return 2
  fi

  local days=$(( (today_secs - last_secs) / 86400 ))
  local weeks=$(( days / 7 ))

  if   (( weeks <= 1 ));  then printf 'Normal\n'
  elif (( weeks <= 4 ));  then printf 'DoubleBudget\n'
  elif (( weeks <= 12 )); then printf 'IncrementalCatchup\n'
  else                         printf 'FreshStart\n'
  fi
}

# Convert YYYY-MM-DD to a Unix timestamp, portable across BSD and GNU
# date. Prints the timestamp on stdout; returns non-zero on parse
# failure.
_cadence_iso_to_secs() {
  local iso="$1"
  local secs
  # BSD/macOS form. TZ=UTC pins parsing to UTC so DST transitions
  # between the two compared dates can't drop a day from the diff.
  secs=$(TZ=UTC date -j -f "%Y-%m-%d %H:%M:%S" "$iso 00:00:00" "+%s" 2>/dev/null) && {
    printf '%s\n' "$secs"
    return 0
  }
  # GNU/Linux form. The Z suffix already pins to UTC.
  secs=$(date -u -d "${iso}T00:00:00Z" "+%s" 2>/dev/null) && {
    printf '%s\n' "$secs"
    return 0
  }
  printf '_cadence_iso_to_secs: cannot parse %s\n' "$iso" >&2
  return 1
}

# Caller-facing advisory text for the non-Normal tiers, suitable for
# printing in a preflight step. The Normal case prints nothing.
cadence_advisory() {
  local tier="$1"
  case "$tier" in
    Normal) ;;
    DoubleBudget)
      cat <<'EOF'
ADVISORY: it has been 2-4 weeks since the last successful sync.
Tier: DoubleBudget. Budget 30-60 min — conflicts may compound when
upstream has touched the projects subsystem. See docs/fork-strategy.md
section 4.
EOF
      ;;
    IncrementalCatchup)
      cat <<'EOF'
ADVISORY: it has been 5-12 weeks since the last successful sync.
Tier: IncrementalCatchup. Consider syncing against successive upstream
stable cuts (upstream/cherrypick/stable_release/*) instead of one big
master merge. See docs/fork-strategy.md section 4.
EOF
      ;;
    FreshStart)
      cat <<'EOF'
ADVISORY: it has been 13+ weeks since the last successful sync.
Tier: FreshStart. The integration cost has likely exceeded the value
of incremental absorption. Consider abandoning this branch and
cherry-picking the customisation commits onto fresh upstream/master
as a new personal/main-v2. See docs/fork-strategy.md section 4.
EOF
      ;;
    *)
      printf 'cadence_advisory: unknown tier %s\n' "$tier" >&2
      return 1
      ;;
  esac
}
