# PRD — Project Tab Rename

> Personal fork feature (branch `feat/projects-palette`). Sibling to
> [`projects-tabs-redesign.md`](./projects-tabs-redesign.md),
> [`projects-origin-simplification.md`](./projects-origin-simplification.md),
> [`projects-handoff.md`](./projects-handoff.md),
> [`projects-glossary.md`](./projects-glossary.md), and
> [`projects-ui-polish.md`](./projects-ui-polish.md). Adds a per-tab
> display-name override so users can label open project-tabs by what they
> *are about* instead of by the auto-generated identity name.
>
> **Supersedes** [`projects-ui-polish.md`'s Step 4](./projects-ui-polish.md#step-4--persisted-project-rename),
> which was sketched before the four-variant origin enum collapsed to two and
> is now half-stale. This PRD is the fresh design built ground-up from the
> post-simplification code state.
>
> **Status:** spec approved (grilled 2026-06-02), implementation pending.

## Problem Statement

After [origin simplification](./projects-origin-simplification.md), every
project-tab is named by exactly one of three mechanisms — none of which the
user controls at display time:

- `Config { config_name }`: the literal `config_name` from the YAML. The
  user controls it at config-save time, but updating it means editing a
  YAML file for a UI display preference.
- `Template { template_name }`: a sequence slot `<template>-N` allocated by
  `template_sequence::next_template_sequence_name` at open time. The user
  has zero control over the slot.
- The synthetic root: the literal string `"root"`. No control.

A common Templates workflow makes the pain visible quickly: `cmd-shift-N
~/work/api` → tab labeled `default-1`. Next day, `cmd-shift-N
~/work/frontend` → `default-2`. By Friday the project bar reads `default-1
default-2 default-3 default-4 root` and the user is reading file trees to
remember which is which.

For Config tabs the YAML-edit workaround works but is heavy. For Templates
and root there is no workaround at all.

## Solution

A per-workspace display-name override:

- **Scope: per open tab** (workspace-scoped). The override is a property of
  the open workspace, not the project's identity. Close the tab → override
  dies. Reopen the same identity later → fresh default name.
- **Trigger: double-click on the active tab pill, or `F2` while a
  project-tab is focused.** Pill becomes an in-place text editor.
- **Commit: Enter or click-outside. Cancel: Esc. Empty string ⇒ clears the
  override** (pill snaps back to the default name).
- **Read everywhere a tab's name shows:** the project bar pill, the projects
  palette open-projects row, the Alt+Tab row. Palette fuzzy search matches
  against *both* the override and the original identity name so a renamed
  tab is findable by either label.

The override does not affect the dedupe key, the template-sequence
allocator, or the project identity stamp — it is a pure display layer.

## User Stories

1. As a user with `cmd-shift-N ~/work/api` open as `default-1`, I want to
   rename the pill to `api-prod` so the project bar reads what the project
   is about, not how it was opened.
2. As a user who renamed `default-1` to `api-prod`, I want to find that tab
   in the projects palette by typing either `api-prod` or `default-1` — so
   I can switch to it even when I half-remember the rename.
3. As a user who opened the same `dotfiles.yaml` Config in two different
   work sessions (different days), I want each tab to start labeled
   `dotfiles` regardless of any rename I did the previous day — because
   rename is for the open tab, not the project identity.
4. As a user who renamed `default-1` to `api-prod`, then opened a second
   ad-hoc tab via `cmd-shift-N ~/work/frontend`, I want the second tab named
   `default-2` (not `default-1`, the slot I "freed" by renaming) — renames
   don't reshuffle the sequence allocator.
5. As a user who quit Warp with `default-1 → api-prod` on the bar, I want
   the rename to come back on relaunch — the workspace is persisted; the
   override travels with it.
6. As a user who closes `api-prod` via the palette's secondary action and
   later opens `~/work/api` again, I'm OK with the new tab starting as
   `default-N`. The rename was per-tab; the tab is gone.
7. As a user who renames the synthetic root to "Home", I want the rename to
   apply while it's open, and I accept that the next time the synthetic
   root auto-spawns on empty state it will be back to `"root"`.
8. As a user, I want F2 and double-click to behave identically — the only
   difference is keyboard vs. mouse; the editing experience is the same.
9. As a user, I want my rename to commit if I click somewhere else (palette,
   another tab, an editor pane) — if I clicked away I'm done editing.
10. As a user who clears the rename field and hits Enter, I want the tab to
    revert to its original auto-generated name — "empty string" is the
    universal "go back to default" gesture.
11. As a developer auditing the codebase, I want there to be exactly one
    function — `Workspace::display_name(...)` — that everywhere-that-shows-
    a-name calls, so the override is impossible to accidentally bypass.

## Implementation Decisions

### Storage

A single nullable column on the existing `windows` table:

```sql
display_name_override TEXT NULL
```

Added via an `ALTER TABLE` migration; existing rows get `NULL`. No new
table, no FK, no separate persistence path. The override is part of the
`WindowSnapshot` shape that already round-trips per
`app_state.rs::get_app_state` and `open_from_restored`.

The state-wipe migration from origin-simplification already empties
`windows` on first launch after that change ships; this PRD's migration is
layered on top and applies to the freshly-empty table.

### In-memory model

A field on `Workspace`:

```rust
pub struct Workspace {
    // ...existing fields...
    /// User-typed display-name override for this workspace. `None` means use
    /// the identity's stamped name.
    display_name_override: Option<String>,
}
```

Resolved at read time via:

```rust
impl Workspace {
    pub fn display_name(&self, switcher: &ProjectSwitcher) -> String {
        if let Some(o) = self.display_name_override.as_deref() {
            return o.to_string();
        }
        switcher
            .identity(self.id())
            .map(|i| i.name.clone())
            .unwrap_or_else(|| "project".to_string())
    }
}
```

Override-first. The identity's `name` field is the canonical/default
(`default-1`, `dotfiles`, `root`). The override, when present, replaces it
for all display purposes.

### `template_sequence` does NOT consult overrides

`template_sequence::next_template_sequence_name(template_name, in_use_names)`
sources `in_use_names` from the **identity** name of each stamped workspace,
not from the display name. This is deliberate: a rename does not free up
the `default-N` slot it visually replaced — opening another template at a
new path picks the next free slot above the highest in-use identity name.
Otherwise renames would reshuffle the allocator and produce surprise gaps
in the live sequence.

This is the only subtle invariant in the design; every other piece is
local. The enforcement point is the `in_use_names` source in
`focus_or_spawn_project`, which must pass `switcher.identity(id).name`,
NOT `workspace.display_name(...)`.

### Trigger

Two paths, both dispatching the same `WorkspaceAction::RenameProjectTab`
action:

- **`F2`** while a project-tab is focused. Scoped to the `Workspace`
  context, same pattern as the existing `F3` binding for project-bar
  visibility. Mac-only — `view_components/find.rs` already binds F2 to
  "find next" on Linux/Windows; we don't shadow that there.
- **Double-click** on the *already-active* project-tab pill. The first
  click is the existing single-click-activate; the second click on the
  now-active tab opens the editor. Matches macOS Finder's "click selects,
  click on selected renames" gesture. Cross-platform.

The action handler stores the editing workspace's `EntityId` in a field on
`Workspace` so `render_project_bar` knows to render the editor in place of
the pill's label.

### Editor UX

- **Pre-populated with the current display name** (override if set, else
  the identity name). What you see on the pill becomes what's editable.
- **Select-all on entry.** Type to immediately replace; arrow keys to
  position the cursor and edit. Matches Finder rename, VS Code, Chrome tab
  rename — every familiar "rename this label" gesture.
- **Commit: Enter or click-outside.** Persists the trimmed buffer as the
  override.
- **Cancel: Esc.** Editor closes, no change.
- **Empty-string commit** clears the override (`display_name_override =
  None`). The pill snaps back to the identity name.
- **Trim** leading/trailing whitespace on commit; internal whitespace
  preserved as typed.
- **No validation:** no length cap, no character restrictions, no duplicate
  detection across tabs. Two tabs may both display `"api"`; the user chose
  the collision.
- **No live preview:** the pill width adjusts to fit the editor's content
  during edit, but other surfaces (palette, Alt+Tab) only update on commit.

If the workspace closes mid-edit (e.g. last session-tab closes the
workspace out from under the editor), the editor dies with it — no commit,
no persistence, no special teardown beyond the workspace's normal close
path.

### Surfaces

Three call sites read the override; all go through
`Workspace::display_name(...)`:

1. **Project-bar pill** (`workspace/project_tab.rs`): the label span
   sources from `display_name(...)`.
2. **Projects palette open-projects row** (`search/command_palette/projects/
   data_source.rs::run_query`): when building `OpenRow.name` for stamped
   workspaces, source from `display_name(...)`.
3. **Alt+Tab row**: same data source, same code path.

The macOS window title is currently a static `"Warp"` and is **not**
modified by this PRD.

### Palette fuzzy search

In `data_source.rs::open_window_section`, the fuzzy matcher's input is
extended from `&row.name` to the *union* of `[row.name,
row.identity_name]`, where `row.identity_name` is the original
`ProjectIdentity.name` (always present for stamped workspaces, `None` for
plain `cmd+n` tabs). The row appears if *either* string fuzzy-matches the
query; match indices are taken from whichever string produced the higher
score (for highlighting on the displayed `row.name`).

This lets the user find a renamed tab by either label without changing the
displayed name.

## Testing Decisions

Tests observe external behavior only:

1. **Persistence round-trip** — extend
   `test_sqlite_round_trips_project_identity` in
   `persistence/sqlite_tests.rs` with a workspace whose
   `display_name_override = Some("api-prod")`; assert it round-trips intact
   through save → load.

2. **Override-first resolution** — a unit test on `Workspace::display_name`
   covering: override set ⇒ override wins; override `None` ⇒ identity name
   surfaces; identity missing AND override `None` ⇒ `"project"` fallback.

3. **`template_sequence` ignores overrides** — an integration-level
   assertion that the `in_use_names` source in
   `root_view::focus_or_spawn_project` reads identity names, not
   `display_name(...)`. Documentation-style — a code-review check more
   than a runtime test.

4. **Palette search** — a test on the projects data-source query that
   stamps a workspace with identity name `"default-1"` and override
   `"api-prod"`, then asserts both `"default"` and `"api-prod"` fuzzy
   queries surface the row.

5. **Editor commit/cancel/clear** — pure-function tests on whatever helper
   computes the new `Option<String>` from the editor buffer + trigger
   (commit / cancel / empty-clear). The editor's render layer is
   integration-tested manually via `warpfresh --build`.

No new end-to-end harness work in scope; manual smoke via `warpfresh
--build` covers F2/double-click, click-outside, click-elsewhere,
restart-with-tab-open.

## Out of Scope

- **Identity-scoped persistence** (rename surviving close → reopen). This
  would need a dedicated `project_display_names` table keyed by `(origin,
  path)`. Revisit only if the per-tab scope feels too forgetful in
  practice — most likely candidate is the synthetic root, which closes
  and respawns frequently.
- **Symlink-equivalence on the path comparison.** The override is per-tab
  here, not path-keyed, so this question doesn't arise in scope. If we
  later promote to identity-scoped persistence, `identity_dedupe`'s path
  comparison and the override's lookup must agree on whatever
  normalization (if any) gets added — that's a single change touching
  both, deferred until needed.
- **Renaming a closed project from the palette's Available section.** The
  override is per open tab; an Available row has no live workspace to
  attach to. If the user wants to label a not-yet-open project, they open
  it first.
- **macOS window title.** Stays `"Warp"`. The rename is for the tab, not
  the window.
- **Validation / duplicate detection / length caps.** None of these.
- **Live preview in the palette while typing.** Pill width adapts during
  edit; other surfaces only update on commit.
- **Rewriting the underlying YAML on a Config rename.** The override is
  display-only; `dotfiles.yaml`'s `name:` field is untouched.
- **Rename via right-click context menu.** F2 and double-click cover the
  trigger surface; a context menu is an avenue for later if discoverability
  is still an issue.

## Further Notes

- **Per-tab vs identity-scoped was the most-debated decision.** The polish
  doc's Step 4 originally aimed for identity-scoped (rename survives
  close → reopen). The grilling session (2026-06-02) traded that for a
  simpler one-column-on-`windows` storage shape and accepted the
  consequence that synthetic-root renames evaporate on close. The upgrade
  path if that feels bad: add a `project_display_names` table, key by
  `(origin, path)`, and switch the override resolution from "field on
  Workspace" to "lookup at workspace-creation time, copy into the field."
  The display-resolution code stays the same; only the storage and
  trigger-on-commit paths change.

- **Why select-all and not cursor-at-end** (departing from
  `new_project_popup`): the popup is for *appending* to a path; rename is
  for *replacing* a label. Different gesture, different default. macOS
  Finder, VS Code, Chrome tabs all select-all on rename entry.

- **The grill transcript that produced this PRD** lives in the
  conversation history of the agent that drafted it — not persisted in
  repo. Decision rationale is captured in the body of this doc; the
  "rejected forks" detail (identity-scoped persistence, symlink
  resolution, validation rules, in-place vs floating editor) is left out
  of scope sections rather than enumerated as forks.

## Issue breakdown

Suggested vertical slices to land this in independently-grabbable issues
(mirrors the pattern from
[`projects-origin-simplification.md`](./projects-origin-simplification.md)):

1. **Schema + persistence.** `display_name_override TEXT NULL` column on
   `windows`; ALTER migration; `WindowSnapshot` field + serde; `Workspace`
   field. Round-trip test in `sqlite_tests.rs`.
2. **Display read-path.** `Workspace::display_name(switcher) -> String`
   method; flip the three call sites (project-bar pill, projects palette,
   Alt+Tab) to use it. Unit test on the resolution function.
3. **Palette search matches both names.** Extend the fuzzy matcher in
   `data_source.rs::open_window_section` to consult `[name,
   identity_name]`. Unit test on the data-source query.
4. **Inline editor on the pill + commit/cancel/clear.** The UI half:
   editor primitive on `project_tab.rs`, select-all on entry, Enter /
   click-outside / Esc / empty-string handling, trim on commit. Pure-
   function tests on the buffer-to-action mapper.
5. **Triggers + action.** `WorkspaceAction::RenameProjectTab`,
   `EditableBinding` for `F2` (mac-only, `Workspace` scope), double-click
   on already-active pill dispatches the same action. Wires #4's editor
   to live input.

Each issue has its own acceptance criteria; #5 depends on #4, #1 is
prerequisite to #2 and #3. #2 and #3 are independent.
