#!/usr/bin/env bash
#
# Local fallback for the weekly upstream-absorption sync.
# See docs/fork-strategy.md §2.4.
#
# Mirrors the GitHub Actions weekly-sync workflow's sequence so this
# script remains operational when CI is down or a mid-week sync is
# needed. Refuses to run on a dirty tree or off personal/main.

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib/sync-helpers.sh
source "$script_dir/lib/sync-helpers.sh"
# shellcheck source=lib/cadence.sh
source "$script_dir/lib/cadence.sh"

YES=0
while (( $# > 0 )); do
  case "$1" in
    --yes|-y) YES=1; shift ;;
    --help|-h)
      cat <<'EOF'
Usage: scripts/weekly-sync.sh [--yes]

Runs the full local weekly-sync sequence — see docs/fork-strategy.md section 2.4.

  --yes, -y  Skip the FreshStart-tier confirmation prompt. Other tier
             advisories are non-blocking and unaffected. Useful when
             scripting this sync from a cron or CI step.
EOF
      exit 0
      ;;
    *) die "unknown flag: $1" ;;
  esac
done

cd_repo_root

# 1. Preflight.
require_branch "$PERSONAL_BRANCH"
require_clean_tree
require_remote "$FORK_REMOTE"
require_remote "$UPSTREAM_REMOTE"

# Cadence advisory: read the latest -post tag's date (or empty if no
# successful sync yet), classify the tier, and surface a non-blocking
# advisory message. Only the FreshStart tier blocks (with --yes
# bypass), per docs/fork-strategy.md section 4.
latest_post_tag=$(git tag --list "$TAG_PREFIX/*-post" --sort=-creatordate | head -n1)
if [[ -n "$latest_post_tag" ]]; then
  # Tag form: personal/sync/YYYY-MM-DD-post -> YYYY-MM-DD
  last_post_date="${latest_post_tag#$TAG_PREFIX/}"
  last_post_date="${last_post_date%-post}"
else
  last_post_date=""
fi

tier=$(cadence_tier "$(today_iso)" "$last_post_date") \
  || die "cadence tier classification failed"
note "cadence tier for this run: $tier (last -post: ${last_post_date:-none})"

if [[ "$tier" != "Normal" ]]; then
  note ""
  cadence_advisory "$tier"
  note ""
fi

if [[ "$tier" == "FreshStart" && "$YES" != "1" ]]; then
  printf 'Continue with the normal weekly-sync workflow anyway? [y/N] '
  IFS= read -r answer
  case "$answer" in
    y|Y|yes|YES) ;;
    *) die "aborted by user; see docs/fork-strategy.md section 4 for fresh-start mode" ;;
  esac
fi

# 2. Fetch upstream.
note "fetching $UPSTREAM_REMOTE..."
git fetch "$UPSTREAM_REMOTE" --quiet

# 3. Fast-forward master and push to fork.
note "fast-forwarding master to $UPSTREAM_REMOTE/master..."
git switch master --quiet
git merge --ff-only "$UPSTREAM_REMOTE/master" --quiet
git push "$FORK_REMOTE" master
git switch "$PERSONAL_BRANCH" --quiet

# Short-circuit: master already reachable from personal/main = no work.
master_sha=$(git rev-parse master)
if git merge-base --is-ancestor "$master_sha" "$PERSONAL_BRANCH"; then
  note "no-op: $UPSTREAM_REMOTE/master is already reachable from $PERSONAL_BRANCH; nothing to sync"
  exit 0
fi

today=$(today_iso)
pre_tag="$TAG_PREFIX/${today}-pre"
post_tag="$TAG_PREFIX/${today}-post"
candidate="$TAG_PREFIX/${today}-candidate"

# Same-day reruns aren't supported — the -pre tag is the rollback rung
# for this date's sync and must point at the personal/main tip at the
# start of THIS run, not a previous run's start.
if git rev-parse -q --verify "refs/tags/$pre_tag" >/dev/null; then
  die "tag $pre_tag already exists; resolve the previous run before starting a new one"
fi

# Same-day candidate branch left over from a previous run = previous run
# failed and the candidate wasn't promoted or cleaned up. Refuse rather
# than silently overwriting it.
if git rev-parse -q --verify "refs/heads/$candidate" >/dev/null; then
  die "branch $candidate exists locally; resolve or delete it before re-running"
fi

# 4. Tag -pre on the current personal/main tip.
note "tagging $pre_tag on $PERSONAL_BRANCH..."
git tag "$pre_tag" "$PERSONAL_BRANCH"

# 5. Create the candidate branch from personal/main.
note "creating candidate $candidate..."
git switch -c "$candidate" "$PERSONAL_BRANCH" --quiet

# 6. Merge master into candidate.
note "merging master into $candidate..."
if ! git merge --no-ff master -m "merge upstream/master into $candidate"; then
  print_policy_reminder
  printf 'MERGE CONFLICT — resolve manually and re-run.\n' >&2
  printf 'Pre-tag %s remains as the rollback baseline if you abandon this sync.\n' "$pre_tag" >&2
  exit 2
fi

# 7. Build the OSS-channel WarpOss.app (unsigned, arm64-only).
note ""
note "building WarpOss.app (./script/bundle --channel oss --nosign --nouniversal)..."
if ! ./script/bundle --channel oss --nosign --nouniversal; then
  die "build failed; candidate $candidate left for inspection. personal/main was not moved."
fi

# 8. Interactive smoke checklist. Items mirror docs/fork-strategy.md §6;
#    keep them in sync if the doc evolves.
note ""
note "Smoke checklist — install the just-built WarpOss.app and walk through each item."
note "See docs/fork-strategy.md §6 for the full procedure per item."
note ""

smoke_items=(
  "Smoke 1 — Palette open and search (override and identity hits both match)"
  "Smoke 2 — F2 inline rename commits via Enter; empty buffer reverts to identity"
  "Smoke 3 — Persistence: open + rename + close non-last + Cmd-Q + relaunch restores correctly"
  "Smoke 4 — Synthetic root: with all windows closed, new-window entrypoint spawns root"
  "Smoke 5 — Project-bar strip order preserved across Cmd-Q + relaunch"
)

for item in "${smoke_items[@]}"; do
  printf '  %s [y/N] ' "$item"
  IFS= read -r answer
  case "$answer" in
    y|Y|yes|YES) ;;
    *)
      printf '\nsmoke item failed: %s\n' "$item" >&2
      printf 'Candidate %s is left in place for inspection. personal/main was not moved.\n' "$candidate" >&2
      printf 'To roll back if a previous sync caused the trouble: scripts/rollback-last-sync.sh\n' >&2
      exit 3
      ;;
  esac
done

# 9. Promote: fast-forward personal/main to the candidate, tag -post, push.
note ""
note "all smoke items green; promoting $candidate -> $PERSONAL_BRANCH..."
git switch "$PERSONAL_BRANCH" --quiet
git merge --ff-only "$candidate" --quiet
git tag "$post_tag" "$PERSONAL_BRANCH"
git push "$FORK_REMOTE" "$PERSONAL_BRANCH" "refs/tags/$pre_tag" "refs/tags/$post_tag" "refs/heads/$candidate"

note "sync complete: $PERSONAL_BRANCH advanced to $(git rev-parse --short HEAD), tagged $post_tag"
