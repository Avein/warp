# 04 — Extract `identity_dedupe` as a pure tested module

## Parent

[`docs/projects-origin-simplification.md`](../projects-origin-simplification.md)

## What to build

After the enum collapse (#03), the dedupe lookup inside `ProjectSwitcher` lives
in two methods (`live_workspace_for_name` for Config; the new
`live_workspace_for_template_and_path` for Template). Pull the matching logic
out into a pure module `identity_dedupe`:

> `find_live_workspace(identity, stamps, is_live_fn) -> Option<EntityId>`

Where:
- `identity: &ProjectIdentity` — what to look for.
- `stamps: &HashMap<EntityId, ProjectIdentity>` — the switcher's stamp map.
- `is_live_fn: impl Fn(EntityId) -> bool` — injected liveness check (the
  `ProjectSwitcher` caller wires this to `WorkspaceRegistry::is_workspace_live`).

The function routes by `ProjectOrigin` variant:
- `Config { config_name }` → match on stamp's `config_name`, ignoring path.
- `Template { template_name }` → match on `(template_name, path)` jointly.

`ProjectSwitcher::live_workspace_for_name` and any equivalent template lookup
are reimplemented as thin wrappers around `find_live_workspace`. No external
behavior change.

## Acceptance criteria

- [ ] New module file `identity_dedupe.rs` (or similar) with one public
      function; no app context, no `ProjectSwitcher` dependency.
- [ ] `ProjectSwitcher`'s public lookup methods route through the new
      function.
- [ ] Unit tests cover:
      - Config lookup matches on `config_name` regardless of path.
      - Template lookup matches `(template_name, path)` jointly — same
        template at a different path is not a match.
      - Liveness filter excludes closed workspaces (use a synthetic
        `is_live_fn`).
      - Mixed-origin stamp set: a Config and a Template with the same string
        as their name field do not collide.
- [ ] `cargo check`, `cargo clippy -- -D warnings`, `cargo test` green.
- [ ] Manual smoke: behaviors from #03 still hold — duplicate-open focuses
      existing, etc.

## Blocked by

- #03 (core enum collapse)
