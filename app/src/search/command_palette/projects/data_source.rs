use std::cmp::Ordering;
use std::path::Path;
use std::sync::Arc;

use fuzzy_match::match_indices_case_insensitive;
use warpui::{AppContext, Entity, SingletonEntity};

use crate::launch_configs::launch_config::LaunchConfig;
use crate::search::command_palette::mixer::CommandPaletteItemAction;
use crate::search::command_palette::projects::search_item::SearchItem;
use crate::search::data_source::{Query, QueryResult};
use crate::search::mixer::{DataSourceRunErrorWrapper, SyncDataSource};
use crate::user_config::WarpConfig;
use crate::workspace::ProjectSwitcher;

/// Datasource for the `projects:` palette.
///
/// Reads saved launch configs together with live switcher state (open windows + MRU order) at
/// query time, so the list always reflects which projects are currently open without needing to
/// subscribe to events.
pub struct DataSource;

impl DataSource {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DataSource {
    fn default() -> Self {
        Self::new()
    }
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
        let configs = WarpConfig::as_ref(app).launch_configs();

        let items: Vec<SearchItem> = if term.is_empty() {
            // Unfiltered: most-recently-used projects first, then the rest alphabetically.
            let mut ordered: Vec<_> = configs.iter().collect();
            ordered.sort_by(|a, b| {
                match (switcher.mru_rank(&a.name), switcher.mru_rank(&b.name)) {
                    (Some(x), Some(y)) => x.cmp(&y),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                }
            });

            let len = ordered.len();
            ordered
                .into_iter()
                .enumerate()
                .map(|(idx, config)| {
                    let is_open = switcher.live_window(&config.name, app).is_some();
                    // Higher score sorts first; assign descending scores to preserve MRU order.
                    let sort_score = (len - idx) as f64;
                    let (path, branch) = project_details(config);
                    SearchItem::new(
                        Arc::new(config.clone()),
                        Vec::new(),
                        sort_score,
                        is_open,
                        path,
                        branch,
                    )
                })
                .collect()
        } else {
            // Filtered: rank by fuzzy relevance so a typed query surfaces the right project.
            configs
                .iter()
                .filter_map(|config| {
                    let result = match_indices_case_insensitive(&config.name, &term)?;
                    let is_open = switcher.live_window(&config.name, app).is_some();
                    let (path, branch) = project_details(config);
                    Some(SearchItem::new(
                        Arc::new(config.clone()),
                        result.matched_indices,
                        result.score as f64,
                        is_open,
                        path,
                        branch,
                    ))
                })
                .collect()
        };

        Ok(items.into_iter().map(QueryResult::from).collect())
    }
}

impl Entity for DataSource {
    type Event = ();
}

/// Computes the home-relative path and current git branch for a project's primary working
/// directory, for display in the palette row.
fn project_details(config: &LaunchConfig) -> (Option<String>, Option<String>) {
    let Some(cwd) = config.primary_cwd() else {
        return (None, None);
    };
    let path = Some(warp_core::paths::home_relative_path(cwd));
    (path, current_branch(cwd))
}

/// Returns the current git branch (or short commit for a detached HEAD) of the repo containing
/// `cwd`. Opening the repo and reading HEAD does not scan the working tree, so this stays cheap
/// enough to run on each query.
fn current_branch(cwd: &Path) -> Option<String> {
    let repo = git2::Repository::discover(cwd).ok()?;
    let head = repo.head().ok()?;
    head.shorthand().map(str::to_owned)
}
