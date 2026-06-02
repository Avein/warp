# Projects-as-Tabs — Handoff

> Orientation doc for the next agent picking up this feature on branch `feat/projects-palette` of the
> personal Warp fork (`/Users/avein/personal/opensource/warp`, OSS channel `warposs`).
>
> **Read first — the authoritative companions:**
> - [`projects-tabs-redesign.md`](./projects-tabs-redesign.md) — **the current design + "as-built"
>   notes + per-phase status**. This is the source of truth for behavior.
> - [`projects-glossary.md`](./projects-glossary.md) — shared vocabulary (OS window / RootView /
>   workspace=project-tab / session-tab / pane) and the keybindings table.
> - [`projects-redesign.md`](./projects-redesign.md) — *older* "windows-as-projects" design, largely
>   superseded by the tabs redesign; historical only.

---

## What the feature is (one paragraph)

Projects open as **tabs inside the current OS window** instead of spawning a new OS window each. The
nesting is: **OS Window → RootView → N workspaces (one active) → session-tabs → panes**. A
"workspace" *is* a project-tab. They're switched via a **project bar** (second tab strip, top), the
**projects palette** (`projects:` mode, global across windows), or **Alt+Tab** (MRU within the
current window). Each workspace carries a `ProjectIdentity { name, path, origin }` where origin is
`Config` (saved launch config) · `Template` (path-less config rooted on open) · `Default` (ad-hoc,
e.g. `newds`) · `Root` (startup workspace).

---

## Status by phase (see redesign doc for detail)

| Phase | What | State |
|---|---|---|
| 1 | RootView holds N workspaces + `WorkspaceRegistry` rekey (WindowId→workspace) | **done** (working tree) |
| 2 | Project bar UI (horizontal top + hide toggle) | **done** |
| 3 | ProjectSwitcher rekey + 2-section palette + Alt+Tab within-window | **done** |
| 4 | Open flow + `cmd-shift-N` new-project popup + ad-hoc projects + same-basename disambiguation | **done** (popup committed) |
| 5 | Lifecycle: last session-tab closes the workspace; `cmd-shift-w` dropped | **done** |
| 6 | **Merge Windows command** | **DEFERRED — not built.** Plan captured in redesign doc; user will restart the app to regroup instead. |
| 7 | Persistence of workspace grouping | **done** |

---

## ⚠️ Git state — IMPORTANT

Two commits on the branch are mine (this agent):
- `0b3a4bee` — **feat: new-project-tab popup with shell-style folder completion** (the popup +
  binding fix + 7 tests + doc updates).
- `207f87c8` — **docs: defer Phase 6 (Merge Windows)**.

**But Phases 1–3/7 are implemented in the working tree and NOT fully committed.** These files are
still uncommitted (modified, from the prior session's redesign work):

```
app/src/app_state.rs
app/src/search/command_palette/mixer.rs
app/src/search/command_palette/projects/data_source.rs
app/src/search/command_palette/projects/search_item.rs
app/src/search/command_palette/view.rs
app/src/workspace/project_switcher.rs     (+197/-68)
app/src/workspace/registry.rs             (+112/-14)
app/src/workspace/view_tests.rs
```

These were left uncommitted deliberately (this agent only committed the popup work it authored).
**First task for the next agent: review and commit these** (they belong to Phases 1/3/7). Nothing is
pushed; history is local to this branch.

---

## Build / run / verify — read this, it bit us repeatedly

```sh
# cargo isn't on PATH in non-interactive shells — export first:
export PATH="/Users/avein/.rustup/toolchains/1.92.0-aarch64-apple-darwin/bin:$HOME/.cargo/bin:$PATH"

# fast compile check / tests (bare binary; NOT what the .app runs):
cargo build -p warp --bin warp-oss --features gui
cargo test  -p warp --lib --features gui new_project_popup   # the popup tests
cargo fmt   -p warp
cargo clippy -p warp --bin warp-oss --features gui -- -D warnings
```

**The running app is a bundle, not the bare binary. To actually test changes:**
- `warpfresh --build` — the user's helper: kills the running instance, runs `./script/run`
  (= `cargo bundle --features gui`, dev profile, ad-hoc sign) → `target/debug/bundle/osx/WarpOss.app`,
  then launches it. **This is the only way your code reaches the running app.**
- **Plain `warpfresh` only relaunches the EXISTING bundle** — it does NOT rebuild. We wasted a long
  time testing a stale bundle because of this.
- The bare `cargo build` (`target/debug/warp-oss`) differs from the bundled binary and is **not** what
  `open WarpOss.app` runs.
- **Do NOT use `./script/macos/bundle`** for local iteration — it uses a different feature set
  (`release_bundle,nld_*`) and `rm`s the bundle early; killing it mid-run deletes the app.
- To rebuild+launch manually without the helper:
  ```sh
  export WARP_BIN_NAME=warp-oss WARP_CHANNEL=oss FEATURES=gui WARP_SKIP_COMMON_SKILLS_INSTALL=1
  pkill -f "WarpOss.app/Contents/MacOS/warp-oss"
  ./script/macos/run --dont-open      # builds + bundles; omit --dont-open to also launch
  open target/debug/bundle/osx/WarpOss.app
  ```
- Logs: `~/Library/Logs/warp-oss.log` (rotated on each launch; INFO captured).
- `warpfresh` / `newds` live in `~/personal/dotfiles/zsh/.config/.zsh/aliases.zsh`.

---

## The `cmd-shift-N` new-project popup (this agent's main deliverable)

File: `app/src/new_project_popup.rs` (hosted by `RootView`). Behavior:
- Opens at the **home directory** (`~/`, always — not the active tab's cwd), cursor at the **end,
  unselected**.
- **Tab / ↓** complete-or-cycle the folder being typed: unique match completes the name (no trailing
  `/` — the user types that to descend); multiple matches extend to the longest common prefix, then
  once there's no shared-prefix progress (e.g. after a `/`) repeated Tab/↓ **cycles** the matching
  folders; **↑** cycles backward. Case-insensitive, directories only.
- **Enter** opens an ad-hoc project-tab at the path (`~` expanded) via
  `root_view:open_default_session` (the `newds` mechanism). **Escape** / click-outside dismisses.
- Pure completion logic is split into `build_completion`/`longest_common_prefix` and covered by 7
  unit tests in the same file.

### Design decisions & rejected alternatives (popup)

These were the forks in the road — recording them so the next agent doesn't relitigate them.

- **UX style: rejected file-picker + rejected validation-only, chose shell-style Tab completion.**
  First pass tried a native file picker; user wanted to stay on keyboard. Second pass tried
  type-and-validate (red border on bad paths); user response: *"validation is not enough tbh — can we
  make a simple tab completion? without dropdowns etc?"*. Final design is bash-ish: in-line
  completion, no popup list, no dropdown.
- **Initial path = home directory, always.** Not the active tab's cwd. User asked for it explicitly
  after the first build used `ActiveSession::path_if_local`.
- **Initial buffer ends with a trailing `/`.** `RootView::show_new_project_popup` appends `/` to the
  starting path (`~/`) so the *very first* Tab cycles the contents of `~`, not its siblings. Once the
  user types a name and then `/` to descend, the same property holds at each level. (Cycle outputs
  themselves never include trailing `/` — see next bullet.)
- **Cycle/Replace outputs never carry a trailing `/`.** The user types `/` to descend; auto-appending
  it produced `//` after a cycle. Fix: unique-match Replace and Cycle both emit bare folder names;
  trailing `/` is the user's signal "descend now."
- **Bash-like "LCP first, then cycle on the next Tab".** A single Tab extends to the longest common
  prefix among matches; if nothing more can be added (cursor already at the LCP), subsequent Tabs
  cycle. Detected by comparing buffer with `last_inserted` — any user keystroke that changes the
  buffer drops out of cycle mode.
- **Cursor at end, no selection.** `select_all_on_focus: false` + we removed the `editor.select_all`
  call in `open()`. User: *"can we start with home without text being highlighted?"*
- **Case-insensitive matching, directories only.** macOS HFS+/APFS is case-insensitive in practice;
  matching follows.
- **`~` expansion is preserved.** Completion operates on the *display* path (`~/foo`); expansion via
  `shellexpand::tilde` only happens at Enter time and for `read_dir` listing of the `dir_portion`.
- **Pure logic split from filesystem.** `build_completion` + `longest_common_prefix` take a
  `&[String]` candidate list — no FS access. `complete_dir_path` does the `read_dir` and calls into
  them. This is what makes the 7 unit tests possible (`tempdir` is only needed for the dir-listing
  test).
- **`CustomAction::NewProjectTab` is kept as a dead variant returning `None`.** Removing it would
  shift discriminants of every later variant (the enum is `#[repr(isize)]` + derives `Sequence`).
  Cheaper to leave it inert in `util/bindings.rs` and bind via `EditableBinding` instead.

---

## Gotchas / learnings (will save you hours)

1. **macOS custom-trigger bindings don't fire from the keyboard.** `Trigger::Custom` fixed bindings
   only fire via a **Mac menu item** — the custom→keystroke conversion is
   `#[cfg(not(target_os = "macos"))]` (see `app/src/lib.rs` ~line 996 and
   `crates/warpui_core/src/keymap/matcher.rs`). With no menu item, a `FixedBinding::custom(...)` is
   dead on macOS. **Use an `EditableBinding` with an explicit `.with_mac_key_binding(...)`** scoped to
   a context (e.g. `id!("Workspace")`). This is how the popup's `cmd-shift-N` is bound in
   `workspace/mod.rs`.
2. **Shift+letter chords MUST be uppercase.** `cmd-shift-N`, not `cmd-shift-n` — the keymap validator
   (`keymap.rs` ~line 950) **panics at startup** on shift+lowercase. (We shipped a lowercase one once
   and the app crashed silently on launch — no UI, only a panic in the log.)
3. **Shell hooks mangle `rg` output** for tokens like `new`/`add`/`open`/`repository` (they get
   rewritten to `n`/`ln`). **Use the Read tool for accuracy** when those words matter. Hooks also
   enforce `rg`/`eza`/`bat`/`fd`/`sd` over `grep`/`ls`/`cat`/`find`/`sed`, and **block `rm -rf`** (ask
   the user).
4. **`CustomAction` enum:** add new variants at the **END** (it derives `Sequence` and is
   `#[repr(isize)]`; order = discriminants).
5. **Close-confirmation ("are you sure?") is gated on the `show_warning_before_quitting` setting**
   (`~/.warp/settings.toml`), not a bug. With it off (the dev's local config), no dialog ever appears
   even with a live `sleep 1000`.
6. **`cmd-shift-N` had FOUR claimants — three had to move.** When we added the popup binding, the
   keystroke was already silently swallowed by other contexts. The conflicts we hit and resolved:
   - `WelcomeView` had a `cmd-shift-N` for "new pane" — moved to `ctrl-alt-n`.
   - `project_buttons` (project bar) had it for a different "new" action — moved to `ctrl-alt-n`.
   - The terminal input editor scope had a duplicate — moved to `ctrl-alt-n`.
   - A duplicate `AddWindow`-style binding existed too — removed.
   Lesson: when a binding "doesn't fire," check **every** context that might match; the highest-
   priority/closest context wins silently. `rg "cmd-shift-N"` and `rg 'cmd-shift-"N"'` both matter
   (quoting varies across the codebase).
7. **`propagate_and_no_op_vertical_navigation_keys: Always` is what makes Tab/↑/↓ reach the popup.**
   Counter-intuitively, this field on the editor also gates **Tab** (the field is reused for the Tab
   key, not just vertical nav). Without it, the editor swallows Tab and you never get a `Navigate`
   event. Setting it to `Always` is what turns Tab + Up + Down into `Event::Navigate(NavigationKey::…)`
   that the popup view handles. If completion stops working, check this field first.
8. **In a single-line editor the cursor is on the first AND last line simultaneously** — that's why
   both `Navigate(Up)` and `Navigate(Down)` propagate out and our handler can wire ↑ to cycle-prev
   and ↓ to cycle-next without fighting the editor.
9. **Cycle detection is equality-based, not flag-based.** We store `last_inserted: Option<String>`;
   on each completion call, if `current_buffer == last_inserted` we advance the cycle index,
   otherwise we recompute candidates. This means **any user keystroke drops the popup out of cycle
   mode automatically** — no explicit reset needed.

---

## Key files map

| Area | File |
|---|---|
| Per-window container, N workspaces, open flow, popup host | `app/src/root_view.rs` |
| Workspace registry (WindowId↔workspaces, active, MRU helpers) | `app/src/workspace/registry.rs` |
| ProjectSwitcher: identity stamps, MRU, origin enum | `app/src/workspace/project_switcher.rs` |
| Workspace view: tabs, lifecycle, `TransferredTab`, snapshot/persist | `app/src/workspace/view.rs` |
| New-project popup (Tab completion) | `app/src/new_project_popup.rs` |
| Keybinding registration (`cmd-shift-N` EditableBinding, fixed bindings) | `app/src/workspace/mod.rs` |
| `WorkspaceAction` enum + dispatch | `app/src/workspace/action.rs`, `app/src/workspace/view.rs` |
| Custom action → default keystroke map (macOS gotcha lives here) | `app/src/util/bindings.rs` |
| Projects palette: sections + Alt+Tab + disambiguation | `app/src/search/command_palette/projects/data_source.rs` |
| Palette row render + per-origin icon | `app/src/search/command_palette/projects/search_item.rs` |
| Persistence (grouping columns, round-trip) | `app/src/persistence/sqlite.rs` + `crates/persistence/{schema,model}.rs` + diesel migration |
| `warpfresh` / `newds` shell helpers | `~/personal/dotfiles/zsh/.config/.zsh/aliases.zsh` |
| Cross-window content transfer (reference for Phase 6) | `app/src/workspace/cross_window_tab_drag.rs` |

---

## Open items / next steps

1. **Commit the uncommitted Phases 1/3/7 files** (see Git state above).
2. **Phase 6 (Merge Windows)** — deferred; full implementation plan is in
   `projects-tabs-redesign.md` under "Phase 6 — Merge Windows (DEFERRED — not implemented)". Resolve
   the open decisions there (picker UI: multi-select checklist modal vs merge-all; trigger: palette
   vs keybinding) before building. Reuse `TransferredTab` (`get_tab_transfer_info` →
   `insert_transferred_tab_at_index`) + `TerminationMode::ContentTransferred`.
3. **Things needing human eyes** (not unit-testable): project-bar appearance with >1 project-tab;
   palette icons per origin; Alt+Tab MRU feel; restart restores the consolidated grouping.
4. Vertical (`Left`/`Right`) project bar polish; quick-launch ⌘-1..9; per-project metadata;
   upstreaming.

---

## Origin simplification (2026-06)

After the projects-as-tabs work landed, the `ProjectOrigin` enum was collapsed from 4 variants
(`Config` · `Template` · `Default` · `Root`) down to **2**:

- `Config { config_name }` — saved launch config with baked `cwd`s, including the runtime-synthetic
  startup `root` (no `root.yaml` on disk). Dedupe key: `config_name` alone.
- `Template { template_name }` — path-less config applied at a path at open time, covers
  `cmd-shift-N`, `newds`, the `default` template behind `cmd-n`, and any user-saved templates.
  Dedupe key: `(template_name, path)`. Display name follows a global `<template>-N` sequence
  (`default-1`, `default-2`, `simple_template-1`, …) with gap-fill on close.

Source of truth: [`projects-origin-simplification.md`](./projects-origin-simplification.md) (PRD)
plus the six issue files under [`issues/projects-origin-0[1-6]-*.md`](./issues/). The work shipped
as six commits on `feat/projects-palette` between `99472eea` and `9873f533`. A SQLite migration
(`2026-06-02-000000_wipe_windows_for_origin_simplification`) wipes the `windows` table on first
launch after rollout, so no in-the-wild legacy origin records survive. Notable consequences for
later work:

- `ProjectSwitcher::claim_root` and `disambiguate_names` are gone. Root auto-spawn lives in
  `root_view::spawn_synthetic_root` (dispatched from `launch()` on empty state); per-origin dedupe
  lives in `workspace::identity_dedupe::find_live_workspace`.
- `ProjectIdentity::path` is non-`Option` everywhere — both variants always carry a concrete path.
- Icon mapping is centralised in `workspace::project_icon` (`Folder` for `Config`, `LayoutAlt01`
  for `Template`); both the project bar and the palette read it from there.

---

## Process notes

- Global rules in effect: think before coding, surgical changes only, never run destructive git
  commands without confirmation, verify before reporting complete (build/test, don't claim from code
  existence alone).
- For UI work, read the `warp-ui-guidelines` skill up front.
- Rust unit tests: `cargo test -p warp --lib --features gui <filter>`.
