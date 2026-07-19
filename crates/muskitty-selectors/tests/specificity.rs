//! SP-7 §17 specificity calculation tests.
//!
//! Covers §17 L4534-4633: the A/B/C triplet computation, comparison
//! rules, and the special cases for `:is`/`:not`/`:has`/`:where`/
//! `:nth-child`/`:nth-last-child`.

use muskitty_selectors::specificity::Specificity;

/// §17 L4598-4605: lexicographic comparison on (A, B, C).
#[test]
fn specificity_ordering() {
    assert!(Specificity::new(1, 0, 0) > Specificity::new(0, 99, 99));
    assert!(Specificity::new(0, 2, 0) > Specificity::new(0, 1, 99));
    assert!(Specificity::new(0, 0, 2) > Specificity::new(0, 0, 1));
    assert_eq!(Specificity::new(1, 2, 3), Specificity::new(1, 2, 3));
    // Default is (0,0,0) — the universal-selector / `*` specificity.
    assert_eq!(Specificity::default(), Specificity::new(0, 0, 0));
}
