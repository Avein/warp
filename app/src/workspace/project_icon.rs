//! Single source of truth for the `ProjectOrigin → Icon` mapping shared between the project bar
//! ([`super::project_tab`]) and the projects palette
//! ([`crate::search::command_palette::projects::search_item`]).
//!
//! Kept in one place so the two surfaces never drift visually.

use crate::ui_components::icons::Icon;
use crate::workspace::ProjectOrigin;

/// Returns the glyph that identifies a project-tab's origin. `None` means an unstamped (plain
/// `cmd+n`) tab and renders as the terminal glyph.
pub fn icon_for_origin(origin: Option<&ProjectOrigin>) -> Icon {
    match origin {
        Some(ProjectOrigin::Config { .. }) => Icon::Folder,
        Some(ProjectOrigin::Template { .. }) => Icon::LayoutAlt01,
        None => Icon::Terminal,
    }
}
