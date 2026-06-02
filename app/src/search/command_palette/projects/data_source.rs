use std::path::{Path, PathBuf};
use std::sync::Arc;

use fuzzy_match::match_indices_case_insensitive;
use warpui::{AppContext, Entity, EntityId, SingletonEntity, WindowId};

use crate::launch_configs::launch_config::LaunchConfig;
use crate::root_view::synthetic_root_config;
use crate::search::command_palette::launch_config::renderer::DiffStats;
use crate::search::command_palette::mixer::CommandPaletteItemAction;
use crate::search::command_palette::projects::search_item::SearchItem;
use crate::search::command_palette::separator_search_item::SeparatorSearchItem;
use crate::search::data_source::{Query, QueryResult};
use crate::search::mixer::{DataSourceRunErrorWrapper, SyncDataSource};
use crate::user_config::WarpConfig;
use crate::workspace::{ProjectOrigin, ProjectSwitcher, WorkspaceRegistry};

/// Which surface this data source is currently feeding.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// The `projects:` palette (⌃⌘P): three sections — open projects, open plain windows, and
    /// available projects/templates — with section headers.
    #[default]
    Palette,
    /// The Alt+Tab switcher: a flat, MRU-ordered list of open *project* windows (current included
    /// at the front), like an OS window switcher. No plain windows, no available configs.
    AltTab,
}

/// Datasource for the projects switcher surfaces.
///
/// Reads saved launch configs together with live window state ([`WorkspaceRegistry`] +
/// [`ProjectSwitcher`] stamps + MRU) at query time, so the list always reflects which windows are
/// open without needing to subscribe to events.
///
/// A project-tab is either *stamped* (a [`crate::workspace::ProjectIdentity`] from the palette /
/// `newds` / `cmd+shift+N` / synthetic-root auto-spawn) or *plain* `cmd+n` (unstamped). Saved
/// launch configs that have no live stamped tab are "available": a config with baked-in `cwd`s is
/// a project, one without is a path-less template.
#[derive(Default)]
pub struct DataSource {
    surface: Surface,
}

impl DataSource {
    pub fn new() -> Self {
        Self::default()
    }

    /// Selects which surface this source feeds (see [`Surface`]).
    pub fn set_surface(&mut self, surface: Surface) {
        self.surface = surface;
    }
}

/// A resolved open project-tab row (project or plain), carrying everything needed to render and
/// target it without re-reading app state. A project-tab is a workspace; `window_id` is the OS
/// window that hosts it (needed to focus the window and switch to the tab).
struct OpenRow {
    name: String,
    workspace_id: EntityId,
    window_id: WindowId,
    path: Option<String>,
    branch: Option<String>,
    /// Working-tree diff stats vs HEAD for the row's cwd, when it is a git repo. Cached at query
    /// time alongside `branch` so the palette doesn't reach into libgit2 twice per row.
    diff_stats: Option<DiffStats>,
    /// Project origin for the row's icon; `None` for plain (`cmd+n`) tabs.
    origin: Option<ProjectOrigin>,
}

impl SyncDataSource for DataSource {
    type Action = CommandPaletteItemAction;

    fn run_query(
        &self,
        query: &Query,
        app: &AppContext,
    ) -> Result<Vec<QueryResult<Self::Action>>, DataSourceRunErrorWrapper> {
        let term = query.text.trim().to_lowercase();
        let switcher = ProjectSwitcher::as_ref(app);
        let registry = WorkspaceRegistry::as_ref(app);
        let configs = WarpConfig::as_ref(app).launch_configs();
        let active_window = app.windows().active_window();
        // The active project-tab is the active workspace of the active window.
        let active_workspace = active_window.and_then(|id| registry.active_id(id));

        // Open project-tabs, MRU order (most recent first), each resolved to a display row.
        let open_projects: Vec<OpenRow> = switcher
            .projects_mru(app)
            .into_iter()
            .filter_map(|workspace_id| {
                let window_id = registry.window_for_workspace(workspace_id, app)?;
                let identity = switcher.identity(workspace_id);
                let name = identity
                    .map(|i| i.name.clone())
                    .unwrap_or_else(|| "project".to_string());
                // Stamped path for projects; fall back to live cwd for plain tabs. Reads from the
                // workspace itself rather than the window's active session, so a background tab
                // shows its own directory, not the active one's.
                let cwd = identity
                    .map(|i| i.path.clone())
                    .or_else(|| workspace_cwd(workspace_id, app));
                let (path, branch, diff_stats) = path_details(cwd.as_deref());
                Some(OpenRow {
                    name,
                    workspace_id,
                    window_id,
                    path,
                    branch,
                    diff_stats,
                    origin: identity.map(|i| i.origin.clone()),
                })
            })
            .collect();

        // The "Available" section matches saved configs against open project display names. Since
        // sequence-named templates (`default-1`, `simple_template-1`) and config names are kept
        // distinct by construction, a plain set-difference is enough.
        let open_project_names: Vec<String> =
            open_projects.iter().map(|r| r.name.clone()).collect();

        // The Alt+Tab switcher lists open project-tabs, flat and MRU-ordered, with the *active*
        // project dropped — so the top item (selected at offset 0) is the most-recently-used other
        // project. A single Alt+Tab toggles to it; switching touches MRU, so the next Alt+Tab
        // toggles back (X↔Y). Holding Option and tapping walks further down the MRU list.
        if self.surface == Surface::AltTab {
            let rows: Vec<OpenRow> = open_projects
                .into_iter()
                .filter(|row| Some(row.workspace_id) != active_workspace)
                .collect();
            // Higher score sorts to the top, so assign descending scores to preserve MRU order.
            let len = rows.len();
            let items = rows
                .into_iter()
                .enumerate()
                .map(|(idx, row)| open_window_item(row, (len - idx) as f64))
                .collect();
            return Ok(items);
        }

        // Open plain (cmd+n) tabs: every live workspace that is not a stamped project, sorted by
        // name for a stable listing.
        let mut open_windows: Vec<OpenRow> = registry
            .all_workspaces(app)
            .into_iter()
            .filter(|(_, workspace)| !switcher.is_project(workspace.id()))
            .map(|(window_id, workspace)| {
                let cwd = workspace_cwd(workspace.id(), app);
                let name = cwd
                    .as_deref()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "window".to_string());
                let (path, branch, diff_stats) = path_details(cwd.as_deref());
                OpenRow {
                    name,
                    workspace_id: workspace.id(),
                    window_id,
                    path,
                    branch,
                    diff_stats,
                    origin: None,
                }
            })
            .collect();
        open_windows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        // Available configs: every saved config whose project is not already open (matched by name),
        // plus a synthetic `root` entry whenever no `Config { config_name: "root" }` tab is live —
        // so the user can reopen root from the palette after closing it. The synthetic config lives
        // for the duration of this query only; selecting it dispatches through the shared
        // `focus_or_spawn_project` path with the same identity the startup auto-spawn would stamp.
        let root_is_open = open_projects.iter().any(|row| {
            matches!(
                &row.origin,
                Some(ProjectOrigin::Config { config_name }) if config_name == "root"
            )
        });
        let synthetic_root = (!root_is_open).then(synthetic_root_config);
        let mut available: Vec<&LaunchConfig> = configs
            .iter()
            .filter(|config| !open_project_names.contains(&config.name))
            .collect();
        if let Some(root) = synthetic_root.as_ref() {
            available.push(root);
        }
        available.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        // Build the three sections, keeping their separators in both the empty-query and typed-query
        // cases. For a typed query each section is fuzzy-filtered and sorted by relevance; for an
        // empty query the natural order (MRU / alphabetical) is kept. Either way the rows share
        // score 0.0, so the mixer preserves insertion order and the headers stay in place — this is
        // what lets you tell same-named open/available entries apart while typing.
        let open_project_items = open_window_section(&open_projects, &term);
        let open_window_items = open_window_section(&open_windows, &term);
        let available_items = available_section(&available, &term);

        Ok(assemble_sections(
            open_project_items,
            open_window_items,
            available_items,
        ))
    }
}

impl Entity for DataSource {
    type Event = ();
}

/// Builds the rendered rows for an open-window section (open projects or open plain windows), in
/// display order. With a non-empty `term` only fuzzy-matching rows are kept, sorted by match score
/// (best first); with an empty term every row is kept in its given order. All rows get score 0.0 so
/// the mixer preserves this order and the section separators stay put.
fn open_window_section(rows: &[OpenRow], term: &str) -> Vec<QueryResult<CommandPaletteItemAction>> {
    if term.is_empty() {
        return rows
            .iter()
            .map(|row| open_window_item_ref(row, Vec::new()))
            .collect();
    }
    let mut matched: Vec<(f64, &OpenRow, Vec<usize>)> = rows
        .iter()
        .filter_map(|row| {
            let result = match_indices_case_insensitive(&row.name, term)?;
            Some((result.score as f64, row, result.matched_indices))
        })
        .collect();
    matched.sort_by(|a, b| b.0.total_cmp(&a.0));
    matched
        .into_iter()
        .map(|(_, row, indices)| open_window_item_ref(row, indices))
        .collect()
}

/// Builds the rendered rows for the "Available" section (saved projects + templates not currently
/// open), in display order. Filtering/sorting mirrors [`open_window_section`].
fn available_section(
    configs: &[&LaunchConfig],
    term: &str,
) -> Vec<QueryResult<CommandPaletteItemAction>> {
    let make = |config: &LaunchConfig, indices: Vec<usize>| {
        let (path, branch, diff_stats) = path_details(config.primary_cwd());
        // A path-less config is a template; one with baked cwds is a project. The origin's tag
        // carries the underlying name so a later `focus_or_spawn_project` finds the existing tab.
        let origin = if config.is_template() {
            ProjectOrigin::Template {
                template_name: config.name.clone(),
            }
        } else {
            ProjectOrigin::Config {
                config_name: config.name.clone(),
            }
        };
        QueryResult::from(SearchItem::available(
            Arc::new(config.clone()),
            indices,
            0.0,
            path,
            branch,
            diff_stats,
            origin,
        ))
    };
    if term.is_empty() {
        return configs
            .iter()
            .map(|config| make(config, Vec::new()))
            .collect();
    }
    let mut matched: Vec<(f64, &LaunchConfig, Vec<usize>)> = configs
        .iter()
        .filter_map(|config| {
            let result = match_indices_case_insensitive(&config.name, term)?;
            Some((result.score as f64, *config, result.matched_indices))
        })
        .collect();
    matched.sort_by(|a, b| b.0.total_cmp(&a.0));
    matched
        .into_iter()
        .map(|(_, config, indices)| make(config, indices))
        .collect()
}

/// Assembles the three pre-built sections into the final result list, inserting a header above each
/// non-empty section when more than one section is present. The palette renders insertion order
/// bottom-to-top, so sections are pushed bottom-first (Available, Open Windows, Open Projects) and
/// each section's items are reversed, with its header pushed last so it lands on top of its group.
fn assemble_sections(
    open_projects: Vec<QueryResult<CommandPaletteItemAction>>,
    open_windows: Vec<QueryResult<CommandPaletteItemAction>>,
    available: Vec<QueryResult<CommandPaletteItemAction>>,
) -> Vec<QueryResult<CommandPaletteItemAction>> {
    let has_projects = !open_projects.is_empty();
    let has_windows = !open_windows.is_empty();
    let has_available = !available.is_empty();
    let show_headers = [has_projects, has_windows, has_available]
        .iter()
        .filter(|present| **present)
        .count()
        > 1;

    let mut results: Vec<QueryResult<CommandPaletteItemAction>> = Vec::new();

    results.extend(available.into_iter().rev());
    if show_headers && has_available {
        results.push(SeparatorSearchItem::new("Available".to_string()).into());
    }

    results.extend(open_windows.into_iter().rev());
    if show_headers && has_windows {
        results.push(SeparatorSearchItem::new("Open Windows".to_string()).into());
    }

    results.extend(open_projects.into_iter().rev());
    if show_headers && has_projects {
        results.push(SeparatorSearchItem::new("Open Projects".to_string()).into());
    }

    results
}

/// Builds a palette row (with an explicit score, used by the flat Alt+Tab list) that focuses
/// (Enter) / closes (secondary) an open project-tab.
fn open_window_item(row: OpenRow, score: f64) -> QueryResult<CommandPaletteItemAction> {
    QueryResult::from(SearchItem::open_window(
        row.name,
        row.workspace_id,
        row.window_id,
        Vec::new(),
        score,
        row.path,
        row.branch,
        row.diff_stats,
        row.origin,
    ))
}

/// Builds a palette row for an open project-tab in a grouped section (score 0.0 so insertion order
/// is preserved), optionally highlighting `matched_indices` from a typed query.
fn open_window_item_ref(
    row: &OpenRow,
    matched_indices: Vec<usize>,
) -> QueryResult<CommandPaletteItemAction> {
    QueryResult::from(SearchItem::open_window(
        row.name.clone(),
        row.workspace_id,
        row.window_id,
        matched_indices,
        0.0,
        row.path.clone(),
        row.branch.clone(),
        row.diff_stats.clone(),
        row.origin.clone(),
    ))
}

/// The live working directory of a specific workspace's (project-tab's) active session, if local.
/// Reads the workspace directly so a background tab reports its own cwd, not the window's active one.
fn workspace_cwd(workspace_id: EntityId, app: &AppContext) -> Option<PathBuf> {
    WorkspaceRegistry::as_ref(app)
        .workspace_handle(workspace_id, app)?
        .as_ref(app)
        .active_session_path(app)
}

/// Computes the home-relative path, current git branch, and working-tree diff stats for a working
/// directory, for the palette detail line.
fn path_details(cwd: Option<&Path>) -> (Option<String>, Option<String>, Option<DiffStats>) {
    let Some(cwd) = cwd else {
        return (None, None, None);
    };
    let path = Some(warp_core::paths::home_relative_path(cwd));
    (path, current_branch(cwd), current_diff_stats(cwd))
}

/// Returns the current git branch (or short commit for a detached HEAD) of the repo containing
/// `cwd`. Opening the repo and reading HEAD does not scan the working tree, so this stays cheap
/// enough to run on each query.
fn current_branch(cwd: &Path) -> Option<String> {
    let repo = git2::Repository::discover(cwd).ok()?;
    let head = repo.head().ok()?;
    head.shorthand().map(str::to_owned)
}

/// Returns the working-tree diff stats vs HEAD (files changed, insertions, deletions) for the repo
/// containing `cwd`. Returns `None` for a non-git path, an unborn HEAD, or a clean working tree —
/// callers treat all three the same: hide the pill.
fn current_diff_stats(cwd: &Path) -> Option<DiffStats> {
    let repo = git2::Repository::discover(cwd).ok()?;
    let head_tree = repo.head().ok()?.peel_to_tree().ok()?;
    let diff = repo
        .diff_tree_to_workdir_with_index(Some(&head_tree), None)
        .ok()?;
    let stats = diff.stats().ok()?;
    let files = stats.files_changed() as u32;
    let insertions = stats.insertions() as u32;
    let deletions = stats.deletions() as u32;
    if files == 0 && insertions == 0 && deletions == 0 {
        return None;
    }
    Some(DiffStats {
        files,
        insertions,
        deletions,
    })
}
