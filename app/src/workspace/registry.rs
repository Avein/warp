use std::collections::HashMap;

use warpui::{AppContext, Entity, EntityId, SingletonEntity, WeakViewHandle, WindowId};

use super::Workspace;

/// A registry that tracks all workspace views, grouped by the OS window that hosts them.
///
/// Historically this was one workspace per window. With the projects-as-tabs model an OS window
/// hosts *N* workspaces (project-tabs) with one active, so each window maps to an ordered list of
/// workspaces plus the id of the active one. The list order is the project-tab order shown in the
/// project bar.
///
/// Single-workspace windows behave exactly as before: the list has one entry which is also the
/// active one, so [`Self::get`] returns it and [`Self::all_workspaces`] yields the same set.
pub struct WorkspaceRegistry {
    /// Ordered workspaces per window (front = first project-tab).
    workspaces: HashMap<WindowId, Vec<WeakViewHandle<Workspace>>>,
    /// The active workspace's view id per window.
    active: HashMap<WindowId, EntityId>,
}

impl Default for WorkspaceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceRegistry {
    pub fn new() -> Self {
        Self {
            workspaces: HashMap::new(),
            active: HashMap::new(),
        }
    }

    /// Registers a workspace as a project-tab of the given window, appended after any existing ones.
    /// The first workspace registered for a window becomes its active one.
    pub fn register(&mut self, window_id: WindowId, workspace: WeakViewHandle<Workspace>) {
        let view_id = workspace.id();
        let list = self.workspaces.entry(window_id).or_default();
        if !list.iter().any(|w| w.id() == view_id) {
            list.push(workspace);
        }
        self.active.entry(window_id).or_insert(view_id);
    }

    /// Unregisters every workspace for the given window. Called when the OS window closes.
    pub fn unregister(&mut self, window_id: WindowId) {
        self.workspaces.remove(&window_id);
        self.active.remove(&window_id);
    }

    /// Unregisters a single workspace (project-tab) from a window. If it was the active one, the
    /// last remaining workspace becomes active. Removes the window entry entirely once empty.
    pub fn unregister_workspace(&mut self, window_id: WindowId, view_id: EntityId) {
        if let Some(list) = self.workspaces.get_mut(&window_id) {
            list.retain(|w| w.id() != view_id);
            if list.is_empty() {
                self.workspaces.remove(&window_id);
                self.active.remove(&window_id);
            } else if self.active.get(&window_id) == Some(&view_id) {
                let new_active = list.last().expect("list is non-empty").id();
                self.active.insert(window_id, new_active);
            }
        }
    }

    /// Marks `view_id` as the active project-tab of `window_id` (no-op if it isn't registered there).
    pub fn set_active(&mut self, window_id: WindowId, view_id: EntityId) {
        if let Some(list) = self.workspaces.get(&window_id) {
            if list.iter().any(|w| w.id() == view_id) {
                self.active.insert(window_id, view_id);
            }
        }
    }

    /// Returns the id of the active project-tab of `window_id`, if any.
    pub fn active_id(&self, window_id: WindowId) -> Option<EntityId> {
        self.active.get(&window_id).copied()
    }

    /// Returns the active workspace for the given window, if it is still alive. Falls back to the
    /// first still-alive workspace when the recorded active one has gone away.
    pub fn get(
        &self,
        window_id: WindowId,
        app: &AppContext,
    ) -> Option<warpui::ViewHandle<Workspace>> {
        let list = self.workspaces.get(&window_id)?;
        if let Some(active_id) = self.active.get(&window_id) {
            if let Some(handle) = list
                .iter()
                .find(|w| w.id() == *active_id)
                .and_then(|w| w.upgrade(app))
            {
                return Some(handle);
            }
        }
        list.iter().find_map(|w| w.upgrade(app))
    }

    /// Returns all still-alive workspaces (project-tabs) of a window, in project-tab order.
    pub fn workspaces_for_window(
        &self,
        window_id: WindowId,
        app: &AppContext,
    ) -> Vec<warpui::ViewHandle<Workspace>> {
        self.workspaces
            .get(&window_id)
            .map(|list| list.iter().filter_map(|w| w.upgrade(app)).collect())
            .unwrap_or_default()
    }

    /// Returns the window hosting the workspace with the given view id, if that workspace is still
    /// alive. Used to resolve a project-tab (keyed by workspace) back to the OS window that must be
    /// focused / told to switch tabs.
    pub fn window_for_workspace(&self, view_id: EntityId, app: &AppContext) -> Option<WindowId> {
        self.workspaces.iter().find_map(|(window_id, list)| {
            list.iter()
                .any(|w| w.id() == view_id && w.upgrade(app).is_some())
                .then_some(*window_id)
        })
    }

    /// Whether a workspace (project-tab) with the given view id is still alive in any window.
    pub fn is_workspace_live(&self, view_id: EntityId, app: &AppContext) -> bool {
        self.window_for_workspace(view_id, app).is_some()
    }

    /// Returns the live workspace view with the given view id, from any window. Lets callers read a
    /// specific project-tab (e.g. its working directory) without it having to be the active one.
    pub fn workspace_handle(
        &self,
        view_id: EntityId,
        app: &AppContext,
    ) -> Option<warpui::ViewHandle<Workspace>> {
        self.workspaces.values().find_map(|list| {
            list.iter()
                .find(|w| w.id() == view_id)
                .and_then(|w| w.upgrade(app))
        })
    }

    /// Returns all registered workspaces that are still alive, as `(WindowId, ViewHandle)` pairs.
    pub fn all_workspaces(
        &self,
        app: &AppContext,
    ) -> Vec<(WindowId, warpui::ViewHandle<Workspace>)> {
        self.workspaces
            .iter()
            .flat_map(|(window_id, list)| {
                list.iter()
                    .filter_map(move |weak| weak.upgrade(app).map(|handle| (*window_id, handle)))
            })
            .collect()
    }
}

impl Entity for WorkspaceRegistry {
    type Event = ();
}

impl SingletonEntity for WorkspaceRegistry {}
