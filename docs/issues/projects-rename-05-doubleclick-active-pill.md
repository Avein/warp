# 05 — Add double-click trigger on active pill

## Parent

[`docs/projects-rename.md`](../projects-rename.md)

## What to build

The mouse-trigger complement to F2. Double-clicking a project-tab pill
opens the rename editor, but only if that tab is the *already-active*
one — so single-click-activate semantics for inactive tabs are preserved.

This slice is the trigger path only; the editor itself ships in #02 and
is reused wholesale.

### Behavior

The user-visible gesture matches macOS Finder ("click selects, click on
selected renames") and browser tab rename:

- **Single click on an inactive tab:** activates it (existing behavior).
  No rename.
- **Single click on the already-active tab:** noop (existing behavior).
- **Double-click on the already-active tab body:** opens the rename
  editor (dispatches `WorkspaceAction::RenameProjectTab`).
- **Double-click on an inactive tab body:** the first click activates,
  the second click on the now-active tab opens the editor. (Falls out
  naturally from the rule above — two single-click handlers, the second
  one fires when the tab is already active.)
- **Click on the pill's close `×`:** still closes the tab. Never
  triggers rename, even on double-click.

### Implementation sketch

In `workspace/project_tab.rs::ProjectTabComponent`:

- Detect double-click on the pill *body* (excluding the close `×`'s hit
  region). Mouse-state tracking already exists for the close-button's
  hover via `ProjectTabMouseStates`; extending it for click counting
  is local.
- On the second click of a double-click pair, if `workspace_id ==
  active_workspace_id`, dispatch `WorkspaceAction::RenameProjectTab`
  for that workspace; otherwise treat as the existing single-click
  activate.

### Tests

- Mostly manual via `warpfresh --build` — the four cases listed in the
  Acceptance criteria below cover the gesture matrix.
- A unit test on the trigger-decision helper (input: `(is_active,
  click_count)` → output: `Activate | OpenRename | Noop`) if the helper
  is cleanly extractable.

## Acceptance criteria

- [ ] Single-clicking an inactive project-tab activates it (no
      rename), matching existing behavior.
- [ ] Double-clicking the already-active project-tab opens the rename
      editor.
- [ ] Click-then-double-click on an inactive tab: first click
      activates, second click opens the editor (Finder-style).
- [ ] Clicking the close `×` on a tab still closes that tab —
      single-click or double — and does not open the editor.
- [ ] Both F2 and double-click produce identical editor behavior (same
      pre-population, same select-all, same commit/cancel paths).
- [ ] `cargo check`, `cargo clippy -- -D warnings`, `cargo test` green.
- [ ] Manual smoke via `warpfresh --build`: verify all four gesture
      cases above.

## Blocked by

- #02 (reuses the `RenameProjectTab` action and the editor host; can
  land in parallel with #03 and #04)
