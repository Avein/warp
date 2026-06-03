//! Audit table for the `workspace:save_app` dispatch sites that mutate the
//! project-tab state persisted to SQLite.
//!
//! The module is **purely descriptive**: it owns no dispatch, subscribes to
//! nothing, and changes no runtime behavior. Its single job is to be the
//! grep-discoverable list a reviewer can scan to answer "is every event that
//! mutates the persisted project-tab set covered by a `workspace:save_app`
//! dispatch?".
//!
//! Adding a variant to [`PersistedStateMutation`] is required (and enforced by
//! `save_policy_tests::dispatch_site_label_is_non_empty`,
//! `labels_are_unique`, and `all_variants_is_exhaustive`) whenever a new
//! lifecycle event mutates the persisted project-tab set. Each variant must
//! be paired with a real `ctx.dispatch_global_action("workspace:save_app",
//! ())` call at the site named by [`PersistedStateMutation::dispatch_site`].
//!
//! Scope: project-tab persistence — the `windows` SQLite table written by
//! `save_app_state`. Other AppState mutations that piggyback on
//! `workspace:save_app` (LLM preferences, MCP server lifecycle, blocklist
//! agent-view state, agent-management filters) are deliberately outside
//! this audit and have no matching variant.

/// Every event that mutates the project-tab set as persisted to SQLite.
///
/// Adding a variant here MUST be paired with a
/// `ctx.dispatch_global_action("workspace:save_app", ())` at the site named
/// by [`Self::dispatch_site`]. The unit tests in `save_policy_tests` assert
/// that every variant has a non-empty, unique dispatch-site label and that
/// [`Self::ALL`] enumerates every variant (the latter via an exhaustive
/// `match self` that fails to compile when a variant is added without an
/// arm).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PersistedStateMutation {
    // === Project-tab set (per OS window) ===
    /// A project-tab is opened into an already-open OS window (via the
    /// projects palette, the `⌘⇧N` new-project popup, `newds`, or a
    /// `warp://` URL handler). All paths route through
    /// `root_view::focus_or_spawn_project` → `RootView::open_project_tab`.
    /// Pending [`projects-persistence-02`](../../../../docs/issues/projects-persistence-02-save-on-open-into-window.md).
    ProjectTabOpenedInExistingWindow,
    /// A non-last project-tab is closed inside an OS window (the pill `×`,
    /// the projects palette's secondary action, etc.). The last-tab branch
    /// closes the host OS window and is covered incidentally by
    /// [`Self::OsWindowClosed`]. Pending
    /// [`projects-persistence-03`](../../../../docs/issues/projects-persistence-03-save-on-close-non-last.md).
    ProjectTabClosedNonLastInWindow,
    /// The user commits a display-name override on a project-tab pill (F2
    /// or double-click → inline editor → Enter / Blurred), dispatched from
    /// `Workspace::finish_project_tab_rename`.
    ProjectTabRenamed,

    // === OS-window lifecycle ===
    /// A new OS window is opened (Dock "New Window", the "New Window"
    /// menu item, a launch-config menu item, or any
    /// `dispatch_global_action("root_view:open_new", …)` caller).
    NewOsWindowOpened,
    /// An OS window is closed while the app is NOT terminating. The
    /// terminating case goes through [`Self::AppWillTerminate`] because
    /// `on_window_will_close` short-circuits during `ApplicationStage::Terminating`.
    OsWindowClosed,
    /// The active OS window changes (focus moves between windows).
    ActiveOsWindowChanged,
    /// An OS window is moved or resized — bounds need to round-trip.
    OsWindowMovedOrResized,

    // === App lifecycle ===
    /// The app is quitting (`⌘Q` or equivalent). Final save dispatched
    /// from `on_will_terminate` before `PersistenceWriter::terminate()`
    /// joins the writer thread, so the snapshot's SQL work completes
    /// before shutdown. Pending
    /// [`projects-persistence-04`](../../../../docs/issues/projects-persistence-04-save-on-app-terminate.md).
    AppWillTerminate,

    // === In-workspace mutations (session-tabs, panes inside a project-tab) ===
    /// A `WorkspaceAction` whose
    /// `WorkspaceAction::should_save_app_state_on_action()` returns `true`
    /// was handled. Single dispatch in `Workspace::handle_action` covers
    /// ~60 in-workspace mutations (session-tab add/close/move/rename,
    /// pane add/rename, project-tab cycling, `RenameProjectTab`, etc.).
    WorkspaceActionRequiringSave,
    /// A session-tab's shell finishes bootstrapping (cwd + env now known).
    /// Dispatched from `TerminalView::handle_login_shell_bootstrapped`.
    SessionShellBootstrapped,
    /// A pane group inside a workspace fires
    /// `pane_group::Event::AppStateChanged` (split, close, in-pane state
    /// change). Dispatched from `Workspace::handle_file_tree_event`.
    WorkspacePaneGroupStateChanged,
    /// A session-tab is removed from the workspace's tab strip
    /// (`Workspace::remove_tab` and the telemetry path in
    /// `Workspace::close_tab`).
    SessionTabRemoved,
    /// A session-tab or pane name editor commits a rename or clear
    /// (`Workspace::finish_tab_rename`, `Workspace::clear_pane_name`)
    /// outside of the `WorkspaceAction` matrix.
    SessionTabOrPaneRenameCommitted,
    /// A cross-window pane-group transfer finalizes
    /// (`Workspace::adopt_transferred_pane_group`) — the source workspace's
    /// snapshot needs to reflect the now-detached pane.
    CrossWindowTabTransferFinalized,
    /// The universal-search overlay is resized — dispatched from the
    /// `Resize` branch in the workspace's universal-search event handler.
    UniversalSearchResized,

    // === Undo-close ===
    /// Undo-close restored a previously-closed window, session-tab, or
    /// pane (`UndoCloseStack::undo_close`). The live workspace state
    /// diverges from disk again until persisted.
    UndoCloseRestored,
}

impl PersistedStateMutation {
    /// Every variant of [`PersistedStateMutation`], in declaration order.
    ///
    /// Kept in sync with the enum via
    /// `save_policy_tests::all_variants_is_exhaustive`, which performs an
    /// exhaustive `match self` over the enum and refuses to compile when a
    /// new variant is added without being listed here.
    pub const ALL: &'static [PersistedStateMutation] = &[
        Self::ProjectTabOpenedInExistingWindow,
        Self::ProjectTabClosedNonLastInWindow,
        Self::ProjectTabRenamed,
        Self::NewOsWindowOpened,
        Self::OsWindowClosed,
        Self::ActiveOsWindowChanged,
        Self::OsWindowMovedOrResized,
        Self::AppWillTerminate,
        Self::WorkspaceActionRequiringSave,
        Self::SessionShellBootstrapped,
        Self::WorkspacePaneGroupStateChanged,
        Self::SessionTabRemoved,
        Self::SessionTabOrPaneRenameCommitted,
        Self::CrossWindowTabTransferFinalized,
        Self::UniversalSearchResized,
        Self::UndoCloseRestored,
    ];

    /// Human-readable label naming the source-code call site that
    /// dispatches `workspace:save_app` for this mutation. The string is
    /// for human auditors and the unit tests in `save_policy_tests`; the
    /// codebase is not searched for it at runtime.
    ///
    /// The three new gap-fix variants ship with `pending: …` labels in
    /// this slice; their real labels land with their corresponding bug-fix
    /// slice (`projects-persistence-02`/`-03`/`-04`).
    pub fn dispatch_site(&self) -> &'static str {
        // Exhaustive match — adding a variant above without an arm here
        // fails compilation, forcing the contributor to think about which
        // call site owns the dispatch.
        match self {
            Self::ProjectTabOpenedInExistingWindow => {
                "root_view::persist_project_tab_opened_into_existing_window \
                 (called from focus_or_spawn_project)"
            }
            Self::ProjectTabClosedNonLastInWindow => {
                "root_view::persist_project_tab_closed_non_last_in_window \
                 (called from close_workspace)"
            }
            Self::ProjectTabRenamed => "workspace::view::Workspace::finish_project_tab_rename",
            Self::NewOsWindowOpened => {
                "lib::on_new_window_requested + app_menus::{dock_menu, open_new_window} + launch-config menu items"
            }
            Self::OsWindowClosed => "lib::on_window_will_close (non-terminating)",
            Self::ActiveOsWindowChanged => "lib::on_active_window_changed",
            Self::OsWindowMovedOrResized => "lib::on_window_moved + lib::on_window_resized",
            Self::AppWillTerminate => {
                "lib::persist_app_will_terminate (called from on_will_terminate)"
            }
            Self::WorkspaceActionRequiringSave => {
                "workspace::view::Workspace::handle_action (should_save_app_state_on_action() == true)"
            }
            Self::SessionShellBootstrapped => {
                "terminal::view::TerminalView::handle_login_shell_bootstrapped"
            }
            Self::WorkspacePaneGroupStateChanged => {
                "workspace::view::Workspace::handle_file_tree_event (AppStateChanged)"
            }
            Self::SessionTabRemoved => "workspace::view::Workspace::{remove_tab, close_tab}",
            Self::SessionTabOrPaneRenameCommitted => {
                "workspace::view::Workspace::{finish_tab_rename, clear_pane_name}"
            }
            Self::CrossWindowTabTransferFinalized => {
                "workspace::view::Workspace::adopt_transferred_pane_group"
            }
            Self::UniversalSearchResized => {
                "workspace::view::Workspace::handle_universal_search_event (Resize)"
            }
            Self::UndoCloseRestored => "undo_close::stack::UndoCloseStack::undo_close",
        }
    }
}

#[cfg(test)]
#[path = "save_policy_tests.rs"]
mod tests;
