# 05 — Synthetic root: auto-spawn on empty state + palette Available entry

## Parent

[`docs/projects-origin-simplification.md`](../projects-origin-simplification.md)

## What to build

Two coupled changes implementing the "root is a synthetic Config, always
reopenable" model.

### Empty-state auto-spawn

On app-state load (where the windows snapshot is materialized), if the total
live workspace count across all loaded windows is zero, the app spawns:

- One OS window
- Containing one workspace
- Stamped `ProjectIdentity { name: "root", path: ~, origin: Config { config_name: "root" } }`
- Hosting a single session-tab with one plain-shell pane at `~`

No `root.yaml` file is read or written. The root is purely runtime-synthetic.

This fires:
- On every first launch (no persistence at all).
- On every launch after a state wipe (#03's migration triggers this on the
  first run after that change lands).
- After the user closes the last workspace in the last window during a
  session and persistence subsequently records an empty snapshot — only on
  the *next* app launch.

It does NOT fire mid-session when the user closes their last tab (Phase 5's
"close last tab closes the OS window" rule wins; the window closes, no
respawn).

### Palette Available entry

The projects palette's Available section gains a synthetic "root" row
whenever root is **not** currently among the live stamps. Selecting it
re-spawns root through the same `focus_or_spawn_project` path, with the
identity `Config { config_name: "root" }` and `path: ~`. Per Config dedupe,
if root is already open (in any window) the row is omitted.

The Available section continues to list user-saved Configs (when not open)
and all user-saved Templates (always).

## Acceptance criteria

- [ ] After `warpfresh --build` with a wiped state, the app launches showing
      exactly one window with one workspace named `root`.
- [ ] The root workspace's path resolves to `~` for branch / diff-stats
      display.
- [ ] Closing root via the palette's secondary action (or session-tab close
      cascade) closes its window per Phase 5; no auto-respawn mid-session.
- [ ] After closing root and reopening the projects palette, an entry named
      `root` appears in the Available section.
- [ ] Selecting the synthetic root row spawns a new root workspace in the
      current window (or new window per the existing open-flow).
- [ ] When root is open, the palette's Available section does **not** list a
      root row (Config dedupe applies normally).
- [ ] The root tab renders with the Folder icon — identical to any other
      Config.
- [ ] `cargo check`, `cargo clippy -- -D warnings`, `cargo test` green.

## Blocked by

- #03 (core enum collapse — `Config { config_name: "root" }` variant exists)
