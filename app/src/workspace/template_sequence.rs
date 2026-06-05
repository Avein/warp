//! Pure naming for template-origin project-tabs.
//!
//! Each template (`default`, `simple_template`, …) gets its own
//! `<template>-N` sequence of display names, allocated globally across the
//! app. Slots freed by closing a tab are reused: the next opened instance
//! fills the lowest gap.
//!
//! This module is intentionally context-free — it takes the in-use name set
//! as input rather than reading from `ProjectSwitcher` — so the sequence
//! logic can be unit-tested without an `AppContext`. The caller is
//! responsible for gathering the names of currently-stamped tabs.

use std::collections::HashSet;

/// Returns the next free display name in `template_name`'s sequence.
///
/// The result is `<template_name>-N` for the smallest positive integer `N`
/// such that the resulting string is not in `in_use_names`. Names in
/// `in_use_names` that don't match the `<template>-N` shape (renamed tabs,
/// names from other templates, …) are simply absent from the candidate set
/// and don't block the search.
pub fn next_template_sequence_name(template_name: &str, in_use_names: &HashSet<String>) -> String {
    let mut n = 1usize;
    loop {
        let candidate = format!("{template_name}-{n}");
        if !in_use_names.contains(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_set_returns_first_slot() {
        assert_eq!(
            next_template_sequence_name("default", &HashSet::new()),
            "default-1"
        );
    }

    #[test]
    fn contiguous_set_returns_next_slot() {
        let in_use = names(&["default-1", "default-2", "default-3"]);
        assert_eq!(next_template_sequence_name("default", &in_use), "default-4");
    }

    #[test]
    fn gap_set_fills_lowest_gap() {
        let in_use = names(&["default-1", "default-3"]);
        assert_eq!(next_template_sequence_name("default", &in_use), "default-2");
    }

    #[test]
    fn other_template_names_do_not_block_search() {
        // Asking for `default` while `simple_template-1` is in the set returns the
        // first free `default-N` — names from other templates don't collide.
        let in_use = names(&["default-1", "simple_template-1"]);
        assert_eq!(next_template_sequence_name("default", &in_use), "default-2");
        assert_eq!(
            next_template_sequence_name("simple_template", &in_use),
            "simple_template-2"
        );
    }

    #[test]
    fn renamed_tab_names_do_not_block_search() {
        // A renamed tab whose name doesn't match the `<template>-N` shape is
        // ignored — `dragon-fire` doesn't claim any slot in the `default-N`
        // sequence, so the search returns `default-1`.
        let in_use = names(&["dragon-fire", "default-3"]);
        assert_eq!(next_template_sequence_name("default", &in_use), "default-1");
    }

    #[test]
    fn idempotent_for_equivalent_inputs() {
        // Two equal `in_use` sets must produce the same result on repeated
        // calls — there is no internal state, no clock, no randomness.
        let in_use = names(&["default-1", "default-2", "default-4"]);
        let first = next_template_sequence_name("default", &in_use);
        let second = next_template_sequence_name("default", &in_use);
        let third = next_template_sequence_name("default", &in_use);
        assert_eq!(first, "default-3");
        assert_eq!(first, second);
        assert_eq!(second, third);
    }

    #[test]
    fn successive_opens_advance_through_the_sequence() {
        // Simulates opening N tabs in sequence: each call uses the result of
        // the previous one as part of the next `in_use_names`. The expected
        // order is `default-1` → `default-2` → `default-3` → ... with no
        // skips and no duplicates, even after a gap is introduced by a
        // close.
        let mut in_use: HashSet<String> = HashSet::new();
        let opened: Vec<String> = (0..5)
            .map(|_| {
                let name = next_template_sequence_name("default", &in_use);
                in_use.insert(name.clone());
                name
            })
            .collect();
        assert_eq!(
            opened,
            vec!["default-1", "default-2", "default-3", "default-4", "default-5"]
        );

        // Close `default-3`. The next open fills the gap, not slot 6.
        in_use.remove("default-3");
        assert_eq!(next_template_sequence_name("default", &in_use), "default-3");
    }
}
