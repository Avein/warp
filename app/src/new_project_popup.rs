//! The `cmd+shift+N` "new project-tab" popup: a single-line path input. The user types/edits a
//! directory path (with Tab completing the next folder name in place) and presses Enter to open an
//! ad-hoc project-tab rooted there (the `newds` / `open_default_session` mechanism). It is hosted by
//! [`crate::root_view::RootView`], which opens it prepopulated with the home directory (text
//! pre-selected so the first keystroke replaces it) and, on confirm, dispatches the project open.
#![cfg_attr(target_family = "wasm", allow(dead_code, unused_imports))]

use std::path::PathBuf;

use warp_editor::editor::NavigationKey;
use warpui::elements::{
    Align, Border, ChildView, ConstrainedBox, Container, CornerRadius, DropShadow, Flex,
    ParentElement, Radius, Text,
};
use warpui::{
    AppContext, Element, Entity, FocusContext, SingletonEntity, TypedActionView, View, ViewContext,
    ViewHandle,
};

use crate::appearance::Appearance;
use crate::editor::{
    EditorView, Event as EditorEvent, InteractionState, PropagateAndNoOpNavigationKeys,
    SingleLineEditorOptions, TextOptions,
};

const POPUP_WIDTH: f32 = 420.;
const LABEL_FONT_SIZE: f32 = 12.;
const EDITOR_FONT_SIZE: f32 = 12.;
const EDITOR_PADDING: f32 = 6.;
const EDITOR_BORDER_WIDTH: f32 = 1.;
const EDITOR_BORDER_RADIUS: f32 = 6.;
const ROW_SPACING: f32 = 6.;
const PANEL_PADDING: f32 = 12.;

#[derive(Debug)]
pub enum Event {
    Close,
    Confirm { path: String },
}

/// State for cycling through directory candidates on repeated Tab presses (menu-completion). Created
/// when Tab is pressed on an ambiguous stem with no shared-prefix progress (e.g. a trailing `/`),
/// and advanced on each subsequent Tab as long as the buffer still holds the last inserted value.
struct CompletionCycle {
    /// The buffer text up to and including the last `/` — what each candidate is appended to.
    stem: String,
    /// Sorted directory names that matched the stem.
    candidates: Vec<String>,
    /// Index of the currently shown candidate.
    index: usize,
    /// The exact buffer string we last inserted, used to detect whether the user has since edited.
    last_inserted: String,
}

/// A single-line path input for opening a new ad-hoc project-tab. See the module docs.
pub struct NewProjectPopup {
    path_editor: ViewHandle<EditorView>,
    is_open: bool,
    completion_cycle: Option<CompletionCycle>,
}

impl NewProjectPopup {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let path_editor = ctx.add_typed_action_view(|ctx| {
            let appearance = Appearance::as_ref(ctx);
            let mut editor = EditorView::single_line(
                SingleLineEditorOptions {
                    text: TextOptions::ui_text(Some(EDITOR_FONT_SIZE), appearance),
                    // Leave the prepopulated home path unselected with the cursor at the end, so the
                    // user can immediately append/Tab-complete rather than overwrite it.
                    select_all_on_focus: false,
                    clear_selections_on_blur: false,
                    propagate_and_no_op_vertical_navigation_keys:
                        PropagateAndNoOpNavigationKeys::Always,
                    ..Default::default()
                },
                ctx,
            );
            editor.set_placeholder_text("Project directory path", ctx);
            editor
        });

        ctx.subscribe_to_view(&path_editor, |me, _, event, ctx| {
            me.handle_editor_event(event, ctx);
        });

        let appearance_handle = Appearance::handle(ctx);
        ctx.observe(&appearance_handle, |_, _, ctx| {
            ctx.notify();
        });

        Self {
            path_editor,
            is_open: false,
            completion_cycle: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Opens the popup prepopulated with `initial_path` (the home directory), leaving the cursor at
    /// the end and the text unselected so the user can append or Tab-complete immediately. A
    /// deliberate Enter opens a project at whatever path is in the buffer.
    pub fn open(&mut self, initial_path: String, ctx: &mut ViewContext<Self>) {
        self.is_open = true;
        self.completion_cycle = None;
        self.path_editor.update(ctx, |editor, ctx| {
            editor.set_interaction_state(InteractionState::Editable, ctx);
            // `set_buffer_text` leaves the cursor at the end of the inserted text (no selection).
            editor.set_buffer_text(&initial_path, ctx);
        });
        ctx.notify();
    }

    pub fn close(&mut self, ctx: &mut ViewContext<Self>) {
        self.is_open = false;
        ctx.notify();
    }

    fn handle_editor_event(&mut self, event: &EditorEvent, ctx: &mut ViewContext<Self>) {
        match event {
            EditorEvent::Enter => {
                let path = self.path_editor.as_ref(ctx).buffer_text(ctx);
                ctx.emit(Event::Confirm { path });
            }
            EditorEvent::Escape => {
                ctx.emit(Event::Close);
            }
            // Tab and Down reach us as navigation events (the editor is configured to propagate
            // them rather than edit). Tab/Down complete or cycle forward; Up cycles backward.
            EditorEvent::Navigate(NavigationKey::Tab | NavigationKey::Down) => {
                self.complete_path(ctx);
            }
            EditorEvent::Navigate(NavigationKey::Up) => {
                self.cycle_prev(ctx);
            }
            _ => {}
        }
    }

    /// Shell-style completion of the directory name being typed, triggered by Tab or Down. A unique
    /// match completes the folder name (no trailing `/` — the user adds that to descend); multiple
    /// matches first extend to their longest common prefix, and once there is no shared-prefix
    /// progress (e.g. a trailing `/`), repeated Tab/Down cycles through the matching directories.
    /// Always leaves the cursor at the end.
    fn complete_path(&mut self, ctx: &mut ViewContext<Self>) {
        let current = self.path_editor.as_ref(ctx).buffer_text(ctx);

        // If we're mid-cycle and the user hasn't edited since our last insertion, advance to the
        // next candidate instead of recomputing.
        if self.advance_cycle_if_active(&current, true, ctx) {
            return;
        }

        self.completion_cycle = None;
        match complete_dir_path(&current) {
            Completion::None => {}
            Completion::Replace(text) => {
                if text != current {
                    self.set_buffer(&text, ctx);
                }
            }
            Completion::Cycle { stem, candidates } => {
                let first = format!("{}{}", stem, candidates[0]);
                self.set_buffer(&first, ctx);
                self.completion_cycle = Some(CompletionCycle {
                    stem,
                    candidates,
                    index: 0,
                    last_inserted: first,
                });
            }
        }
    }

    /// Cycles backward through the active completion candidates (triggered by Up). Does nothing when
    /// there is no active cycle or the user has edited since the last insertion.
    fn cycle_prev(&mut self, ctx: &mut ViewContext<Self>) {
        let current = self.path_editor.as_ref(ctx).buffer_text(ctx);
        self.advance_cycle_if_active(&current, false, ctx);
    }

    /// If a cycle is active and `current` still matches our last insertion, step to the next
    /// (`forward`) or previous candidate, update the buffer, and return `true`. Otherwise `false`.
    fn advance_cycle_if_active(
        &mut self,
        current: &str,
        forward: bool,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        let Some(cycle) = self.completion_cycle.as_mut() else {
            return false;
        };
        let len = cycle.candidates.len();
        if cycle.last_inserted != current || len <= 1 {
            return false;
        }
        cycle.index = if forward {
            (cycle.index + 1) % len
        } else {
            (cycle.index + len - 1) % len
        };
        let next = format!("{}{}", cycle.stem, cycle.candidates[cycle.index]);
        cycle.last_inserted = next.clone();
        self.set_buffer(&next, ctx);
        true
    }

    fn set_buffer(&mut self, text: &str, ctx: &mut ViewContext<Self>) {
        self.path_editor.update(ctx, |editor, ctx| {
            editor.set_buffer_text(text, ctx);
        });
    }
}

/// The outcome of computing a Tab completion for the current buffer.
#[derive(Debug, PartialEq)]
enum Completion {
    /// Nothing matched, or there is no further progress to make.
    None,
    /// Replace the buffer with this text (a unique match, or a longest-common-prefix extension).
    Replace(String),
    /// The stem is ambiguous with no shared-prefix progress; cycle through `candidates` (directory
    /// names) appended to `stem` on repeated Tab.
    Cycle { stem: String, candidates: Vec<String> },
}

/// Returns the home directory as a path string (e.g. `/Users/alice`), used as the base for
/// completing a bare (slash-less) input.
fn home_dir_string() -> String {
    PathBuf::from(shellexpand::tilde("~").into_owned())
        .to_string_lossy()
        .into_owned()
}

/// Computes the Tab completion for the partial path the user has typed by reading the relevant
/// directory off disk, then delegating the decision to [`build_completion`].
///
/// The directory portion the user typed is preserved verbatim (so a leading `~` stays a `~`); only
/// the final, in-progress component is completed.
fn complete_dir_path(input: &str) -> Completion {
    let (dir_portion, prefix) = match input.rfind('/') {
        Some(idx) => (&input[..=idx], &input[idx + 1..]),
        None => ("", input),
    };

    // Resolve the directory we should read for candidates. A slash-less input completes against the
    // home directory; otherwise expand `~` in the typed directory portion.
    let fs_dir = if dir_portion.is_empty() {
        PathBuf::from(home_dir_string())
    } else {
        PathBuf::from(shellexpand::tilde(dir_portion).into_owned())
    };

    let prefix_lower = prefix.to_lowercase();
    let read = match std::fs::read_dir(&fs_dir) {
        Ok(read) => read,
        Err(_) => return Completion::None,
    };
    let matches: Vec<String> = read
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
        .filter(|name| name.to_lowercase().starts_with(&prefix_lower))
        .collect();

    build_completion(dir_portion, prefix, matches)
}

/// Pure completion logic, split out from the filesystem read so it can be unit-tested. Given the
/// verbatim directory portion the user typed, the in-progress `prefix`, and the matching directory
/// names, decides how Tab should behave:
/// - no matches → [`Completion::None`];
/// - one match → complete the folder name, no trailing `/` ([`Completion::Replace`]);
/// - many matches with a longer shared prefix → extend to that prefix ([`Completion::Replace`]);
/// - many matches with no shared-prefix progress → [`Completion::Cycle`] through them.
fn build_completion(dir_portion: &str, prefix: &str, mut matches: Vec<String>) -> Completion {
    if matches.is_empty() {
        return Completion::None;
    }
    matches.sort_by_key(|name| name.to_lowercase());

    if matches.len() == 1 {
        return Completion::Replace(format!("{dir_portion}{}", matches[0]));
    }

    let lcp = longest_common_prefix(&matches);
    if lcp.len() > prefix.len() {
        return Completion::Replace(format!("{dir_portion}{lcp}"));
    }
    // No shared-prefix progress (e.g. an empty prefix after a trailing `/`): cycle the candidates.
    Completion::Cycle {
        stem: dir_portion.to_string(),
        candidates: matches,
    }
}

/// Longest common prefix of the given names, compared case-insensitively but returned using the
/// casing of the first name (the list is assumed pre-sorted case-insensitively).
fn longest_common_prefix(names: &[String]) -> String {
    let first = &names[0];
    let mut len = first.len();
    for name in &names[1..] {
        let common = first
            .chars()
            .zip(name.chars())
            .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
            .count();
        // `common` counts chars; map back to a byte length within `first`.
        let byte_len: usize = first.chars().take(common).map(char::len_utf8).sum();
        len = len.min(byte_len);
    }
    first[..len].to_string()
}

impl Entity for NewProjectPopup {
    type Event = Event;
}

impl TypedActionView for NewProjectPopup {
    type Action = ();

    fn handle_action(&mut self, _action: &(), _ctx: &mut ViewContext<Self>) {}
}

impl View for NewProjectPopup {
    fn ui_name() -> &'static str {
        "NewProjectPopup"
    }

    fn on_focus(&mut self, focus_ctx: &FocusContext, ctx: &mut ViewContext<Self>) {
        if focus_ctx.is_self_focused() {
            ctx.focus(&self.path_editor);
            ctx.notify();
        }
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();

        let label = Text::new_inline(
            "New project tab",
            appearance.ui_font_family(),
            LABEL_FONT_SIZE,
        )
        .with_color(theme.active_ui_text_color().into())
        .finish();

        let input_field = Container::new(ChildView::new(&self.path_editor).finish())
            .with_padding_left(8.)
            .with_padding_right(4.)
            .with_padding_top(EDITOR_PADDING)
            .with_padding_bottom(EDITOR_PADDING)
            .with_background(theme.surface_1())
            .with_border(Border::all(EDITOR_BORDER_WIDTH).with_border_fill(theme.surface_3()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(EDITOR_BORDER_RADIUS)))
            .finish();

        let mut content = Flex::column().with_child(
            Container::new(label)
                .with_margin_bottom(ROW_SPACING)
                .finish(),
        );
        content.add_child(input_field);

        let panel = Container::new(
            ConstrainedBox::new(
                Container::new(content.finish())
                    .with_background(theme.surface_2())
                    .finish(),
            )
            .with_width(POPUP_WIDTH)
            .finish(),
        )
        .with_uniform_padding(PANEL_PADDING)
        .with_background(theme.surface_2())
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(EDITOR_BORDER_RADIUS)))
        .with_drop_shadow(DropShadow::default())
        .finish();

        Align::new(panel).top_center().finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{build_completion, complete_dir_path, longest_common_prefix, Completion};

    fn names(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn unique_match_completes_folder_name_without_trailing_slash() {
        let completed = build_completion("/Users/avein/", "per", names(&["personal"]));
        assert_eq!(
            completed,
            Completion::Replace("/Users/avein/personal".to_string())
        );
    }

    #[test]
    fn multiple_matches_complete_to_longest_common_prefix() {
        // "pro" -> shared prefix "project" (no trailing slash, still ambiguous).
        let completed = build_completion("/src/", "pro", names(&["projects", "project-x"]));
        assert_eq!(completed, Completion::Replace("/src/project".to_string()));
    }

    #[test]
    fn no_prefix_progress_cycles_the_candidates() {
        // Already typed "project"; the two candidates share exactly "project", so Tab cycles them
        // (sorted case-insensitively) rather than extending the text.
        let completed = build_completion("/src/", "project", names(&["projects", "project-x"]));
        assert_eq!(
            completed,
            Completion::Cycle {
                stem: "/src/".to_string(),
                candidates: names(&["project-x", "projects"]),
            }
        );
    }

    #[test]
    fn empty_matches_yield_no_completion() {
        assert_eq!(build_completion("/src/", "zzz", names(&[])), Completion::None);
    }

    #[test]
    fn directory_portion_is_preserved_verbatim_including_tilde() {
        let completed = build_completion("~/code/", "wa", names(&["warp"]));
        assert_eq!(completed, Completion::Replace("~/code/warp".to_string()));
    }

    #[test]
    fn longest_common_prefix_is_case_insensitive_but_keeps_first_casing() {
        // Sorted case-insensitively the first is "Apple"; shared prefix with "appleseed" is "Apple".
        let lcp = longest_common_prefix(&names(&["Apple", "appleseed"]));
        assert_eq!(lcp, "Apple");
    }

    #[test]
    fn completes_real_directories_and_ignores_files() {
        let base = std::env::temp_dir().join(format!("npp_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("alpha")).unwrap();
        std::fs::create_dir_all(base.join("alabama")).unwrap();
        std::fs::write(base.join("alfred.txt"), b"not a dir").unwrap();

        // Unique directory prefix completes the name (and the file `alfred.txt` is ignored).
        let unique = complete_dir_path(&format!("{}/alp", base.display()));
        assert_eq!(
            unique,
            Completion::Replace(format!("{}/alpha", base.display()))
        );

        // Two directories ("alpha", "alabama") share exactly the typed "al" -> cycle them.
        let cycle = complete_dir_path(&format!("{}/al", base.display()));
        assert_eq!(
            cycle,
            Completion::Cycle {
                stem: format!("{}/", base.display()),
                candidates: names(&["alabama", "alpha"]),
            }
        );

        std::fs::remove_dir_all(&base).unwrap();
    }
}
