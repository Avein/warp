#!/usr/bin/env bash
#
# Roll personal/main back one rung on the -post tag ladder.
# See docs/fork-strategy.md §5.
#
# Discovers the two most recent personal/sync/*-post tags, prints the
# diff that would be discarded, prompts for confirmation, then resets
# the branch and pushes with --force-with-lease (never plain --force).

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib/sync-helpers.sh
source "$script_dir/lib/sync-helpers.sh"

cd_repo_root

# Preflight.
require_branch "$PERSONAL_BRANCH"
require_clean_tree
require_remote "$FORK_REMOTE"

# 1. List -post tags newest-first. Loop form for macOS system bash 3.2
#    compatibility (no `mapfile`).
post_tags=()
while IFS= read -r tag_line; do
  post_tags+=("$tag_line")
done < <(git tag --list "$TAG_PREFIX/*-post" --sort=-creatordate)

# 2. Refuse if fewer than two — nothing to roll back to.
if [[ ${#post_tags[@]} -lt 2 ]]; then
  die "fewer than two ${TAG_PREFIX}/*-post tags exist (${#post_tags[@]} found); nothing to roll back to. Use git reset against ${TAG_PREFIX}/*-initial manually if you really mean to."
fi

current_tag=${post_tags[0]}
target_tag=${post_tags[1]}
current_sha=$(git rev-parse "$current_tag")
target_sha=$(git rev-parse "$target_tag")
head_sha=$(git rev-parse HEAD)

# 3. Print current and proposed state.
note "current $PERSONAL_BRANCH tip:        $(git rev-parse --short HEAD)"
note "most-recent post tag:               $current_tag ($(git rev-parse --short "$current_tag"))"
note "proposed rollback target:           $target_tag ($(git rev-parse --short "$target_tag"))"

if [[ "$head_sha" != "$current_sha" ]]; then
  note ""
  note "warning: $PERSONAL_BRANCH (HEAD) is not at the most-recent -post tag."
  note "         HEAD: $head_sha"
  note "         $current_tag: $current_sha"
  note "         The rollback will still target $target_tag, but inspect first."
fi

# 4. Diff summary of what would be discarded.
note ""
note "Commits that would be removed from $PERSONAL_BRANCH ($target_tag..HEAD):"
git log --oneline --no-merges "$target_tag..HEAD" || true
note ""

# 5. Prompt.
printf 'Reset %s to %s and push --force-with-lease to %s/%s? [y/N] ' \
  "$PERSONAL_BRANCH" "$target_tag" "$FORK_REMOTE" "$PERSONAL_BRANCH"
IFS= read -r answer
case "$answer" in
  y|Y|yes|YES) ;;
  *) die "rollback aborted by user" ;;
esac

# 6. Reset and push with lease.
git reset --hard "$target_sha"
git push --force-with-lease "$FORK_REMOTE" "$PERSONAL_BRANCH"

note ""
note "rolled back: $PERSONAL_BRANCH now at $(git rev-parse --short HEAD) (= $target_tag)"
