# 01 — Extract `template_sequence` as a pure tested module

## Parent

[`docs/projects-origin-simplification.md`](../projects-origin-simplification.md)

## What to build

Extract the existing `next_default_name` logic from `root_view.rs` into a new
pure module `template_sequence`. The module exposes one function:

> `next_template_sequence_name(template_name, in_use_names) -> String`

It returns `<template_name>-N` for the smallest positive integer `N` such that
the resulting string is not in `in_use_names`. No app context. No FS access.
No coupling to `ProjectSwitcher`.

The existing call site in `open_default_session` is updated to call the new
function, passing `"default"` as the template name. Behavior is unchanged
end-to-end — the same `default`/`default 1`/... sequence is produced for the
same inputs.

This is pre-work for the enum collapse: once `ProjectOrigin::Template` carries
a `template_name`, the same function generalizes to any template name with no
further code changes.

## Acceptance criteria

- [ ] New module file exists with one public function and no `ctx` / `app`
      parameters.
- [ ] `open_default_session` (or its caller) routes through the new function;
      no inline sequence logic remains in `root_view.rs`.
- [ ] Unit tests in the new module cover: empty `in_use_names`, contiguous
      set, gap-fill, multi-template (asking for `default` while
      `simple_template-1` is in the set ignores it), renamed-tab names that
      don't match the `<template>-N` pattern are ignored.
- [ ] `cargo check -p warp --bin warp-oss --features gui` is green.
- [ ] `cargo clippy -p warp --bin warp-oss --features gui -- -D warnings` is
      clean.
- [ ] `cargo test -p warp --lib --features gui template_sequence` passes.
- [ ] Manual smoke: open the app, trigger `cmd+shift+N` twice at different
      paths, confirm the second tab is named `default 1` (or whatever the
      existing format produces — sequence behavior is preserved).

## Blocked by

None — can start immediately.
