# Projects Palette — Handoff

> Branch `feat/projects-palette` of the personal Warp fork (`/Users/avein/personal/opensource/warp`,
> OSS channel `warposs`). This is the orientation doc for the next agent picking up the feature.
> Read it together with the two companion docs:
> - [`projects-redesign.md`](./projects-redesign.md) — the design spec + "as built" notes (the
>   authoritative description of behavior).
> - [`../IMPLEMENTATION_NOTES_projects_palette.md`](../IMPLEMENTATION_NOTES_projects_palette.md) —
>   the original M1 source spikes / M2 plan (historical; some details superseded by the redesign).

## What the feature is

A `projects:` command-palette mode (⌃⌘P) plus an Alt+Tab switcher that treats **windows as
projects**. A project is a live window stamped with a `ProjectIdentity { name, path, origin }`.
The palette lists three sections — **Open Projects / Open Windows / Available** — and Alt+Tab
toggles between the two most-recently-used project windows (hold Option to walk the MRU list).

Projects come from saved launch configs (`~/.warp-oss/launch_configurations/*.yaml`):
- a config **with** baked `cwd`s is a **Project** (`origin = Config`);
- a config **without** `cwd`s is a path-less **Template** (`origin = Template`), opened *at* a path
  supplied at launch time;
- `newds` / default sessions are `origin = Default`; the startup window is `origin = Root`.

## Current state (this hand-off)

Working and verified by build + tests + a launched instance:
- Three-section palette with separators, fuzzy search preserved per section.
- Alt+Tab drop-active + offset-0 toggle (X↔Y), plain windows excluded.
- Reuse-if-plain open flow; root project auto-registered from boot.
- `cwd: Option<PathBuf>` on pane templates → Template vs Project distinction; `default.yaml` is
  path-less.
- **Origin model + per-origin palette icon:** `Folder` (Config) · `LayoutAlt01` (Template) ·
  `Navigation` (Default) · `Gear` (Root) · `Terminal` (plain window).
- **Identity persisted across restart** in a new `windows.project_identity` TEXT column (serde_json,
  via diesel migration `2026-05-25-000000_add_project_identity_to_windows`). Restore naming policy:
  Config/Template/Root keep the persisted name even if the tab cwd changed; Default re-derives from
  the current cwd basename; sessions saved before the column fall back to cwd basename as Default.

## Key files

| Area | File |
|---|---|
| Identity + MRU + origin enum | `app/src/workspace/project_switcher.rs` |
| Stamp on open / reuse-if-plain / `newds` / close | `app/src/root_view.rs` |
| Root claim + restore re-stamp + `snapshot()` persist | `app/src/workspace/view.rs` |
| Palette sections (3) + Alt+Tab surface | `app/src/search/command_palette/projects/data_source.rs` |
| Row rendering + per-origin icon | `app/src/search/command_palette/projects/search_item.rs` |
| Action → arg dispatch (origin derivation) | `app/src/search/command_palette/view.rs` |
| Launch config model (`cwd` optional, `is_template`) | `app/src/launch_configs/launch_config.rs` |
| Persistence column (save/restore) | `app/src/persistence/sqlite.rs` + `crates/persistence/{schema,model}.rs` + `migrations/…_add_project_identity_to_windows/` |
| `warpfresh` / `newds` shell helpers | `~/personal/dotfiles/zsh/.config/.zsh/aliases.zsh` |

## Build / run / verify

```sh
export PATH="/Users/avein/.rustup/toolchains/1.92.0-aarch64-apple-darwin/bin:$PATH"
cargo check  -p warp --bin warp-oss --features gui
cargo fmt    -p warp
cargo clippy -p warp --bin warp-oss --features gui -- -D warnings
cargo test   -p warp --features gui -- persistence::sqlite launch_configs::launch_config
./script/run    # builds debug bundle + launches WarpOss.app
```

Standing preference: on every rebuild, kill all running instances and launch exactly one fresh
copy — `warpfresh` (or `warpfresh --build`) does this; or `pkill -f
"WarpOss.app/Contents/MacOS/warp-oss"` then `./script/run`.

Persistence is covered by `test_sqlite_round_trips_project_identity` in
`app/src/persistence/sqlite_tests.rs`.

## What still needs human eyes (UI, not unit-testable)

- Palette icons differ across a Config project, a Template, a `newds` session, and the root.
- After a restart: Config/Template names persist even if the tab was `cd`'d elsewhere; a `newds`
  session follows its new cwd; Alt+Tab still lists/toggles project windows.

## Next steps / open items

- **M3 — resource measurement** (deferred): spawn N project windows and record idle GPU + RAM delta
  per window (`powermetrics` / Activity Monitor) to decide whether windows-as-projects needs
  revisiting. (The "M3" the user is doing now is just a working-as-expected smoke check, not this.)
- Legacy fallback re-stamps an identity-less restored window (incl. a real plain `cmd+n` window) as
  a Default project; distinguishing the two would need a session-version marker.
- Quick-launch shortcuts (⌘-1..9) and per-project metadata.
- Upstreaming.

## Process notes for the next agent

- Global rules: think before coding, surgical changes only, never run destructive git commands
  without confirmation, verify before reporting complete. Hooks enforce `rg`/`eza`/`bat`/`fd`/`sd`.
- Nothing is pushed; commit history lives only on this branch.
