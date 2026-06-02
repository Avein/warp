# Projects-as-Tabs — Glossary

Shared vocabulary for the projects-as-tabs feature, so we can describe behavior precisely. Terms are
ordered from the outside in (biggest container first). See
[`projects-tabs-redesign.md`](./projects-tabs-redesign.md) for the design.

## The nesting

```
OS Window  →  RootView  →  [ Workspace … ]  (one active)  →  session-tabs  →  panes
                              └ "project-tab" / "project"     └ the normal tab strip
```

## Terms

- **OS window** — a real macOS window (its own title bar + traffic lights). `cmd+n` makes a new one.
  Closing an OS window closes everything inside it. **`cmd+shift+W` = Close Window** — this is the
  standard macOS binding (`util/bindings.rs`), and it closes the *entire OS window* with all its
  project-tabs. It is **not** part of this feature; it has always meant "close the window".

- **RootView** — the per-OS-window container. Holds N workspaces (project-tabs) and shows the
  **project bar** when there is more than one.

- **Workspace** = **project-tab** = **project** — the thing an OS window used to hold exactly one of.
  Now an OS window holds N of them with one active. Each has its own session-tab strip, panes, cwd,
  and git branch. Switched via the project bar, the projects palette, or Alt+Tab.
  - Every workspace has an **origin**: `Config { config_name }` (saved launch config with baked
    `cwd`s — also covers the synthetic startup `root`) · `Template { template_name }` (path-less
    config applied at a path supplied at open time, including the built-in `default` template behind
    `cmd-shift-N` and `newds`). See [`projects-origin-simplification.md`](./projects-origin-simplification.md)
    for the dedupe rules and the collapse from the earlier four-variant model.

- **Session-tab** — a tab *inside* a workspace (the original Warp tab strip). `cmd+t` makes one;
  `cmd+w` closes one. Closing the **last** session-tab closes the **workspace** (and, if it was the
  window's last workspace, the OS window).

- **Pane** — a split within a session-tab (one shell PTY). Unchanged by this feature.

- **Root project** — the runtime-synthetic startup workspace at `~`. There is no `root.yaml` on
  disk; it's a `Config { config_name: "root" }` auto-spawned when persisted state contains zero
  windows (first launch, after the origin-simplification state wipe, or after a session that left
  no windows behind). Reopenable from the projects palette's Available section any time it isn't
  currently live.

## Surfaces

- **Project bar** — the top bar with one button per project-tab in the current OS window. Only shown
  when the window has >1 project-tab.
- **Projects palette** (`projects:`) — the global switcher across *all* OS windows. Sections:
  Open Projects / Open Windows / Available. Enter focuses; a secondary action closes.
- **Alt+Tab** — quick MRU switch among project-tabs, current dropped so a single press toggles X↔Y.

## How opening places a project (important!)

Opening a project (palette / `newds` / template) adds it **as a project-tab in whichever OS window
is currently active** (`focus_or_spawn_project`). It does **not** always make a new OS window. So:

- If the active window is your root/plain window, the project becomes a **second tab in that window**.
- If you `cmd+n` first (new OS window) and open a project there, it lands in that new window.

This is why projects can end up grouped differently than expected — placement follows focus.

## Close confirmation ("are you sure?")

Closing can pop a warning dialog. It appears only when **`show_warning_before_quitting`** (Settings →
General, default **on**) is set *and* the thing being closed contains a **live command**
(`CommandContext::RunningCommand`/`RunningAIBlock`), a **shared session**, or **unsaved code changes**
(`UnsavedStateSummary::should_display_warning`).

- A short command like `sleep 10` only counts **while it is actually running** — once it exits there
  is nothing to warn about. Test with something that does not exit on its own: `tail -f /dev/null`,
  `sleep 600`, or `cat`.
- **The setting gates everything.** If `show_warning_before_quitting` is `false` (it lives in
  `~/.warp/settings.toml`; the developer's local config had it off) **no dialog ever appears**, even
  with a live `sleep 1000` — detection still counts the command, but `should_display_warning`
  short-circuits to `false`. Enable it in Settings → General to get prompts.
- **Closing the last session-tab of a single-project window** → the OS window closes, and the prompt
  (if any) is raised by the window-close handler (`on_should_close_window` → `for_window`).
- **Closing the last session-tab of a project-tab when the window has other project-tabs** → only that
  workspace closes, and the prompt (if any) is raised per-tab (`close_tabs` → `for_tabs`). The
  per-tab confirmation is deliberately **not** skipped in this case (see `close_tab`).

## Keybindings (current)

| Key | Closes / does | Scope |
|---|---|---|
| `cmd+w` | Close active **session-tab** | inside a workspace; last one closes the workspace |
| `cmd+shift+W` | **Close Window** (macOS standard) | the whole **OS window** + all its project-tabs |
| `cmd+t` | New session-tab | inside the active workspace |
| `cmd+n` | New **OS window** | — |
| `cmd+shift+N` | Open the **new-project-tab** path popup (home-rooted; Tab/↓ complete-or-cycle folders, ↑ back) | adds a project-tab in the active window |
| Alt+Tab | Cycle project-tabs (MRU) | within the current OS window |
| (palette secondary action) | Close a **project-tab** | any window |
