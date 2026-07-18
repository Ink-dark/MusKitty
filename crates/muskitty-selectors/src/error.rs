//! Selector parse errors.
//!
//! Errors returned by the selector parser. The [`SelectorParseError`]
//! enum lists the distinct failure modes encountered while parsing
//! Selectors Level 4 syntax. It carries enough information for callers
//! (CSSOM, developer tooling) to report a meaningful diagnostic; the
//! parser itself does not produce structured source-range data in
//! this iteration.

/// A selector parse error.
///
/// Variants correspond to the distinct failure modes recognised by the
/// Selectors Level 4 §3.7 "Invalid Selectors and Error Handling"
/// rules plus parser-internal states (e.g. [`Self::NotImplemented`]
/// for entry points whose full algorithm lands in a later SP batch).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectorParseError {
    /// Parser entry point exists but its body is not yet implemented.
    /// Used by SP-1 skeletons; replaced by real algorithms in later
    /// batches.
    NotImplemented,
    /// An unexpected token was encountered. The string carries a
    /// short human-readable description (token kind + value).
    UnexpectedToken(String),
    /// The selector as a whole is structurally invalid (e.g. trailing
    /// combinator, empty input, unclosed block). The string carries
    /// a description.
    InvalidSelector(String),
    /// An `[attr=...]` block was not closed with `]`.
    UnclosedBlock,
    /// The `An+B` argument to `:nth-child()` / `:nth-of-type()` /
    /// related pseudo-classes does not match the §13.5 An+B grammar.
    InvalidAnPlusB,
    /// An unknown pseudo-class name (e.g. `:foobar`). Carries the
    /// offending name.
    UnknownPseudoClass(String),
    /// An unknown pseudo-element name (e.g. `::foobar`). Carries the
    /// offending name.
    UnknownPseudoElement(String),
    /// The selector list is empty (zero-length input or all
    /// whitespace).
    EmptySelector,
}
