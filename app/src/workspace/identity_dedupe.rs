//! Pure identity-based dedupe for project-tabs.
//!
//! Given a [`ProjectIdentity`] and a stamp map, finds the live workspace whose stamp matches the
//! identity under the per-origin dedupe rule (see [`ProjectOrigin`]):
//!
//! - `Config { config_name }` matches on `config_name` alone — path is ignored, so a Config whose
//!   `cwd` later changes still focuses the existing tab.
//! - `Template { template_name }` matches on `(template_name, path)` jointly — the same template
//!   at a *different* path spawns a new tab.
//!
//! The function is context-free: liveness is injected via a callback so the rule can be unit-tested
//! without an `AppContext` or a `WorkspaceRegistry`.

use std::collections::HashMap;

use warpui::EntityId;

use super::{ProjectIdentity, ProjectOrigin};

/// Looks up the live workspace whose stamped identity dedupe-matches `identity`.
///
/// The `name` field on `identity` is ignored; dedupe is keyed on `origin` (and `path` for
/// `Template`). The `is_live` callback filters out workspaces whose tab has since closed.
pub fn find_live_workspace(
    identity: &ProjectIdentity,
    stamps: &HashMap<EntityId, ProjectIdentity>,
    is_live: impl Fn(EntityId) -> bool,
) -> Option<EntityId> {
    stamps
        .iter()
        .find(|(id, stamp)| dedupe_matches(identity, stamp) && is_live(**id))
        .map(|(id, _)| *id)
}

/// Whether two identities are the same project for dedupe purposes. Different variants never
/// match — a `Config { config_name: "default" }` does not collide with a
/// `Template { template_name: "default" }`.
fn dedupe_matches(lhs: &ProjectIdentity, rhs: &ProjectIdentity) -> bool {
    match (&lhs.origin, &rhs.origin) {
        (ProjectOrigin::Config { config_name: a }, ProjectOrigin::Config { config_name: b }) => {
            a == b
        }
        (
            ProjectOrigin::Template { template_name: a },
            ProjectOrigin::Template { template_name: b },
        ) => a == b && lhs.path == rhs.path,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn id(value: usize) -> EntityId {
        EntityId::from_usize(value)
    }

    fn config(name: &str, path: &str) -> ProjectIdentity {
        ProjectIdentity {
            name: name.to_string(),
            path: PathBuf::from(path),
            origin: ProjectOrigin::Config {
                config_name: name.to_string(),
            },
        }
    }

    fn template(template_name: &str, display_name: &str, path: &str) -> ProjectIdentity {
        ProjectIdentity {
            name: display_name.to_string(),
            path: PathBuf::from(path),
            origin: ProjectOrigin::Template {
                template_name: template_name.to_string(),
            },
        }
    }

    fn stamps(entries: Vec<(EntityId, ProjectIdentity)>) -> HashMap<EntityId, ProjectIdentity> {
        entries.into_iter().collect()
    }

    fn all_live(_: EntityId) -> bool {
        true
    }

    #[test]
    fn config_lookup_matches_on_config_name_regardless_of_path() {
        // The stamp is at `/Users/me/elsewhere`; the lookup is at `/somewhere/else`. Config dedupe
        // ignores the path, so the lookup still focuses the existing tab.
        let stored = config("dotfiles", "/Users/me/elsewhere");
        let stamps = stamps(vec![(id(1), stored)]);
        let needle = config("dotfiles", "/somewhere/else");
        assert_eq!(find_live_workspace(&needle, &stamps, all_live), Some(id(1)));
    }

    #[test]
    fn config_lookup_misses_on_different_config_name() {
        let stamps = stamps(vec![(id(1), config("dotfiles", "/Users/me/dotfiles"))]);
        let needle = config("notes", "/Users/me/dotfiles");
        assert_eq!(find_live_workspace(&needle, &stamps, all_live), None);
    }

    #[test]
    fn template_lookup_matches_on_template_and_path_jointly() {
        let stamps = stamps(vec![(
            id(1),
            template("default", "default-1", "/home/me/api"),
        )]);
        // Same template + same path → match.
        let needle = template("default", "_unused_", "/home/me/api");
        assert_eq!(find_live_workspace(&needle, &stamps, all_live), Some(id(1)));
    }

    #[test]
    fn template_lookup_misses_on_different_path() {
        let stamps = stamps(vec![(
            id(1),
            template("default", "default-1", "/home/me/api"),
        )]);
        // Same template, different path → no match.
        let needle = template("default", "_unused_", "/home/me/web");
        assert_eq!(find_live_workspace(&needle, &stamps, all_live), None);
    }

    #[test]
    fn template_lookup_misses_on_different_template() {
        let stamps = stamps(vec![(
            id(1),
            template("default", "default-1", "/home/me/api"),
        )]);
        let needle = template("simple_template", "_unused_", "/home/me/api");
        assert_eq!(find_live_workspace(&needle, &stamps, all_live), None);
    }

    #[test]
    fn liveness_filter_excludes_closed_workspaces() {
        let stamps = stamps(vec![(id(1), config("dotfiles", "/Users/me/dotfiles"))]);
        let needle = config("dotfiles", "/Users/me/dotfiles");
        // Workspace marked dead — must not match even though origin/path agree.
        assert_eq!(find_live_workspace(&needle, &stamps, |_| false), None);
    }

    #[test]
    fn mixed_origin_with_same_name_does_not_collide() {
        // A Config { config_name: "default" } and a Template { template_name: "default" } share
        // the underlying string but live in different variants. Neither lookup direction
        // accidentally matches the other.
        let stamps = stamps(vec![
            (id(1), config("default", "/home/me/some-config")),
            (
                id(2),
                template("default", "default-1", "/home/me/some-template"),
            ),
        ]);

        let config_needle = config("default", "/home/me/some-config");
        assert_eq!(
            find_live_workspace(&config_needle, &stamps, all_live),
            Some(id(1))
        );

        let template_needle = template("default", "_unused_", "/home/me/some-template");
        assert_eq!(
            find_live_workspace(&template_needle, &stamps, all_live),
            Some(id(2))
        );
    }
}
