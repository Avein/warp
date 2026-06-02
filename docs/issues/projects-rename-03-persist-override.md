# 03 — Persist the override across quit/restart

## Parent

[`docs/projects-rename.md`](../projects-rename.md)

## What to build

Wire the override field added in #02 through the persistence layer so it
survives quit/relaunch — as long as the workspace is in the snapshot.
Closing the tab still drops the override (workspace-scoped, by design).

### SQLite schema

- New migration:
  `crates/persistence/migrations/<date>_add_display_name_override/up.sql`
  ```sql
  ALTER TABLE windows ADD COLUMN display_name_override TEXT;
  ```
  `down.sql` either a no-op comment or `ALTER TABLE windows DROP COLUMN
  display_name_override;` if the project's SQLite version is ≥ 3.35.
- The origin-simplification wipe already cleared `windows`, so this ALTER
  applies cleanly to an empty (or near-empty) table.

### Diesel

- Regenerate / hand-edit `crates/persistence/schema.rs` to include the
  new column.
- Update any hand-maintained model structs that mirror the `windows`
  table (e.g. the `NewWindow` / `Window` rows in `persistence/sqlite.rs`).

### Snapshot layer

- Add `display_name_override: Option<String>` to the per-window snapshot
  struct (whatever `app_state.rs::get_app_state` writes into and
  `open_from_restored` reads back).
- **Save path** (`get_app_state`): for each persisted workspace, write
  its current `display_name_override` into the snapshot.
- **Restore path** (`open_from_restored` → `RootView::new` /
  `restore_project_tab`): hydrate `Workspace::display_name_override` from
  the loaded snapshot when constructing the workspace.

### Tests

- Extend `test_sqlite_round_trips_project_identity` in
  `persistence/sqlite_tests.rs` to cover a workspace with
  `display_name_override = Some("api-prod")`. Save → load → assert the
  field round-trips intact on the restored `Workspace` / `WindowSnapshot`.

## Acceptance criteria

- [ ] Migration applies cleanly to a fresh database; existing (empty)
      rows get `NULL` for the new column.
- [ ] A workspace's `display_name_override` round-trips through save →
      load intact; covered by the extended unit test.
- [ ] Closing a tab still drops the override: the row is no longer in
      the next snapshot, so reopening the same identity later gets a
      fresh default name (verified manually).
- [ ] `cargo check`, `cargo clippy -- -D warnings`, `cargo test` green.
- [ ] Manual smoke via `warpfresh --build`: rename a tab to "api-prod",
      quit Warp, relaunch — the pill still says "api-prod". Then close
      the tab via the palette's secondary action, reopen the same path —
      tab is back to `default-N`.

## Blocked by

- #02 (depends on the `display_name_override` field existing on
  `Workspace`)
