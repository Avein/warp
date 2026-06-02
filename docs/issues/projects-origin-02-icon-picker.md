# 02 — Consolidate icon picker into a shared helper

## Parent

[`docs/projects-origin-simplification.md`](../projects-origin-simplification.md)

## What to build

The mapping from `ProjectOrigin` to `Icon` is currently duplicated in two
files: `app/src/workspace/project_tab.rs` and
`app/src/search/command_palette/projects/search_item.rs`. Both implement the
same 4-arm match (`Config → Folder`, `Template → LayoutAlt01`,
`Default → Navigation`, `Root → Gear`).

Consolidate into a single helper — `icon_for_origin(Option<&ProjectOrigin>) -> Icon` —
exposed from one home (e.g. alongside `ProjectOrigin` in
`workspace/project_switcher.rs`, or as a small helper module). Both call sites
import and use it.

The enum is still 4-variant at this point. This slice does not change the
mapping or the user-visible icons; it just removes the duplication so the
follow-up enum collapse only touches one mapping site.

## Acceptance criteria

- [ ] One public helper function returns an `Icon` for a given
      `Option<&ProjectOrigin>`.
- [ ] Both `project_tab.rs` and `search_item.rs` call the helper; neither
      contains the per-variant match inline.
- [ ] The fallback for `None` matches today's behavior (whatever the inline
      match returned).
- [ ] `cargo check`, `cargo clippy -- -D warnings`, `cargo test` all green.
- [ ] Manual smoke: project bar tabs and palette rows render with identical
      icons before and after the change.

## Blocked by

None — can start immediately. Parallel with #01.
