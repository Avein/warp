//! The `projects:` palette mode.
//!
//! Lists saved launch configurations ("projects") by name, MRU-ordered, with open projects marked.
//! Selecting a project focuses its window if it is already open (singleton), otherwise spawns the
//! launch config in a new window. The secondary action closes an open project's window.
//!
//! This reuses the launch-config data model and row renderer; the project-specific behavior
//! (focus-or-spawn, close, MRU ordering, open marking) lives here and in
//! [`crate::workspace::ProjectSwitcher`].

pub mod data_source;
pub mod search_item;

pub use data_source::DataSource;
