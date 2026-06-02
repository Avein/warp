# 01 — Refactor display-name reads to a single source

## Parent

[`docs/projects-rename.md`](../projects-rename.md)

## What to build

Pure refactor with zero behavioral change. Introduce a single API for
"give me the workspace's display name" so the later slices have one place
to add override-resolution logic.

- Add a method on `Workspace`:
  ```rust
  pub fn display_name(&self, switcher: &ProjectSwitcher) -> String {
      switcher
          .identity(self.id())
          .map(|i| i.name.clone())
          .unwrap_or_else(|| "project".to_string())
  }
  ```
- Flip the three existing display-name call sites to use it:
  - Project-bar pill (`workspace/project_tab.rs`).
  - Projects palette open-projects row (`search/command_palette/projects/data_source.rs::run_query`, where `OpenRow.name` is built for stamped workspaces).
  - Alt+Tab row (same data source, different `Surface` branch — same `OpenRow` construction).
- A small unit test on `Workspace::display_name` confirming: identity
  present ⇒ name surfaces; identity absent (plain `cmd+n` tab) ⇒
  `"project"` fallback.

After this slice the codebase reads display names through exactly one
method — but no override field exists yet, so behavior is identical
everywhere. The slices below add real override logic in one place
instead of three.

## Acceptance criteria

- [ ] `Workspace::display_name(switcher)` exists and is the only path
      for resolving a workspace's display name.
- [ ] The three call sites above all read through it; no
      `switcher.identity(id).name` reads remain in those files for the
      display-name purpose.
- [ ] Unit test covers the identity-present and identity-absent cases.
- [ ] `cargo check`, `cargo clippy -- -D warnings`, `cargo test` green.
- [ ] Manual smoke via `warpfresh --build`: bar pill, palette open-projects
      rows, and Alt+Tab rows all show the same names as before (no visible
      change).

## Blocked by

- None — can start immediately
