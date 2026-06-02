# 02 — End-to-end rename via F2, in-memory only

## Parent

[`docs/projects-rename.md`](../projects-rename.md)

## What to build

The full rename experience via the keyboard, end-to-end — but with no
persistence yet. Restart loses the rename; that's #03's job. This slice
is deliberately thick (UI + action + in-memory state) because splitting
it produces non-demoable halves.

### Workspace state

- Add a field to `Workspace`:
  ```rust
  display_name_override: Option<String>,
  ```
- Update `Workspace::display_name(switcher)` to be **override-first**:
  ```rust
  if let Some(o) = self.display_name_override.as_deref() {
      return o.to_string();
  }
  // ...existing identity-name fallback from #01
  ```

### Action + binding

- Add `WorkspaceAction::RenameProjectTab` variant, handled by entering an
  "editing" state on the active workspace (storing its `EntityId` in a
  field on `Workspace` so the render layer knows which pill should host
  the editor).
- Register an `EditableBinding` for `F2`, scoped to `id!("Workspace")`,
  with `.with_mac_key_binding("f2")` **only**. F2 is bound to "find
  next" on Linux/Windows in `view_components/find.rs`; we don't shadow
  that binding there (same call as the `F3` project-bar-visibility
  binding in the polish doc).

### Editor host on the pill

- When a workspace is in editing mode, `project_tab.rs::ProjectTabComponent`
  renders an in-place text editor in place of the label span for the
  matching tab. The rest of the pill chrome (icon, close `×`,
  active-tint, border) is unchanged.
- Editor specifics:
  - **Pre-populated** with the current display name (override if set,
    else identity name).
  - **Select-all on entry.** Type to immediately replace; arrow keys to
    position the cursor. (Deliberate departure from `new_project_popup`,
    which uses cursor-at-end-no-selection because it's an append-gesture;
    rename is a replace-gesture.)
  - **Enter** or **click-outside** commits: trim leading/trailing
    whitespace; if the trimmed buffer is empty, set
    `display_name_override = None`; else set it to `Some(trimmed)`.
  - **Esc** cancels: editor closes, no state change.
  - **No validation** (no length cap, no character restrictions, no
    duplicate detection).
  - **No live preview** in other surfaces (palette / Alt+Tab); they
    only update on commit. The pill's own width adapts to the editor's
    content while editing.
- If the workspace closes mid-edit (e.g. last session-tab closes the
  workspace), the editor dies with it — no commit, no persistence, no
  special teardown.

### `template_sequence` invariant (the only subtle bit)

The `in_use_names` set passed to
`template_sequence::next_template_sequence_name` in
`root_view::focus_or_spawn_project` MUST continue to source from
`switcher.identity(id).name`, NOT from `workspace.display_name(...)`.
Renames do not free up the `<template>-N` slot they visually replaced —
opening another template picks the next free slot above the highest
in-use **identity** name. Verify with a code-review-style assertion in
the relevant call site (an inline comment is fine) plus an integration-
flavored test if convenient.

### Tests

- Pure-function test on the commit/cancel/clear mapper that turns
  `(buffer, trigger)` into either "no change" (cancel) or "new override
  value" (where the value is `Option<String>`: `Some(name)` to set,
  `None` to clear).
- Unit test on `Workspace::display_name` confirming the override-first
  ordering.

## Acceptance criteria

- [ ] F2 on a focused project-tab opens the rename editor in-place on
      the active tab's pill.
- [ ] The editor opens with the current display name pre-selected
      (select-all).
- [ ] Enter commits a non-empty trimmed buffer as the override; the
      pill, palette open-projects row, and Alt+Tab row all show the new
      name (since they read through `Workspace::display_name(...)`).
- [ ] Click-outside commits identically to Enter.
- [ ] Esc cancels — no state change.
- [ ] Clearing the buffer and committing snaps the pill back to the
      identity name (`default-N` / config name / `root`).
- [ ] Leading/trailing whitespace is trimmed on commit.
- [ ] Opening a fresh Template tab while a sibling is renamed produces
      the next slot above the highest-in-use **identity** name. Concrete
      check: rename `default-1` to `"api-prod"`; open another template
      at a different path; the new tab is `default-2`, not `default-1`.
- [ ] Workspace closing while editing kills the editor without panic;
      no orphan state remains.
- [ ] `cargo check`, `cargo clippy -- -D warnings`, `cargo test` green.
- [ ] Manual smoke via `warpfresh --build`: rename via F2; verify all
      four exit paths (Enter / click-outside / Esc / empty-clear); quit
      and relaunch — rename is gone (expected at this slice, persistence
      lives in #03).

## Blocked by

- #01 (depends on `Workspace::display_name(...)` being the unified read
  path for display names)
