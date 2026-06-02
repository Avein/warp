use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Flex, Highlight,
    ParentElement, Radius, Shrinkable, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use warpui::ui_components::text::Span;
use warpui::Element;

use crate::appearance::Appearance;
use crate::launch_configs::launch_config::LaunchConfig;
use crate::search::result_renderer::ItemHighlightState;
use crate::themes::theme::Fill;
use crate::ui_components::icons::Icon;

/// Working-tree git diff stats vs `HEAD` for a project's primary cwd. Rendered in the projects
/// palette as a `📄 N · +X -Y` pill next to the branch pill.
#[derive(Clone, Debug)]
pub struct DiffStats {
    pub files: u32,
    pub insertions: u32,
    pub deletions: u32,
}

/// Extra project metadata rendered by the `projects:` palette beneath a config's name: the
/// home-relative working directory (`None` for a path-less template like `default`) and, when the
/// directory is a git repo, the current branch (rendered with a leading icon) and working-tree
/// diff stats. The presence of this struct (vs `None`) tells the renderer to use the
/// projects-palette row style (taller row, +2pt name); the path subtitle is only rendered when
/// it is `Some` so path-less templates share the same chrome as path-bearing projects.
pub(crate) struct ProjectRowDetails {
    pub path: Option<String>,
    pub branch: Option<String>,
    pub diff_stats: Option<DiffStats>,
}

impl LaunchConfig {
    /// Renders a [`LaunchConfig`] using a [`StylesProvider`]. Any character indices of the launch
    /// config title contained within `highlighted_indices` are highlighted in bold. When `is_open`
    /// is true (used by the `projects:` palette for projects with a live window), an extra "open"
    /// pill is rendered alongside the window/tab description. When `project` is `Some` (the
    /// `projects:` palette), a git-branch pill is added and the working directory is rendered on a
    /// second line below the name.
    pub(crate) fn render(
        &self,
        appearance: &Appearance,
        item_highlight_state: ItemHighlightState,
        highlight_indices: Vec<usize>,
        is_open: bool,
        project: Option<ProjectRowDetails>,
        show_description: bool,
    ) -> Box<dyn Element> {
        let bg_color = background_fill(item_highlight_state, appearance);

        let text_color = appearance.theme().main_text_color(bg_color).into_solid();

        let highlight = Highlight::new()
            .with_properties(Properties::default().weight(Weight::Bold))
            .with_foreground_color(text_color);

        // The projects palette and Alt-Tab (two-line rows) want a touch larger type than the
        // regular launch-configs palette (single-line rows), so the project name reads as the
        // anchor of the row rather than the same size as the path subtitle / pills. +2pt only
        // when this is a project row; the single-line layout keeps the original font size.
        let name_font_size = if project.is_some() {
            appearance.monospace_font_size() + 2.
        } else {
            appearance.monospace_font_size()
        };
        let label = self
            .render_launch_config_name(appearance, item_highlight_state, name_font_size)
            .with_single_highlight(highlight, highlight_indices)
            .finish();

        // Build the right-side pill stack once. The two-line and single-line layouts below both
        // consume it, so the conditionals (is_open / branch / diff_stats / description) live in one
        // place. Pills are accumulated in display order — open first, then branch + diff stats,
        // then the window/tab description.
        let mut pills: Vec<Box<dyn Element>> = Vec::new();
        if is_open {
            pills.push(
                Container::new(Self::render_string_with_pill_styling("open", appearance)).finish(),
            );
        }
        if let Some(branch) = project.as_ref().and_then(|p| p.branch.clone()) {
            pills.push(
                Container::new(Self::render_icon_pill(Icon::GitBranch, branch, appearance))
                    .finish(),
            );
        }
        if let Some(stats) = project.as_ref().and_then(|p| p.diff_stats.clone()) {
            if let Some(pill) = Self::render_diff_stats_pill(stats, appearance) {
                pills.push(Container::new(pill).finish());
            }
        }
        if show_description {
            pills.push(
                Container::new(self.render_config_description(appearance))
                    .with_margin_right(14.)
                    .finish(),
            );
        }

        match project {
            Some(ProjectRowDetails { path, .. }) => {
                // Projects-palette row style: name + optional path subtitle on the left, pills
                // hugging the right and vertically centered against the left column. Path-less
                // configs (like the `default` template) take the same 60pt row + +2pt name as
                // path-bearing projects — they just skip the subtitle line — so the projects
                // palette reads as one consistent list rather than `default` looking like a
                // smaller, single-line row pasted in from the regular launch-configs palette.
                // Path subtitle is plain `monospace` (was `monospace - 2`) so it sits one tier
                // below the +2pt name without disappearing.
                let mut left_column = Flex::column();
                left_column.add_child(Align::new(label).left().finish());
                if let Some(path) = path {
                    let path_text = Text::new_inline(
                        path,
                        appearance.ui_font_family(),
                        appearance.monospace_font_size(),
                    )
                    .with_color(appearance.theme().hint_text_color(bg_color).into_solid())
                    .finish();
                    left_column.add_child(Align::new(path_text).left().finish());
                }
                let mut pills_row =
                    Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
                for pill in pills {
                    pills_row.add_child(pill);
                }
                let row = Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(Shrinkable::new(1., left_column.finish()).finish())
                    .with_child(pills_row.finish())
                    .finish();
                ConstrainedBox::new(row).with_height(60.).finish()
            }
            None => {
                // Single-line layout (regular launch-configs palette): name shrinks, pills follow
                // to the right of it on the same line. Identical to the original behaviour.
                let mut top_row = Flex::row();
                top_row.add_child(Shrinkable::new(1., Align::new(label).left().finish()).finish());
                for pill in pills {
                    top_row.add_child(pill);
                }
                ConstrainedBox::new(top_row.finish())
                    .with_height(40.)
                    .finish()
            }
        }
    }

    fn default_pill_styles(appearance: &Appearance) -> UiComponentStyles {
        UiComponentStyles {
            font_family_id: Some(appearance.ui_font_family()),
            font_size: Some(appearance.monospace_font_size()),
            font_color: Some(
                appearance
                    .theme()
                    .hint_text_color(appearance.theme().background())
                    .into_solid(),
            ),
            border_radius: Some(CornerRadius::with_all(Radius::Pixels(4.))),
            background: Some(appearance.theme().background().into()),
            height: Some(24.),
            padding: Some(Coords::default().left(6.).right(6.)),
            margin: Some(Coords::default().left(3.)),
            ..Default::default()
        }
    }

    /// Renders a pill whose body is `[icon][text]`, sharing the chrome (background, border,
    /// radius, padding, margins, height) with `render_string_with_pill_styling` so the icon-led
    /// branch pill sits flush with the plain text pills used for `open` / `N windows` / etc.
    fn render_icon_pill(
        icon: Icon,
        str: impl Into<String>,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let style = Self::default_pill_styles(appearance);
        let icon_color = style
            .font_color
            .unwrap_or_else(|| appearance.theme().foreground().into_solid());
        let icon_size = appearance.monospace_font_size();
        let icon_el = ConstrainedBox::new(
            Align::new(icon.to_warpui_icon(Fill::Solid(icon_color)).finish()).finish(),
        )
        .with_width(icon_size)
        .with_height(icon_size)
        .finish();
        let span = Span::new(str.into(), style).build().finish();
        let body = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(Container::new(icon_el).with_margin_right(4.).finish())
            .with_child(span)
            .finish();
        Self::wrap_in_pill_chrome(body, &style)
    }

    /// Renders the working-tree diff stats as a file-icon-led pill: `📄 N · +X -Y` with `+X` in
    /// green and `-X` in red. Returns `None` for an effectively empty diff (no files changed and
    /// no inserts/deletes) so callers can skip the pill rather than render an inert `0 · +0 -0`.
    fn render_diff_stats_pill(
        stats: DiffStats,
        appearance: &Appearance,
    ) -> Option<Box<dyn Element>> {
        if stats.files == 0 && stats.insertions == 0 && stats.deletions == 0 {
            return None;
        }
        let style = Self::default_pill_styles(appearance);
        let neutral_color = style
            .font_color
            .unwrap_or_else(|| appearance.theme().foreground().into_solid());
        let theme = appearance.theme();
        let green_style = UiComponentStyles {
            font_color: Some(theme.terminal_colors().normal.green.into()),
            ..style
        };
        let red_style = UiComponentStyles {
            font_color: Some(theme.terminal_colors().normal.red.into()),
            ..style
        };

        let icon_size = appearance.monospace_font_size();
        let icon_el = ConstrainedBox::new(
            Align::new(
                Icon::File
                    .to_warpui_icon(Fill::Solid(neutral_color))
                    .finish(),
            )
            .finish(),
        )
        .with_width(icon_size)
        .with_height(icon_size)
        .finish();

        let mut body = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(Container::new(icon_el).with_margin_right(4.).finish())
            .with_child(
                Span::new(format!("{}", stats.files), style)
                    .build()
                    .finish(),
            );
        if stats.insertions > 0 || stats.deletions > 0 {
            body.add_child(Span::new(" · ".to_string(), style).build().finish());
        }
        if stats.insertions > 0 {
            body.add_child(
                Span::new(format!("+{}", stats.insertions), green_style)
                    .build()
                    .finish(),
            );
        }
        if stats.insertions > 0 && stats.deletions > 0 {
            body.add_child(Span::new(" ".to_string(), style).build().finish());
        }
        if stats.deletions > 0 {
            body.add_child(
                Span::new(format!("-{}", stats.deletions), red_style)
                    .build()
                    .finish(),
            );
        }
        Some(Self::wrap_in_pill_chrome(body.finish(), &style))
    }

    /// Wraps an arbitrary pill body in the standard pill chrome (border, padding, background,
    /// rounded corners, fixed height, outer margin). Factored out so the plain-text, icon+text,
    /// and diff-stats pills all share identical sizing — they read as one design language.
    fn wrap_in_pill_chrome(body: Box<dyn Element>, style: &UiComponentStyles) -> Box<dyn Element> {
        let mut container = Container::new(Align::new(body).finish());
        let mut border = Border::all(style.border_width.unwrap_or_default());
        if let Some(border_color) = style.border_color {
            border = border.with_border_fill(border_color);
        }
        container = container.with_border(border);
        if let Some(padding) = style.padding {
            container = container
                .with_padding_top(padding.top)
                .with_padding_right(padding.right)
                .with_padding_bottom(padding.bottom)
                .with_padding_left(padding.left);
        }
        if let Some(radius) = style.border_radius {
            container = container.with_corner_radius(radius);
        }
        if let Some(background_color) = style.background {
            container = container.with_background(background_color);
        }
        let mut sized_container = ConstrainedBox::new(container.finish());
        if let Some(width) = style.width {
            sized_container = sized_container.with_width(width);
        }
        if let Some(height) = style.height {
            sized_container = sized_container.with_height(height);
        }
        let mut outer = Container::new(Align::new(sized_container.finish()).finish());
        if let Some(margin) = style.margin {
            outer = outer
                .with_margin_top(margin.top)
                .with_margin_right(margin.right)
                .with_margin_bottom(margin.bottom)
                .with_margin_left(margin.left);
        }
        outer.finish()
    }

    fn render_string_with_pill_styling(
        str: impl Into<String>,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let style = Self::default_pill_styles(appearance);
        let mut container =
            Container::new(Align::new(Span::new(str.into(), style).build().finish()).finish());
        let mut border = Border::all(style.border_width.unwrap_or_default());
        if let Some(border_color) = style.border_color {
            border = border.with_border_fill(border_color);
        }
        container = container.with_border(border);
        if let Some(padding) = style.padding {
            container = container
                .with_padding_top(padding.top)
                .with_padding_right(padding.right)
                .with_padding_bottom(padding.bottom)
                .with_padding_left(padding.left);
        }
        if let Some(radius) = style.border_radius {
            container = container.with_corner_radius(radius);
        }
        if let Some(background_color) = style.background {
            container = container.with_background(background_color);
        }
        let mut sized_container = ConstrainedBox::new(container.finish());
        if let Some(width) = style.width {
            sized_container = sized_container.with_width(width);
        }
        if let Some(height) = style.height {
            sized_container = sized_container.with_height(height);
        }
        let mut container = Container::new(Align::new(sized_container.finish()).finish());
        if let Some(margin) = style.margin {
            container = container
                .with_margin_top(margin.top)
                .with_margin_right(margin.right)
                .with_margin_bottom(margin.bottom)
                .with_margin_left(margin.left);
        }
        container.finish()
    }

    fn render_config_description(&self, appearance: &Appearance) -> Box<dyn Element> {
        let num_windows = self.windows.len();
        let num_tabs: usize = self.windows.iter().map(|window| window.tabs.len()).sum();
        let mut windows_str = num_windows.to_string();
        match num_windows {
            1 => windows_str.push_str(" window "),
            _ => windows_str.push_str(" windows"),
        }
        let mut tabs_str = num_tabs.to_string();
        match num_tabs {
            1 => tabs_str.push_str(" tab "),
            _ => tabs_str.push_str(" tabs"),
        }
        Flex::row()
            .with_children(vec![
                Self::render_string_with_pill_styling(windows_str, appearance),
                Self::render_string_with_pill_styling(tabs_str, appearance),
            ])
            .finish()
    }

    fn render_launch_config_name(
        &self,
        appearance: &Appearance,
        item_highlight_state: ItemHighlightState,
        font_size: f32,
    ) -> Text {
        let text = Text::new_inline(self.name.clone(), appearance.ui_font_family(), font_size);

        let bg_color = background_fill(item_highlight_state, appearance);
        text.with_color(appearance.theme().sub_text_color(bg_color).into_solid())
    }
}

fn background_fill(item_highlight_state: ItemHighlightState, appearance: &Appearance) -> Fill {
    item_highlight_state
        .container_background_fill(appearance)
        .unwrap_or_else(|| appearance.theme().surface_2())
}
