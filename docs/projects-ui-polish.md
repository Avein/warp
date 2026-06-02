# Projects UI Polish — Plan

> Sibling doc to [`projects-handoff.md`](./projects-handoff.md), [`projects-tabs-redesign.md`](./projects-tabs-redesign.md),
> and [`projects-glossary.md`](./projects-glossary.md). Captures the design decisions reached on
> 2026-05-28 for cleaning up the project-bar chrome, the projects / Alt-Tab popups, and adding a
> persisted project rename.
>
> **Scope:** UI/UX polish only. No new tabs concepts; no Phase 6 (Merge Windows) work; no churn to
> the per-workspace persistence beyond the new `project_display_names` table.

---

## Why this exists

After the Phase 1–5/7 work landed on `feat/projects-palette`, three problems remain:

1. **Project bar looks bolted on.** Its background, padding, and active-tab Accent fill (originally `root_view.rs::render_project_bar`, now `workspace::view::Workspace::render_project_bar`) make it read as a separate band glued above the workspace, not as part of the same chrome the session-tab bar lives in. *(Resolved — see "Built so far".)*
2. **Projects palette + Alt-Tab popup rows lack visual hierarchy.** Origin icon, name, path, branch and the "open" marker all render at similar weight; the "open" pill looks indistinguishable from the branch pill; selection highlight doesn't span the full row.
3. **No way to rename a project.** A project's display name is whatever derivation we picked at open time (basename, config name, etc.). Users want to label projects by what they *are about* and have that survive close + restart.

This doc records the decisions, not the implementation. Files listed are the expected targets.

---

## Decisions (grilling session, 2026-05-28)

| # | Topic | Decision |
|---|---|---|
| Q2 | **Project-bar layout** | Originally: bar in the title-bar row above the session-tab bar. **Pivoted (2026-05-31)**: bar lives **inside** the workspace, slotted between the top header bar (search bar / sidebar icons / Update button — `Workspace::render_tab_bar`) and the panels row. Reads as in-app chrome instead of a band glued to the title bar. |
| Q3 | **Pill rendering** | Hand-rolled `ProjectTabComponent` (`app/src/workspace/project_tab.rs`) matching session-tab visual tokens (no corner radius, side-only borders shared between adjacent tabs, **same `fg_overlay_1`/`fg_overlay_2`/outline tokens**). Active tab gets a 15% `theme.accent()` tint — the one intentional departure from session-tab neutrals so the active project reads at a glance. No reuse of `TabComponent`. |
| Q4 | **Rename scope** | Persisted project name, origin-aware. Not display-only/ephemeral. |
| Q5 | **Rename key** | `(origin, canonical_path)` where `origin` is one of the two simplified variants — `Config { config_name }` or `Template { template_name }` (see [`projects-origin-simplification.md`](./projects-origin-simplification.md)). New `project_display_names` table; `ProjectSwitcher::identity()` consults it before falling back to default name. |
| Q6 | **Rename trigger** | Double-click the pill → inline editor in-place + `F2` keybinding for the active project tab. Enter commits, Esc cancels. Empty string ⇒ revert to default name. |
| Q7 | **Single-tab visibility** | Always show the bar, even with one project tab (drop the current `tab_count > 1` gate at `root_view.rs:3836`). Chrome stability + discoverability. |
| Q8 | **Hide/show** | `F3` toggles bar visibility, persisted globally. Default visible. |
| Q9 | **Bar affordances** | No `+` button. No `×` close button. Drag-to-reorder filed as nice-to-have, deferred. Closing the last session-tab in a workspace already closes the workspace (Phase 5), so project tabs go away naturally. |
| Q10 | **Popup scope** | Project palette and Alt-Tab are the same `command_palette/view.rs::View` with different `NavigationMode`. Row redesign covers both; Alt-Tab popup shape revisited after rows are real. |
| Q11 | **Row redesign** | Style freedom granted; use existing bundled SVG icon set (`app/assets/bundled/svg/`). SF Pro / SF Symbols **not** in scope (would need new infra). Build first, screenshot, iterate. |

Pain points the row redesign must solve (from user feedback):

- Selection highlight should span the full row width, not just the inner content area.
- "Open" marker must look visually distinct from the branch pill — different shape / colour / placement (e.g. dark right-aligned status chip).
- Name + path want bigger type and more colour differentiation, with the **project name primary** and the path supporting (flip of the `sessions:` picker hierarchy, where the path is primary because the row represents a directory, not a project).

---

## Built so far (2026-05-28)

Iterated live with the user. Final landing differs from the original plan in two ways:
the row redesign went through the **existing** `launch_config::renderer` (which already
handled the projects-palette row) rather than rewriting `projects/search_item.rs`, and
the project-bar / persisted-rename work (Steps 3 & 4 below) has not started.

### Renderer-side pill polish (`launch_config/renderer.rs`)

- New `pub struct DiffStats { files, insertions, deletions }`. Plumbed into
  `ProjectRowDetails` (`projects/search_item.rs` builds it, `projects/data_source.rs`
  computes it via libgit2).
- Branch pill now renders with a leading `Icon::GitBranch` (was text-only). New helper
  `render_icon_pill(icon, text, …)` wraps an icon+span in the same chrome that
  `render_string_with_pill_styling` already uses.
- New `render_diff_stats_pill(stats, …)` renders the working-tree diff as
  `📄 N · +X -Y` with `+X` in the theme's green and `-X` in red. Returns `None` for a
  clean working tree so callers skip the pill rather than render an inert `0 · +0 -0`.
- Chrome factored into `wrap_in_pill_chrome(body, style)` so plain-text, icon+text,
  and diff-stats pills all share identical sizing / corner radius / margins.
- Two-line layout restructured: the pills now sit in a sibling `Flex::row` of the
  `[name + path]` column with `CrossAxisAlignment::Center` on the wrapping row, so
  pills are vertically centered against the **whole** two-line block rather than
  glued to the first line. The single-line (regular launch-configs) layout is
  unchanged — same `Flex::row` it always used.

### Projects palette wiring (`projects/{search_item,data_source}.rs`)

- `SearchItem` carries a new `diff_stats: Option<DiffStats>` field; both constructors
  (`available`, `open_window`) take it.
- The `projects:` palette now calls `LaunchConfig::render` with `is_open=false` and
  `show_description=false` **unconditionally** — drops the `open` chip and the
  `N windows / N tabs` description from this surface only. The section header
  (`Open Projects` / `Available`) already conveys the open/available distinction, and
  the window/tab count belongs to the regular launch-configs palette, not here. That
  other caller (`launch_config/search_item.rs`) still flips both flags on, so the
  regular launch-configs palette is untouched.
- New `current_diff_stats(cwd)` helper next to `current_branch` in `data_source.rs` —
  uses `git2::Repository::discover` + `diff_tree_to_workdir_with_index` + `.stats()`.
  Returns `None` for non-git, unborn HEAD, or a clean tree.

### Popup chrome (`command_palette/view.rs`, `palette_styles.rs`)

- Added a matching `with_padding_top(10.)` to the palette container (was 0 top, 10
  bottom). Most visible in Alt-Tab mode, which has no search bar above the body —
  the first row's highlight used to touch the popup's top edge and the rounded
  corners read as clipped.
- Bumped `result_outer_horizontal_padding_fn` from `4.0 * monospace_ui_scalar()` to
  a flat `10.0` so the per-row highlight has equal breathing room from the popup's
  left/right edges as from the top/bottom (both 10pt). Shared across all command
  palette surfaces, not just `projects:` — the wider gutter shows up everywhere.

### Project-bar redesign (`workspace::view::Workspace::render_project_bar`, `workspace/project_tab.rs`)

**Architecture pivot (2026-05-31).** First pass put the bar in `root_view.rs`,
stacked above the workspace as the topmost strip of the window — with traffic
lights inset on its left edge and a row of tabs to the right of them. After
iteration that looked like a band "glued on top" of the existing window chrome,
not part of it. Moved the entire render into `Workspace::render_project_bar`
and slotted it in the workspace's `outer_column` **between** `render_tab_bar`
(the top header bar with search bar / sidebar icons / Update button) and the
panels row. Traffic lights stay where the top header bar already handles them;
the project bar starts flush at `x=0` underneath.

**Tab visuals — `app/src/workspace/project_tab.rs` (new module).** Hand-rolled
`ProjectTabComponent` that mirrors session-tab `NewTabStyling`
(`tab.rs::render_tab_container_internal`):

- No corner radius. Rectangular blocks that share the strip's top/bottom edges.
- Borders only on right + first-tab-left (`Border::all(1.).with_sides(false,
  is_first, false, true)`) — the right border of tab N is the visual separator
  between tab N and tab N+1, so adjacent tabs don't double up.
- Active background = 15% `theme.accent()` tint (`ACTIVE_BG_OPACITY = 38`),
  hover = `fg_overlay_1`, inactive = no background (the strip's own
  `fg_overlay_1` shows through). The accent tint is the project bar's one
  intentional departure from session-tab neutrals.
- Active label uses Medium weight (`Properties::default().weight(Weight::Medium)`).
- Each tab wrapped in `Expanded::new(1.0, …)` at the call site so N tabs evenly
  split the bar width: 1 tab → full width, 2 → halves, etc. Inner row uses
  `MainAxisSize::Max` + `MainAxisAlignment::Center` so the icon+label group
  sits centered inside each tab's slot.
- Leading [`Icon`] keyed off [`ProjectOrigin`] (Folder / LayoutAlt01 /
  Navigation / Gear / Terminal) — same mapping `projects/search_item.rs` uses
  for the palette rows, so the bar and the palette stay visually consistent.
- Close `×` only renders on tab hover (cheap mouse-state gate); click
  dispatches `root_view:close_project_workspace`, the same path the projects
  palette uses.

**Strip chrome.** `TAB_BAR_HEIGHT` (34pt) row + `fg_overlay_1` overlay
background + 1pt `theme.outline()` bottom border — identical to what
`render_tab_bar` paints, so the two strips read as one design language. No
outer terminal-bg layer (the workspace's outer `Container` already wraps
everything in `get_terminal_background_fill`).

**Mouse-state storage.** `ProjectTabMouseStates { pill, close }` per workspace
`EntityId`, stored in `Workspace::project_tab_mouse_states:
RefCell<HashMap<EntityId, _>>` (lazily populated by `render_project_bar` since
`View::render` only gets `&self`). Lives on `Workspace` rather than `RootView`
so the bar can render inside the workspace.

**Gating.** Dropped the original `tab_count > 1` early-out. Always renders when
`TabSettings::project_bar_visible` is true (default), so single-project windows
keep the bar and the chrome doesn't shift when a 2nd tab opens. Excluded from
the simplified-WASM render branch — Warp Drive object / shared-session /
transcript views don't host projects.

### Project-bar visibility toggle (F3)

- New `TabSettings::project_bar_visible` bool (`appearance.project_bar.visible`,
  global sync). Default true. Lives next to the other tab/appearance prefs in
  `app/src/workspace/tab_settings.rs`.
- New `WorkspaceAction::ToggleProjectBar` variant
  (`app/src/workspace/action.rs`), handled in `workspace/view.rs` next to
  `ToggleRightPanel`: flips the setting via `TabSettings::handle(ctx).update`
  and calls `ctx.notify()`. `Workspace::render` reads `project_bar_visible`
  on each render and shows/hides the project bar accordingly.
- `EditableBinding` registered in `workspace/mod.rs` next to
  `workspace:toggle_left_panel` with `with_mac_key_binding("f3")`. **F3 is
  mac-only** — `view_components/find.rs` already binds it to "find next" on
  linux/windows; chose not to shadow that binding on those platforms. A
  cross-platform alternative would need a different key (e.g. `cmd-shift-p`
  is taken by the projects palette already).
- Added a `TabSettingsChangedEvent::ProjectBarVisible` match arm to
  `handle_tab_settings_change` (`workspace/view.rs:3583`) that calls
  `ctx.notify()` to kick a workspace re-render when the setting changes.

### Out-of-palette: default-name sequence (`workspace/template_sequence.rs`)

- Template-origin tabs are named `<template>-N` (`default-1`, `default-2`, …) — pure helper
  `template_sequence::next_template_sequence_name(template_name, in_use_names)` picks the smallest
  free slot for the given template, with gap-fill on close. Originally `default`/`default 1`/…
  under a `Default` origin via `next_default_name(ctx)` in `root_view.rs`; refactored into a pure
  module and generalized to every template by the origin-simplification work (see
  [`projects-origin-simplification.md`](./projects-origin-simplification.md)).

### Failed attempt, reverted

The first pass tried to redesign rows directly in `projects/search_item.rs` —
hand-rolled `Flex::row` with `Container.with_background_color` + `Text::new_inline`
pills, then switched to `render_udi_chip` / `chip_container` from `display_chip.rs`.
In both cases the chip's text laid out at zero width (icon visible, label missing)
while the row's path subtitle (same `Text::new_inline` primitive, different parent)
rendered fine. Root cause not isolated; reverted to HEAD on both
`projects/search_item.rs` and `projects/data_source.rs` and re-did the work through
the existing `launch_config::renderer` instead, where the same primitives lay text
out correctly. The lesson: in this codebase, **route new pill UI through the
renderer that already owns the surface** rather than building parallel chip code.

### Still TODO from original plan

| Original ask | Status | Notes |
|---|---|---|
| Diff-stats pill `📄 N · +X -Y` | ✅ done | Via renderer.rs, not hand-rolled. |
| Drop `open` chip in projects palette | ✅ done | `is_open=false` from the projects caller only. |
| Drop `N windows / N tabs` in projects palette | ✅ done | `show_description=false` ditto. |
| Branch pill with icon | ✅ done | `render_icon_pill(Icon::GitBranch, …)`. |
| Path **below** name as subtitle | ✅ already worked | Renderer's `Flex::column` did this all along. |
| Section headers always shown | ❌ | `data_source.rs::assemble_sections` still gates on `show_headers > 1`. |
| `default` template sorted at bottom of Available | ❌ | `available_section` sorts alphabetically. |
| Template-name sequence `<template>-N` (gap-fill) | ✅ done | `workspace/template_sequence::next_template_sequence_name`; the original `next_default_name` was generalised and pulled into a pure module by the origin simplification. |
| Symmetric popup chrome | ✅ done | view.rs padding_top + palette_styles.rs gutter to 10. |
| Project bar redesign (Step 3) | ✅ done | Bar lives inside `Workspace::render` between the top header bar and the panels row. Session-tab-styled `ProjectTabComponent` (rectangular, shared right separators, 15% accent active tint), tabs `Expanded` to evenly split the bar width with centered labels. F3 toggles, persisted globally. `~`-named tab investigation still open. |

## Build order (remaining)

Steps 1 & 2 are folded into "Built so far" above. Step 3 is done except for the
`~`-tab investigation noted below. Step 4 is still to do, with notes preserved
from the original plan.

### Step 3 — Project bar redesign (remaining)
- Investigate `~`-named tab. Probably a default-origin tab whose home-relative
  path was used as the label; needs to use the project name instead (or the
  `default N` sequence).

### Step 4 — Persisted project rename

> ⚠️ **Superseded by [`projects-rename.md`](./projects-rename.md)** — the fresh
> PRD built ground-up after the origin simplification landed. The bullets
> below are the May 2026 sketch; several decisions there (identity-scoped
> persistence via a `project_display_names` table, `(origin, canonical_path)`
> key) were re-grilled and **traded for a simpler per-tab override** (nullable
> column on `windows`, dies with the workspace). See the new PRD for the
> current spec.

- **Original sketch (May 2026, partially stale):** new migration +
  `project_display_names` table — keys: `origin TEXT`, `canonical_path TEXT`,
  `name TEXT`, `updated_at TIMESTAMP`. `PRIMARY KEY (origin, canonical_path)`.
  Files: `app/src/persistence/sqlite.rs`,
  `crates/persistence/migrations/<new>/{up,down}.sql`,
  `crates/persistence/{schema,model}.rs`,
  `app/src/workspace/project_switcher.rs` (read-path consult),
  `app/src/workspace/project_tab_pill.rs` (inline editor host),
  `app/src/workspace/action.rs` + `app/src/workspace/mod.rs`
  (`RenameProjectTab` action + `F2` binding).
- **Current spec** ([`projects-rename.md`](./projects-rename.md)): per-tab
  override via a nullable `display_name_override TEXT` column on `windows`;
  read-path through `Workspace::display_name(...)`; F2 + double-click on the
  active pill; surfaces include the bar pill, palette open-projects row, and
  Alt+Tab row; palette fuzzy search matches both override and identity name.

### Step 5 — `git status` "first command" — dropped
- User confirms this is on their side (shell prompt / dotfiles), not Warp.
- Pre-investigation finding for the record: `current_branch()` at `data_source.rs:388` uses `git2::Repository::discover` — pure libgit2, no subprocess, silently returns `None` on non-git paths. So this branch-pill code path is **not** the source of any visible failure.

---

## Out of scope (deliberately)

- **Phase 6 — Merge Windows.** Stays deferred per `projects-handoff.md:36`.
- **SF Pro / SF Symbols integration.** Would need new font-glyph infra.
- **Drag-to-reorder project tabs.** Nice-to-have; only land if Steps 1–5 leave room.
- **Cross-window project-tab drag.** Separate effort, depends on the deferred Phase 6.
- **Rewriting launch-config files on rename.** Rename is a display override only — `Config`-origin projects' on-disk config name is untouched. (See Q4-vs-Q5 trade-off; we chose the override-table path explicitly.)
- **Vertical project bar.** Bar stays horizontal; users who prefer vertical session tabs are the primary target.

---

## Keybindings introduced

| Key | Action | Scope | Notes |
|---|---|---|---|
| `F2` | Rename active project tab | `Workspace` context | Universal rename convention; unlikely to collide. Validate against existing keymap at implementation. |
| `F3` | Toggle project-bar visibility | Window / `Workspace` | Default visible; persisted globally. Validate against existing keymap. |

If either F-key collides, surface the conflict and pick an alternative before binding. Per handoff Gotcha #6, `cmd-shift-N` had four claimants — assume nothing.

---

## Cross-references

- [`projects-handoff.md`](./projects-handoff.md) — orientation, git state, build / run instructions, Gotchas (especially #1, #2, #6).
- [`projects-tabs-redesign.md`](./projects-tabs-redesign.md) — source of truth for behaviour and phase status.
- [`projects-glossary.md`](./projects-glossary.md) — vocabulary (OS window / RootView / workspace=project-tab / session-tab / pane).
