# PRD — Project Origin Simplification

> Personal fork feature (branch `feat/projects-palette`). Sibling to
> [`projects-tabs-redesign.md`](./projects-tabs-redesign.md),
> [`projects-handoff.md`](./projects-handoff.md),
> [`projects-glossary.md`](./projects-glossary.md), and
> [`projects-ui-polish.md`](./projects-ui-polish.md). Collapses the four-variant
> `ProjectOrigin` model down to two and deletes the special-casing that grew up
> around `Default` and `Root`.
>
> **Status:** spec approved (grilled 2026-06-02), implementation pending.

## Problem Statement

After landing the projects-as-tabs work, the model has accumulated four origins
(`Config`, `Template`, `Default`, `Root`) with overlapping semantics:

- `Default` is "the path-less default template applied at a path" — i.e. the
  same mechanism as `Template`, just gated to the built-in `default.yaml`. It
  carries a different restore-naming policy (re-derive from cwd) and a different
  icon, but the user-facing distinction is invisible.
- `Root` is "the startup workspace at `~`" — really just an instance of the
  default template auto-stamped on boot. The only thing that makes it special is
  *when* it's spawned (once per session, before any user input).
- The naming derivation for `Default`-origin tabs (cwd basename) collides across
  unrelated projects (`~/work/api`, `~/play/api`) — which is why
  `disambiguate_names` exists at all.
- Three entry points (`cmd+shift+N`, `newds`, palette → template) all converge
  on the same underlying mechanism but produce different origin stamps depending
  on which path you came in through.

The result is a codebase with parallel branches for each origin in restore
policy, palette rendering, icon picking, and dedupe — for a distinction the
user never sees and never asked for.

## Solution

Collapse the four-variant enum down to two: **`Config`** (saved YAML with baked
`cwd`s) and **`Template`** (a path-less blueprint applied at a path at open
time). Everything ad-hoc — `Default`, `Root`, `newds`, `cmd+shift+N` — becomes
`Template(default)` applied at the supplied path. The startup root workspace
becomes a synthetic `Config(root)` at `~`, auto-spawned only when persisted
state is empty.

Template instances get a globally-unique sequence name per template
(`default-1`, `default-2`, `simple_template-1`, …) with gap-fill on close.
Names no longer derive from the directory basename, which eliminates the only
collision mode `disambiguate_names` was built to handle.

Configs continue to dedupe globally by config name. Templates dedupe globally
by `(template_name, path)` — re-opening at the same path focuses the existing
tab. Renames continue to be display overrides via the `project_display_names`
table planned in [`projects-ui-polish.md`](./projects-ui-polish.md) Step 4.

## User Stories

1. As a developer reading the codebase, I want exactly two project origins so
   that I can reason about palette rendering, persistence, and dedupe without
   tracking four parallel branches.
2. As a user opening the default template via `cmd+shift+N`, I want my new tab
   named `default-1` (or the next free slot) so that opening multiple ad-hoc
   projects at different paths produces distinguishable names without ` —
   <parent-dir>` suffixes.
3. As a user opening the default template via `cmd+shift+N` at a path that's
   already open as a `default-*` tab, I want focus to jump to that tab so that
   I don't end up with two tabs at the same directory.
4. As a user opening a saved Config (e.g. `dotfiles`) when it's already open in
   some window, I want focus to jump to that workspace so that I'm never asked
   to choose between identical-looking duplicates.
5. As a user with a saved path-less template called `simple_template.yaml`, I
   want each open to spawn a tab named `simple_template-N` so that the
   sequence-naming convention is consistent across all templates, not just
   default.
6. As a user closing a `default-2` tab and later opening the default template
   again, I want the next tab named `default-2` (gap-fill) so that the sequence
   stays compact.
7. As a user launching the app for the first time (or after wiping state), I
   want a single workspace named `root` at my home directory so that the app
   doesn't open empty.
8. As a user who closes the root tab, I want to be able to reopen it from the
   projects palette's Available section so that root isn't a one-shot thing.
9. As a user pressing `cmd+n`, I want a new OS window with a fresh template
   tab — the existing behavior — so that nothing about the keyboard shortcut
   changes.
10. As a user opening `newds <path>` from the shell, I want it to dedupe
    identically to `cmd+shift+N` at that path so that the GUI and CLI entry
    points produce the same observable behavior.
11. As a user opening the projects palette, I want one icon per origin (Folder
    for Config, LayoutAlt01 for Template) so that the visual vocabulary stays
    minimal and matches what's already in the project bar.
12. As a user, I want the root tab to look identical to any other Config tab
    (same Folder icon, same chrome) so that there's no visual asymmetry — its
    name "root" already signals what it is.
13. As a developer auditing the palette code, I want `disambiguate_names` and
    its 4 unit tests gone so that the open-projects render path is straight-line.
14. As a user on first launch after this change ships, I'm OK with my existing
    persisted state being wiped so that the implementation avoids a migration
    shim and starts clean.
15. As a user, I want my renamed projects (when the rename feature lands) to
    persist as display overrides keyed by `(origin, canonical_path)` so that
    rename works uniformly across both origins and across restart.
16. As a developer extending the open flow later (e.g. a "save current
    workspace as template" feature), I want the identity model rooted in two
    origins so that I don't have to consider four-way branches for new code.
17. As a developer writing tests, I want `template_sequence` exposed as a pure
    function so that I can unit-test gap-fill, multi-template, and edge cases
    without spinning up an app context.
18. As a developer writing tests, I want `identity_dedupe` exposed as a pure
    lookup over a stamp map so that I can unit-test Config-name lookup and
    `(template_name, path)` lookup without an app context.
19. As a user with the `default.yaml` template file deleted from
    `~/.warp-oss/launch_configurations/`, I want the app to fall back to a
    built-in synthetic "single pane, plain shell" default so that the default
    template entry points never fail.
20. As a user who customizes `~/.warp-oss/launch_configurations/default.yaml`
    (commands, panes, layout), I want my edits respected when the default
    template is opened so that the existing power-user knob keeps working.
21. As the next agent picking up this branch, I want the docs
    (`projects-glossary.md`, `projects-tabs-redesign.md`) updated in lockstep
    with the code so that vocabulary doesn't drift between source and spec.

## Implementation Decisions

### Origin enum collapse

The `ProjectOrigin` enum collapses from 4 variants to 2, both carrying their
identifying string:

```rust
pub enum ProjectOrigin {
    Config { config_name: String },
    Template { template_name: String },
}
```

(Encoded from the grill — this is the schema, not a code snippet.) The variants
that disappear:

- `Default` → replaced everywhere by `Template { template_name: "default" }`.
- `Root` → replaced by a synthetic `Config { config_name: "root" }`.

`ProjectIdentity` keeps its `{ name, path, origin }` shape but `path` becomes
**required (non-`Option`)** for both variants — root's path is `~`, every
template instance has a known path, and every Config has its baked `cwd`. (The
old `path: None` was only ever used for the root case.)

### Identity & dedupe keys

| Origin | Dedupe key |
|---|---|
| `Config { config_name }` | `config_name` (global, regardless of path) |
| `Template { template_name }` | `(template_name, path)` (global) |

Re-opening through `focus_or_spawn_project` consults the relevant key and
focuses the existing live workspace if found, else spawns a new one. The root
tab is a `Config` with `config_name == "root"` and shares the same dedupe rule
— it is genuinely a singleton.

### Sequence naming for templates

Template instance display names are auto-allocated by a new pure module
`template_sequence`. Interface:

> `next_template_sequence_name(template_name, in_use_names) -> String`

Returns `<template_name>-N` where `N` is the smallest positive integer such
that the resulting string is not in `in_use_names`. Implementation generalizes
the existing `next_default_name` in `root_view.rs`. Gap-fill is preserved.
Pure — no app context, no FS access. Lives in
`app/src/workspace/template_sequence.rs` (new file).

### Identity dedupe lookup

A new pure module `identity_dedupe` exposes:

> `find_live_workspace(identity, stamps, is_live_fn) -> Option<EntityId>`

Where `stamps` is the existing `HashMap<EntityId, ProjectIdentity>` and
`is_live_fn` is a callback that filters by `WorkspaceRegistry` liveness.
Routes by origin variant: `Config` matches on `config_name`; `Template` matches
on `(template_name, path)`. Lives alongside `template_sequence`. Pure — no app
context internal; the liveness check is injected.

### Synthetic root auto-spawn

On app startup, after the persistence layer loads its windows snapshot, if the
total live workspace count across all windows is zero, the app spawns one OS
window containing one workspace stamped as
`Config { config_name: "root" }` with `path = ~`. The session inside is a
single plain shell pane. No `root.yaml` file exists or is read — root is
purely runtime-synthetic.

If the root workspace is closed during a session, the user can reopen it from
the projects palette's Available section (a synthetic Config entry is rendered
whenever root is not currently among the live stamps).

### `cmd+n` flow (unchanged)

`cmd+n` continues to open a new OS window whose starting workspace is stamped
as `Template { template_name: "default" }`. Its name is allocated via
`next_template_sequence_name`. No special-case code paths added; the existing
auto-stamp wiring is re-pointed at the new enum.

### `default.yaml` handling

`~/.warp-oss/launch_configurations/default.yaml` continues to be a real,
user-editable YAML file with no baked `cwd` (per existing layout-only template
semantics). If the file is missing or fails to parse, the app falls back to a
built-in synthetic "single session-tab, single pane, plain shell at the
supplied path." All template entry points (`cmd+shift+N`, `newds`, palette →
default, `cmd+n`) use this resolved template definition.

### Palette `Available` section

Listed rows:
- Saved `Config`s not currently open.
- All saved `Template`s (always — selecting one opens a new instance at the
  active window's cwd, subject to dedupe).
- A synthetic `root` entry when root is not currently open.

`disambiguate_names` is deleted entirely. The `OpenRow` rendering path no
longer mutates names.

### Restore policy

The Default-re-derives-from-cwd branch in
`configure_new_workspace` is removed. All restored stamps use their persisted
name verbatim. Since state is wiped on first launch after this lands, no
in-the-wild legacy records survive.

### Persistence

A new SQLite migration (`<date>_wipe_windows_for_origin_simplification`)
empties the `windows` table on first launch after rollout. No serde shim, no
backward-compat for the old enum. The `project_identity` column's JSON shape
is updated to reflect the new enum.

### Icon mapping

Two icons only:
- `Config` → `Icon::Folder` (regardless of `config_name`; root included).
- `Template` → `Icon::LayoutAlt01` (regardless of `template_name`).

The duplicated mapping in `app/src/workspace/project_tab.rs` and
`app/src/search/command_palette/projects/search_item.rs` is consolidated into
a single helper.

### Docs

`docs/projects-glossary.md` and `docs/projects-tabs-redesign.md` are updated
in the same change set to reflect the new vocabulary. The historical
`projects-redesign.md` is left as-is (already marked superseded). This PRD
itself becomes a sibling under `docs/`.

## Testing Decisions

Good tests for this change observe **external behavior only**: identity-keyed
dedupe outcomes, sequence-name allocation results, persistence round-trip
JSON shape, restore-from-empty-state behavior. They do not assert on internal
HashMap layouts or call ordering.

Modules to be unit-tested:

1. **`template_sequence`** — `next_template_sequence_name`:
   - Empty `in_use_names` returns `<template>-1`.
   - Contiguous set `{default-1, default-2, default-3}` returns `default-4`.
   - Gap set `{default-1, default-3}` returns `default-2` (gap-fill).
   - Mixed-template set `{default-1, simple_template-1}` asked for `default`
     returns `default-2`; asked for `simple_template` returns
     `simple_template-2`.
   - Renamed tabs (names that don't match the `<template>-N` pattern) are
     ignored — `{dragon-fire, default-3}` asked for `default` returns
     `default-1`.

   Prior art: the existing `next_default_name` is unit-tested implicitly via
   `view_tests.rs` integration tests; this module pulls the logic out so it
   can be tested as a pure function.

2. **`identity_dedupe`** — `find_live_workspace`:
   - Config lookup matches on `config_name` regardless of path.
   - Template lookup matches on `(template_name, path)` jointly — same
     template at a different path is not a match.
   - Closed workspaces filtered out via the injected `is_live_fn`.
   - Mixed-origin stamp set with the same string in `config_name` and
     `template_name` returns the correct match for each lookup direction.

   Prior art: the existing `workspace_for_name_filtered` /
   `projects_mru_filtered` pure helpers in `project_switcher.rs` (with their
   7 unit tests) — same testing style.

3. **`ProjectSwitcher` integration** — black-box behavior through the public
   API:
   - Stamping a Config + stamping a Template + looking up each by identity.
   - MRU touch order preserved across stamp/lookup interleavings.
   - `forget()` removes both stamp and MRU entry.

   Prior art: existing tests in `project_switcher.rs::tests` mod
   (`claim_root_succeeds_only_once`, etc.) — extend that mod.

4. **Persistence round-trip** — extend
   `test_sqlite_round_trips_project_identity` in `persistence/sqlite_tests.rs`:
   - A `Config { config_name: "echo" }` round-trips intact.
   - A `Template { template_name: "default" }` round-trips intact.
   - The wipe migration on a database with legacy records (synthesized by
     hand for the test) leaves the windows table empty.

   Prior art: the existing round-trip test in `sqlite_tests.rs`.

No integration / end-to-end test work is in scope for this PRD; manual
verification of the open flow, project bar, and palette is handled per
`projects-handoff.md`'s build/run section (`warpfresh --build`).

## Out of Scope

- **The rename feature itself** (`F2` / double-click inline editor +
  `project_display_names` table). Tracked in `projects-ui-polish.md` Step 4.
  This PRD assumes its identity key is `(origin, canonical_path)` and that's
  it — the rename UI is a separate change set.
- **Phase 6 Merge Windows** — still deferred per `projects-tabs-redesign.md`.
- **`newds` shell helper changes** — `~/personal/dotfiles/zsh/.config/.zsh/aliases.zsh`
  is left untouched; its `warposs://action/new_default_session?path=…` URI
  contract is preserved. The handler resolves the URI to
  `Template { template_name: "default" }` internally.
- **Quick-launch shortcuts (⌘-1..9)** — out of scope; not affected by this
  change.
- **Vertical project bar** — out of scope.
- **Customizing root** — root is purely synthetic. There is no `root.yaml`.
  If you want a customized startup project, save a regular Config and rename
  it after opening (once the rename feature lands).
- **Multi-OS-window persistence grouping** — `host_group_id` columns from the
  original spec stay deferred. Restored state still collapses into one
  window. Out of scope here.
- **Same-basename disambiguation as a rename-collision fallback** — explicitly
  rejected. If a rename produces a visible name collision, the user fixes it
  via another rename.

## Further Notes

- **Wipe on first launch is deliberate.** The user has no production data
  worth migrating on this branch; eliminating the migration shim saves real
  implementation complexity for a one-time cost.
- **Persistence schema bumping.** The `project_identity` JSON shape changes
  (new variant tags). The wipe migration ensures no legacy row needs to
  deserialize against the new shape.
- **`focus_or_spawn_project` already exists** and routes by `is_template()` on
  the launch config. The change here is to additionally route by the resolved
  `ProjectOrigin` variant for the dedupe-lookup step, not just at the spawn
  step.
- **Two pure modules (`template_sequence`, `identity_dedupe`)** are the
  primary opportunity to extract deep, testable behavior. Both have tiny
  surfaces (one function each) and zero app-context dependencies — they
  should age well even if the surrounding plumbing churns.
- **Icon consolidation** is small but worth doing in the same change to avoid
  the new two-variant mapping being duplicated in two files from day one.
- **The grill transcript** that produced this PRD lives in the conversation
  history of the agent that drafted it — not persisted in repo. Decision
  rationale is captured in the body of this doc; the "Q3 forks rejected"-style
  detail is intentionally left out.
