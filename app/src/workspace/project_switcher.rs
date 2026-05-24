use std::collections::HashMap;

use warpui::{AppContext, Entity, SingletonEntity, WindowId};

use super::WorkspaceRegistry;

/// Tracks project (launch-config) windows opened via the `projects:` palette along with their
/// most-recently-used (MRU) ordering.
///
/// This is switcher-local state: only windows spawned through the projects palette are recorded
/// here. Liveness is always re-checked against [`WorkspaceRegistry`] before a window is focused or
/// closed, so stale entries (for example a window the user closed with Cmd+W) are handled
/// gracefully — a missing live window simply causes a fresh spawn.
#[derive(Default)]
pub struct ProjectSwitcher {
    /// Maps a launch-config name to the window it was most recently spawned into.
    windows: HashMap<String, WindowId>,
    /// Launch-config names in most-recently-used order (front = most recent).
    mru: Vec<String>,
}

impl ProjectSwitcher {
    /// Records that `name` was spawned into `window_id` and marks it most-recently-used.
    pub fn record_open(&mut self, name: &str, window_id: WindowId) {
        self.windows.insert(name.to_owned(), window_id);
        self.touch(name);
    }

    /// Marks `name` as most-recently-used, moving it to the front of the MRU order.
    pub fn touch(&mut self, name: &str) {
        self.mru.retain(|n| n != name);
        self.mru.insert(0, name.to_owned());
    }

    /// Returns the live window for `name`, if one was spawned via the switcher and is still open.
    pub fn live_window(&self, name: &str, app: &AppContext) -> Option<WindowId> {
        let window_id = *self.windows.get(name)?;
        WorkspaceRegistry::as_ref(app)
            .get(window_id, app)
            .map(|_| window_id)
    }

    /// Forgets any window association for `name` (for example after closing it). The MRU entry is
    /// intentionally retained so a just-closed project stays near the top for quick reopening.
    pub fn forget(&mut self, name: &str) {
        self.windows.remove(name);
    }

    /// Returns the MRU rank of `name` (0 = most recent), or `None` if it was never opened via the
    /// switcher.
    pub fn mru_rank(&self, name: &str) -> Option<usize> {
        self.mru.iter().position(|n| n == name)
    }
}

impl Entity for ProjectSwitcher {
    type Event = ();
}

impl SingletonEntity for ProjectSwitcher {}
