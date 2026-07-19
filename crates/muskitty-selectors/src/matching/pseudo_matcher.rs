//! Pseudo-class matching.
//!
//! Implements the matching rules for §13 tree-structural
//! pseudo-classes, §13.3 An+B pseudo-classes (`:nth-child` / etc.),
//! and §4 logical combinations (`:is` / `not` / `:where` / `:has`).
//!
//! Pseudo-classes outside the §13/§4 scope (UI / location /
//! linguistic / resource state / display state / input — §7-§12)
//! are parsed by SP-4 but matching returns `false` per the parent
//! SP-1..SP-8 plan.
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §4 L1358-1804,
//! §13 L3792-4359.

use crate::matching::Element;
use crate::types::PseudoClass;

/// §13/§4: match a pseudo-class against an element.
///
/// SP-8 Task 3 stub: returns `false` unconditionally. Task 4 fills
/// in the real dispatch for tree-structural pseudo-classes.
pub fn matches_pseudo_class<E: Element>(_pc: &PseudoClass, _element: &E) -> bool {
    false
}
