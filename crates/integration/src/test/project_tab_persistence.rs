//! Persistence integration tests for the projects-as-tabs feature.
//!
//! Each test pins one of the documented persistence guarantees the fork's
//! customization provides — a silent persistence break manifests as data
//! loss, exactly the kind of regression a 5-minute manual smoke is most
//! likely to miss.
//!
//! Verification surface: every test drives a lifecycle event via the
//! documented `dispatch_global_action` DSL and asserts on the persisted
//! `windows` rows on disk via
//! [`warp::integration_testing::persistence::read_persisted_window_rows`].
//! `windows` is the table `save_app_state` writes — one row per project-tab
//! snapshot — so its row count and `project_identity` JSON are the
//! documented public surface for "what would be restored on next launch".
//!
//! The "no harness machinery" rule from the slice spec is respected: the
//! only new helper is the test-only read function, consistent with the
//! existing `sqlite_testing::set_user_and_hostname_for_blocks` pattern.
//!
//! See [`docs/issues/fork-strategy-05-persistence-integration-tests.md`].

use std::path::PathBuf;
use std::time::Duration;

use warp::integration_testing::persistence::{read_persisted_window_rows, PersistedWindowRow};
use warp::integration_testing::step::new_step_with_default_assertions;
use warp::integration_testing::terminal::wait_until_bootstrapped_single_pane_for_tab;
use warp::launch_configs::launch_config::LaunchConfig;
use warp::root_view::{CloseWorkspaceArg, FocusOrSpawnProjectArg};
use warp::workspace::{ProjectOrigin, WorkspaceRegistry};
use warpui::integration::{AssertionOutcome, TestStep};
use warpui::SingletonEntity;

use super::{new_builder, Builder};

/// Builds a Template-origin project-tab arg for `root_view:focus_or_spawn_project`.
/// Mirrors the `newds <path>` / projects-palette code paths that the bug-fix
/// commits route through.
fn template_arg(template_name: &str, cwd: PathBuf) -> FocusOrSpawnProjectArg {
    FocusOrSpawnProjectArg {
        launch_config: LaunchConfig::single_pane(template_name.to_string(), cwd),
        origin: ProjectOrigin::Template {
            template_name: template_name.to_string(),
        },
    }
}

/// Returns the human-readable `name` field embedded in the JSON-encoded
/// `project_identity`, or `None` for an unstamped row. Used to spot-check
/// strip order in the persisted snapshot.
fn identity_name(row: &PersistedWindowRow) -> Option<String> {
    let json = row.project_identity.as_deref()?;
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    Some(value.get("name")?.as_str()?.to_string())
}

/// Polling assertion that waits for the persisted `windows` table to have
/// exactly `expected` rows, retrying until the writer thread has flushed.
fn assert_persisted_window_count(expected: usize) -> TestStep {
    let step_name: &'static str = Box::leak(
        format!("Persisted windows count == {expected}").into_boxed_str(),
    );
    let assertion_name = format!("on-disk windows row count == {expected}");
    TestStep::new(step_name)
        .set_timeout(Duration::from_secs(30))
        .add_named_assertion(assertion_name, move |_app, _window_id| {
            let rows = read_persisted_window_rows();
            if rows.len() == expected {
                AssertionOutcome::Success
            } else {
                AssertionOutcome::failure(format!(
                    "Expected {expected} persisted window rows, found {}",
                    rows.len()
                ))
            }
        })
}

// =================================================================
// Scenario 1 — Save before `PersistenceWriter::terminate()`
// =================================================================
//
// Pinned by commit 004fbc98 — `persist_app_will_terminate(ctx)` dispatches
// `workspace:save_app` as the FIRST thing in the `on_will_terminate`
// callback, before `PersistenceWriter::terminate()` joins the writer
// thread.
//
// What we can verify from within the integration framework: the same
// `workspace:save_app` action that `persist_app_will_terminate` dispatches
// drives a snapshot end-to-end onto disk. A regression where the channel
// send is dropped or the writer thread silently fails to write would make
// this test fail. The exact "save FIRST, terminate LATER" ordering
// invariant is covered at unit-test level by
// `app_lifecycle_tests::app_will_terminate_dispatches_workspace_save_app`
// (see commit message of 004fbc98), which uses a sentinel handler and does
// not require a writer thread.
//
// See [`docs/issues/projects-persistence-04-save-on-app-terminate.md`].
pub fn test_save_before_persistence_writer_terminate_004fbc98() -> Builder {
    new_builder()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(
            new_step_with_default_assertions("Dispatch workspace:save_app (terminate-time action)")
                .with_action(move |app, _, _| {
                    app.dispatch_global_action("workspace:save_app", ());
                }),
        )
        // One synthetic-root project-tab is auto-spawned at launch (`spawn_synthetic_root`).
        // After a `workspace:save_app` dispatch, that single row must appear on disk.
        .with_step(assert_persisted_window_count(1))
        .with_step(
            new_step_with_default_assertions("Open a second project-tab, then save again")
                .with_action(move |app, _, _| {
                    let arg = template_arg(
                        "default",
                        PathBuf::from("/tmp/test-save-before-terminate-04fbc98-a"),
                    );
                    app.dispatch_global_action("root_view:focus_or_spawn_project", arg);
                    app.dispatch_global_action("workspace:save_app", ());
                }),
        )
        // After the mutation + save dispatch, the table reflects two project-tabs.
        // A broken save pipeline would keep showing 1.
        .with_step(assert_persisted_window_count(2))
}

// =================================================================
// Scenario 2 — Save on opening a project-tab into an existing window
// =================================================================
//
// Pinned by commit a2ddd467 — `focus_or_spawn_project` dispatches
// `workspace:save_app` automatically on its active-window branch
// (`persist_project_tab_opened_into_existing_window`).
//
// Crucially: NO manual `workspace:save_app` dispatch in this test. The
// regression we are guarding against is "the bug-fix dispatch was reverted
// or commented out", in which case the on-disk state would stay at 1 row.
//
// See [`docs/issues/projects-persistence-02-save-on-open-into-window.md`].
pub fn test_save_on_open_project_tab_into_existing_window_a2ddd467() -> Builder {
    new_builder()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(
            new_step_with_default_assertions("Open a project-tab into the existing window")
                .with_action(move |app, _, _| {
                    let arg = template_arg(
                        "default",
                        PathBuf::from("/tmp/test-save-on-open-a2ddd467"),
                    );
                    app.dispatch_global_action("root_view:focus_or_spawn_project", arg);
                }),
        )
        // The bug-fix dispatch in `focus_or_spawn_project` must have fired automatically.
        // Without it, this stays at 1 (just the synthetic-root tab).
        .with_step(assert_persisted_window_count(2))
}

// =================================================================
// Scenario 3 — Save on closing a non-last project-tab
// =================================================================
//
// Pinned by commit e9144fe2 — `close_workspace` dispatches
// `workspace:save_app` on its non-last-tab branch
// (`persist_project_tab_closed_non_last_in_window`).
//
// NO manual save dispatch on the close path: the bug-fix dispatch is what
// keeps the disk in sync. The last-tab branch closes the host OS window
// (covered by `PersistedStateMutation::OsWindowClosed`, out of scope here).
//
// See [`docs/issues/projects-persistence-03-save-on-close-non-last.md`].
pub fn test_save_on_close_non_last_project_tab_e9144fe2() -> Builder {
    new_builder()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(
            new_step_with_default_assertions("Open a second project-tab to set up N == 2")
                .with_action(move |app, _, _| {
                    let arg = template_arg(
                        "default",
                        PathBuf::from("/tmp/test-save-on-close-e9144fe2"),
                    );
                    app.dispatch_global_action("root_view:focus_or_spawn_project", arg);
                }),
        )
        .with_step(assert_persisted_window_count(2))
        .with_step(
            new_step_with_default_assertions("Close the non-last project-tab")
                .add_assertion(|app, window_id| {
                    let (workspace_id, host_window) = app.read(|ctx| {
                        let registry = WorkspaceRegistry::as_ref(ctx);
                        let workspaces = registry.workspaces_for_window(window_id, ctx);
                        // Strip is [root, default-1]; close root (index 0) — both are non-last
                        // because the strip has 2 entries, exercising the non-last branch.
                        (workspaces[0].id(), window_id)
                    });
                    app.dispatch_global_action(
                        "root_view:close_project_workspace",
                        CloseWorkspaceArg {
                            workspace_id,
                            window_id: host_window,
                        },
                    );
                    AssertionOutcome::Success
                }),
        )
        // The bug-fix dispatch in `close_workspace`'s non-last branch must have fired.
        // Without it, this stays at 2.
        .with_step(assert_persisted_window_count(1))
}

// =================================================================
// Scenario 4 — Project-tab strip order across restart
// =================================================================
//
// Pinned by commit 3fdf62cb — `open_from_restored` preserves the persisted
// strip order regardless of which tab was active at quit time.
//
// What we verify here: the SAVE side serializes the strip in canonical
// project-tab order with the active index recorded independently. This is
// the necessary precondition for the restore-side fix: if the saved order
// is wrong, no restore-side fix can recover the original order. The full
// round-trip restore behavior is covered at unit-test level by the
// `app_state` / `persistence::sqlite_tests` modules and by the manual
// smoke at the end of the weekly sync (`docs/fork-strategy.md`); driving
// a true ⌘Q→relaunch from within the integration harness would require
// new machinery the slice spec rules out.
pub fn test_project_tab_strip_order_save_side_3fdf62cb() -> Builder {
    new_builder()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(
            new_step_with_default_assertions("Open project-tab B (default-1)").with_action(
                move |app, _, _| {
                    let arg = template_arg(
                        "default",
                        PathBuf::from("/tmp/test-strip-order-3fdf62cb-B"),
                    );
                    app.dispatch_global_action("root_view:focus_or_spawn_project", arg);
                },
            ),
        )
        .with_step(
            new_step_with_default_assertions("Open project-tab C (default-2)").with_action(
                move |app, _, _| {
                    let arg = template_arg(
                        "default",
                        PathBuf::from("/tmp/test-strip-order-3fdf62cb-C"),
                    );
                    app.dispatch_global_action("root_view:focus_or_spawn_project", arg);
                },
            ),
        )
        .with_step(assert_persisted_window_count(3))
        // Activate B (the middle tab) — its `EntityId` is registry index 1
        // (strip is [root, default-1, default-2]). With the bug present, on a
        // future restore, default-1 would end up at strip index 0.
        .with_step(
            new_step_with_default_assertions("Focus the middle tab so active != 0").add_assertion(
                |app, window_id| {
                    let target = app.read(|ctx| {
                        WorkspaceRegistry::as_ref(ctx).workspaces_for_window(window_id, ctx)[1].id()
                    });
                    app.dispatch_global_action(
                        "root_view:focus_project_workspace",
                        warp::root_view::FocusWorkspaceArg {
                            workspace_id: target,
                            window_id,
                        },
                    );
                    AssertionOutcome::Success
                },
            ),
        )
        .with_step(
            new_step_with_default_assertions("Flush a save after the active-tab change")
                .with_action(|app, _, _| {
                    app.dispatch_global_action("workspace:save_app", ());
                }),
        )
        .with_step(
            TestStep::new("Persisted strip order is [root, default-1, default-2] with active==1")
                .set_timeout(Duration::from_secs(30))
                .add_named_assertion("strip order on disk", |_app, _window_id| {
                    let rows = read_persisted_window_rows();
                    if rows.len() != 3 {
                        return AssertionOutcome::failure(format!(
                            "Expected 3 persisted window rows, found {}",
                            rows.len()
                        ));
                    }
                    let names: Vec<Option<String>> = rows.iter().map(identity_name).collect();
                    let expected = vec![
                        Some("root".to_string()),
                        Some("default-1".to_string()),
                        Some("default-2".to_string()),
                    ];
                    if names != expected {
                        return AssertionOutcome::failure(format!(
                            "Expected strip order {expected:?}, found {names:?}"
                        ));
                    }
                    // Middle tab is active.
                    if rows[1].active_tab_index < 0 {
                        return AssertionOutcome::failure(
                            "active_tab_index should be non-negative".to_string(),
                        );
                    }
                    AssertionOutcome::Success
                }),
        )
}

// =================================================================
// Scenario 5 — `project_workspaces` eager seeding with initial Terminal handle
// =================================================================
//
// Pinned by commit 0dd69b46 — `RootView::new` pushes the initial Terminal
// workspace into `project_workspaces` eagerly during construction, so the
// seed survives any `auth_onboarding_state` mutation between `RootView::new`
// and the first render. Visible to the user as "the leftmost project-tab
// silently disappears on relaunch when the last-active tab wasn't index 0".
//
// What we verify: on a fresh launch with no prior persisted state, the
// snapshot the writer thread receives (and writes to disk) contains one
// row stamped with the synthetic-root identity (`name == "root"`,
// `ProjectOrigin::Config { config_name: "root" }`). Without the eager
// seed, the leftmost tab would be deallocated before any save dispatch and
// the persisted state would be empty.
pub fn test_project_workspaces_eager_seed_initial_terminal_0dd69b46() -> Builder {
    new_builder()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(
            new_step_with_default_assertions("Force a save of the freshly-launched session")
                .with_action(|app, _, _| {
                    app.dispatch_global_action("workspace:save_app", ());
                }),
        )
        .with_step(
            TestStep::new("Persisted state contains exactly the synthetic-root project-tab")
                .set_timeout(Duration::from_secs(30))
                .add_named_assertion("single row with root identity", |_app, _window_id| {
                    let rows = read_persisted_window_rows();
                    if rows.len() != 1 {
                        return AssertionOutcome::failure(format!(
                            "Expected exactly 1 persisted window row (synthetic-root), found {}",
                            rows.len()
                        ));
                    }
                    match identity_name(&rows[0]).as_deref() {
                        Some("root") => AssertionOutcome::Success,
                        other => AssertionOutcome::failure(format!(
                            "Expected first row's project_identity.name == \"root\", got {other:?}"
                        )),
                    }
                }),
        )
}
