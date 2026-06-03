use std::sync::{Arc, Mutex};

use warp_core::user_preferences::GetUserPreferences as _;
use warpui::{App, AppContext, SingletonEntity};

use super::{
    has_completed_local_onboarding, persist_project_tab_closed_non_last_in_window,
    persist_project_tab_opened_into_existing_window, RootView, HAS_COMPLETED_ONBOARDING_KEY,
};
use crate::auth::auth_manager::AuthManager;
use crate::auth::AuthStateProvider;
use crate::server::server_api::ServerApiProvider;

fn initialize_app(app: &mut App) {
    app.update(crate::settings::init_and_register_user_preferences);
    app.add_singleton_model(|_ctx| ServerApiProvider::new_for_test());
    app.add_singleton_model(|_| AuthStateProvider::new_for_test());
    app.add_singleton_model(AuthManager::new_for_test);
}

fn set_local_onboarding_completed(app: &mut App, completed: bool) {
    app.update(|ctx| {
        ctx.private_user_preferences()
            .write_value(
                HAS_COMPLETED_ONBOARDING_KEY,
                serde_json::to_string(&completed).unwrap(),
            )
            .unwrap();
    });
}

/// Regression test for the bug fixed by introducing
/// `RootView::sync_local_onboarding_to_server`: when a user completed onboarding
/// pre-login and later authenticated via a non-login-slide entrypoint (i.e. while
/// already in `Terminal` state), the server-side `is_onboarded` flag was never
/// flipped. The helper runs unconditionally on `AuthComplete` and must flip the
/// flag when all preconditions hold.
#[test]
fn test_sync_flips_server_is_onboarded_when_local_onboarding_completed() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        // Seed the "has_completed_local_onboarding" preference and make the user
        // appear not yet onboarded on the server. The default test user is
        // non-anonymous, so the guards in the helper won't short-circuit.
        set_local_onboarding_completed(&mut app, true);
        app.update(|ctx| {
            AuthStateProvider::as_ref(ctx).get().set_is_onboarded(false);
            assert!(has_completed_local_onboarding(ctx));
            assert_eq!(
                AuthStateProvider::as_ref(ctx).get().is_onboarded(),
                Some(false)
            );
        });

        app.update(|ctx| {
            let auth_state = AuthStateProvider::as_ref(ctx).get().clone();
            RootView::sync_local_onboarding_to_server(&auth_state, ctx);
        });

        app.read(|ctx| {
            assert_eq!(
                AuthStateProvider::as_ref(ctx).get().is_onboarded(),
                Some(true),
                "sync should have invoked AuthManager::set_user_onboarded"
            );
        });
    });
}

/// If the user hasn't completed local onboarding, the helper must leave the
/// server-side flag untouched — onboarding hasn't actually happened yet.
#[test]
fn test_sync_noop_when_local_onboarding_not_completed() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        // Do not set HAS_COMPLETED_ONBOARDING_KEY; it defaults to false.
        app.update(|ctx| {
            AuthStateProvider::as_ref(ctx).get().set_is_onboarded(false);
        });

        app.update(|ctx| {
            let auth_state = AuthStateProvider::as_ref(ctx).get().clone();
            RootView::sync_local_onboarding_to_server(&auth_state, ctx);
        });

        app.read(|ctx| {
            assert_eq!(
                AuthStateProvider::as_ref(ctx).get().is_onboarded(),
                Some(false),
                "sync should not have changed is_onboarded when local onboarding is incomplete"
            );
        });
    });
}

/// Per-fix targeted test for [`projects-persistence-02`](../../docs/issues/projects-persistence-02-save-on-open-into-window.md):
/// the bug-fix dispatch helper invoked from `focus_or_spawn_project`
/// (right after `RootView::open_project_tab` returns on the active-window
/// branch) must produce a `workspace:save_app` global action so the writer
/// thread picks the project-tab up before the user `⌘Q`s.
///
/// We test the helper directly with a sentinel `workspace:save_app`
/// handler rather than driving `root_view:focus_or_spawn_project`
/// end-to-end: the latter would require a full `RootView`-hosting OS
/// window plus the workspace/registry/switcher singleton ladder, while
/// the helper is what actually owns the dispatch and is the only call
/// site `focus_or_spawn_project` reaches for this code path (private
/// `fn` in the same module — `cargo`'s dead-code lint enforces the link).
#[test]
fn open_into_existing_window_dispatches_workspace_save_app() {
    App::test((), |mut app| async move {
        let dispatches: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

        let dispatches_for_handler = dispatches.clone();
        app.update(move |ctx: &mut AppContext| {
            // Sentinel handler: counts every `workspace:save_app` dispatch.
            // Global actions chain (`add_global_action` appends), so this
            // composes cleanly with whatever the real handler would do.
            ctx.add_global_action(
                "workspace:save_app",
                move |_: &(), _ctx: &mut AppContext| {
                    *dispatches_for_handler
                        .lock()
                        .expect("mutex should not be poisoned") += 1;
                },
            );
        });

        app.update(persist_project_tab_opened_into_existing_window);

        assert_eq!(
            *dispatches.lock().expect("mutex should not be poisoned"),
            1,
            "persist_project_tab_opened_into_existing_window must dispatch \
             workspace:save_app exactly once per call"
        );
    });
}

/// Per-fix targeted test for [`projects-persistence-03`](../../docs/issues/projects-persistence-03-save-on-close-non-last.md):
/// the bug-fix dispatch helper invoked from `close_workspace`'s non-last
/// branch (right after `RootView::close_project_tab` returns) must
/// produce a `workspace:save_app` global action so the snapshot reflects
/// the now-smaller set of project-tabs before the user `⌘Q`s.
///
/// Same sentinel-handler pattern as
/// `open_into_existing_window_dispatches_workspace_save_app` — the
/// helper is the unit-testable surface; the link from `close_workspace`
/// is enforced by Rust's dead-code lint (private fn, single call site).
#[test]
fn close_non_last_project_tab_dispatches_workspace_save_app() {
    App::test((), |mut app| async move {
        let dispatches: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

        let dispatches_for_handler = dispatches.clone();
        app.update(move |ctx: &mut AppContext| {
            ctx.add_global_action(
                "workspace:save_app",
                move |_: &(), _ctx: &mut AppContext| {
                    *dispatches_for_handler
                        .lock()
                        .expect("mutex should not be poisoned") += 1;
                },
            );
        });

        app.update(persist_project_tab_closed_non_last_in_window);

        assert_eq!(
            *dispatches.lock().expect("mutex should not be poisoned"),
            1,
            "persist_project_tab_closed_non_last_in_window must dispatch \
             workspace:save_app exactly once per call"
        );
    });
}

/// The server-side flag should also be left untouched when it is already set,
/// even if local onboarding is complete — avoids redundant server calls.
#[test]
fn test_sync_noop_when_already_onboarded_on_server() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        set_local_onboarding_completed(&mut app, true);
        app.update(|ctx| {
            // User::test() defaults to is_onboarded = true; assert that and
            // leave it in place.
            assert_eq!(
                AuthStateProvider::as_ref(ctx).get().is_onboarded(),
                Some(true)
            );
        });

        app.update(|ctx| {
            let auth_state = AuthStateProvider::as_ref(ctx).get().clone();
            RootView::sync_local_onboarding_to_server(&auth_state, ctx);
        });

        app.read(|ctx| {
            assert_eq!(
                AuthStateProvider::as_ref(ctx).get().is_onboarded(),
                Some(true)
            );
        });
    });
}
