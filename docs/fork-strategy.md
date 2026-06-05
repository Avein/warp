# Fork Strategy — `Avein/warp`

> Single source of truth for how this personal fork of `warpdotdev/warp`
> absorbs upstream changes, keeps its customisations alive, and gets
> verified before each new build replaces the daily driver.
>
> **Parent PRD:** [`docs/prd-fork-strategy.md`](./prd-fork-strategy.md).
> Per-slice issues live under
> [`docs/issues/fork-strategy-01..07-*.md`](./issues/).
>
> **Status:** Slices 01–07 landed. The procedure described below is
> fully executable end-to-end.

## 1. Topology

| Ref | Lives on | Purpose |
|---|---|---|
| `master` | `origin` *and* `upstream` | Strict fast-forward-only mirror of `warpdotdev/warp:master`. No commits of mine ever land here. Exists for bisect baselines, the weekly merge source, and GitHub's ahead/behind diff. |
| `personal/main` | `origin` only | The customisation branch — every commit of mine sits here, on top of `master`. Weekly upstream absorption uses a `--no-ff` merge of `master` into a candidate, then a fast-forward of `personal/main`. |
| `personal/sync/YYYY-MM-DD-candidate` | `origin` (transient) | Per-sync attempt branch. Created from `personal/main`, gets the weekly `master` merge resolved on it, the test suite run against it, and the `WarpOss.app` built from it. Promoted by fast-forwarding `personal/main`. Kept indefinitely on a failed sync, optionally deleted on success. |
| `personal/sync/YYYY-MM-DD-initial` | `origin` (permanent) | Tag — only created once, at bootstrap. The first rung of the rollback ladder before any `-post` tag exists. |
| `personal/sync/YYYY-MM-DD-pre` | `origin` (permanent) | Tag — `personal/main`'s tip *before* a sync's candidate is promoted. Used to compute the "diff this week's sync produced". |
| `personal/sync/YYYY-MM-DD-post` | `origin` (permanent) | Tag — `personal/main`'s tip *after* a sync's candidate is promoted. The rollback ladder is the chronological sequence of `-post` tags. |

Remotes (local):

- `origin` → `git@github.com:Avein/warp.git` (the fork).
- `upstream` → `https://github.com/warpdotdev/warp.git` (read-only).

## 2. Weekly procedure

Three moving parts: CI runs the absorption work unattended Monday
morning; a manual smoke gate decides whether to promote; a promote
workflow makes the candidate the new `personal/main`.

### 2.1 CI weekly sync (`.github/workflows/weekly-sync.yml`, Slice 06)

Cron: `0 8 * * 1` (Mondays 08:00 UTC) plus `workflow_dispatch` for
ad-hoc runs. Runs on `macos-14` (arm64). Sequence:

1. **Preflight.** Read the most recent `personal/sync/*-post` tag,
   compute weeks elapsed via the cadence-tier function (§4), log the
   tier. Tier is advisory in CI; never blocks.
2. **Fetch upstream.** Configure `upstream` remote, `git fetch upstream`.
3. **Fast-forward `master`.** `git push origin master:refs/heads/master`
   only if `--ff-only` succeeds.
4. **Branch candidate.** `git switch -c personal/sync/$(date -I)-candidate
   personal/main`.
5. **Merge `master`.** `git merge --no-ff master`.
   - **On conflict:** open Issue `sync-conflict: YYYY-MM-DD` with the
     conflicted file list and the policy reminder from §3. Halt
     non-zero. Nothing is pushed.
6. **Test.** `cargo nextest run` against the tier-(ii) selectors
   (pure-module unit tests from Slice 04 + persistence integration
   tests from Slice 05).
   - **On failure:** push the candidate branch (so it can be inspected
     remotely), upload test logs as a workflow artifact, open Issue
     `sync-test-fail: YYYY-MM-DD` linking artifact + branch. Halt
     non-zero. `personal/main` is not moved.
7. **Tag `-pre`.** `git tag personal/sync/$(date -I)-pre personal/main`.
8. **Build.** Run the OSS-channel bundle to produce `WarpOss.app`.
9. **Push candidate + `-pre` tag.** `git push origin <candidate>
   personal/sync/$(date -I)-pre`.
10. **Upload artifact.** `WarpOss.app` as a workflow artifact (90-day
    retention).
11. **Open `sync-ready` Issue** titled `Sync candidate ready: YYYY-MM-DD`
    with the artifact link, the smoke checklist from §6, and the
    one-line promote command:
    `gh workflow run promote-candidate.yml -f date=YYYY-MM-DD`.

The workflow uses three Issue labels — created idempotently on first
run, so manual repository setup is not required:

| Label | Opened when | Title pattern |
|---|---|---|
| `sync-conflict` | `git merge --no-ff master` hits a conflict | `Sync conflict: YYYY-MM-DD` |
| `sync-test-fail` | tier-(ii) `cargo nextest` gate fails | `Sync test failure: YYYY-MM-DD` |
| `sync-ready` | clean run; candidate + artifact ready to smoke | `Sync candidate ready: YYYY-MM-DD` |

### 2.2 Manual smoke gate

The developer downloads the candidate `WarpOss.app` from the
`sync-ready` Issue, installs via the §6 procedure, walks the §6 smoke
checklist. The candidate either passes (→ promote) or fails (→ rollback
or leave the candidate for further investigation, do not promote).

### 2.3 Promote workflow (`.github/workflows/promote-candidate.yml`, Slice 07)

`workflow_dispatch`-only with a required `date` input. Runs on
`ubuntu-latest`. Sequence:

1. Verify `personal/sync/${date}-candidate` exists.
2. Verify `personal/main → ${date}-candidate` is a fast-forward.
3. Download the candidate's `WarpOss.app` workflow artifact.
4. Fast-forward `personal/main` to the candidate's tip; push.
5. Tag `personal/sync/${date}-post` on the new tip; push the tag.
6. Create GitHub Release `v${date}` with `WarpOss.app` attached.
7. Close the corresponding `sync-ready` Issue with a comment linking
   the Release.

Thin local wrapper: `scripts/promote-candidate.sh` (Slice 07) resolves
the date from the most recent candidate branch on `origin` by default;
`--date YYYY-MM-DD` overrides.

### 2.4 Local fallback (`scripts/weekly-sync.sh`, Slice 02)

Same logical sequence as 2.1, run locally for when CI is down or an
immediate mid-week sync is needed. Refuses to run on a dirty working
tree or from any branch other than `personal/main`. The smoke checklist
is an interactive y/N per item. Tier-detection warnings are advisory;
`--yes` bypasses the `FreshStart` prompt.

## 3. Conflict policy

The CI workflow **never auto-resolves**. On any merge conflict it
halts, opens a `sync-conflict` Issue, and waits for a human. The
human (= me) resolves under this policy:

### 3.1 Default: favor upstream

Start every conflict from `git checkout --theirs <path>`. If the local
feature still needs to exist, re-implement it on top of upstream's new
shape inside the same merge commit.

**Why:** the fork's customisation is scoped to one subsystem
(projects). Outside that subsystem, my code is incidental — upstream's
shape is canonical. Defaulting to upstream avoids progressive
divergence that would make every subsequent sync more expensive.

### 3.2 Exception 1 — documented invariant

If upstream's change breaks a behaviour the fork explicitly guarantees
in one of:

- [`docs/projects-persistence.md`](./projects-persistence.md)
- [`docs/issues/projects-persistence-0[1-4]-*.md`](./issues/)
- [`docs/projects-rename-summary.md`](./projects-rename-summary.md)

…then the resolution favours the local guarantee. The implementation
is migrated to upstream's new substrate; the documented behaviour
must remain observable in the smoke checklist (§6).

**Worked example:** if upstream restructures `PersistenceWriter` so
the `terminate()` join order changes, the guarantee "save before
`PersistenceWriter::terminate()`" (from
`projects-persistence-04-save-on-app-terminate.md`, pinned by commit
`004fbc98`) must remain — even if the call has to move to a different
callback.

### 3.3 Exception 2 — pure feature removal

If upstream deletes a behaviour the fork actively depends on (e.g.
removes a public method, deprecates a global action, retires an enum
variant), re-add the removed behaviour as a **private helper inside
the dependent module**. Do not re-add it as a fork-of-upstream public
API; the goal is to keep the fork's blast radius local.

**Worked example:** if upstream removes
`Workspace::display_name(view_id, switcher)` after migrating its own
call sites elsewhere, the rename feature's resolver
(`resolve_workspace_display_name`, pinned by commit `8de00657`) keeps
the helper private inside `workspace/view.rs`. Upstream's new public
API is preferred for all *other* call sites that happen to fit it.

## 4. Cadence tiers

The cost of catching up scales non-linearly with the gap since the
last successful sync. The cadence-tier function lives at
`scripts/lib/cadence.sh` (pure Bash, with `scripts/lib/cadence-test.sh`
as its test runner) and classifies the upcoming sync:

| Weeks since last `-post` | Tier | Treatment |
|---|---|---|
| 0–1 | `Normal` | Run the standard procedure. ~5 min if upstream did not touch the projects subsystem; ~30 min if it did. |
| 2–4 | `DoubleBudget` | Run the standard procedure. Budget 30–60 min — conflicts compound. Advisory message in preflight only; no prompt. |
| 5–12 | `IncrementalCatchup` | Sync against successive upstream stable cuts (`upstream/cherrypick/stable_release/*`) instead of one large `master` merge. Resolve conflicts per cut, not all at once. Advisory message in preflight. |
| 13+ | `FreshStart` | Stop. The integration cost has exceeded the value of incremental absorption. Cherry-pick the 40 customisation commits onto fresh `upstream/master` as a new `personal/main-v2`; tag the abandoned tip; switch the daily driver. Interactive y/N prompt in preflight; `--yes` bypasses. |

The function is pure (`(today_iso, last_post_iso) → Tier`) and
unit-tested on both sides of every boundary by
`scripts/lib/cadence-test.sh` (13 assertions: the four boundary
weeks 1/4/12/13, the eight off-by-one days flanking each, plus the
empty-input case). Both `scripts/weekly-sync.sh` and the CI workflow
source the same module — there is no second implementation.

The boundary numbers come from the PRD; do not negotiate them in this
document — change the PRD if a different cadence proves correct after
a few months of live operation.

## 5. Rollback procedure

The rollback ladder is the chronological sequence of
`personal/sync/*-post` tags on `origin`. Rollback walks one rung back.

Local recipe (the `scripts/rollback-last-sync.sh` helper from Slice 02
automates this; the recipe below is what the script encapsulates so
the manual fallback is also documented):

```sh
# 1. List the ladder, newest first.
git tag --list 'personal/sync/*-post' --sort=-creatordate

# 2. Identify the second entry (the previous good post).
PREV=$(git tag --list 'personal/sync/*-post' --sort=-creatordate | sed -n '2p')

# 3. Inspect the diff between current tip and the rollback target.
git log --oneline --no-merges $PREV..personal/main

# 4. Reset and force-with-lease push.
git switch personal/main
git reset --hard "$PREV"
git push --force-with-lease origin personal/main
```

**`--force-with-lease`, never `--force`.** The lease check protects
against rolling back over a sync somebody else (or a forgotten CI run)
pushed since the local view was last refreshed.

If fewer than two `-post` tags exist, the rollback target is the
`personal/sync/*-initial` tag from bootstrap; the script refuses
automatically in that case and surfaces the situation as a manual
decision.

## 6. Smoke checklist

Five manual actions, walked in order on the candidate `WarpOss.app`
before promoting. Each action exercises one of the documented
behaviours the fork guarantees. A single No anywhere = do not promote.

The unsigned-binary install prelude:

```sh
xattr -dr com.apple.quarantine ~/Downloads/WarpOss.app
mv ~/Downloads/WarpOss.app /Applications/
open /Applications/WarpOss.app
```

### Smoke 1 — Palette open and search

Cmd⇧P (or the configured projects-palette shortcut). Type a fragment
that matches **only one project's override name** (set in a previous
session via F2). Confirm:

- Both override-name and identity-name rows match in the Open section.
- Highlighted characters track the displayed (override) name.
- Press Enter on the highlighted row → project-tab opens in the
  current window.

Source: dual-search matcher, commits `42fe134f` + `8de00657`;
`docs/projects-rename-summary.md` "Palette dual-search".

### Smoke 2 — F2 inline rename

With a project-tab active:

- Press F2 → in-pill editor appears, pre-selected with the current
  display name.
- Type a new name → Enter → pill updates to the new name.
- Press F2 again → empty the buffer → Enter → pill reverts to the
  identity name (override cleared).

Source: F2 binding + editor lifecycle, commits `4e9ef6ff` +
`0311bb16`; `docs/projects-rename-summary.md` "F2 binding".

### Smoke 3 — Project-tab persistence across restart

In a single OS window:

- Open project-tab A via Smoke 1's path.
- Rename it via Smoke 2 to "smoke-rename-N".
- Open project-tab B via the new-project popup (`⌘⇧N` → some path).
- Close project-tab B (leaving A active).
- Quit with `⌘Q`.
- Relaunch from Dock.

Expect: project-tab A is back with its "smoke-rename-N" label;
project-tab B is gone. The same window contains A.

Source: persistence save-trigger fixes, commits `004fbc98`,
`a2ddd467`, `e9144fe2`; `docs/projects-persistence.md`; tier-(ii)
integration tests in Slice 05.

### Smoke 4 — Synthetic root on empty new-window

With no windows open (close them all first):

- Trigger a new-window entrypoint: `⌘N`, the Dock icon click, or
  `warp` from the shell.

Expect: the new window opens with a synthetic `root` workspace
pre-spawned and ready to accept input — no empty palette, no zero
workspaces.

Source: synthetic-root auto-spawn, commit `6b17abbd` ("spawn synthetic
root from every new-window entrypoint when zero windows") and
`9873f533` ("synthetic root auto-spawn + palette Available entry").

### Smoke 5 — Project-bar order across restart

In a single OS window, open three project-tabs in a known order
(A → B → C). Drag them into a different order (e.g. C → A → B).
Quit with `⌘Q`. Relaunch from Dock.

Expect: the strip shows C → A → B in that exact order; no
re-ordering on restore.

Source: strip-order preservation, commit `3fdf62cb`
("preserve project-tab strip order across restart"); tier-(ii)
integration test in Slice 05.

## 7. Bootstrap recipe

How this fork was first set up (Slice 01, completed 2026-06-04). Kept
here so a future re-bootstrap (e.g. to a new machine) is reproducible
without re-reading the PRD.

```sh
# 1. On the GitHub web UI: Fork warpdotdev/warp into Avein/warp.

# 2. Locally, in an existing clone of warpdotdev/warp:
git remote rename origin upstream
git remote add origin git@github.com:Avein/warp.git

# 3. Mirror master to the fork.
git fetch upstream
git switch master
git merge --ff-only upstream/master
git push -u origin master

# 4. Rename the customisation branch and push it verbatim.
git branch -m feat/projects-palette personal/main
git push -u origin personal/main      # 40 commits, no rewrites

# 5. Tag the rollback baseline.
git tag personal/sync/$(date -I)-initial personal/main
git push origin personal/sync/$(date -I)-initial

# 6. Land this document on personal/main, then the supporting PRDs.
#    (This step is what you're reading.)

# 7. As subsequent slices land, they add: tier-(ii) tests (Slices 04, 05),
#    the cadence-tier module (Slice 03), the local scripts (Slices 02, 07),
#    and the two GitHub Actions workflows (Slices 06, 07).
```

The bootstrap mutates four public refs only: `origin/master`,
`origin/personal/main`, the `-initial` tag, and the two doc commits.
Everything else is local-only state.

## 8. CI quota budget

Math for the GitHub Actions free tier on public repos:

- Free-tier ceiling: 2000 minutes/month.
- `macos-14` multiplier: **×10** (so 1 minute of wall time costs 10
  minutes of quota).
- Expected weekly-sync wall time: ~30 minutes.
- Effective monthly cost: 4 syncs × 30 min × 10 = **1200 effective
  minutes/month**. Headroom: ~800 effective minutes/month for ad-hoc
  `workflow_dispatch` runs, conflict iterations, and the
  promote-candidate workflow (which runs on `ubuntu-latest`,
  multiplier ×1, negligible).

Promote-candidate runs on Linux precisely because it does only git +
GitHub API operations — no build, no multiplier hit. If a future slice
wants to re-run the build on promote (e.g. to add code-signing), it
should stay on `macos-14` and the quota math must be re-checked.

If the budget tightens, two levers exist:

1. Drop the build step from CI; build locally before promoting. Saves
   ~25 wall minutes / ~250 quota minutes per sync.
2. Skip syncs in `DoubleBudget` weeks where upstream did not touch the
   projects subsystem (detect via path-filtered `git log`). Saves a
   whole sync's worth of quota.

Neither lever is engaged by default; the current budget has comfortable
headroom.
