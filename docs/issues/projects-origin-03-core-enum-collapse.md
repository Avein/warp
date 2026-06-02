# 03 — Core enum collapse: 4 origins → 2

## Parent

[`docs/projects-origin-simplification.md`](../projects-origin-simplification.md)

## What to build

The foundation slice. Collapse `ProjectOrigin` from four variants to two and
rewire every downstream consumer to the new shape in one atomic change.

### Enum + identity

```rust
pub enum ProjectOrigin {
    Config { config_name: String },
    Template { template_name: String },
}
```

(Schema fragment from the design — not a code snippet to paste verbatim.)

`ProjectIdentity.path` becomes non-`Option<PathBuf>` (the only previous user
of `None` — the root case — gets its `path` set to `~`).

### Behavior changes folded in

- **Restore policy** in `view.rs`'s `configure_new_workspace`: drop the
  `Default`-re-derive-from-cwd arm and the `Root` arm; one uniform "keep
  persisted name" rule. (Migration in this slice wipes legacy records, so no
  in-flight records can hit a missing variant.)
- **`ProjectSwitcher`**: drop `claim_root` + `root_claimed`. Add a new lookup
  method for the Template `(template_name, path)` dedupe key alongside the
  existing Config `config_name` lookup. (Pure-module extraction of dedupe is
  deferred to #04.)
- **`focus_or_spawn_project`** (`root_view.rs`): dedupe routing branches on
  the `ProjectOrigin` variant — Config matches by `config_name`, Template
  matches by `(template_name, path)`. Existing single-window adoption logic
  stays.
- **Open-default-session**: stamps as `Template { template_name: "default" }`
  instead of `Default`. Sequence name still allocated via #01's
  `template_sequence` (now generalized; passes `"default"`).
- **`disambiguate_names`** in `data_source.rs` and its 4 unit tests:
  **deleted entirely**. Call sites cleaned up.
- **Icon picker** (the shared helper from #02): updated to a 2-arm match.

### Persistence

- New SQLite migration `<date>_wipe_windows_for_origin_simplification` that
  empties the `windows` table on first run. No serde shim, no backward-compat
  for the old enum tags.
- `test_sqlite_round_trips_project_identity` in `sqlite_tests.rs` updated to
  exercise the new enum shape — Config with `config_name`, Template with
  `template_name`.

### Out of scope for this slice

- Synthetic root auto-spawn on empty state (→ #05).
- Palette Available's synthetic root entry (→ #05).
- Pure-module extraction of `identity_dedupe` (→ #04).
- Doc updates (→ #06).

## Acceptance criteria

- [ ] `ProjectOrigin` enum has exactly two variants; both carry a `String`.
- [ ] `ProjectIdentity.path` is non-`Option`.
- [ ] All previous `Default` and `Root` match arms are gone from the workspace
      view, project switcher, project bar, palette data source, and palette
      search item.
- [ ] `disambiguate_names` is deleted; its 4 unit tests are deleted; the
      `OpenRow` build path in `data_source.rs` no longer mutates names.
- [ ] New SQLite migration registered in `crates/persistence/migrations/`; on
      first launch the `windows` table is empty.
- [ ] `test_sqlite_round_trips_project_identity` covers Config and Template
      round-trips with the new enum shape.
- [ ] `cargo check -p warp --bin warp-oss --features gui` is green.
- [ ] `cargo clippy -p warp --bin warp-oss --features gui -- -D warnings` is
      clean.
- [ ] `cargo test -p warp --features gui -- persistence::sqlite project_switcher`
      passes (or the relevant test filters).
- [ ] Manual smoke after `warpfresh --build`: app launches with one root tab
      (auto-spawn from #05 may not be in yet — for this slice, a manually
      stamped root via the same code path that #05 will formalize is
      acceptable, or the test launches with an empty window and the user adds
      a project via the palette). Tabs spawned via `cmd+shift+N` are named
      `default-1`, `default-2`, ... with gap-fill. Re-opening the default
      template at an already-open path focuses the existing tab. Opening a
      saved Config that's already open focuses it.

## Blocked by

- #01 (`template_sequence` extracted)
- #02 (icon picker consolidated)
