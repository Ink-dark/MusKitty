//! §17 Calculating a selector's specificity.
//!
//! Implements the A/B/C triplet computation per Selectors Level 4
//! §17 L4534-4633. Specificity is computed for a parsed selector
//! (independent of any element); element-matching specificity is
//! therefore identical to selector specificity (SP-8 will define the
//! matching-side contract).
//!
//! # Components
//!
//! Per §17 L4539-4542:
//! - `A` = number of ID selectors in the selector
//! - `B` = number of class selectors + attribute selectors + pseudo-classes
//! - `C` = number of type selectors + pseudo-elements
//! - The universal selector (`*`) is ignored.
//!
//! # Special pseudo-classes
//!
//! Per §17 L4550-4566:
//! - `:is()`, `:not()`, `:has()` → specificity is replaced by the
//!   max specificity of the complex selectors in the argument list.
//! - `:nth-child()`, `:nth-last-child()` → specificity is the
//!   pseudo-class itself (1×B) **plus** the max specificity of the
//!   complex selectors in the `of S` argument (if present).
//! - `:where()` → specificity is replaced by zero.
//!
//! # Comparison
//!
//! Per §17 L4598-4605: lexicographic on (A, B, C).
//!
//! # Spec source
//!
//! `D:\CSSWG\selectors-4\Overview.md`, §17 L4534-4633.

use crate::types::{
    ComplexSelector, CompoundSelector, PseudoClass, SelectorList, SubclassSelector,
};

/// §17 L4534-4548: A selector's specificity, expressed as the (A, B, C)
/// triplet. Comparison is lexicographic per L4598-4605.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Specificity {
    /// §17 L4539: count of ID selectors.
    pub a: u32,
    /// §17 L4540: count of class selectors + attribute selectors +
    /// pseudo-classes.
    pub b: u32,
    /// §17 L4541: count of type selectors + pseudo-elements.
    pub c: u32,
}

impl Specificity {
    /// Construct a new specificity triplet.
    pub const fn new(a: u32, b: u32, c: u32) -> Self {
        Self { a, b, c }
    }
}

impl Ord for Specificity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // §17 L4598-4605: lexicographic comparison on (A, B, C).
        if self.a != other.a {
            return self.a.cmp(&other.a);
        }
        if self.b != other.b {
            return self.b.cmp(&other.b);
        }
        self.c.cmp(&other.c)
    }
}

impl PartialOrd for Specificity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::ops::Add for Specificity {
    type Output = Self;
    /// Component-wise addition. Used to sum the specificities of
    /// multiple compound selectors in a complex selector.
    fn add(self, other: Self) -> Self {
        Self {
            a: self.a + other.a,
            b: self.b + other.b,
            c: self.c + other.c,
        }
    }
}

impl std::ops::AddAssign for Specificity {
    fn add_assign(&mut self, other: Self) {
        self.a += other.a;
        self.b += other.b;
        self.c += other.c;
    }
}

impl Specificity {
    /// §17 L4547-4548: for a selector list, the specificity in effect
    /// is that of the most specific selector in the list that matches.
    /// Since we have no element here (matching is SP-8), this returns
    /// the max specificity over all complex selectors in the list.
    pub fn max_of_list(list: &SelectorList) -> Self {
        list.0
            .iter()
            .map(specificity_of_complex)
            .max()
            .unwrap_or_default()
    }
}

/// §17 L4536-4548: compute the specificity of a single complex
/// selector. Sums the specificities of all compound units; the
/// combinator on each unit contributes nothing.
pub fn specificity_of_complex(cs: &ComplexSelector) -> Specificity {
    let mut s = Specificity::default();
    for unit in &cs.units {
        s += specificity_of_compound(&unit.compound);
    }
    s
}

/// §17 L4539-4542: compute the specificity of a single compound
/// selector. Walks the type selector (if any), each subclass, and
/// each pseudo-compound.
pub fn specificity_of_compound(compound: &CompoundSelector) -> Specificity {
    let mut s = Specificity::default();

    // Type selector (or universal). Universal contributes 0.
    if let Some(ref ts) = compound.type_selector {
        match &ts.name {
            crate::types::TypeSelectorName::Universal => {}
            crate::types::TypeSelectorName::Name(_) => s.c += 1,
        }
    }

    // Subclass selectors: id / class / attribute / pseudo-class.
    for sub in &compound.subclasses {
        s += classify_subclass(sub);
    }

    // Pseudo-compounds: pseudo-element (+ any trailing pseudo-classes
    // that apply to it). §3 L762-787.
    for pc in &compound.pseudo_compounds {
        // Pseudo-element itself: +1 C.
        s.c += 1;
        // Trailing pseudo-classes on this pseudo-compound.
        for trailing in &pc.trailing_pseudo_classes {
            s += specificity_of_pseudo_class(trailing);
        }
    }

    s
}

/// §17 L4539-4542: classify a subclass selector into its specificity
/// contribution. ID → (1,0,0); class/attribute/pseudo-class → (0,1,0).
fn classify_subclass(s: &SubclassSelector) -> Specificity {
    match s {
        SubclassSelector::Id(_) => Specificity::new(1, 0, 0),
        SubclassSelector::Class(_) | SubclassSelector::Attribute(_) => Specificity::new(0, 1, 0),
        // Pseudo-classes need full recursive handling for
        // :is/:not/:has/:where/:nth-child. Defer to
        // `specificity_of_pseudo_class`.
        SubclassSelector::PseudoClass(pc) => specificity_of_pseudo_class(pc),
    }
}

/// §17 L4550-4566: compute the specificity contribution of a
/// pseudo-class. The default case (a plain pseudo-class like `:hover`)
/// contributes (0,1,0). The special cases for `:is`/`:not`/`:has`/
/// `:where`/`:nth-child`/`:nth-last-child` are handled in
/// [`special_pseudo_class_specificity`] (Task 5).
fn specificity_of_pseudo_class(pc: &PseudoClass) -> Specificity {
    // §17 L4540: a pseudo-class counts as one B.
    let base = Specificity::new(0, 1, 0);
    match special_pseudo_class_specificity(pc) {
        Some(special) => special,
        None => base,
    }
}

/// §17 L4550-4566: returns `Some(s)` for the special pseudo-classes
/// (`:is`, `:not`, `:has`, `:where`, `:nth-child`, `:nth-last-child`)
/// whose specificity is replaced/extended per the spec. Returns `None`
/// for ordinary pseudo-classes (which use the default (0,1,0)).
fn special_pseudo_class_specificity(pc: &PseudoClass) -> Option<Specificity> {
    use crate::types::PseudoClassArgument;
    // §17 L4555-4558: `:is`/`:not`/`:has` — replaced by max of args.
    if matches!(pc.name.as_str(), "is" | "not" | "has") {
        return pc.argument.as_ref().and_then(|arg| match arg {
            PseudoClassArgument::SelectorList(list) => Some(Specificity::max_of_list(list)),
            _ => None,
        });
    }
    // §17 L4566: `:where` — replaced by zero.
    if pc.name == "where" {
        return Some(Specificity::default());
    }
    // §17 L4560-4564: `:nth-child` / `:nth-last-child` — pseudo-class
    // base (1×B) plus max of `of S` (if present).
    if matches!(pc.name.as_str(), "nth-child" | "nth-last-child") {
        return pc.argument.as_ref().and_then(|arg| match arg {
            PseudoClassArgument::AnPlusB(_, Some(of_s)) => {
                // Base pseudo-class + max of S.
                let base = Specificity::new(0, 1, 0);
                let max_of_s = Specificity::max_of_list(of_s);
                Some(base + max_of_s)
            }
            // Without `of S`: just the base (0,1,0). But this case is
            // already handled by the default path in
            // `specificity_of_pseudo_class`. Returning `None` here
            // lets the default path apply.
            PseudoClassArgument::AnPlusB(_, None) => None,
            _ => None,
        });
    }
    None
}
