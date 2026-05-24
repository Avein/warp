use std::sync::Arc;

use ordered_float::OrderedFloat;
use warpui::{AppContext, Element, SingletonEntity};

use crate::appearance::Appearance;
use crate::launch_configs::launch_config::LaunchConfig;
use crate::search::command_palette::launch_config::renderer::ProjectRowDetails;
use crate::search::command_palette::mixer::CommandPaletteItemAction;
use crate::search::command_palette::render_util::render_search_item_icon;
use crate::search::result_renderer::ItemHighlightState;
use crate::ui_components::icons::Icon;

/// SearchItem for a project (a saved [`LaunchConfig`]) in the `projects:` palette.
///
/// The `sort_score` is computed by the data source: for an empty query it encodes MRU order, and
/// for a non-empty query it is the fuzzy match score so relevance dominates. `matched_indices`
/// drives name highlighting; it is empty for the unfiltered (MRU) listing.
#[derive(Debug)]
pub struct SearchItem {
    launch_config: Arc<LaunchConfig>,
    matched_indices: Vec<usize>,
    sort_score: f64,
    /// Whether this project currently has a live window opened via the switcher.
    is_open: bool,
    /// Home-relative working directory of the project's first pane, shown below the name.
    path: Option<String>,
    /// Current git branch of the project's working directory, shown as a pill.
    branch: Option<String>,
}

impl SearchItem {
    pub fn new(
        launch_config: Arc<LaunchConfig>,
        matched_indices: Vec<usize>,
        sort_score: f64,
        is_open: bool,
        path: Option<String>,
        branch: Option<String>,
    ) -> Self {
        Self {
            launch_config,
            matched_indices,
            sort_score,
            is_open,
            path,
            branch,
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
        // Open projects get an accent-colored icon so they stand out in the list; closed ones use
        // the standard foreground color.
        let color = if self.is_open {
            theme.accent().into_solid()
        } else {
            theme.foreground().into_solid()
        };
        render_search_item_icon(appearance, Icon::Navigation, color, highlight_state)
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
        self.launch_config.render(
            appearance,
            highlight_state,
            self.matched_indices.clone(),
            self.is_open,
            project,
        )
    }

    fn score(&self) -> OrderedFloat<f64> {
        OrderedFloat::from(self.sort_score)
    }

    fn accept_result(&self) -> Self::Action {
        CommandPaletteItemAction::FocusOrSpawnProject {
            config: self.launch_config.clone(),
        }
    }

    fn execute_result(&self) -> Self::Action {
        CommandPaletteItemAction::CloseProject {
            config: self.launch_config.clone(),
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
