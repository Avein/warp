#!/usr/bin/env bash
# shellcheck shell=bash
#
# Shared helpers for scripts/weekly-sync.sh and scripts/rollback-last-sync.sh.
# Sourced, not executed. See docs/fork-strategy.md §2.4 and §5.

PERSONAL_BRANCH="personal/main"
TAG_PREFIX="personal/sync"
FORK_REMOTE="origin"
UPSTREAM_REMOTE="upstream"

# Conflict-resolution policy reminder — single source of truth for both
# scripts and the canonical doc. The phrasing mirrors
# docs/fork-strategy.md §3; keep them in sync.
read -r -d '' SYNC_POLICY_REMINDER <<'EOF' || true
Conflict resolution policy (docs/fork-strategy.md §3):

  DEFAULT — favor upstream.
    Start every conflict from: git checkout --theirs <path>
    Re-implement the local feature on top of upstream's new shape
    inside the same merge commit.

  EXCEPTION 1 — documented invariant.
    If upstream's change breaks a behaviour guaranteed in
    docs/projects-persistence.md, docs/projects-rename-summary.md,
    or docs/issues/projects-persistence-0[1-4]-*.md, favor the local
    guarantee. Migrate the implementation to upstream's new substrate.

  EXCEPTION 2 — pure feature removal.
    If upstream removed a behaviour this fork depends on, re-add it
    as a private helper inside the dependent module — not as a
    fork-of-upstream public API.

After resolving:
  git add <resolved files>
  git commit --no-edit       # accept the prepared merge message
  scripts/weekly-sync.sh     # re-run from the top
EOF

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

note() {
  printf '%s\n' "$*"
}

print_policy_reminder() {
  printf '\n------------------------------------------------------------------------\n'
  printf '%s' "$SYNC_POLICY_REMINDER"
  printf '\n------------------------------------------------------------------------\n\n'
}

# Refuse if any tracked file is staged or unstaged. Untracked files are
# tolerated — the supporting PRD docs live untracked alongside the repo
# by design.
require_clean_tree() {
  if ! git diff --quiet --ignore-submodules; then
    die "unstaged changes present; commit or stash before running"
  fi
  if ! git diff --cached --quiet --ignore-submodules; then
    die "staged changes present; commit or stash before running"
  fi
}

require_branch() {
  local want="$1"
  local cur
  cur=$(git symbolic-ref --short HEAD 2>/dev/null || true)
  if [[ "$cur" != "$want" ]]; then
    die "must run from $want (currently on ${cur:-<detached HEAD>})"
  fi
}

require_remote() {
  local name="$1"
  if ! git remote get-url "$name" >/dev/null 2>&1; then
    die "remote '$name' is not configured"
  fi
}

today_iso() {
  date -u +%Y-%m-%d
}

# Move CWD to the repo root, so the script can be invoked from anywhere.
cd_repo_root() {
  local root
  root=$(git rev-parse --show-toplevel 2>/dev/null) \
    || die "not inside a git working tree"
  cd "$root" || die "cannot cd to repo root $root"
}
