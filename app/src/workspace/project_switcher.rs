use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use warpui::{AppContext, Entity, EntityId, SingletonEntity};

use super::WorkspaceRegistry;

/// Where an open project came from. Persisted with the project, it decides how the project's
/// display name is restored across a restart and which icon the palette shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectOrigin {
    /// Opened from a saved launch config with baked-in `cwd`s (a real "Project", e.g. `dotfiles`).
    /// Keeps its config name across restart even if the tab's working directory later changed.
    Config,
    /// Opened from a path-less *template* re-rooted at a path at launch time. Keeps the name it was
    /// given when opened (the directory basename at that moment), not re-derived on restart.
    Template,
    /// A default/`newds` session not tied to a saved config. Its name follows the current working
    /// directory, so it may be re-derived from the tab's cwd on restart.
    Default,
    /// The startup "root project" — the first project-tab of a session, auto-stamped as `~`. Kept
    /// distinct from `Default` so the palette can give it its own icon.
    Root,
}

/// Identity stamped on a project-tab (a [`super::Workspace`]) that represents an open *project* (as
/// opposed to a plain `cmd+n` tab). Created when a project/template is opened via the projects
/// palette, `newds`, or the startup "root project" auto-registration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectIdentity {
    /// Display name (a launch-config name, a directory basename for `newds`, or the root project).
    pub name: String,
    /// The project's primary working directory, if known. `None` for the root project, whose path
    /// is read live from the tab's active session instead.
    pub path: Option<PathBuf>,
    /// Where this project came from — decides the restore naming policy and palette icon.
    pub origin: ProjectOrigin,
}

/// Tracks which live project-tabs are *projects* and their most-recently-used (MRU) ordering.
///
/// In the projects-as-tabs model a single OS window hosts *N* project-tabs (each a
/// [`super::Workspace`]), so the switcher is keyed by the workspace's [`EntityId`] rather than by
/// window: every project-tab is recorded independently, and several stamped tabs can coexist in one
/// window. Plain `cmd+n` tabs are never stamped, which is how the palette and Alt+Tab tell projects
/// apart from throwaway tabs.
///
/// Liveness is always re-checked against [`WorkspaceRegistry`] before a project-tab is surfaced or
/// focused, so entries for closed tabs are filtered out lazily — there is no explicit close hook to
/// keep in sync.
#[derive(Default)]
pub struct ProjectSwitcher {
    /// Maps a live project-tab (workspace) to its identity.
    stamps: HashMap<EntityId, ProjectIdentity>,
    /// Project-tabs in most-recently-used order (front = most recent).
    mru: Vec<EntityId>,
    /// Whether the startup "root project" tab has already been claimed. Ensures only the very first
    /// workspace registered in a session is auto-stamped as the root project.
    root_claimed: bool,
}

impl ProjectSwitcher {
    /// Stamps `workspace_id` as a project with the given identity and marks it most-recently-used.
    pub fn stamp(&mut self, workspace_id: EntityId, identity: ProjectIdentity) {
        self.stamps.insert(workspace_id, identity);
        self.touch(workspace_id);
    }

    /// Marks `workspace_id` as most-recently-used, moving it to the front of the MRU order. No-op for
    /// project-tabs that are not stamped projects.
    pub fn touch(&mut self, workspace_id: EntityId) {
        if !self.stamps.contains_key(&workspace_id) {
            return;
        }
        self.mru.retain(|id| *id != workspace_id);
        self.mru.insert(0, workspace_id);
    }

    /// Returns the identity of a project-tab, if it is stamped.
    pub fn identity(&self, workspace_id: EntityId) -> Option<&ProjectIdentity> {
        self.stamps.get(&workspace_id)
    }

    /// Whether `workspace_id` is a stamped project-tab (as opposed to a plain tab).
    pub fn is_project(&self, workspace_id: EntityId) -> bool {
        self.stamps.contains_key(&workspace_id)
    }

    /// Returns the live project-tab currently open for project `name`, if any. Used to enforce the
    /// singleton: opening an already-open project focuses it instead of spawning a duplicate.
    pub fn live_workspace_for_name(&self, name: &str, app: &AppContext) -> Option<EntityId> {
        let registry = WorkspaceRegistry::as_ref(app);
        self.workspace_for_name_filtered(name, |id| registry.is_workspace_live(id, app))
    }

    /// Forgets any project association for `workspace_id` (for example after closing its tab).
    pub fn forget(&mut self, workspace_id: EntityId) {
        self.stamps.remove(&workspace_id);
        self.mru.retain(|id| *id != workspace_id);
    }

    /// Returns the live project-tabs in MRU order (most recent first), pruning any that have since
    /// closed. The currently-focused project, if any, is included.
    pub fn projects_mru(&self, app: &AppContext) -> Vec<EntityId> {
        let registry = WorkspaceRegistry::as_ref(app);
        self.projects_mru_filtered(|id| registry.is_workspace_live(id, app))
    }

    /// Claims the startup root-project slot. Returns `true` exactly once per session (for the first
    /// workspace registered), so the caller stamps it as the root project; subsequent calls return
    /// `false`.
    pub fn claim_root(&mut self) -> bool {
        if self.root_claimed {
            return false;
        }
        self.root_claimed = true;
        true
    }

    /// Pure MRU ordering filtered by an injected liveness predicate (front = most recent). Split out
    /// from [`Self::projects_mru`] so the ordering logic is unit-testable without an `AppContext`.
    fn projects_mru_filtered(&self, is_live: impl Fn(EntityId) -> bool) -> Vec<EntityId> {
        // MRU-ordered tabs first, then any stamped tabs never touched (defensive — `stamp` always
        // touches, so this is effectively empty), all filtered to still-live tabs.
        let mut ordered: Vec<EntityId> = self
            .mru
            .iter()
            .copied()
            .filter(|id| self.stamps.contains_key(id) && is_live(*id))
            .collect();
        for id in self.stamps.keys() {
            if !ordered.contains(id) && is_live(*id) {
                ordered.push(*id);
            }
        }
        ordered
    }

    /// Pure name lookup filtered by an injected liveness predicate. Split out from
    /// [`Self::live_workspace_for_name`] so it is unit-testable without an `AppContext`.
    fn workspace_for_name_filtered(
        &self,
        name: &str,
        is_live: impl Fn(EntityId) -> bool,
    ) -> Option<EntityId> {
        self.stamps
            .iter()
            .find(|(id, identity)| identity.name == name && is_live(**id))
            .map(|(id, _)| *id)
    }
}

impl Entity for ProjectSwitcher {
    type Event = ();
}

impl SingletonEntity for ProjectSwitcher {}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(name: &str) -> ProjectIdentity {
        ProjectIdentity {
            name: name.to_string(),
            path: None,
            origin: ProjectOrigin::Default,
        }
    }

    fn id(value: usize) -> EntityId {
        EntityId::from_usize(value)
    }

    /// All tabs alive — used as the liveness predicate in tests that don't exercise pruning.
    fn all_live(_: EntityId) -> bool {
        true
    }

    #[test]
    fn stamp_marks_tab_as_project_and_records_identity() {
        let mut switcher = ProjectSwitcher::default();
        switcher.stamp(id(1), identity("alpha"));

        assert!(switcher.is_project(id(1)));
        assert!(!switcher.is_project(id(2)));
        assert_eq!(
            switcher.identity(id(1)).map(|i| i.name.as_str()),
            Some("alpha")
        );
    }

    #[test]
    fn touch_moves_tab_to_front_of_mru() {
        let mut switcher = ProjectSwitcher::default();
        switcher.stamp(id(1), identity("a"));
        switcher.stamp(id(2), identity("b"));
        switcher.stamp(id(3), identity("c"));

        // Most-recently stamped is first.
        assert_eq!(
            switcher.projects_mru_filtered(all_live),
            vec![id(3), id(2), id(1)]
        );

        // Touching an older tab brings it to the front.
        switcher.touch(id(1));
        assert_eq!(
            switcher.projects_mru_filtered(all_live),
            vec![id(1), id(3), id(2)]
        );
    }

    #[test]
    fn touch_is_noop_for_unstamped_tab() {
        let mut switcher = ProjectSwitcher::default();
        switcher.stamp(id(1), identity("a"));
        switcher.touch(id(99)); // not a project
        assert_eq!(switcher.projects_mru_filtered(all_live), vec![id(1)]);
    }

    #[test]
    fn projects_mru_prunes_dead_tabs() {
        let mut switcher = ProjectSwitcher::default();
        switcher.stamp(id(1), identity("a"));
        switcher.stamp(id(2), identity("b"));

        // Only id(1) is live: id(2) is filtered out even though it is stamped + most recent.
        let live = switcher.projects_mru_filtered(|i| i == id(1));
        assert_eq!(live, vec![id(1)]);
    }

    #[test]
    fn forget_removes_stamp_and_mru_entry() {
        let mut switcher = ProjectSwitcher::default();
        switcher.stamp(id(1), identity("a"));
        switcher.stamp(id(2), identity("b"));

        switcher.forget(id(2));
        assert!(!switcher.is_project(id(2)));
        assert_eq!(switcher.projects_mru_filtered(all_live), vec![id(1)]);
    }

    #[test]
    fn workspace_for_name_matches_live_stamped_tab() {
        let mut switcher = ProjectSwitcher::default();
        switcher.stamp(id(1), identity("dotfiles"));

        assert_eq!(
            switcher.workspace_for_name_filtered("dotfiles", all_live),
            Some(id(1))
        );
        assert_eq!(
            switcher.workspace_for_name_filtered("missing", all_live),
            None
        );
        // A dead tab of that name does not count.
        assert_eq!(
            switcher.workspace_for_name_filtered("dotfiles", |_| false),
            None
        );
    }

    #[test]
    fn claim_root_succeeds_only_once() {
        let mut switcher = ProjectSwitcher::default();
        assert!(switcher.claim_root());
        assert!(!switcher.claim_root());
        assert!(!switcher.claim_root());
    }
}
