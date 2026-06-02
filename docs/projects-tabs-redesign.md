# Projects-as-Tabs Redesign — Workspaces inside one Window

> Personal fork feature (branch `feat/projects-palette`). This document **supersedes the
> windows-as-projects model** described in [`projects-redesign.md`](./projects-redesign.md) and
> [`projects-handoff.md`](./projects-handoff.md). Where those treat **a window = a project**, this
> spec treats **a workspace (a switchable tab-group living inside one OS window) = a project** —
> the "tmux sessions, but with a proper terminal" model.
>
> **Status:** core implemented & verified working (open-as-tab, focus, palette/Alt-Tab, restore-as-tabs, per-tab cwd,
> last-session-tab closes the workspace, `cmd-shift-N` new-project-tab popup, same-basename
> disambiguation). Only Merge Windows remains open;
> `cmd-shift-w` was **dropped by decision** (closing a project is done from the projects palette). See
> **[Implementation status (as built)](#implementation-status-as-built)** for the precise as-built
> state and where it diverges from the design below.

## Motivation

The shipped model spawns **one OS window per project**, so a handful of projects becomes a pile of
floating OS windows. The user wants projects to switch *inside* a single window — like tmux sessions
— without that window proliferation. (Note: Warp windows are **not** separate OS processes; the real
per-pane cost is the shell PTY, identical whether panes live in tabs or windows. The genuine
per-window cost is a GPU render surface + window chrome — real but modest.)

## The model

New nesting, **one level deeper than today**:

```
OS Window  →  RootView  →  [ Workspace … ]  (one active)  →  session tabs  →  panes
                              └ "project-tab"      └ today's tab strip (unchanged internally)
```

- A **project-tab is a `Workspace`** — exactly what an OS window holds *one* of today, relocated to
  *N per OS window* with one active. A Workspace's internals (the flat tab strip, activate/close/
  reorder/transfer, pane layouts, session restore) are **untouched**.
- One OS window hosts a **new top-level "project bar"** (distinct from the session-tab strip) that
  switches the active Workspace.
- **Multiple OS windows still exist** (multimonitor, explicit new-window). Adding a project defaults
  to the *current* window.

### Every project-tab is a project

There is **no "plain workspace"** concept. Every Workspace is a project, classified by `origin`:
`Config { config_name }` (saved launch config with baked `cwd`s — covers the synthetic startup
`root`) · `Template { template_name }` (path-less config applied at a path at open time — covers
`newds`, the `cmd-shift-N` popup, and `cmd-n`'s starting tab via the built-in `default` template).

The 4-variant origin enum (`Config` · `Template` · `Default` · `Root`) was collapsed to these two
in 2026-06; see [`projects-origin-simplification.md`](./projects-origin-simplification.md) for the
PRD and the dedupe rules.

A fresh project-tab with no saved config is an **ad-hoc project** — `Template { template_name:
"default" }` applied at the open path, named `default-N` (next free slot, gap-filled on close).

### Project ↔ Workspace cardinality

**One launch config = one Workspace.** A config declaring multiple `windows` is **flattened**: every
declared window's tabs are appended into the single workspace's tab strip (loop over
`open_launch_config_window`). `active_window_index` maps to which tab is focused. This directly fixes
the original "5 windows, can only iterate over 2" bug.

## Keybindings

| Key | Action | Change |
|---|---|---|
| `cmd-n` | New **OS window** (its lone workspace is an ad-hoc/root project) | unchanged |
| `cmd-shift-N` | New **project-tab** in the current window — opens the path popup | **new** |
| `cmd-t` | New **session tab** inside the active workspace | unchanged |
| `cmd-w` | Close the active **session tab** | unchanged |
| `cmd-shift-w` | ~~Close the active project-tab~~ | **dropped** — close from the projects palette instead; last `cmd-w` closes the workspace |
| Alt+Tab | Cycle **project-tabs within the current window**, MRU, X↔Y toggle | re-pointed |

Verify `cmd-shift-N` / `cmd-shift-w` are free before wiring (absent from the app-menu layer).

## `cmd-shift-N` — new project-tab popup

- A **single-line path input with shell-style folder completion** — **no mouse browse / native
  picker** (user preference). Type/edit the path; **Enter confirms**.
- **Prepopulated with the home directory** (always `~/`, not the active tab's cwd), **cursor at the
  end, unselected** so you can append/complete immediately.
- **Tab** (or **↓**) completes the directory name being typed: a unique match fills the folder name
  (no trailing `/` — you type that yourself to descend); multiple matches first extend to the
  longest common prefix, then, once there's no shared-prefix progress (e.g. after a `/`), repeated
  Tab/↓ **cycles** the matching folders. **↑** cycles backward. Matching is case-insensitive and
  ignores non-directories.
- On confirm: create an ad-hoc project-tab at the **chosen** path, applying the path-less `default`
  template re-rooted there (the `open_default_session` / `newds` mechanism).
- **Name = basename of the chosen/final path** — never the origin path.
- `newds <path>` (shell) remains the no-popup scriptable equivalent.

### Same-basename collision

Two open projects sharing a basename (`~/work/api`, `~/play/api`) keep both; **disambiguate the
display label with a parent-dir suffix** — `api — work` / `api — play`. Bare `api` when unique.

## Open flow (palette / `newds` / template)

Two cases only (the old three-case "reuse-if-plain" is **dropped** — adoption was a workaround for
window proliferation that no longer exists):

1. **Already open** (a live workspace is stamped with this project) → **activate that project-tab**
   and raise its host OS window (singleton; re-points `show_window_and_focus_app`).
2. **Not open** → create a **new project-tab in the current OS window**, stamp it, make it active.
   No new OS window, no mutating the workspace you were on.

Opening a **Template** follows the same rule, rooted at the supplied path; an already-open
project-tab for the same name+path is focused instead.

## Project bar UI

- A **second tab bar**, distinct from the session-tab strip, mirroring the session-tab settings
  shape so it gains orientation + hide.
- **Placement is a setting**: `Top | Left | Right` (`appearance.project_tabs.position`). **MVP ships
  `Top` (horizontal) + a hide toggle**; `Left`/`Right` wired but lower-polish (vertical renderer is
  the fast-follow). Two stacked horizontal bars (project over session) is a supported state.
- **Hide toggle** (mirrors the vertical-tabs-panel toggle) + per-window open state.
- Both-vertical edge case (project bar + session panel both on a side): allow, but **force opposite
  sides**; if both point at the same side, the project bar wins it and the session panel falls back.
- Per-`origin` icons (post-simplification): `Folder` (Config, including the synthetic `root`) ·
  `LayoutAlt01` (Template). Mapping lives in `workspace::project_icon` and is shared by the project
  bar and the projects palette.

## Switcher / MRU

- `ProjectSwitcher` is **re-keyed from `WindowId` to a workspace/project id** (a per-Workspace
  handle). Stamps, MRU, liveness-against-registry all carry over, re-pointed. (The original
  `claim_root` hook was removed by the origin-simplification work — root is now auto-spawned from
  `launch()` when persisted state is empty.)
- **MRU is global** across all workspaces in all OS windows (so the palette spans everything).
- **Alt+Tab uses the within-window slice** of that MRU; the **palette is the global switcher**.

## Lifecycle / close

- Closing the **last session-tab** in a workspace closes **that workspace** (today's
  "last-child-closes-parent", one level deeper).
- Closing the **last workspace** in an OS window closes the **OS window** (no empty chrome).
- `cmd-shift-w` (and an ✕ on each project-tab) closes a workspace (`ForceTerminate`), reusing the
  `is_last_tab` running-process confirmation.
- When the **active** project-tab closes and others remain, the **MRU-next** workspace becomes
  active.

## Merge windows

- A **"Merge Windows" command** (no mouse drag — user preference; drag-to-merge dropped entirely).
- **Target = the focused window.** Opens a **flat checklist of all *other* OS windows** (labeled by
  their project-tabs), multi-select. No display logic — merging across monitors is the user's call.
- Confirm → each checked window's workspaces re-parent into the target as project-tabs (reuse
  `TransferredTab` / `ContentTransferred`), appended after the target's existing ones in source
  order, MRU preserved; emptied source windows close. The target's active project-tab stays active.

## Root project

- The startup workspace is **runtime-synthetic**, not loaded from any `root.yaml`. When persisted
  state contains zero windows (first launch, after the origin-simplification state wipe, or after a
  session that left no windows), `launch()` dispatches `root_view:spawn_synthetic_root`, which
  opens a `Config { config_name: "root" }` workspace at `~` via the same
  `focus_or_spawn_project` path the palette uses.
- Reopenable from the palette's Available section after close. Originally `origin = Root` /
  `claim_root` — both replaced by the simplification work.

## Persistence (in MVP)

Persist the **workspace grouping** so a restart restores the consolidated layout (no "re-explode").
The existing `windows.project_identity` column carries over **per-workspace** unchanged. Add
grouping via **columns on the existing table** (chosen over a separate `window_groups` table — the
record already *is* the per-workspace unit and already holds the identity; nullable columns give a
free fallback):

- `host_group_id` — which OS window this workspace restores into.
- `workspace_order` — tab order within the group.
- `is_active_workspace` — which project-tab is focused in the group.

- **Save**: snapshot writes *N records per OS window* (one per workspace), each tagged with its
  window's group id + order + active flag.
- **Restore**: `open_from_restored` groups records by `host_group_id`, opens **one OS window per
  group**, loads each group's workspaces in order, activates the marked one. Separate groups →
  separate windows (multimonitor survives).
- **Fallback**: records with no `host_group_id` (pre-migration) restore **one-window-each** — i.e.
  today's behavior; nothing pre-existing breaks.
- Extend `test_sqlite_round_trips_project_identity` to assert group/order/active round-trip.

Identity restore naming policy: every restored stamp keeps its persisted name verbatim — the
old Default-only "re-derive from cwd basename" branch was deleted by the origin-simplification
work along with the `Default` and `Root` variants. (Templates now carry their `default-N` sequence
name from open through restart unchanged.)

## What carries over vs. what is reworked

| Carries over (built by prior agent) | Reworked for the pivot |
|---|---|
| `ProjectIdentity { name, path, origin }`, origin enum, per-origin icons | `ProjectSwitcher` re-keyed `WindowId` → workspace id |
| Palette rendering, fuzzy search, search-item rows | 3 sections → 2 (drop "Open Windows" / plain) |
| Alt+Tab MRU + X↔Y toggle logic | scope → within-window |
| `cwd: Option<PathBuf>`, Template vs Project, `is_template` | open flow: 3-case reuse-if-plain → 2-case |
| `newds` / `open_default_session` | `project_identity` column → per-workspace + grouping cols |
| | **New:** RootView N-workspaces restructure, project bar, popup, merge command |

## Implementation status (as built)

> Snapshot of what is actually on `feat/projects-palette` as of 2026-05-26, and the deliberate
> divergences from the design above. Verified by `cargo check`/`cargo test --no-run` (green) and by
> manual quit/relaunch testing.
>
> **Origin model update (2026-06):** the `ProjectOrigin` enum was subsequently collapsed from 4
> variants (`Config` · `Template` · `Default` · `Root`) to 2 (`Config { config_name }` ·
> `Template { template_name }`). The PRD is
> [`projects-origin-simplification.md`](./projects-origin-simplification.md). This section reads
> mostly as written — the per-phase plumbing is unchanged — but specific mentions of `Default`,
> `Root`, `claim_root`, `disambiguate_names`, and the `next_default_name` helper below describe
> code paths that have since been deleted; cross-referenced inline where each appears.

### Done and verified

- **Open project → in-window tab.** `focus_or_spawn_project` (`root_view.rs`) no longer spawns an OS
  window. If the project is already open it focuses that tab (`focus_workspace`); otherwise it opens
  a new project-tab in the **active** window via `RootView::open_project_tab` (falls back to a new
  window only when there is no active window). `cmd-n` still makes a fresh OS window.
- **Switching keeps keyboard focus.** `open_project_tab` / `activate_project_tab` /
  `close_project_tab` all call `self.focus(ctx)` after swapping the active workspace — without it,
  input kept targeting the previous (hidden) workspace and shortcuts looked dead. This was the main
  "can't use any shortcut" bug.
- **`ProjectSwitcher` re-keyed `WindowId` → workspace `EntityId`** (`project_switcher.rs`). MRU /
  stamp / liveness re-pointed; ordering split into the pure helper `projects_mru_filtered` with
  unit tests. *(Update 2026-06: the `claim_root` hook and the `workspace_for_name_filtered` helper
  were removed by the origin simplification — root now auto-spawns from `launch()` instead, and the
  per-origin dedupe rule lives in `workspace::identity_dedupe::find_live_workspace` with its own 7
  unit tests.)*
- **`WorkspaceRegistry`** (`registry.rs`) holds N workspaces/window (active id tracked) and gained
  `window_for_workspace` / `is_workspace_live` reverse lookups.
- **Palette + Alt-Tab are workspace-keyed.** `projects/data_source.rs` enumerates workspaces
  (`projects_mru` + `all_workspaces`); rows carry `(workspace_id, window_id)`; Alt-Tab drops the
  active *workspace*. Actions renamed `FocusWindow`/`CloseWindow` → `FocusWorkspace`/`CloseWorkspace`
  (`mixer.rs`, `search_item.rs`, `command_palette/view.rs`), dispatching
  `root_view:focus_project_workspace` / `:close_project_workspace`.
- **Restore brings projects back as tabs in one window** (`open_from_restored`): collects the normal
  windows, restores the previously-active one as the host, appends the rest as project-tabs via
  `RootView::restore_project_tab`. The macOS native window-tab "mess" is gone because there is now a
  single window.
- **Save persists every project-tab** (`app_state.rs::get_app_state`): snapshots **all** workspaces
  across all windows (not just the active one per window), so background tabs / `cmd-n` windows are
  no longer dropped on quit. `Workspace::snapshot` now takes the workspace's own `EntityId` so each
  tab persists *its own* identity. This is what makes the restore round-trip (and the "`cmd-n` window
  gets combined as a tab after restart") work.
- **Project bar** (`render_project_bar`): horizontal, top, one button per tab labelled with the
  project name, active highlighted, traffic-light inset. **Only rendered when the window has >1 tab**,
  so a single-project window keeps its original chrome.
- **Per-tab cwd is workspace-scoped** (`data_source.rs::workspace_cwd`): the palette/Alt-Tab path +
  branch line for a root project (no stamped path) and for plain `cmd-n` tabs now reads the *tab's
  own* active session via `WorkspaceRegistry::workspace_handle` → `Workspace::active_session_path`,
  instead of the window-keyed `ActiveSession`. A background tab no longer shows the active tab's
  directory.

### Divergences from the design above

- **Persistence is NOT via new DB columns.** The design called for `host_group_id` /
  `workspace_order` / `is_active_workspace`. The shipped approach instead writes a flat per-workspace
  snapshot list and **restore collapses *all* groups into one window**. Consequence: **multi-OS-window
  grouping is not preserved across restart** — everything comes back as tabs in a single window. This
  matches the user's explicit "restore projects as tabs" choice; preserving separate windows per
  monitor is a future refinement (would need the grouping columns).
- **No `+` button.** A debug `+`/`new_project_tab` was added then removed at the user's request;
  project-tabs are created only through the open flow.
- **Project bar has no orientation/hide setting yet** (horizontal-top only); per-origin icons not yet
  shown in the bar (palette rows still use them).

### Phase 4 — new-project-tab popup + disambiguation (done)

- **`cmd-shift-N` opens a path popup** (`new_project_popup.rs`, hosted by `RootView`): a single-line
  path input with **shell-style folder completion**, **no native picker**, prepopulated with the
  **home directory** (cursor at the end, unselected). **Tab**/**↓** completes-or-cycles the folder
  being typed, **↑** cycles backward (case-insensitive, dirs only; no trailing `/` is auto-added —
  the user types `/` to descend). **Enter** opens an ad-hoc project-tab rooted at the typed path
  (`~` expanded) via the shared `root_view:open_default_session` mechanism; **Escape** or clicking
  outside dismisses it.
  - **`cmd-shift-N` was claimed for the popup.** It previously carried a redundant `cmd-n` duplicate
    (`CustomAction::AddWindow`, dropped to no-keystroke) and two context-scoped welcome-screen
    bindings — "Add repository" (`welcome_view:open_project`) and "Create new project"
    (`project_buttons:create_new_project`). Those two welcome bindings were **moved to `ctrl-alt-n`**
    (their Mac chord; Linux/Windows chords unchanged) so the popup owns `cmd-shift-N` cleanly.
  - **Binding mechanism (macOS gotcha):** on macOS, `Trigger::Custom` fixed bindings only fire via a
    Mac **menu item** (custom→keystroke conversion is `#[cfg(not(target_os = "macos"))]`). With no
    menu item, the original `FixedBinding::custom(CustomAction::NewProjectTab, …)` never fired. The
    binding is therefore an **`EditableBinding`** with an explicit `.with_mac_key_binding("cmd-shift-N")`
    (uppercase — the keymap validator rejects shift+lowercase) / `.with_linux_or_windows_key_binding("ctrl-alt-n")`,
    scoped to `id!("Workspace")`, dispatching `WorkspaceAction::NewProjectTab` →
    `root_view:show_new_project_popup`. `CustomAction::NewProjectTab` now owns no default keystroke.
- **Same-basename disambiguation** (`data_source.rs::disambiguate_names`): originally appended
  ` — <parent-dir>` suffixes when two open project-tabs shared a basename (`api — work` / `api —
  play`). *(Deleted by the origin simplification: ad-hoc tabs now carry globally-unique
  `default-N` sequence names instead of cwd basenames, so the collision that motivated the
  disambiguator no longer occurs. See `template_sequence::next_template_sequence_name`.)*

### Not yet implemented

- **Phase 6** — Merge Windows command. Likely unnecessary now that restore auto-combines; revisit
  only if on-demand merging across live windows is wanted.

### Phase 5 — lifecycle (done, with a scope decision)

- **`cmd-w` unchanged** — still closes the active *session-tab* (the user explicitly did not want its
  semantics changed).
- **Last session-tab closes its workspace** (`view.rs::remove_tab`): when the last session-tab of a
  workspace closes, if the window hosts other project-tabs it closes **just that workspace** (deferred
  `root_view:close_project_workspace`), otherwise it closes the OS window as before. The guard
  `Workspace::is_only_project_tab` means pre-feature single-workspace windows behave **identically**
  to the old `ctx.close_window()`. `close_tab` no longer force-skips the running-process confirmation
  when only the workspace (not the window) will close, so that prompt is preserved.
- **Off-by-one in `is_only_project_tab` (fixed).** `remove_tab` runs while the workspace is mid-update,
  so its *own* weak handle cannot upgrade and `WorkspaceRegistry::workspaces_for_window` returns only
  the **other** project-tabs. The original `len() <= 1` check therefore treated the second-to-last tab
  as the last and closed the whole OS window while another project-tab was still open. The guard now
  tests `workspaces_for_window(...).all(|w| w.id() == self_id)` — i.e. the window closes only when
  **no other** workspace remains. (In `root_view.rs::close_workspace`, which runs as a free function
  *not* inside the workspace update, the handle does upgrade and `len() <= 1` is correct.)
- **`cmd-shift-w` dropped by decision** — closing a project-tab is done from the projects palette's
  secondary action; no separate binding was added.
- **Close confirmation is gated on a setting, not a bug.** Closing a tab/window with a live command
  (e.g. `sleep 1000`) only prompts when `show_warning_before_quitting` (Settings → General /
  `~/.warp/settings.toml`) is **true**. The detection works regardless (`long_running` counts
  correctly), but `UnsavedStateSummary::should_display_warning` short-circuits to `false` when the
  setting is off, so no dialog appears. This was confirmed with the setting at `false`.

### Phase 6 — Merge Windows (DEFERRED — not implemented)

**TODO / not built.** Decision (2026-05-27): deferred indefinitely. The author doesn't need it —
regrouping windows can be achieved by restarting the app (Phase 7 persistence restores the
consolidated layout). No code was written; the design below stands as the implementation plan for a
future pickup.

**Intended behavior** (see the "Merge windows" section above): target = focused window; a picker of
the *other* OS windows; on confirm each chosen window's workspaces re-parent into the target as
project-tabs (source order, MRU preserved), and the emptied source windows close.

**Implementation plan (grounded in the existing machinery):**
- Views are window-bound and **cannot migrate** across windows, so a merge must *reconstruct* each
  tab's `PaneGroup` in the target window — exactly what cross-window tab drag already does
  (`workspace/cross_window_tab_drag.rs`).
- Per chosen source window, enumerate its workspaces via
  `WorkspaceRegistry::workspaces_for_window(window_id, app)` (tab order).
- Per source workspace → create one new project-tab in the target `RootView` (the open-flow already
  builds workspaces via `NewWorkspaceSource`; `NewWorkspaceSource::TransferredTab` exists for the
  first tab), carrying its `ProjectIdentity` stamp so it stays the same project in the palette/MRU.
- Move each of that workspace's session-tabs across with the proven
  `Workspace::get_tab_transfer_info(index)` → `insert_transferred_tab_at_index(tab, index)` path
  (`workspace/view.rs:24844`/`24882`), the same `TransferredTab` flow as drag.
- Close each emptied source window with
  `ctx.windows().close_window(window_id, TerminationMode::ContentTransferred)` (silent, no
  "Close window?" prompt — content was preserved). `close_window_for_content_transfer`
  (`view.rs:24876`) is the reference.
- Enumerate windows with `WindowManager::ordered_window_ids()`; the projects palette data source
  (`search/command_palette/projects/data_source.rs`) already lists windows + their project labels and
  is the model for building the picker's row labels.

**Open decisions (resolve before building):**
- **Picker UI**: the spec calls for a multi-select checklist modal (host it like `new_project_popup`
  / `paste_auth_token_modal` via `Dismiss` + `stack.add_child`; checkboxes via
  `Appearance::checkbox`). A simpler first cut is a no-picker "merge ALL other windows" command.
- **Trigger**: command-palette entry (no chord) vs. also a keybinding (would need a verified-free
  chord; `cmd-shift-N` and `cmd-shift-W` are taken).
- **Persistence**: no new work expected — the registry is the source of truth, so the next snapshot
  records the new grouping automatically.

### Touched files

`workspace/registry.rs`, `workspace/project_switcher.rs`, `workspace/view.rs`,
`workspace/view_tests.rs`, `workspace/mod.rs`, `workspace/action.rs`, `root_view.rs`, `app_state.rs`,
`new_project_popup.rs` (new), `util/bindings.rs`,
`search/command_palette/{mixer.rs, view.rs, projects/data_source.rs, projects/search_item.rs}`.

## Out of scope / deferred

- Vertical (`Left`/`Right`) project bar polish (fast-follow after horizontal+hide).
- M3 resource measurement; quick-launch ⌘-1..9; per-project metadata; upstreaming.

## Build / run / verify

```sh
export PATH="/Users/avein/.rustup/toolchains/1.92.0-aarch64-apple-darwin/bin:$PATH"
cargo check  -p warp --bin warp-oss --features gui
cargo fmt    -p warp
cargo clippy -p warp --bin warp-oss --features gui -- -D warnings
cargo test   -p warp --features gui -- persistence::sqlite launch_configs::launch_config
./script/run    # builds debug bundle + launches WarpOss.app (use `warpfresh` to kill+relaunch one copy)
```

**Done when:**
- `cmd-shift-N` opens the path popup (prepopulated home dir, Tab/arrow folder completion) and adds a project-tab
  in the current window; the project bar (horizontal, top) shows it and can be hidden.
- Switching project-tabs swaps the session-tab strip; Alt+Tab toggles within the current window.
- One launch config opens as exactly one project-tab (multi-window flattened).
- Closing the last session-tab closes the workspace; closing the last workspace closes the window.
- ~~"Merge Windows" pulls selected other windows' projects into the focused window.~~ (deferred — see Phase 6)
- After restart, the consolidated multi-project window comes back intact (grouping persisted).
</content>
</invoke>
