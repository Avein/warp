#[cfg(feature = "local_fs")]
pub use crate::persistence::{database_file_path_for_scope, PersistenceScope};

/// One row of the persisted `windows` table, projected to the columns the
/// projects-persistence integration tests need to verify lifecycle saves.
///
/// `windows` is the table `save_app_state` writes — one row per project-tab
/// snapshot, in the canonical project-tab strip order (`id` ascending).
/// `project_identity` is the JSON-encoded `ProjectIdentity` for stamped
/// project-tabs and `None` for plain tabs.
#[cfg(all(feature = "integration_tests", not(target_family = "wasm")))]
#[derive(Debug, Clone, diesel::Queryable)]
pub struct PersistedWindowRow {
    pub id: i32,
    pub active_tab_index: i32,
    pub project_identity: Option<String>,
    pub display_name_override: Option<String>,
}

/// Test-only read of the persisted `windows` rows, ordered by `id`
/// (insertion order). Returns an empty vec if the SQLite file does not yet
/// exist or cannot be opened, so polling assertions can retry while the
/// writer thread flushes.
///
/// Used by the projects-persistence integration tests to verify the
/// `windows` table reflects a lifecycle event's mutation end-to-end —
/// catches both a missing dispatch site and a stale snapshot.
#[cfg(all(feature = "integration_tests", not(target_family = "wasm")))]
pub fn read_persisted_window_rows() -> Vec<PersistedWindowRow> {
    use diesel::prelude::*;

    use crate::persistence::{
        database_file_path_for_scope, establish_ro_connection, schema, PersistenceScope,
    };

    let path = database_file_path_for_scope(&PersistenceScope::App);
    if !path.exists() {
        return Vec::new();
    }
    // `establish_ro_connection` wraps the path in `file:…?mode=ro` itself, so
    // pass the raw filesystem path without scheme.
    let db_url = path.display().to_string();
    let mut conn = match establish_ro_connection(&db_url) {
        Ok(conn) => conn,
        Err(_) => return Vec::new(),
    };
    schema::windows::dsl::windows
        .select((
            schema::windows::columns::id,
            schema::windows::columns::active_tab_index,
            schema::windows::columns::project_identity,
            schema::windows::columns::display_name_override,
        ))
        .order(schema::windows::columns::id.asc())
        .load::<PersistedWindowRow>(&mut conn)
        .unwrap_or_default()
}
