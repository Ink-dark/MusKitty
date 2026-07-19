//! Pseudo-class matching.
//!
//! Implements the matching rules for §13 tree-structural
//! pseudo-classes, §13.3 An+B pseudo-classes (`:nth-child` / etc.),
//! and §4 logical combinations (`:is` / `:not` / `:where` / `:has`).
//!
//! Pseudo-classes outside the §13/§4 scope (UI / location /
//! linguistic / resource state / display state / input — §7-§12)
//! are parsed by SP-4 but matching returns `false` per the parent
//! SP-1..SP-8 plan.
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §4 L1358-1804,
//! §13 L3792-4359.

use crate::matching::Element;
use crate::types::{PseudoClass, PseudoClassArgument};

/// §13/§4: match a pseudo-class against an element.
pub fn matches_pseudo_class<E: Element>(pc: &PseudoClass, element: &E) -> bool {
    match pc.name.as_str() {
        // §13.2 L3820: :root
        "root" => element.is_root(),
        // §13.3 L3837-3845: :empty
        "empty" => element.is_empty(),
        // §13.3 L3869: child-indexed pseudo-classes (non-An+B).
        "first-child" => element.index_among_siblings() == 1,
        "last-child" => element.index_among_siblings() == element.count_among_siblings(),
        "only-child" => element.count_among_siblings() == 1,
        "first-of-type" => element.index_among_type() == 1,
        "last-of-type" => element.index_among_type() == element.count_among_type(),
        "only-of-type" => element.count_among_type() == 1,
        // §13.3 L3968: An+B pseudo-classes — implemented in Task 5.
        "nth-child" | "nth-last-child" | "nth-of-type" | "nth-last-of-type" => {
            matches_nth_pseudo_class(pc, element)
        }
        // §4 logical combinations — implemented in Task 6.
        "is" | "where" => matches_is_where(pc, element),
        "not" => !matches_is_where(pc, element),
        "has" => matches_has(pc, element),
        // §5.4 L1956-1995: :defined — always true for non-custom-
        // element trees. Custom element tracking is out of scope.
        "defined" => true,
        // §8 L2817-3007: :scope — matches when no scoping root is
        // provided, equivalent to :root. Scoping-root-aware matching
        // is out of scope for SP-8.
        "scope" => element.is_root(),
        // §7-§12 stub pseudo-classes: matching returns false.
        // (UI / location / linguistic / resource / display / input.)
        "hover" | "active" | "focus" | "focus-visible" | "focus-within" | "link" | "visited"
        | "any-link" | "local-link" | "target" | "target-within" | "playing" | "paused"
        | "seeking" | "buffering" | "stalled" | "muted" | "volume-locked" | "enabled"
        | "disabled" | "read-only" | "read-write" | "placeholder-shown" | "default" | "checked"
        | "indeterminate" | "valid" | "invalid" | "in-range" | "out-of-range" | "required"
        | "optional" | "blank" | "current" | "past" | "future" | "lang" | "dir" | "host"
        | "host-context" => false,
        // Unknown pseudo-class: spec says it must not match
        // (parse-time rejection would have happened earlier).
        _ => false,
    }
}

/// §13.3 L3968 + §13.4 L4077: An+B pseudo-class matching.
///
/// Handles `:nth-child(An+B [of S]?)`, `:nth-last-child(An+B [of S]?)`,
/// `:nth-of-type(An+B)`, `:nth-last-of-type(An+B)`.
///
/// - `*-child` variants without `of S`: index among ALL element
///   siblings (1-based).
/// - `*-child` variants with `of S`: filter siblings to those matching
///   `S`, then index within that filtered list (self must also match
///   `S`, else it is not in the filtered list).
/// - `*-of-type` variants: index among siblings of the same type.
///
/// Implementation walks the appropriate sibling chain (previous for
/// `nth-*`, next for `nth-last-*`) counting siblings that satisfy the
/// filter, then adds 1 for self. This avoids identity comparison
/// (which `Element` trait does not expose).
fn matches_nth_pseudo_class<E: Element>(pc: &PseudoClass, element: &E) -> bool {
    let (anb, of_s) = match pc.argument.as_ref() {
        Some(PseudoClassArgument::AnPlusB(anb, of_s)) => (*anb, of_s.as_ref()),
        _ => return false,
    };

    let from_last = pc.name == "nth-last-child" || pc.name == "nth-last-of-type";
    let of_type = pc.name == "nth-of-type" || pc.name == "nth-last-of-type";

    let my_name = element.local_name();
    // Filter predicate applied to each sibling (and self) to decide
    // whether it counts toward the index.
    let sib_matches = |sib: &E| {
        if of_type {
            sib.local_name().eq_ignore_ascii_case(&my_name)
        } else if let Some(s) = of_s {
            crate::matching::matches_complex_list(s, sib)
        } else {
            true
        }
    };

    // Self must match the filter (else it is not in the filtered list).
    if !sib_matches(element) {
        return false;
    }

    // Walk the appropriate sibling direction counting matches.
    let mut position: usize = 1;
    let mut cur = if from_last {
        element.next_sibling_element()
    } else {
        element.previous_sibling_element()
    };
    while let Some(sib) = cur {
        if sib_matches(&sib) {
            position += 1;
        }
        cur = if from_last {
            sib.next_sibling_element()
        } else {
            sib.previous_sibling_element()
        };
    }

    an_plus_b_matches(anb.a, anb.b, position as i64)
}

/// §13.5: An+B math. Returns true if `index = A*k + B` for some
/// non-negative integer `k`. `index` is 1-based per §13.3 L3982.
fn an_plus_b_matches(a: i64, b: i64, index: i64) -> bool {
    if a == 0 {
        return index == b;
    }
    let diff = index - b;
    // diff must be divisible by `a` and the quotient k = diff/a
    // must be >= 0.
    diff % a == 0 && diff / a >= 0
}

/// §4.2/§4.4: `:is(args)` / `:where(args)` match if any complex
/// selector in args matches the element. (Specificity differs, but
/// matching is identical.)
///
/// Placeholder body — Task 6 implements the real logic.
fn matches_is_where<E: Element>(_pc: &PseudoClass, _element: &E) -> bool {
    false
}

/// §4.5 L1650-1804: `:has(args)` matches if any relative selector
/// in args matches some element related to `element` (descendant or
/// sibling, depending on the relative selector's leading combinator).
///
/// Placeholder body — Task 6 implements the real logic.
fn matches_has<E: Element>(_pc: &PseudoClass, _element: &E) -> bool {
    false
}

/// Whether `arg` is one of the An+B pseudo-class argument shapes.
/// Helper for sanity-checking pseudo-class argument kind.
#[allow(dead_code)]
fn is_an_plus_b_arg(arg: &PseudoClassArgument) -> bool {
    matches!(arg, PseudoClassArgument::AnPlusB(_, _))
}
