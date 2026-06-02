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
use warpui::{Element, EntityId, WindowId};

use crate::appearance::Appearance;
use crate::root_view::CloseWorkspaceArg;
use crate::ui_components::icons::Icon;
use crate::workspace::project_icon::icon_for_origin;
use crate::workspace::ProjectOrigin;

/// 13pt — matches the default `ui_font_size` used by session tabs.
pub const PROJECT_TAB_LABEL_FONT_SIZE: f32 = 13.;

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

        let active_border_fill = internal_colors::fg_overlay_2(theme);
        let inactive_border_fill = internal_colors::fg_overlay_1(theme);

        Hoverable::new(self.tab_mouse_state, {
            let close_state = self.close_mouse_state;
            let label = self.label;
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

                let mut text =
                    Text::new_inline(label.clone(), ui_font_family, PROJECT_TAB_LABEL_FONT_SIZE)
                        .with_color(label_color.into());
                if is_active {
                    text = text.with_style(Properties::default().weight(Weight::Medium));
                }
                inner.add_child(text.finish());

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
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _app, _v2f| {
            ctx.dispatch_action("root_view:activate_project_tab", view_id);
        })
        .finish()
    }
}
