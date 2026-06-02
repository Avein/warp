# Projects Redesign — Templates · Projects · Windows

> ⚠️ **Doubly superseded — historical reference only.** This document was first superseded by the
> projects-as-tabs pivot in [`projects-tabs-redesign.md`](./projects-tabs-redesign.md) (windows
> stopped being projects; workspaces became the per-project unit). It is now also superseded by
> [`projects-origin-simplification.md`](./projects-origin-simplification.md), which collapsed the
> four-variant origin enum referenced throughout this doc (`Config` · `Template` · `Default` ·
> `Root`) down to two (`Config { config_name }` · `Template { template_name }`) and made root a
> runtime-synthetic Config. Read this file only to understand the original windows-as-projects
> design intent; do not consult it for current behavior.
>
> Personal fork feature (branch `feat/projects-palette`). Original framing: implementation spec
> for reworking how windows, projects, and templates relate in the `projects:` palette + Alt+Tab
> switcher. It supersedes the earlier ad-hoc `ProjectSwitcher` (HashMap `name → WindowId`) model.
>
> **Status:** spec approved, implementation pending. *(Original status line, retained for
> context — both the projects-as-tabs and origin-simplification work have since landed; see the
> banner above.)*

## Why

The previous model recorded only windows spawned through `focus_or_spawn_project`
in a `HashMap<config_name, WindowId>` plus an MRU `Vec<String>`. This caused:

- A multi-window config spawns N windows but records 1 ("5 windows, can only
  iterate over 2").
- Same-basename projects collide and overwrite each other in the map.
- `File > New Window`, restored, and startup windows are invisible to the
  palette and to Alt+Tab.
- No first-class concept of a path-less "template" you can open anywhere.

## Core concepts

The single deciding rule: **a launch config with `cwd`s baked in is a Project;
a launch config with no `cwd`s is a Template** (layout + commands only, opened
*at* a path supplied at launch time).

| Concept | Definition | Lives where |
|---|---|---|
| **Template** | Launch config with **no `cwd`s** — layout + commands only. Opened at a path given at launch time. | YAML (`cwd` optional) |
| **Project** | Launch config **with `cwd`s** baked in (e.g. `dotfiles`). | YAML |
| **Open-project window** | A live window stamped with `ProjectIdentity { name, path }` — created via picker / `newds` / template-at-path. | Runtime stamp on `Workspace` |
| **Plain window** | A live window with no stamp — created via `cmd+n`. | Runtime (unstamped) |
| **Root project** | The startup / first window, auto-stamped as a project (`~` or its cwd, plain shell, no template) so it's listed from boot. | Runtime stamp |

## Window stamping & switcher rework

- Add `Option<ProjectIdentity>` field to `Workspace` (`ProjectIdentity { name: String, path: PathBuf }`).
- `ProjectSwitcher` stops being `HashMap<name → WindowId>`. **Open lists are
  derived live** by enumerating `WorkspaceRegistry::all_workspaces()` and reading
  each window's stamp.
- **MRU becomes per-window** (`Vec<WindowId>`), touched on focus/switch. Every
  stamped window counts independently — no name collisions, no lost windows.

## ⌃⌘P palette — three sections (top → bottom)

1. **Open Projects** — every live *stamped* window, MRU order. `Enter` = focus.
2. **Open Windows** — every live *plain* window (`cmd+n`). `Enter` = focus;
   **`⌘Enter` = close** (reuses the existing close shortcut).
3. **Available** — **Projects + templates** not currently open. `Enter` on a
   project opens it; `Enter` on a template opens it at the *current window's cwd*.

Sections render with `SeparatorSearchItem` headers, only shown when more than one
section is non-empty. A non-empty typed query collapses to a flat fuzzy-ranked
list (the existing filtered path).

## Alt+Tab

- Cycles **all project windows including the current one**, MRU-ordered,
  offset 1 (first Tab = most-recent *other* project).
- Release switches to the selected window **and touches MRU** → clean **X↔Y
  toggle** between the two most-recent projects.
- **Plain windows are excluded** from Alt+Tab.

## Open flow — "reuse-if-plain"

When opening a project or template:

1. Target is **already open as a project** → **focus** that window.
2. Focused window is **plain** → **adopt it**: append the config's tabs to it and
   stamp its identity. No new window.
3. Focused window is **a project** → **new window**.

**Root-project caveat:** the startup / first window auto-registers as a root
project (its cwd or `~`, plain shell, no template) so the projects list is
populated from boot.

## `newds`

`newds` (shell function in dotfiles `zsh/.config/.zsh/aliases.zsh`) follows the
**same reuse rule** as the open flow: adopt the focused plain window, else open a
new window. It uses the path-less `default` **template** re-rooted to the target
path via `warposs://action/new_default_session?path=…`.

## YAML changes

- `PaneTemplate.cwd`: `PathBuf` → `Option<PathBuf>`. Serde is non-strict, so this
  is backward compatible with existing configs.
- `~/.warp-oss/launch_configurations/default.yaml` rewritten **path-less** (a true
  template — layout + commands, no `cwd`).

## Files touched

| File | Change |
|---|---|
| `app/src/launch_configs/launch_config.rs` | `cwd` → `Option<PathBuf>`; `rewrite_cwds` / `single_pane` / `primary_cwd` adjusted for optional cwd |
| `app/src/workspace/view.rs` | `Option<ProjectIdentity>` stamp field; adopt + stamp logic in `open_launch_config_window`; enumerate live windows |
| `app/src/workspace/project_switcher.rs` | Rework to window/stamp based; per-window MRU (`Vec<WindowId>`) |
| `app/src/root_view.rs` | Open flow (reuse-if-plain); root-project auto-register; template-pathless `open_default_session` |
| `app/src/search/command_palette/projects/data_source.rs` | Three sections built from live windows (replaces uncommitted `open_only` 2-section version) |
| `app/src/uri/mod.rs` | `newds` reuse rule for `new_default_session` |
| `app/src/search/command_palette/data_sources.rs` | Mixer reset wiring for the new sections |

## Implementation notes (as built)

Details that firmed up while implementing, for anyone extending this:

- **Stamp ownership.** The `ProjectIdentity { name, path: Option<PathBuf> }` is held centrally
  in `ProjectSwitcher` keyed by `WindowId` (`stamps: HashMap<WindowId, ProjectIdentity>` +
  `mru: Vec<WindowId>`), *not* as a field on `Workspace`. Nothing reads a per-window identity off
  the `Workspace` itself, so this avoided touching the `Workspace` struct.
- **Liveness is lazy.** Closed windows are not actively pruned on a close hook; every read filters
  through `WorkspaceRegistry::get`. `forget(window_id)` is called from the close actions as a
  courtesy but correctness does not depend on it.
- **Root project** is stamped at window registration via `ProjectSwitcher::claim_root()` (fires
  once per session, for the first window). Name = `"~"`, `path = None` → its path/branch render
  from the window's live `ActiveSession` cwd.
- **Template open = path-keyed project.** Picking a template (Available section) routes through
  `focus_or_spawn_project`, which detects `is_template()`, re-roots it at the active window's cwd
  (or `~`), and **renames it to that directory's basename** — so it becomes a concrete project
  keyed by path, exactly like `newds`. This unifies "open template here" and `newds`.
- **Reuse-if-plain** only adopts when the config is single-window *and* the focused window is
  plain. The root project is a project (not plain), so opening a project from the root spawns a
  new window — consistent with "focused window is a project → new window".
- **Open rows vs available rows.** Open rows (projects + plain windows) carry a `target_window`
  and dispatch `FocusWindow` / `CloseWindow` (by id). Available rows carry a config and dispatch
  `FocusOrSpawnProject`; their secondary action is a no-op. Open rows render a synthetic
  window-less `LaunchConfig` (name only) with the window/tab description suppressed via the new
  `show_description` arg to `LaunchConfig::render`.
- **Alt+Tab surface** is selected by `projects::Surface::{Palette, AltTab}` on the shared data
  source. `reset_projects_mixer` sets `AltTab`; `open_projects_palette` restores `Palette`. The
  AltTab branch **drops the active window** and assigns descending MRU scores; the palette opens at
  offset 0 so the first item is the most-recent *other* project → single Alt+Tab toggles X↔Y,
  hold-and-tap walks the MRU list. (An earlier include-active + offset-1 variant felt like it
  "cycled MRU" instead of toggling, so it was reverted to this drop-active form.)
- **Project origin.** Each `ProjectIdentity` carries a `ProjectOrigin { Config, Template, Default }`.
  `Config` = a saved launch config with baked `cwd`s (e.g. `dotfiles`); `Template` = a path-less
  template re-rooted at a path at open time (and `newds`'s underlying default template); `Default`
  = a default/`newds`/root session that follows its cwd. The palette picks a distinct icon per
  origin (`Folder` / `LayoutAlt01` / `Navigation`; plain windows get `Terminal`) so two same-named
  entries from different sources are visually distinguishable. Origin is derived where the project
  is opened: the palette's Available rows and `focus_or_spawn_project` classify by
  `LaunchConfig::is_template()` (path-less ⇒ `Template`, else `Config`); `newds` passes `Default`.
- **Persistence across restart.** The full `ProjectIdentity` (name + path + origin) is now persisted
  per window: a nullable `project_identity` TEXT column on the `windows` table (serde_json),
  mirroring `agent_management_filters` (migration `…_add_project_identity_to_windows`, plus the
  `schema.rs` / `model.rs` `Window`+`NewWindow` and the `WindowSnapshot` field). `Workspace::snapshot`
  reads the live stamp from `ProjectSwitcher`; the `Restored` arm of `configure_new_workspace`
  re-stamps from it with an origin-aware naming policy:
  - `Config` / `Template` → **keep the persisted name** even if the tab's cwd has since changed
    (a `dotfiles` project stays `dotfiles`; a template opened as `echo` stays `echo`).
  - `Default` → **re-derive** the name from the persisted active-tab cwd basename (a `newds` session
    in `echo` that you `cd` to `hermes` comes back as `hermes`).
  - **No persisted identity** (sessions saved before this column, or plain `cmd+n` windows) →
    fall back to the cwd-basename behavior as `Default`, so nothing drops out of the palette.
  The root-project claim is guarded to skip already-stamped windows so it doesn't clobber a
  restored stamp with `~`. Round-tripped by `test_sqlite_round_trips_project_identity`.

## Known follow-ups

- The legacy fallback re-stamps a restored window with **no** persisted identity (including a real
  plain `cmd+n` window) as a `Default` project. This preserves the prior "projects don't vanish"
  behavior; distinguishing a genuinely-plain restored window from a pre-column one would need a
  session-version marker.
- Multiple windows at startup is Warp **session restore**, not instance duplication.

## Out of scope / deferred

- M3 resource measurement (idle GPU/RAM per window).
- Quick-launch shortcuts (⌘-1..9) and per-project metadata.
- Upstreaming.

## Build & verify

```sh
export PATH="/Users/avein/.rustup/toolchains/1.92.0-aarch64-apple-darwin/bin:$PATH"
cargo check -p warp --bin warp-oss --features gui
cargo fmt -p warp
cargo clippy -p warp --bin warp-oss --features gui -- -D warnings
./script/run   # builds debug bundle + launches WarpOss.app
```

**Done when:**

- ⌃⌘P shows three sections (Open Projects / Open Windows / Available) reflecting
  every live window, not just switcher-spawned ones.
- Alt+Tab toggles between the two most-recent project windows and lists all of
  them.
- Opening a project/template into a plain window adopts that window; into a
  project window opens a new one; an already-open project focuses.
- `newds <path>` opens the path-less `default` template re-rooted to `<path>`.
- Startup window appears as the root project immediately.
