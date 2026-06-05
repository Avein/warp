#!/usr/bin/env bash
#
# Thin local wrapper around `gh workflow run promote-candidate.yml -f date=…`.
# See docs/fork-strategy.md §2.3.
#
# Resolves the date from the most recent personal/sync/*-candidate branch
# on origin by default; an explicit --date YYYY-MM-DD argument overrides.
# Refuses to run unless `gh` is authenticated against the fork.

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib/sync-helpers.sh
source "$script_dir/lib/sync-helpers.sh"

WORKFLOW="promote-candidate.yml"
EXPLICIT_DATE=""
DELETE_CANDIDATE=0

while (( $# > 0 )); do
  case "$1" in
    --date)
      shift
      EXPLICIT_DATE="${1:-}"
      [[ -n "$EXPLICIT_DATE" ]] || die "--date requires YYYY-MM-DD"
      shift
      ;;
    --date=*)
      EXPLICIT_DATE="${1#--date=}"
      shift
      ;;
    --delete-candidate)
      DELETE_CANDIDATE=1
      shift
      ;;
    --help|-h)
      cat <<'EOF'
Usage: scripts/promote-candidate.sh [--date YYYY-MM-DD] [--delete-candidate]

Dispatches the promote-candidate.yml workflow on the fork for a smoked
weekly-sync candidate. See docs/fork-strategy.md §2.3.

  --date YYYY-MM-DD     Promote that exact candidate. Default: the most
                        recent personal/sync/*-candidate branch on origin.
  --delete-candidate    Pass delete_candidate=true to the workflow, so it
                        deletes the candidate branch after promotion.
                        Default: keep the candidate branch.

Requires the `gh` CLI authenticated against the fork.
EOF
      exit 0
      ;;
    *) die "unknown flag: $1" ;;
  esac
done

cd_repo_root

# Preflight — refuse if gh is missing or unauthenticated.
command -v gh >/dev/null 2>&1 \
  || die "gh CLI not found; install from https://cli.github.com"
gh auth status >/dev/null 2>&1 \
  || die "gh CLI is not authenticated; run 'gh auth login'"

# Resolve the date. Either the explicit --date override or the suffix of
# the most-recent personal/sync/*-candidate branch on origin. We use
# `ls-remote` so this works without a local fetch — staying lightweight is
# the whole point of the wrapper.
if [[ -n "$EXPLICIT_DATE" ]]; then
  DATE="$EXPLICIT_DATE"
else
  note "no --date passed; resolving from the most-recent candidate branch on $FORK_REMOTE..."
  # Format: refs/heads/personal/sync/YYYY-MM-DD-candidate
  # Sort lexically — YYYY-MM-DD is already date-sortable as a string.
  latest_candidate_ref=$(git ls-remote --heads "$FORK_REMOTE" "refs/heads/${TAG_PREFIX}/*-candidate" \
    | awk '{print $2}' \
    | sort -r \
    | head -n1)
  if [[ -z "$latest_candidate_ref" ]]; then
    die "no ${TAG_PREFIX}/*-candidate branches on $FORK_REMOTE; nothing to promote"
  fi
  # refs/heads/personal/sync/YYYY-MM-DD-candidate -> YYYY-MM-DD
  branch_name="${latest_candidate_ref#refs/heads/}"
  candidate_suffix="${branch_name#"${TAG_PREFIX}/"}"
  DATE="${candidate_suffix%-candidate}"
  note "resolved date: $DATE (from $branch_name)"
fi

# Validate the date shape locally before round-tripping to GitHub.
if [[ ! "$DATE" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]]; then
  die "date '$DATE' is not in YYYY-MM-DD form"
fi

note "dispatching workflow $WORKFLOW with date=$DATE delete_candidate=$DELETE_CANDIDATE..."
if (( DELETE_CANDIDATE == 1 )); then
  gh workflow run "$WORKFLOW" -f "date=$DATE" -f "delete_candidate=true"
else
  gh workflow run "$WORKFLOW" -f "date=$DATE"
fi

note ""
note "dispatched. Watch the run with:"
note "  gh run list --workflow=$WORKFLOW --limit 1"
note "  gh run watch \$(gh run list --workflow=$WORKFLOW --limit 1 --json databaseId --jq '.[0].databaseId')"
