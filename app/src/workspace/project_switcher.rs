use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use warpui::{AppContext, Entity, EntityId, SingletonEntity};

use super::WorkspaceRegistry;

/// Where an open project-tab came from. Persisted with the project; together with
/// [`ProjectIdentity::path`] it forms the dedupe key (`config_name` alone for `Config`,
/// `(template_name, path)` jointly for `Template`) and decides which icon the palette renders.
///
/// The two variants cover every entry point: a saved YAML with baked `cwd`s (`Config`), and a
/// path-less YAML blueprint applied at a path on open (`Template`). Ad-hoc tabs from `newds` /
/// `cmd+shift+N` are template-origin (`template_name = "default"`); the startup root tab is a
/// synthetic config-origin (`config_name = "root"`, no `root.yaml`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectOrigin {
    /// Opened from a saved launch config (real or synthetic, like the startup `root`) with
    /// baked-in `cwd`s. Deduped globally by `config_name` — reopening the same config focuses the
    /// existing tab regardless of which window it lives in.
    Config { config_name: String },
    /// Opened from a path-less template applied at a path at launch time. Deduped globally by
    /// `(template_name, path)` — opening the same template at the same path focuses the existing
    /// tab; at a different path it spawns a new one.
    Template { template_name: String },
}

/// Identity stamped on a project-tab (a [`super::Workspace`]) that represents an open *project* (as
/// opposed to a plain `cmd+n` tab). Created when a project/template is opened via the projects
/// palette, `newds`, or the startup synthetic-root auto-spawn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectIdentity {
    /// Display name. For `Config` origins this is the config name (e.g. `dotfiles`); for `Template`
    /// origins it's the next slot in the template's `<template>-N` sequence (e.g. `default-1`).
    pub name: String,
    /// The project's primary working directory. Always known: a `Config` carries its baked `cwd`,
    /// a `Template` is applied at a path at open time, and the synthetic root is rooted at `~`.
    pub path: PathBuf,
    /// Where this project came from — see [`ProjectOrigin`] for the dedupe semantics.
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

    /// Returns the live `Template`-origin project-tab currently open for `(template_name, path)`,
    /// if any. The `Template` dedupe key is the pair — same template at a *different* path is not a
    /// match; same template at the *same* path focuses the existing tab. `Config`-origin tabs are
    /// ignored (they dedupe via [`Self::live_workspace_for_name`]).
    pub fn live_workspace_for_template_at(
        &self,
        template_name: &str,
        path: &Path,
        app: &AppContext,
    ) -> Option<EntityId> {
        let registry = WorkspaceRegistry::as_ref(app);
        self.workspace_for_template_at_filtered(template_name, path, |id| {
            registry.is_workspace_live(id, app)
        })
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

    /// Pure `(template_name, path)` lookup filtered by an injected liveness predicate. Split out
    /// from [`Self::live_workspace_for_template_at`] so it is unit-testable without an `AppContext`.
    fn workspace_for_template_at_filtered(
        &self,
        template_name: &str,
        path: &Path,
        is_live: impl Fn(EntityId) -> bool,
    ) -> Option<EntityId> {
        self.stamps
            .iter()
            .find(|(id, identity)| {
                matches!(
                    &identity.origin,
                    ProjectOrigin::Template { template_name: t } if t == template_name
                ) && identity.path == path
                    && is_live(**id)
            })
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
        // Default test identity uses the synthetic `root` Config — it only has to be *some* valid
        // origin/path pair; tests that exercise dedupe rules construct their own stamps directly.
        ProjectIdentity {
            name: name.to_string(),
            path: PathBuf::from("/tmp"),
            origin: ProjectOrigin::Config {
                config_name: name.to_string(),
            },
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
    fn template_at_path_lookup_matches_jointly() {
        let mut switcher = ProjectSwitcher::default();
        switcher.stamp(
            id(1),
            ProjectIdentity {
                name: "default-1".to_string(),
                path: PathBuf::from("/home/me/api"),
                origin: ProjectOrigin::Template {
                    template_name: "default".to_string(),
                },
            },
        );
        // Same template + same path → match.
        assert_eq!(
            switcher.workspace_for_template_at_filtered(
                "default",
                Path::new("/home/me/api"),
                all_live
            ),
            Some(id(1))
        );
        // Same template, different path → no match.
        assert_eq!(
            switcher.workspace_for_template_at_filtered(
                "default",
                Path::new("/home/me/web"),
                all_live
            ),
            None
        );
        // Different template, same path → no match.
        assert_eq!(
            switcher.workspace_for_template_at_filtered(
                "simple_template",
                Path::new("/home/me/api"),
                all_live
            ),
            None
        );
    }

    #[test]
    fn template_at_path_lookup_ignores_config_origin() {
        // A Config-origin stamp with the same string in `config_name` does not collide with the
        // template lookup.
        let mut switcher = ProjectSwitcher::default();
        switcher.stamp(
            id(1),
            ProjectIdentity {
                name: "default".to_string(),
                path: PathBuf::from("/home/me/api"),
                origin: ProjectOrigin::Config {
                    config_name: "default".to_string(),
                },
            },
        );
        assert_eq!(
            switcher.workspace_for_template_at_filtered(
                "default",
                Path::new("/home/me/api"),
                all_live
            ),
            None
        );
    }
}
