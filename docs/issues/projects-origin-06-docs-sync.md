# 06 — Docs sync: glossary and tabs-redesign

## Parent

[`docs/projects-origin-simplification.md`](../projects-origin-simplification.md)

## What to build

Update the project documentation to reflect the new two-origin model, in
lockstep with the code that landed in #03–#05.

### `docs/projects-glossary.md`

- Remove the "Default / Root" entries under origin definitions.
- Rewrite the workspace/origin enumeration to: `Config` (saved YAML, baked
  cwd, including the synthetic `root`) and `Template` (saved YAML, path-less,
  applied at a path on open).
- Note that `root` is a runtime-synthetic Config (no `root.yaml`).
- Remove the "Plain window / Open Window" paragraph entirely (the concept is
  gone post-tabs-redesign and definitely post-simplification).
- Keep keybindings table; it's unaffected.

### `docs/projects-tabs-redesign.md`

- Update "Every project-tab is a project" section: drop the `Default` and
  `Root` origin mentions; reduce to `Config` and `Template`.
- Update the "Implementation status (as built)" section to reference the
  simplification.
- Mark the "Restore" sub-section's mention of Default's re-derive-from-cwd
  policy as superseded — restored stamps now uniformly keep their persisted
  name.

### `docs/projects-redesign.md`

- Add a banner at the top noting this doc is **doubly superseded** —
  originally by `projects-tabs-redesign.md`, now also by
  `projects-origin-simplification.md`. Historical reference only.

### `docs/projects-ui-polish.md`

- Update Step 4 (rename) notes: rename override key is now
  `(origin, canonical_path)` where origin is one of two variants. No content
  changes needed beyond the vocabulary alignment.

### `docs/projects-handoff.md`

- Append a short "Origin simplification" section noting that the 4 origins
  collapsed to 2 and pointing at this PRD as the source.

## Acceptance criteria

- [ ] All 5 updated docs render correctly as Markdown.
- [ ] `rg -n 'Default|Root' docs/projects-glossary.md docs/projects-tabs-redesign.md`
      returns only intended occurrences (e.g. the literal "root" workspace
      name); no stale `ProjectOrigin::Default` / `ProjectOrigin::Root`
      references in the docs.
- [ ] `projects-redesign.md` carries an explicit "doubly superseded" banner.
- [ ] No instructions in any doc reference the deleted `disambiguate_names`
      function.

## Blocked by

- #03 (core enum collapse)
- #04 (identity_dedupe extraction)
- #05 (synthetic root)
