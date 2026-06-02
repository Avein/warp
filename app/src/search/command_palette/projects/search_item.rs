use std::sync::Arc;

use ordered_float::OrderedFloat;
use warpui::{AppContext, Element, EntityId, SingletonEntity, WindowId};

use crate::appearance::Appearance;
use crate::launch_configs::launch_config::LaunchConfig;
use crate::search::command_palette::launch_config::renderer::{DiffStats, ProjectRowDetails};
use crate::search::command_palette::mixer::CommandPaletteItemAction;
use crate::search::command_palette::render_util::render_search_item_icon;
use crate::search::result_renderer::ItemHighlightState;
use crate::ui_components::icons::Icon;
use crate::workspace::ProjectOrigin;

/// SearchItem for a row in the `projects:` palette / Alt+Tab switcher.
///
/// A row is one of two kinds, distinguished by [`Self::target`]:
/// - an **open** row targets a concrete live project-tab (a workspace) and the OS window hosting it
///   (an open project or a plain `cmd+n` tab): Enter focuses it and the secondary action closes it;
/// - an **available** row targets a saved [`LaunchConfig`] (a project or a path-less template):
///   Enter focuses-or-spawns it.
///
/// In both cases `launch_config` only supplies the display name; the path/branch detail line comes
/// from [`Self::path`] / [`Self::branch`], and the window/tab description is hidden for open rows
/// (their synthetic config has no windows).
#[derive(Debug)]
pub struct SearchItem {
    launch_config: Arc<LaunchConfig>,
    matched_indices: Vec<usize>,
    sort_score: f64,
    /// Whether this row represents a currently-open project-tab (accent icon + "open" pill).
    is_open: bool,
    /// The live project-tab (workspace) and its host window this row targets, if it is an open row.
    /// `None` for available configs.
    target: Option<(EntityId, WindowId)>,
    /// Home-relative working directory shown below the name.
    path: Option<String>,
    /// Current git branch of the working directory, shown as a pill.
    branch: Option<String>,
    /// Working-tree-vs-HEAD diff stats for the working directory, shown as a `📄 N · +X -Y` pill
    /// next to the branch. `None` when the directory is not a git repo or has no diffable state.
    diff_stats: Option<DiffStats>,
    /// Project origin, picking the row's icon (project vs template vs default). `None` for a plain
    /// (`cmd+n`) window.
    origin: Option<ProjectOrigin>,
}

impl SearchItem {
    /// Builds an available-config row (project or template) that focuses-or-spawns on Enter.
    pub fn available(
        launch_config: Arc<LaunchConfig>,
        matched_indices: Vec<usize>,
        sort_score: f64,
        path: Option<String>,
        branch: Option<String>,
        diff_stats: Option<DiffStats>,
        origin: ProjectOrigin,
    ) -> Self {
        Self {
            launch_config,
            matched_indices,
            sort_score,
            is_open: false,
            target: None,
            path,
            branch,
            diff_stats,
            origin: Some(origin),
        }
    }

    /// Builds an open project-tab row (open project or plain tab) targeting the workspace
    /// `workspace_id` hosted in `window_id`. `origin` is `None` for a plain tab.
    #[allow(clippy::too_many_arguments)]
    pub fn open_window(
        name: String,
        workspace_id: EntityId,
        window_id: WindowId,
        matched_indices: Vec<usize>,
        sort_score: f64,
        path: Option<String>,
        branch: Option<String>,
        diff_stats: Option<DiffStats>,
        origin: Option<ProjectOrigin>,
    ) -> Self {
        Self {
            launch_config: Arc::new(LaunchConfig {
                name,
                active_window_index: None,
                windows: Vec::new(),
            }),
            matched_indices,
            sort_score,
            is_open: true,
            target: Some((workspace_id, window_id)),
            path,
            branch,
            diff_stats,
            origin,
        }
    }
}

impl crate::search::item::SearchItem for SearchItem {
    type Action = CommandPaletteItemAction;

    fn render_icon(
        &self,
        highlight_state: ItemHighlightState,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let theme = appearance.theme();
        // Open rows get an accent-colored icon so they stand out; available configs use the
        // standard foreground color.
        let color = if self.is_open {
            theme.accent().into_solid()
        } else {
            theme.foreground().into_solid()
        };
        // The icon distinguishes a project's origin: a saved project (baked cwds) from a template
        // (path-less, opened at a path) from a default/`newds` session — so two same-named entries
        // from different sources are visually distinct. Plain windows show a terminal glyph.
        let icon = match self.origin {
            Some(ProjectOrigin::Config) => Icon::Folder,
            Some(ProjectOrigin::Template) => Icon::LayoutAlt01,
            Some(ProjectOrigin::Default) => Icon::Navigation,
            Some(ProjectOrigin::Root) => Icon::Gear,
            None => Icon::Terminal,
        };
        render_search_item_icon(appearance, icon, color, highlight_state)
    }

    fn render_item(
        &self,
        highlight_state: ItemHighlightState,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        // Always pass `Some(ProjectRowDetails)`, even for path-less templates (e.g. `default`).
        // The renderer treats the presence of details as "use the projects-palette row style"
        // (60pt row, +2pt name) and renders the path subtitle conditionally — so every row in
        // this palette looks uniform regardless of whether the underlying config has a cwd.
        let project = Some(ProjectRowDetails {
            path: self.path.clone(),
            branch: self.branch.clone(),
            diff_stats: self.diff_stats.clone(),
        });
        // For the projects: palette we deliberately suppress both the "open" chip and the
        // "N windows / N tabs" description pills: the section header (Open Projects / Available)
        // already conveys open-vs-available, and the window/tab count belongs to the regular
        // launch-configs palette, not here. Pass `is_open=false` and `show_description=false`
        // unconditionally — the regular launch-configs palette still flips them on at its own
        // call site.
        self.launch_config.render(
            appearance,
            highlight_state,
            self.matched_indices.clone(),
            false,
            project,
            false,
        )
    }

    fn score(&self) -> OrderedFloat<f64> {
        OrderedFloat::from(self.sort_score)
    }

    fn accept_result(&self) -> Self::Action {
        match self.target {
            Some((workspace_id, window_id)) => CommandPaletteItemAction::FocusWorkspace {
                workspace_id,
                window_id,
            },
            None => CommandPaletteItemAction::FocusOrSpawnProject {
                config: self.launch_config.clone(),
            },
        }
    }

    fn execute_result(&self) -> Self::Action {
        match self.target {
            Some((workspace_id, window_id)) => CommandPaletteItemAction::CloseWorkspace {
                workspace_id,
                window_id,
            },
            // Available configs aren't open, so the secondary "close" action is a no-op.
            None => CommandPaletteItemAction::NoOp,
        }
    }

    fn accessibility_label(&self) -> String {
        format!("Selected project {}.", self.launch_config.name)
    }

    fn accessibility_help_message(&self) -> Option<String> {
        Some(
            "Press enter to open or focus this project; press the secondary action to close it."
                .into(),
        )
    }

    fn tooltip(&self) -> Option<String> {
        if self.is_open {
            Some("Open — enter to focus, secondary action to close".into())
        } else {
            Some("Enter to open in a new window".into())
        }
    }
}
