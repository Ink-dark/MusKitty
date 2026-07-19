//! MusKitty Selectors — Selectors Level 4 parser & matcher.
//!
//! Implements the Selectors Level 4 specification (selector parsing,
//! specificity calculation, and element matching) for the MusKitty
//! browser engine.
//!
//! # Architecture
//!
//! - **Parsing** ([`parser`], [`types`]) — consumes a token stream
//!   produced by `muskitty-css::tokenize` and builds selector data
//!   structures (SelectorList / ComplexSelector / CompoundSelector /
//!   SubclassSelector / ...). No DOM dependency.
//! - **Specificity** ([`specificity`]) — computes the A/B/C triplet per
//!   §17.
//! - **Matching** ([`matching`]) — matches parsed selectors against an
//!   element tree via the [`matching::Element`] trait. A reference
//!   implementation for `muskitty-dom` is provided as a dev-only
//!   integration.
//!
//! # References
//!
//! - Selectors Level 4: <https://drafts.csswg.org/selectors-4/>
//! - Spec source (Markdown): `D:\CSSWG\selectors-4\Overview.md`

pub mod error;
pub mod parser;
pub mod specificity;
pub mod types;

/// Convenience re-export of the [`Specificity`](specificity::Specificity)
/// type for ergonomic access from downstream crates.
pub use specificity::Specificity;
