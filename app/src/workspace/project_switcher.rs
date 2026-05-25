use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use warpui::{AppContext, Entity, SingletonEntity, WindowId};

use super::WorkspaceRegistry;

/// Where an open project window came from. Persisted with the project, it decides how the project's
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
    /// The startup "root project" — the first window of a session, auto-stamped as `~`. Kept
    /// distinct from `Default` so the palette can give it its own icon.
    Root,
}

/// Identity stamped on a window that represents an open *project* (as opposed to a plain `cmd+n`
/// window). Created when a project/template is opened via the projects palette, `newds`, or the
/// startup "root project" auto-registration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectIdentity {
    /// Display name (a launch-config name, a directory basename for `newds`, or the root project).
    pub name: String,
    /// The project's primary working directory, if known. `None` for the root project, whose path
    /// is read live from the window's active session instead.
    pub path: Option<PathBuf>,
    /// Where this project came from — decides the restore naming policy and palette icon.
    pub origin: ProjectOrigin,
}

/// Tracks which live windows are *projects* and their most-recently-used (MRU) ordering.
///
/// Unlike the previous name-keyed map, this is keyed by [`WindowId`]: every project window is
/// recorded independently, so multi-window configs, same-basename `newds` projects, and the
/// startup root project all coexist without collisions. Plain `cmd+n` windows are never stamped,
/// which is how the palette and Alt+Tab tell projects apart from throwaway windows.
///
/// Liveness is always re-checked against [`WorkspaceRegistry`] before a window is surfaced or
/// focused, so entries for closed windows are filtered out lazily — there is no explicit close
/// hook to keep in sync.
#[derive(Default)]
pub struct ProjectSwitcher {
    /// Maps a live project window to its identity.
    stamps: HashMap<WindowId, ProjectIdentity>,
    /// Project windows in most-recently-used order (front = most recent).
    mru: Vec<WindowId>,
    /// Whether the startup "root project" window has already been claimed. Ensures only the very
    /// first window registered in a session is auto-stamped as the root project.
    root_claimed: bool,
}

impl ProjectSwitcher {
    /// Stamps `window_id` as a project with the given identity and marks it most-recently-used.
    pub fn stamp(&mut self, window_id: WindowId, identity: ProjectIdentity) {
        self.stamps.insert(window_id, identity);
        self.touch(window_id);
    }

    /// Marks `window_id` as most-recently-used, moving it to the front of the MRU order. No-op for
    /// windows that are not stamped projects.
    pub fn touch(&mut self, window_id: WindowId) {
        if !self.stamps.contains_key(&window_id) {
            return;
        }
        self.mru.retain(|id| *id != window_id);
        self.mru.insert(0, window_id);
    }

    /// Returns the identity of a project window, if it is stamped.
    pub fn identity(&self, window_id: WindowId) -> Option<&ProjectIdentity> {
        self.stamps.get(&window_id)
    }

    /// Whether `window_id` is a stamped project window (as opposed to a plain window).
    pub fn is_project_window(&self, window_id: WindowId) -> bool {
        self.stamps.contains_key(&window_id)
    }

    /// Returns the live window currently open for project `name`, if any. Used to enforce the
    /// singleton: opening an already-open project focuses it instead of spawning a duplicate.
    pub fn live_window_for_name(&self, name: &str, app: &AppContext) -> Option<WindowId> {
        let registry = WorkspaceRegistry::as_ref(app);
        self.stamps
            .iter()
            .find(|(window_id, identity)| {
                identity.name == name && registry.get(**window_id, app).is_some()
            })
            .map(|(window_id, _)| *window_id)
    }

    /// Forgets any project association for `window_id` (for example after closing its window).
    pub fn forget(&mut self, window_id: WindowId) {
        self.stamps.remove(&window_id);
        self.mru.retain(|id| *id != window_id);
    }

    /// Returns the live project windows in MRU order (most recent first), pruning any windows that
    /// have since closed. The currently-focused project, if any, is included.
    pub fn project_windows_mru(&self, app: &AppContext) -> Vec<WindowId> {
        let registry = WorkspaceRegistry::as_ref(app);
        // MRU-ordered windows first, then any stamped windows never touched (defensive — `stamp`
        // always touches, so this is effectively empty), all filtered to still-live windows.
        let mut ordered: Vec<WindowId> = self
            .mru
            .iter()
            .copied()
            .filter(|id| self.stamps.contains_key(id) && registry.get(*id, app).is_some())
            .collect();
        for window_id in self.stamps.keys() {
            if !ordered.contains(window_id) && registry.get(*window_id, app).is_some() {
                ordered.push(*window_id);
            }
        }
        ordered
    }

    /// Claims the startup root-project slot for `window_id`. Returns `true` exactly once per session
    /// (for the first window registered), so the caller stamps it as the root project; subsequent
    /// calls return `false`.
    pub fn claim_root(&mut self) -> bool {
        if self.root_claimed {
            return false;
        }
        self.root_claimed = true;
        true
    }
}

impl Entity for ProjectSwitcher {
    type Event = ();
}

impl SingletonEntity for ProjectSwitcher {}
