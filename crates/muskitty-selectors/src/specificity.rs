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

use crate::types::{ComplexSelector, CompoundSelector, PseudoClass, SelectorList, SubclassSelector};

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

// Placeholder — full implementations added in Tasks 4 and 5.
pub fn specificity_of_complex(_cs: &ComplexSelector) -> Specificity {
    Specificity::default()
}

fn _specificity_of_compound(_compound: &CompoundSelector) -> Specificity {
    Specificity::default()
}

fn _specificity_of_pseudo_class(_pc: &PseudoClass) -> Specificity {
    Specificity::default()
}

#[allow(dead_code)]
fn _classify_subclass(_s: &SubclassSelector) -> Specificity {
    Specificity::default()
}
