# `projects:` palette — implementation notes (M1 spikes → M2 plan)

Working branch: `feat/projects-palette`. Feature: a `projects:` palette mode that lists
saved Launch Configurations by name, MRU-ordered, and on Enter focuses the project's
window if open (singleton) else spawns it. Inline "close project" action.

## M1 findings (confirmed in source)

### Palette mode registration (clone target)
- `app/src/palette.rs:4` — `PaletteMode` enum: `Command, Navigation, LaunchConfig, WarpDrive, Files, Conversations`. → add `Projects`.
- `app/src/search/data_source.rs` — `FilterAtom` registry (lazy_static). Existing prefixes incl.
  `sessions:` (line 56) and **`launch_configs:`** (line 64). `QueryFilter` enum pairs with `PaletteMode`.
  → add `QueryFilter::Projects` + `PROJECTS_FILTER_ATOM { primary_text: "projects:" }`.
- `app/src/search/command_palette/navigation/{data_source,search,search_item}.rs` — Session Navigation
  data source. MRU sort by `last_focus_ts()` in `navigation/search.rs` (FuzzySessionSearcher ~226, FullText ~308).
  Fuzzy filter `filter_sessions()` ~92 using `fuzzy_match::match_indices_case_insensitive`.
- `app/src/session_management.rs:26` — `last_focus_ts: Option<NaiveDateTime>` (MRU field pattern).
- `app/src/search/command_palette/data_sources.rs` — `reset_search_mixer()` registers sources via
  `mixer.add_sync_source(source, HashSet::from([QueryFilter::...]))`. → register projects source.
- `app/src/search/command_palette/view.rs` ~372 — `is_mode_enabled()` match. → add `(Projects, QueryFilter::Projects)`.
- `app/src/workspace/view.rs` — mode dispatch (~12372) + `open_*_palette()` openers incl.
  `open_launch_config_palette()`. → add `open_projects_palette()` setting `QueryFilter::Projects`.

### Launch config loading + serde
- Loader: `app/src/user_config/native.rs:185` `load_launch_configs(path)`.
- Dir: `app/src/user_config/mod.rs:185` `launch_configs_dir()` → `~/.warp/launch_configurations`.
- Listing getter: `app/src/user_config/mod.rs:105` `WarpConfig::launch_configs() -> &Vec<LaunchConfig>`.
- Model: `app/src/launch_configs/launch_config.rs:15`
  `LaunchConfig { name: String, active_window_index: Option<usize>, windows: Vec<WindowTemplate> }`
  → `WindowTemplate { tabs: Vec<TabTemplate> }` → `TabTemplate { layout: PaneTemplateType, ... }`
  → `PaneTemplateType::{ PaneTemplate { cwd, commands, pane_mode, shell, .. }, PaneBranchTemplate { split_direction, panes } }`.
- Save: `app/src/user_config/native.rs:139` `save_new_launch_config()` (reuse existing authoring UI; no new UI).
- **Serde strictness: NOT strict.** Launch config structs have NO `#[serde(deny_unknown_fields)]`
  (unlike tab_configs which do). → unknown yaml fields are silently ignored. Post-MVP inline
  metadata extension is feasible (no sidecar `projects.toml` needed); fields won't round-trip
  unless added to the struct. (post-MVP decision, not MVP.)

### Spawn path + window/session model
- Spawn entry: `app/src/root_view.rs` `open_launch_config()` (~1230) iterates `launch_config.windows`,
  each → `open_new_with_workspace_source(NewWorkspaceSource::FromTemplate { window_template }, ctx)`
  (~1160) → `ctx.add_window(...)` → `RootView::new`. Global action `"root_view:open_launch_config"`.
- Window UI model: `Workspace` struct in `app/src/workspace/view.rs` (has `window_id: WindowId`).
- In-memory registry: `app/src/workspace/registry.rs` `WorkspaceRegistry { HashMap<WindowId, WeakViewHandle<Workspace>> }`,
  `all_workspaces(ctx) -> Vec<(WindowId, ViewHandle<Workspace>)>`. Registered in `Workspace::new()` (~860).
- Focus: `ctx.windows().show_window_and_focus_app(window_id)`. Close:
  `ctx.windows().close_window(window_id, TerminationMode::ForceTerminate)`.
- DB `Window` model (`crates/persistence/src/model.rs:29`) has no name/source field.
- **Singleton robustness verdict (b is cheap):** add in-memory `source_launch_config: Option<String>`
  to the `Workspace` struct (~30 min, no DB migration). Then "already open" = scan
  `WorkspaceRegistry::all_workspaces()` for a workspace whose `source_launch_config == name`.
  This gives a true-ish singleton without a parallel registry, and avoids stale registry entries.
  (Survives only within a session; cross-restart persistence would need the DB column — defer.)

### Tab Configs (TOML) vs Launch Configs (YAML)
- Tab Configs (`~/.warp/tab_configs/`, `app/src/tab_configs/tab_config.rs`) are newer/preferred,
  flat pane tree, support Agent/Cloud panes, `deny_unknown_fields`.
- **But single-tab by design** (explicit non-goal in spec APP-3575: no multi-tab/multi-window).
- `.dotfiles` project (shell + lazygit + claude across tabs/panes) needs multi-tab.
- **Verdict: base `projects:` on Launch Configs (YAML).** (M1.4 resolved.)

## M2 plan (MVP core)
1. `PaletteMode::Projects` + `QueryFilter::Projects` + `PROJECTS_FILTER_ATOM { "projects:" }`.
2. New `app/src/search/command_palette/projects/` data source: items = `WarpConfig::launch_configs()`,
   MRU-ordered (in-memory `name -> last_accessed` map updated on focus/spawn), open ones marked
   (by scanning `all_workspaces()` for matching `source_launch_config`). Reuse fuzzy filter.
3. Add `source_launch_config: Option<String>` to `Workspace`; set it on spawn through
   `NewWorkspaceSource::FromTemplate`.
4. Enter handler: if a live workspace has `source_launch_config == name` →
   `show_window_and_focus_app`; else fire `open_launch_config` for that config (lazy spawn).
5. Inline "close project" action → `close_window(window_id, ForceTerminate)` + drop registry entry.
6. Register source in `reset_search_mixer()`; enable in `is_mode_enabled()`; add `open_projects_palette()`
   + dispatch case.

## M0 build status (gate)
Lean path (skip ./script/bootstrap; gcloud/powershell/docker not needed for OSS build).
Done: rustup 1.92.0 (active via rust-toolchain.toml), git-lfs pull, brew pkgconf/llvm/clang-format.
BLOCKED on: full Xcode.app install (user) + `cargo install cargo-bundle --git burtonageo` (needs approval).
Build/run via `./script/run` → builds `warp-oss` (no repo access → skips internal channel config).
