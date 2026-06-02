# 04 — Palette fuzzy search matches override AND identity name

## Parent

[`docs/projects-rename.md`](../projects-rename.md)

## What to build

So a renamed tab is findable in the projects palette by *either* label —
the user-typed override OR the original identity-derived name. The
displayed name (and highlight) stays the override; the identity name is
extra search input.

### Data source changes (`search/command_palette/projects/data_source.rs`)

- Extend `OpenRow` with a new field:
  ```rust
  /// The workspace's original identity name (used as an extra search
  /// target so a renamed tab is findable by either label). `None` for
  /// plain `cmd+n` tabs.
  identity_name: Option<String>,
  ```
- In `run_query`, when building rows for stamped workspaces, populate
  `identity_name = Some(switcher.identity(id).name.clone())`. The
  existing `name` field continues to be the display name (override-or-
  identity, sourced through `Workspace::display_name(...)` per #01/#02).
- In `open_window_section`, the fuzzy matcher's input becomes the
  **union** of `[row.name, row.identity_name]`:
  - Run `match_indices_case_insensitive` against each.
  - The row appears if *either* matches.
  - Sort by the higher of the two scores.
  - Highlight indices come from whichever string contributed the higher
    score, preferring matches against `row.name` (so highlights
    visibly track the user's typed query when both match).

### Tests

- Add a unit test on the data-source query that stamps a workspace with
  identity name `"default-1"` and override `"api-prod"`, then asserts:
  - Query `"default"` surfaces the row.
  - Query `"api-prod"` surfaces the row.
  - Query `"nonsense"` does not.
- Existing palette tests should continue to pass unchanged (non-renamed
  rows have no override, so `identity_name == name` for them — the
  union is just a single string, same as before).

## Acceptance criteria

- [ ] `OpenRow` carries `identity_name: Option<String>`; populated for
      stamped workspaces.
- [ ] `open_window_section` matches a row if *either* of `[name,
      identity_name]` fuzzy-matches the query.
- [ ] Renamed tabs are findable in the projects palette by typing
      either the override or the original identity-derived name.
- [ ] Non-renamed tabs behave identically to before (no double-hits,
      same sort order).
- [ ] Plain `cmd+n` tabs (`identity_name = None`) match only their
      derived name as before.
- [ ] Unit test covers both query directions on a renamed row.
- [ ] `cargo check`, `cargo clippy -- -D warnings`, `cargo test` green.
- [ ] Manual smoke via `warpfresh --build`: rename `default-1` to
      "api-prod"; open the projects palette; verify "api" and "default"
      both find it.

## Blocked by

- #02 (no override exists to search for without #02; can land in
  parallel with #03 and #05)
