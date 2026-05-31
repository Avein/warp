//! Self-contained renderer for a single tab in the **project-tab bar**
//! (`app/src/root_view.rs::render_project_bar`).
//!
//! This is *not* the session-tab renderer. Session tabs live in
//! [`app/src/tab.rs::TabComponent`] and are tightly coupled to session models (indicators, rename
//! editor, drag/drop targets, etc.). Project tabs need almost none of that complexity — just a
//! leading origin icon, a label, an active highlight, and a close `×` on hover — so this is a
//! dedicated, intentionally small component rather than a refactor of `TabComponent`.
//!
//! Visual decisions captured in `docs/projects-ui-polish.md` Step 3:
//! - **Chrome is a literal mirror of `TabComponent` with `FeatureFlag::NewTabStyling` on**
//!   ([`app/src/tab.rs::render_tab_container_internal`]):
//!   - No corner radius — tabs are rectangular blocks that share the strip's top/bottom edges.
//!   - Border only on the right (and on the left for the first tab) — the right border of tab N is
//!     the visual separator between tab N and tab N+1, so adjacent tabs don't double up.
//!   - Active background = `internal_colors::fg_overlay_2(theme)`, hover = `fg_overlay_1`,
//!     inactive = no background. The strip itself is already painted with `fg_overlay_1`, so an
//!     inactive tab reads as part of the strip and an active tab "lifts" by a single overlay step.
//! - Each tab renders a leading [`Icon`] keyed off [`ProjectOrigin`] so saved-config / template /
//!   default / root / plain-window projects are visually distinguishable at a glance. The icon is
//!   project-specific affordance the session-tab strip doesn't need.
//! - The close `×` only renders while the tab is hovered (cheap mouse-state gate) and dispatches
//!   `root_view:close_project_workspace`, which already handles the "last tab in window → close
//!   window" chain via [`crate::root_view::close_workspace`].

use warp_core::ui::theme::color::internal_colors;
use warpui::elements::{
    Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Fill, Flex, Hoverable,
    MainAxisSize, MouseStateHandle, ParentElement, Radius, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::platform::Cursor;
use warpui::{Element, EntityId, WindowId};

use crate::appearance::Appearance;
use crate::root_view::CloseWorkspaceArg;
use crate::ui_components::icons::Icon;
use crate::workspace::ProjectOrigin;

/// 13pt — matches `ui_font_size`. Session tabs use the default UI font size; we do the same so
/// the two strips read as one design language rather than a beefy project bar over a thinner
/// session bar.
pub const PROJECT_TAB_LABEL_FONT_SIZE: f32 = 13.;

/// Per-tab data the component needs. Everything is by-value so the caller doesn't have to juggle
/// lifetimes across the render loop.
pub struct ProjectTabComponent<'a> {
    /// Display name (`identity.name`) shown to the right of the leading icon.
    pub label: String,
    /// `None` for a plain `cmd+n` window; otherwise the project's origin, which picks the leading
    /// icon. See `Self::leading_icon`.
    pub origin: Option<ProjectOrigin>,
    /// True if this tab represents the workspace currently in focus. Drives the `fg_overlay_2`
    /// background fill and Medium label weight.
    pub is_active: bool,
    /// True for the **leftmost** tab in the bar. Session tabs do the same trick — only the first
    /// tab gets a left border, so adjacent tabs share a single 1pt vertical separator instead of
    /// drawing two (which would look thicker).
    pub is_first: bool,
    /// The workspace this tab targets. Activating clicks dispatch
    /// `root_view:activate_project_tab` with this id; close `×` clicks dispatch
    /// `root_view:close_project_workspace` with this + `window_id`.
    pub view_id: EntityId,
    /// Host OS window of `view_id`, needed by [`CloseWorkspaceArg`]. The activate path doesn't
    /// need it (the action handler resolves the window from the workspace).
    pub window_id: WindowId,
    /// Hover/press state for the tab body — gates the close-`×` visibility and the hover
    /// background, and is the `Hoverable`'s state handle.
    pub tab_mouse_state: MouseStateHandle,
    /// Hover state for the inner `×` button — drives its own background tint on hover.
    pub close_mouse_state: MouseStateHandle,
    /// Borrowed appearance — used for theme colors, font family, and icon rendering. Borrow lasts
    /// only until [`Self::render`] returns the `Box<dyn Element>`; the closures inside don't
    /// capture it.
    pub appearance: &'a Appearance,
}

impl<'a> ProjectTabComponent<'a> {
    /// Maps a [`ProjectOrigin`] (or `None` for a plain window) to the leading glyph. Same mapping
    /// `projects/search_item.rs::render_icon` already uses for the palette rows — the bar and the
    /// palette stay visually consistent.
    fn leading_icon(&self) -> Icon {
        match self.origin {
            Some(ProjectOrigin::Config) => Icon::Folder,
            Some(ProjectOrigin::Template) => Icon::LayoutAlt01,
            Some(ProjectOrigin::Default) => Icon::Navigation,
            Some(ProjectOrigin::Root) => Icon::Gear,
            None => Icon::Terminal,
        }
    }

    /// Builds the tab element. Consumes `self` because the closures need to own the data.
    pub fn render(self) -> Box<dyn Element> {
        let theme = self.appearance.theme();
        let label_color = theme.foreground();
        let close_color = theme.sub_text_color(theme.background());
        let ui_font_family = self.appearance.ui_font_family();
        let icon_glyph = self.leading_icon();
        let is_active = self.is_active;
        let is_first = self.is_first;
        let view_id = self.view_id;
        let window_id = self.window_id;

        // Active border picks up the same `fg_overlay_2` token the active background uses so the
        // active tab's right-side separator reads slightly louder than its inactive neighbours'.
        let active_border_fill = internal_colors::fg_overlay_2(theme);
        let inactive_border_fill = internal_colors::fg_overlay_1(theme);

        let tab_body = Hoverable::new(self.tab_mouse_state, {
            let close_state = self.close_mouse_state;
            let label = self.label;
            move |state| {
                let mut inner = Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_size(MainAxisSize::Min);

                // Leading origin icon.
                inner.add_child(
                    Container::new(
                        ConstrainedBox::new(
                            icon_glyph.to_warpui_icon(label_color.into()).finish(),
                        )
                        .with_width(14.)
                        .with_height(14.)
                        .finish(),
                    )
                    .with_margin_right(6.)
                    .finish(),
                );

                // Label. Active uses Medium weight so it wins the visual hierarchy even before
                // the `fg_overlay_2` fill registers.
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

                // Close `×` only while the tab is hovered. Its own click handler captures the
                // event inside the X's bounds so it doesn't bubble to the tab-activate handler
                // below.
                if state.is_hovered() {
                    let close_state = close_state.clone();
                    let close_btn = Hoverable::new(close_state, move |close_hover| {
                        let mut wrap = Container::new(
                            ConstrainedBox::new(
                                Icon::X.to_warpui_icon(close_color.into()).finish(),
                            )
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

                // Background follows the session-tab `NewTabStyling` mapping exactly: active gets
                // `fg_overlay_2`, hover gets `fg_overlay_1`, otherwise no background (the strip's
                // own `fg_overlay_1` shows through).
                let background = if is_active {
                    internal_colors::fg_overlay_2(theme).into()
                } else if state.is_hovered() {
                    internal_colors::fg_overlay_1(theme).into()
                } else {
                    Fill::None
                };

                // Border sides match the session-tab trick (`tab.rs:1552-1558`): top/bottom always
                // off (so the tab shares the strip's top/bottom edges), right always on (acts as
                // the separator to the next tab), left only on the first tab (avoids drawing two
                // 1pt lines between adjacent tabs).
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
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _app, _v2f| {
            ctx.dispatch_action("root_view:activate_project_tab", view_id);
        })
        .finish();

        tab_body
    }
}
