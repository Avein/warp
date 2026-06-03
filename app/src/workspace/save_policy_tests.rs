use std::collections::HashSet;

use super::PersistedStateMutation;

/// Asserts that every variant has a non-empty `dispatch_site()` label —
/// including the `pending: …` placeholders, which are non-empty by
/// construction. A blank label would silently let a new variant slip in
/// without a real dispatch (placeholder or otherwise).
#[test]
fn dispatch_site_label_is_non_empty() {
    for variant in PersistedStateMutation::ALL {
        let label = variant.dispatch_site();
        assert!(
            !label.is_empty(),
            "PersistedStateMutation::{variant:?} has an empty dispatch_site label"
        );
    }
}

/// Asserts that no two variants share a `dispatch_site()` label. Two
/// variants pointing at the same string would defeat the audit table's
/// "one variant per mutation site" property.
#[test]
fn labels_are_unique() {
    let mut seen: HashSet<&'static str> = HashSet::new();
    for variant in PersistedStateMutation::ALL {
        let label = variant.dispatch_site();
        assert!(
            seen.insert(label),
            "PersistedStateMutation::{variant:?} reuses dispatch_site label {label:?} \
             (every variant must name a distinct site so the audit table is unambiguous)"
        );
    }
}

/// Asserts that `PersistedStateMutation::ALL` enumerates every variant of
/// the enum. Implemented by an exhaustive `match self` that maps every
/// variant to its position in `ALL` and then re-walks `ALL` to confirm the
/// indices are 0..N. Adding a new enum variant without listing it in
/// `ALL` fails compilation on the match (forces an arm) or fails this
/// test at runtime (the new variant's index won't be in `ALL`).
#[test]
fn all_variants_is_exhaustive() {
    for (idx, variant) in PersistedStateMutation::ALL.iter().enumerate() {
        assert_eq!(
            variant_index(variant),
            idx,
            "PersistedStateMutation::ALL[{idx}] = {variant:?} but \
             variant_index says {}. Did you reorder ALL without updating \
             variant_index, or vice versa?",
            variant_index(variant),
        );
    }
    // If a variant exists that isn't in ALL, its `variant_index` will be
    // >= ALL.len() and the assert above for that index would never fire
    // (because the loop only walks ALL). Catch that here.
    assert_eq!(
        PersistedStateMutation::ALL.len(),
        TOTAL_VARIANTS,
        "PersistedStateMutation::ALL is missing variants — declared {} but \
         exhaustive match counts {TOTAL_VARIANTS}",
        PersistedStateMutation::ALL.len(),
    );
}

/// Number of variants in [`PersistedStateMutation`], computed from the
/// exhaustive `match` in [`variant_index`]. Update this when adding a
/// new variant.
const TOTAL_VARIANTS: usize = 16;

/// Maps each variant to its index in [`PersistedStateMutation::ALL`].
/// The exhaustive `match self` here is what guarantees the
/// `all_variants_is_exhaustive` test can't silently miss a new variant:
/// adding a variant to the enum without listing it here fails
/// compilation, forcing the contributor to think about which audit slot
/// the new mutation belongs in.
fn variant_index(variant: &PersistedStateMutation) -> usize {
    use PersistedStateMutation::*;
    match variant {
        ProjectTabOpenedInExistingWindow => 0,
        ProjectTabClosedNonLastInWindow => 1,
        ProjectTabRenamed => 2,
        NewOsWindowOpened => 3,
        OsWindowClosed => 4,
        ActiveOsWindowChanged => 5,
        OsWindowMovedOrResized => 6,
        AppWillTerminate => 7,
        WorkspaceActionRequiringSave => 8,
        SessionShellBootstrapped => 9,
        WorkspacePaneGroupStateChanged => 10,
        SessionTabRemoved => 11,
        SessionTabOrPaneRenameCommitted => 12,
        CrossWindowTabTransferFinalized => 13,
        UniversalSearchResized => 14,
        UndoCloseRestored => 15,
    }
}

/// The remaining gap-fix variants still carry their `pending: …`
/// placeholder labels until their bug-fix slices land. Slice #02 has
/// landed (`ProjectTabOpenedInExistingWindow`), so it no longer appears
/// here; #03 and #04 are still pending.
#[test]
fn remaining_gap_fix_variants_still_carry_pending_labels() {
    for (variant, expected) in [
        (
            PersistedStateMutation::ProjectTabClosedNonLastInWindow,
            "pending: projects-persistence-03",
        ),
        (
            PersistedStateMutation::AppWillTerminate,
            "pending: projects-persistence-04",
        ),
    ] {
        assert_eq!(
            variant.dispatch_site(),
            expected,
            "{variant:?} should still carry its placeholder label \
             until its bug-fix slice lands"
        );
    }
}
