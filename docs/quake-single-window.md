# Quake Single-Window Model

> Personal fork feature (branch `diag/quake-mode-spaces`). Supersedes the
> two-window quake design and amends the restore-side assumptions in
> [`projects-persistence.md`](./projects-persistence.md) (see the 2026-06-12
> addendum there).
>
> **Status:** implemented and user-verified 2026-06-12; `[quake-diag]`
> instrumentation still present, to be stripped before merge.

## Problem

The original layout ran **two** OS windows: a normal main window hosting the
project-tabs, plus a separate hidden `WindowStyle::Pin` panel that the global
hotkey showed/hid. That split caused a family of bugs:

- The `root` project was lost or mislabelled across `⌘Q`/relaunch (stale or
  missing `project_identity` on the quake row).
- Projects were "shared" between the two windows: the projects palette is
  global, so opening `root` from the quake panel just focused the main
  window — focus jumped away instead of opening anything in the panel.
- Sessions accumulated duplicate / orphan `quake_mode = 1` rows in the
  `windows` table.

## Decision

**One window.** The app creates a single panel-style window that doubles as
the main window: it hosts every project-tab (pill) and the quake hotkey
shows/hides it. There is no separate scratch quake terminal.

The window must be panel-style **from birth**: `WindowStyle::Pin` maps to a
different native class (`WarpPanel`, an `NSPanel` subclass, created via
`create_warp_nspanel`) than normal windows, and an existing `NSWindow` cannot
be converted at runtime. This also keeps the tested Spaces behavior
(`NSWindowCollectionBehaviorMoveToActiveSpace` + settle-window guards in
`window.m` / `root_view.rs`) attached to the only window the user has.

## Implementation map

All in `app/src/root_view.rs` unless noted:

| Piece | What it does |
|---|---|
| `open_panel_with_workspace_source` | The only constructor for the panel: `WindowStyle::Pin`, quake-config bounds, registers the window in `QUAKE_STATE` as `PendingOpen`. Visible and focused on return. |
| `open_from_restored` | Gathers persisted rows into the single panel (rules below), seeds with row 0, `restore_project_tab`s the rest, re-activates the saved active tab. |
| `focus_or_spawn_project` fallback | With no active window, reuses the live panel (opens the project as a tab into it) or creates the panel from the project's template. Never spawns a normal window. |
| `toggle_quake_mode_window` `None` arm | A missing panel is recreated with the synthetic-root project, not an unstamped empty tab. |
| `update_quake_mode_state` | `PendingOpen → Open` only once the panel is actually the active window. Launch fires several `active window changed: None` events back-to-back; promoting on those let the next event auto-hide the only window. |
| `Workspace::tab_bar_mode` (`workspace/view.rs`) | Returns `ShowTabBar::Hidden` for the panel: the search field, header-toolbar buttons, and right-side controls all live inside the tab bar, so this strips them in one place. The pill row (`render_project_bar`) is gated independently and stays. |
| `quake_strip_panel_chrome` (`crates/warpui/.../window.m`) | Also removes `NSWindowStyleMaskTitled`: AppKit's rounded corners come from the titled frame view, so the panel is square-cornered. Key/main status is safe — `WarpPanel` overrides `canBecomeKeyWindow`/`canBecomeMainWindow` to `YES`. |

## Restore semantics

`get_app_state` marks a row `quake_mode = 1` when its workspace lives in the
window matching `quake_mode_window_id()` — which is now **every** project-tab.
The flag is effectively "was in the panel" and restore treats all rows as
project-tabs. Row hygiene while gathering (transitional states from earlier
sessions):

1. **Identity-less quake rows are dropped** — that's the old scratch hotkey
   panel; nothing to keep.
2. **Identity-less normal rows are repaired** to the synthetic-root identity
   (`synthetic_root_identity()`) so the user lands on a labelled `root` tab,
   not a blank pill.
3. **Rows dedupe by identity name, first wins** — older builds could persist
   the same project both as a normal row and a stale quake row.

If nothing survives, zero windows exist after restore and `launch()`'s
existing fallback dispatches `root_view:spawn_synthetic_root`, which now also
lands in the panel (via the `focus_or_spawn_project` fallback).

## Behavior notes

- `hide_window_when_unfocused = true` (the user's setting) now hides the
  *only* window when focus moves to another app; the hotkey brings it back.
  This is standard quake-terminal UX but worth remembering when a "window
  disappeared" report comes in.
- Closing the panel's last tab closes the window; the next hotkey press
  recreates it with a fresh `root`.
- `⌘N` still creates normal windows. Their rows persist as `quake_mode = 0`
  and merge into the panel on the next restore (the pre-existing "all groups
  collapse into one window" limitation, now ending in the panel).
- Project-bar height is configurable: `appearance.project_bar.height`
  (TOML, default `34.0` — the old hard-coded `TAB_BAR_HEIGHT`). Setting lives
  in `TabSettings` (`app/src/workspace/tab_settings.rs`); `render_project_bar`
  clamps to ≥ 1.0.

## Cleanup before merge

- Strip every `[quake-diag]` log line (Rust and ObjC) and the
  `quake_diag_*` helpers in `window.m`.
- Remove the unused `quake_ms_since_space_change` extern if the Rust settle
  guard is the only remaining caller (verify).
- Decide merge path: fold into `personal/sync/*-candidate` vs. standalone PR.
