//! Renderer for one tab in the project-tab bar
//! ([`crate::workspace::view::Workspace::render_project_bar`]).
//!
//! Visually this mirrors the session-tab strip with `FeatureFlag::NewTabStyling` on
//! ([`crate::tab::TabComponent::render_tab_container_internal`]): rectangular tabs, no corner
//! radius, side-only borders (right always, left only on the first tab) so adjacent tabs share
//! a single 1pt separator. The one departure: active background is a 15% accent tint, not the
//! session strip's neutral `fg_overlay_2` — so the user can tell at a glance which strip they're
//! acting on. A leading [`Icon`] keyed off [`ProjectOrigin`] gives the project type away, and a
//! close `×` appears on hover and dispatches `root_view:close_project_workspace`.

use warp_core::ui::theme::color::internal_colors;
use warpui::elements::{
    Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Fill, Flex, Hoverable,
    MainAxisAlignment, MainAxisSize, MouseStateHandle, ParentElement, Radius, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::platform::Cursor;
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use warpui::ui_components::text_input::TextInput;
use warpui::{Element, EntityId, ViewHandle, WindowId};

use crate::appearance::Appearance;
use crate::editor::EditorView;
use crate::root_view::CloseWorkspaceArg;
use crate::ui_components::icons::Icon;
use crate::workspace::project_icon::icon_for_origin;
use crate::workspace::ProjectOrigin;

/// 13pt — matches the default `ui_font_size` used by session tabs.
pub const PROJECT_TAB_LABEL_FONT_SIZE: f32 = 13.;

/// Max width of the inline rename editor field inside a pill. Wide enough for
/// typical project names (≈18 chars at 13pt), narrow enough that the editor +
/// icon stay visually centered inside a normal-width pill via the parent
/// `Flex`'s `MainAxisAlignment::Center`. See `ProjectTabComponent::render` for
/// why a bounded width matters (avoids the editor element's infinite-width
/// panic in `editor/view/element.rs:1659`).
const PROJECT_TAB_EDITOR_MAX_WIDTH: f32 = 140.;

/// 15% accent opacity (38 / 255 ≈ 0.149) — loud enough to read as a "selected" pill, quiet
/// enough not to overpower an active session tab below.
const ACTIVE_BG_OPACITY: u8 = 38;

/// Mouse state for one project tab. The `pill` handle drives the tab body's hover (and gates the
/// close-`×` visibility); the `close` handle drives the inner `×` button's own hover tint.
#[derive(Default, Clone)]
pub struct ProjectTabMouseStates {
    pub pill: MouseStateHandle,
    pub close: MouseStateHandle,
}

/// Per-tab data, by-value so the render closures don't need lifetimes.
pub struct ProjectTabComponent<'a> {
    pub label: String,
    pub origin: Option<ProjectOrigin>,
    pub is_active: bool,
    /// Leftmost tab in the bar. Only the first tab draws a left border, so adjacent tabs share a
    /// single 1pt vertical separator (same trick session tabs use, see `tab.rs:1552-1558`).
    pub is_first: bool,
    pub view_id: EntityId,
    pub window_id: WindowId,
    pub tab_mouse_state: MouseStateHandle,
    pub close_mouse_state: MouseStateHandle,
    pub appearance: &'a Appearance,
    /// When `Some`, this pill is in rename mode: the label span is replaced
    /// by an inline editor. The editor's lifecycle (focus, commit, cancel)
    /// is owned by the workspace via
    /// `Workspace::handle_project_tab_rename_editor_event`. See
    /// `docs/projects-rename.md`.
    pub rename_editor: Option<ViewHandle<EditorView>>,
}

impl<'a> ProjectTabComponent<'a> {
    pub fn render(self) -> Box<dyn Element> {
        let theme = self.appearance.theme();
        let label_color = theme.foreground();
        let close_color = theme.sub_text_color(theme.background());
        let ui_font_family = self.appearance.ui_font_family();
        let icon_glyph = icon_for_origin(self.origin.as_ref());
        let is_active = self.is_active;
        let is_first = self.is_first;
        let view_id = self.view_id;
        let window_id = self.window_id;
        let rename_editor = self.rename_editor;

        let active_border_fill = internal_colors::fg_overlay_2(theme);
        let inactive_border_fill = internal_colors::fg_overlay_1(theme);

        let is_renaming = rename_editor.is_some();

        let hoverable = Hoverable::new(self.tab_mouse_state, {
            let close_state = self.close_mouse_state;
            let label = self.label;
            let rename_editor = rename_editor.clone();
            move |state| {
                // `MainAxisSize::Max` + `MainAxisAlignment::Center` centers the icon + label inside
                // the tab's slot. Without Max the row collapses to intrinsic width and there's
                // nothing to center within. Same pattern session tabs use
                // (`tab.rs::render_tab_container_internal::full_tab_content`).
                let mut inner = Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_main_axis_size(MainAxisSize::Max);

                inner.add_child(
                    Container::new(
                        ConstrainedBox::new(icon_glyph.to_warpui_icon(label_color).finish())
                            .with_width(14.)
                            .with_height(14.)
                            .finish(),
                    )
                    .with_margin_right(6.)
                    .finish(),
                );

                if let Some(editor) = rename_editor.clone() {
                    // Rename in progress on this pill: swap the label span for an inline editor.
                    // Outer pill chrome (icon, border, active tint) stays untouched per
                    // `docs/projects-rename.md`. Transparent background and zero radius/border
                    // keep the editor visually flush with the pill it replaces.
                    //
                    // Width plumbing — three things have to hold simultaneously:
                    //
                    // - The editor element panics on an infinite-width constraint
                    //   (`editor/view/element.rs:1659`), so SOMETHING above it must give it a
                    //   bounded max width.
                    // - We want the icon + editor to stay *centered as a group* (like the
                    //   inactive icon + label combo). A `Shrinkable(1.0, …)` would make the
                    //   editor greedy and push the icon to the left edge.
                    // - The editor needs enough room for a reasonable name length.
                    //
                    // `ConstrainedBox::with_max_width(PROJECT_TAB_EDITOR_MAX_WIDTH)` solves all
                    // three: gives the editor a bounded width, leaves the icon next to it for
                    // the Flex's `MainAxisAlignment::Center` to center together, and is wide
                    // enough for typical names.
                    //
                    // 8pt top margin is the same offset session-tab rename uses to optically
                    // center the editor in a 34pt tab bar — see `tab.rs::render_tab_content`.
                    inner.add_child(
                        ConstrainedBox::new(
                            TextInput::new(
                                editor,
                                UiComponentStyles::default()
                                    .set_background(Fill::None)
                                    .set_border_radius(CornerRadius::with_all(Radius::Pixels(0.)))
                                    .set_border_width(0.),
                            )
                            .with_style(UiComponentStyles {
                                margin: Some(Coords::default().top(8.)),
                                ..Default::default()
                            })
                            .build()
                            .finish(),
                        )
                        .with_max_width(PROJECT_TAB_EDITOR_MAX_WIDTH)
                        .finish(),
                    );
                } else {
                    let mut text = Text::new_inline(
                        label.clone(),
                        ui_font_family,
                        PROJECT_TAB_LABEL_FONT_SIZE,
                    )
                    .with_color(label_color.into());
                    if is_active {
                        text = text.with_style(Properties::default().weight(Weight::Medium));
                    }
                    inner.add_child(text.finish());
                }

                // Close `×` only while the tab is hovered. Inner click handler stops the event
                // inside the X's bounds so it doesn't bubble to the tab-activate handler below.
                if state.is_hovered() {
                    let close_state = close_state.clone();
                    let close_btn = Hoverable::new(close_state, move |close_hover| {
                        let mut wrap = Container::new(
                            ConstrainedBox::new(Icon::X.to_warpui_icon(close_color).finish())
                                .with_width(14.)
                                .with_height(14.)
                                .finish(),
                        )
                        .with_uniform_padding(2.)
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(2.)));
                        if close_hover.is_hovered() {
                            wrap = wrap.with_background(internal_colors::fg_overlay_2(theme));
                        }
                        wrap.finish()
                    })
                    .with_cursor(Cursor::PointingHand)
                    .on_click(move |ctx, _app, _v2f| {
                        ctx.dispatch_action(
                            "root_view:close_project_workspace",
                            CloseWorkspaceArg {
                                workspace_id: view_id,
                                window_id,
                            },
                        );
                    })
                    .finish();
                    inner.add_child(Container::new(close_btn).with_margin_left(6.).finish());
                }

                // Active = 15% accent tint (the "selected" pill). Hover = `fg_overlay_1` lift.
                // Inactive = no background (strip's own `fg_overlay_1` shows through).
                let background = if is_active {
                    theme.accent().with_opacity(ACTIVE_BG_OPACITY).into()
                } else if state.is_hovered() {
                    internal_colors::fg_overlay_1(theme).into()
                } else {
                    Fill::None
                };

                // Sides: top/bottom off (tab shares strip's top/bottom edges), right always on
                // (separator to next tab), left only on the first tab (no double 1pt lines).
                let border_fill = if is_active {
                    active_border_fill
                } else {
                    inactive_border_fill
                };
                let border = Border::all(1.)
                    .with_sides(false, is_first, false, true)
                    .with_border_fill(border_fill);

                Container::new(inner.finish())
                    .with_horizontal_padding(8.)
                    .with_background(background)
                    .with_border(border)
                    .finish()
            }
        });

        // While the pill hosts the rename editor, the pill body should not
        // dispatch `activate_project_tab` on click — that path refocuses the
        // workspace and would yank focus away from the editor. The close `×`
        // keeps its own handler (inner element with `stop_propagation`-style
        // behavior via being a separate Hoverable above).
        if is_renaming {
            hoverable.finish()
        } else {
            hoverable
                .with_cursor(Cursor::PointingHand)
                .on_click(move |ctx, _app, _v2f| {
                    match project_tab_click_outcome(is_active, 1) {
                        ProjectTabClickOutcome::Activate => {
                            ctx.dispatch_action("root_view:activate_project_tab", view_id);
                        }
                        ProjectTabClickOutcome::OpenRename | ProjectTabClickOutcome::Noop => {
                            // Single click on active pill is a no-op; rename only
                            // triggers on the second click of a pair (handled below
                            // by `on_double_click`).
                        }
                    }
                })
                // Second click of a double-click pair: by the time this fires the
                // first click has already activated the pill (see Hoverable
                // semantics in `crates/warpui_core/src/elements/hoverable.rs:644`),
                // so the gate is always "rename this pill". Mirrors the macOS
                // Finder "click selects, click on selected renames" gesture.
                .on_double_click(move |ctx, _app, _v2f| {
                    if matches!(
                        project_tab_click_outcome(is_active, 2),
                        ProjectTabClickOutcome::OpenRename
                    ) {
                        ctx.dispatch_action("root_view:rename_project_tab", view_id);
                    }
                })
                .finish()
        }
    }
}

/// Outcome of a click on a project-tab pill body. Pure decision so the gesture
/// matrix from `docs/issues/projects-rename-05-doubleclick-active-pill.md` is
/// unit-testable without spinning up a workspace.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ProjectTabClickOutcome {
    /// Activate this workspace (single click on inactive pill).
    Activate,
    /// Open the rename editor on this pill (double click on the active pill, or
    /// the second click of an inactive → active → rename sequence — both end
    /// with the pill being active when the second click fires).
    OpenRename,
    /// Do nothing (single click on the already-active pill).
    Noop,
}

/// Maps `(is_active, click_count)` to a [`ProjectTabClickOutcome`]. Centralizes
/// the gesture matrix described in `docs/projects-rename.md` so both the
/// single-click and double-click handlers can stay one-liners.
pub(crate) fn project_tab_click_outcome(
    is_active: bool,
    click_count: u32,
) -> ProjectTabClickOutcome {
    match (is_active, click_count) {
        (false, 1) => ProjectTabClickOutcome::Activate,
        (true, 1) => ProjectTabClickOutcome::Noop,
        // Hoverable activates the pill on the first click of a pair, so by the
        // second click `is_active` is true regardless of where the pair started.
        (_, _) => ProjectTabClickOutcome::OpenRename,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_click_on_inactive_activates() {
        assert_eq!(
            project_tab_click_outcome(false, 1),
            ProjectTabClickOutcome::Activate,
        );
    }

    #[test]
    fn single_click_on_active_is_noop() {
        assert_eq!(
            project_tab_click_outcome(true, 1),
            ProjectTabClickOutcome::Noop,
        );
    }

    #[test]
    fn double_click_on_active_opens_rename() {
        assert_eq!(
            project_tab_click_outcome(true, 2),
            ProjectTabClickOutcome::OpenRename,
        );
    }

    #[test]
    fn second_click_after_activation_opens_rename() {
        // Double-click on a pill that started inactive: first click activates
        // (re-render flips `is_active` to true), second click sees the pill as
        // already active and opens the editor. Same outcome as the active-only
        // case above — that's the point.
        assert_eq!(
            project_tab_click_outcome(true, 2),
            ProjectTabClickOutcome::OpenRename,
        );
    }
}
