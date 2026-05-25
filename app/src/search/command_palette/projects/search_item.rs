use std::sync::Arc;

use ordered_float::OrderedFloat;
use warpui::{AppContext, Element, SingletonEntity, WindowId};

use crate::appearance::Appearance;
use crate::launch_configs::launch_config::LaunchConfig;
use crate::search::command_palette::launch_config::renderer::ProjectRowDetails;
use crate::search::command_palette::mixer::CommandPaletteItemAction;
use crate::search::command_palette::render_util::render_search_item_icon;
use crate::search::result_renderer::ItemHighlightState;
use crate::ui_components::icons::Icon;
use crate::workspace::ProjectOrigin;

/// SearchItem for a row in the `projects:` palette / Alt+Tab switcher.
///
/// A row is one of two kinds, distinguished by [`Self::target_window`]:
/// - an **open** row targets a concrete live window (an open project or a plain `cmd+n` window):
///   Enter focuses it and the secondary action closes it;
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
    /// Whether this row represents a currently-open window (accent icon + "open" pill).
    is_open: bool,
    /// The live window this row targets, if it is an open row. `None` for available configs.
    target_window: Option<WindowId>,
    /// Home-relative working directory shown below the name.
    path: Option<String>,
    /// Current git branch of the working directory, shown as a pill.
    branch: Option<String>,
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
        origin: ProjectOrigin,
    ) -> Self {
        Self {
            launch_config,
            matched_indices,
            sort_score,
            is_open: false,
            target_window: None,
            path,
            branch,
            origin: Some(origin),
        }
    }

    /// Builds an open-window row (open project or plain window) targeting `window_id`. `origin` is
    /// `None` for a plain window.
    pub fn open_window(
        name: String,
        window_id: WindowId,
        matched_indices: Vec<usize>,
        sort_score: f64,
        path: Option<String>,
        branch: Option<String>,
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
            target_window: Some(window_id),
            path,
            branch,
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
        let project = self.path.clone().map(|path| ProjectRowDetails {
            path,
            branch: self.branch.clone(),
        });
        // Open rows have a synthetic (window-less) config, so suppress the "N windows / N tabs"
        // description; available configs show it.
        self.launch_config.render(
            appearance,
            highlight_state,
            self.matched_indices.clone(),
            self.is_open,
            project,
            self.target_window.is_none(),
        )
    }

    fn score(&self) -> OrderedFloat<f64> {
        OrderedFloat::from(self.sort_score)
    }

    fn accept_result(&self) -> Self::Action {
        match self.target_window {
            Some(window_id) => CommandPaletteItemAction::FocusWindow { window_id },
            None => CommandPaletteItemAction::FocusOrSpawnProject {
                config: self.launch_config.clone(),
            },
        }
    }

    fn execute_result(&self) -> Self::Action {
        match self.target_window {
            Some(window_id) => CommandPaletteItemAction::CloseWindow { window_id },
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
