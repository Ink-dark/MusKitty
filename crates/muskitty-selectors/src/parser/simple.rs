//! Simple-selector parsing (type / universal / class / id / ns-prefix /
//! attribute / pseudo-class / pseudo-element).
//!
//! Implements the §3 grammar productions for the basic building blocks
//! of a compound selector. Per §3 L4679-4699:
//!
//! ```text
//! <wq-name>            = <ns-prefix>? <ident-token>
//! <ns-prefix>          = [ <ident-token> | '*' ]? '|'
//! <type-selector>      = <wq-name> | <ns-prefix>? '*'
//! <id-selector>        = <hash-token>            (value must be an identifier)
//! <class-selector>     = '.' <ident-token>
//! <attribute-selector> = '[' <wq-name> ']' |
//!     '[' <wq-name> <attr-matcher> [ <string-token> | <ident-token> ] <attr-modifier>? ']'
//! <attr-matcher>       = [ '~' | '|' | '^' | '$' | '*' ]? '='
//! <attr-modifier>      = i | s
//! ```
//!
//! Plus the pseudo-class / pseudo-element productions referenced by
//! §3 L4684 (`<subclass-selector> = ... | <pseudo-class>`) and §3
//! L4671 (`<compound-selector> = ... <pseudo-compound-selector>*`):
//!
//! ```text
//! <pseudo-class>       = ':' <ident-token> |
//!                        ':' <function-token> <declaration-value>? ')'
//! <pseudo-element>     = '::' <ident-token> [ '(' ... ')' ]?     (modern)
//!                       | ':'  <ident-token>                      (legacy)
//! ```
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §5 L1805-1995
//! (elemental selectors + namespaces), §6.5 L2376-2462 (class), §6.6
//! L2463-2533 (id), §6.1 L2023-2135 + §6.2 L2137-2162 + §6.3
//! L2193-2264 + §6.4 L2266-2313 (attribute), §13 L3792-4359
//! (tree-structural pseudo-classes), §5.4 L1956-1995 (:defined), §8
//! L2817-3007 (:scope), §14 (pseudo-elements), §3 L4679-4699 (grammar).

use crate::error::SelectorParseError;
use crate::parser::an_plus_b::parse_an_plus_b;
use crate::parser::list::{parse_forgiving_selector_list, parse_selector_list};
use crate::parser::relative::parse_relative_selector_list;
use crate::types::{
    AttrMatcher, AttrModifier, AttrValue, AttributeSelector, ClassSelector, IdSelector, NsPrefix,
    NsPrefixKind, PseudoClass, PseudoClassArgument, PseudoElement, SelectorList, TypeSelector,
    TypeSelectorName, WqName,
};
use muskitty_css::parser::TokenStream;
use muskitty_css::tokenizer::{HashType, Token};

/// §3 L4680: Parse an optional `<ns-prefix>` = `[ <ident-token> | '*' ]? '|'`.
///
/// Returns:
/// - `Ok(Some(NsPrefix))` — an ns-prefix was found and consumed.
/// - `Ok(None)` — no ns-prefix here (the next token is not `|`, or not
///   `ident`/`*` followed by `|`). The stream is left unmodified.
///
/// Whitespace is forbidden between the components of an ns-prefix (§3
/// L4715-4719); this parser does not skip whitespace.
///
/// # Disambiguation with `|=` (dash-match)
///
/// Per §3 L4693, `<attr-matcher> = [ '~' | '|' | '^' | '$' | '*' ]? '='`.
/// The `|` in `<attr-matcher>` is the same code point as the namespace
/// separator in `<ns-prefix>`. To disambiguate `ident|` followed by `=`
/// (a dash-match attr-matcher, e.g. `[lang|=en]`) from `ident|name` (an
/// ns-prefix, e.g. `[svg|href]`), this function peeks one token past the
/// `|`: if that token is `=`, the `|` is treated as the start of an
/// attr-matcher and the ns-prefix parse is abandoned (stream restored).
pub fn parse_ns_prefix(stream: &mut TokenStream) -> Result<Option<NsPrefix>, SelectorParseError> {
    // Look ahead at most two tokens without committing: we need either
    // `<ident>|` / `*|` (two-token form) or just `|` (one-token form).
    stream.mark();
    let first = stream.consume_token();
    match first {
        Token::Ident(name) => {
            if matches!(stream.next_token(), Token::Delim('|')) {
                stream.discard_token(); // consume '|'
                                        // Disambiguate `ident|=` (dash-match) from `ident|name`
                                        // (ns-prefix): if the token after `|` is `=`, this is
                                        // NOT an ns-prefix.
                if matches!(stream.next_token(), Token::Delim('=')) {
                    stream.restore_mark();
                    return Ok(None);
                }
                stream.discard_mark();
                Ok(Some(NsPrefix {
                    prefix: NsPrefixKind::Named(name),
                }))
            } else {
                stream.restore_mark();
                Ok(None)
            }
        }
        Token::Delim('*') => {
            if matches!(stream.next_token(), Token::Delim('|')) {
                stream.discard_token(); // consume '|'
                                        // Same disambiguation as above for `*|=`.
                if matches!(stream.next_token(), Token::Delim('=')) {
                    stream.restore_mark();
                    return Ok(None);
                }
                stream.discard_mark();
                Ok(Some(NsPrefix {
                    prefix: NsPrefixKind::Any,
                }))
            } else {
                stream.restore_mark();
                Ok(None)
            }
        }
        Token::Delim('|') => {
            // `|` as the first token: ns-prefix None (empty prefix).
            // But `|=` is a dash-match attr-matcher; do not consume `|`
            // in that case — the caller will produce a proper error
            // (no wq-name precedes the matcher).
            if matches!(stream.next_token(), Token::Delim('=')) {
                stream.restore_mark();
                return Ok(None);
            }
            stream.discard_mark();
            Ok(Some(NsPrefix {
                prefix: NsPrefixKind::None,
            }))
        }
        _ => {
            stream.restore_mark();
            Ok(None)
        }
    }
}

/// §3 L4682: Parse an optional `<type-selector>` =
/// `<wq-name> | <ns-prefix>? '*'`.
///
/// Returns:
/// - `Ok(Some(TypeSelector))` — a type selector was found and
///   consumed. The `name` is [`TypeSelectorName::Name`] for a tag name
///   or [`TypeSelectorName::Universal`] for `*`.
/// - `Ok(None)` — the next token does not start a type selector; the
///   stream is left unmodified.
pub fn parse_type_selector(
    stream: &mut TokenStream,
) -> Result<Option<TypeSelector>, SelectorParseError> {
    // First attempt: ns-prefix? (ident or `*`).
    let ns_prefix = parse_ns_prefix(stream)?;

    match stream.next_token() {
        Token::Delim('*') => {
            stream.discard_token(); // consume '*'
            Ok(Some(TypeSelector {
                ns_prefix,
                name: TypeSelectorName::Universal,
            }))
        }
        Token::Ident(name) => {
            stream.discard_token(); // consume ident
            Ok(Some(TypeSelector {
                ns_prefix,
                name: TypeSelectorName::Name(name),
            }))
        }
        _ => {
            // We may have consumed an ns-prefix but found no name or `*`
            // following it (e.g. `svg|>`). That's a malformed type
            // selector. Restore to before the ns-prefix and report None
            // so the caller can decide whether to treat this as an
            // error or as "no type selector here".
            //
            // Implementation note: parse_ns_prefix already consumed
            // the prefix if it returned Some. We can't easily restore
            // here without re-marking before parse_ns_prefix was
            // called. The caller (parse_compound_selector) wraps the
            // whole attempt in its own mark/restore pair so that on
            // failure the entire attempted type selector is rolled
            // back. For now, if ns_prefix was Some but the next token
            // is not a name/`*`, report an explicit error so the
            // caller doesn't silently misinterpret the input.
            if ns_prefix.is_some() {
                return Err(SelectorParseError::InvalidSelector(
                    "namespace prefix not followed by tag name or '*'".into(),
                ));
            }
            Ok(None)
        }
    }
}

/// §6.5 L2376-2462 + §3 L4689: Parse an optional `<class-selector>` =
/// `'.' <ident-token>`.
///
/// Returns:
/// - `Ok(Some(ClassSelector))` — a class selector was found and
///   consumed.
/// - `Ok(None)` — the next token is not `.`; the stream is left
///   unmodified.
///
/// Whitespace is forbidden between `.` and the ident (§3 L4715-4719).
pub fn parse_class_selector(
    stream: &mut TokenStream,
) -> Result<Option<ClassSelector>, SelectorParseError> {
    stream.mark();
    if matches!(stream.consume_token(), Token::Delim('.')) {
        match stream.consume_token() {
            Token::Ident(name) => {
                stream.discard_mark();
                Ok(Some(ClassSelector { class: name }))
            }
            other => Err(SelectorParseError::UnexpectedToken(format!(
                "expected ident after '.', got {:?}",
                other
            ))),
        }
    } else {
        stream.restore_mark();
        Ok(None)
    }
}

/// §6.6 L2463-2533 + §3 L4687 + L4729: Parse an optional `<id-selector>`
/// = `<hash-token>` whose value is an identifier.
///
/// Returns:
/// - `Ok(Some(IdSelector))` — a valid id selector (HashType::Id) was
///   found and consumed.
/// - `Ok(None)` — the next token is not a hash-token; the stream is
///   left unmodified.
/// - `Err(InvalidSelector)` — the next token is a hash-token but its
///   type is `Unrestricted` (i.e. the value is not an identifier,
///   per §3 L4729). The token is left unconsumed.
pub fn parse_id_selector(
    stream: &mut TokenStream,
) -> Result<Option<IdSelector>, SelectorParseError> {
    match stream.next_token() {
        Token::Hash(value, hash_type) => {
            if matches!(hash_type, HashType::Id) {
                stream.discard_token(); // consume the hash-token
                Ok(Some(IdSelector { id: value }))
            } else {
                // §3 L4729: "In <id-selector>, the hash-token's value
                // must be an identifier." HashType::Unrestricted means
                // the value would not start an ident sequence; reject.
                Err(SelectorParseError::InvalidSelector(format!(
                    "hash-token value {:?} is not an identifier",
                    value
                )))
            }
        }
        _ => Ok(None),
    }
}

/// §3 L4679: Parse a `<wq-name>` = `<ns-prefix>? <ident-token>`.
///
/// Used by attribute selectors (§6) for the attribute name. Unlike
/// `<type-selector>`, the local name must be an `<ident-token>` — the
/// universal selector `*` is not allowed as a wq-name local name.
///
/// Returns `Ok(WqName)` on success, or `Err(UnexpectedToken)` if no
/// ident-token follows the optional ns-prefix.
///
/// # Disambiguation
///
/// Because `<ns-prefix>` and `<attr-matcher>` both begin with `|`
/// (§3 L4691-4694), `parse_ns_prefix` already disambiguates `ident|=`
/// (dash-match) from `ident|name` (ns-prefix). See its docs.
pub fn parse_wq_name(stream: &mut TokenStream) -> Result<WqName, SelectorParseError> {
    let ns_prefix = parse_ns_prefix(stream)?;
    match stream.consume_token() {
        Token::Ident(local) => Ok(WqName {
            ns_prefix,
            local_name: local,
        }),
        other => Err(SelectorParseError::UnexpectedToken(format!(
            "expected <ident-token> for wq-name local name, got {:?}",
            other
        ))),
    }
}

/// §6 L1996-2533: Parse an optional `<attribute-selector>`.
///
/// Implements §3 L4691-4694 grammar:
///
/// ```text
/// <attribute-selector> = '[' <wq-name> ']' |
///     '[' <wq-name> <attr-matcher> [ <string-token> | <ident-token> ] <attr-modifier>? ']'
/// <attr-matcher>       = [ '~' | '|' | '^' | '$' | '*' ]? '='
/// <attr-modifier>      = i | s
/// ```
///
/// Returns:
/// - `Ok(Some(AttributeSelector))` — a valid attribute selector was
///   found and consumed (including the closing `]`).
/// - `Ok(None)` — the next token is not `[`; the stream is left
///   unmodified.
/// - `Err(...)` — the input starts with `[` but is malformed (missing
///   name, missing value, unclosed block, etc.).
///
/// # Whitespace rules
///
/// Per §3 L4709-4720:
/// - Whitespace is forbidden between components of a `<wq-name>` and
///   between the prefix char and `=` of an `<attr-matcher>`.
/// - Whitespace is permitted (and discarded) between `<wq-name>` and
///   `<attr-matcher>`, between `<attr-matcher>` and the value, between
///   the value and `<attr-modifier>`, and between `<attr-modifier>` and
///   `]`.
///
/// # Case-sensitivity of `attr-modifier`
///
/// Per §6.3 L2227-2229, the `i` / `s` identifiers themselves are
/// ASCII case-insensitive; we accept `I` / `S` as well.
pub fn parse_attribute_selector(
    stream: &mut TokenStream,
) -> Result<Option<AttributeSelector>, SelectorParseError> {
    // §3 L4691: must start with `[` (OpenBracket).
    if !matches!(stream.next_token(), Token::OpenBracket) {
        return Ok(None);
    }
    stream.discard_token(); // consume `[`

    stream.discard_whitespace();
    // §6.4 L2266-2313: wq-name may carry an ns-prefix.
    let name = parse_wq_name(stream)?;
    stream.discard_whitespace();

    // §6.1 L2023-2135: presence selector `[attr]` (matcher == None).
    if matches!(stream.next_token(), Token::CloseBracket) {
        stream.discard_token(); // consume `]`
        return Ok(Some(AttributeSelector {
            name,
            matcher: None,
            value: None,
            modifier: None,
        }));
    }

    // §3 L4693: <attr-matcher> = [ '~' | '|' | '^' | '$' | '*' ]? '='.
    // Whitespace is forbidden between the prefix char and `=` (§3 L4720).
    let matcher = parse_attr_matcher(stream)?;
    stream.discard_whitespace();

    // §6.1 L2061 / §6.2 L2165: value must be a string-token or ident-token.
    let value = match stream.consume_token() {
        Token::String(s) => AttrValue::String(s),
        Token::Ident(s) => AttrValue::Ident(s),
        other => {
            return Err(SelectorParseError::UnexpectedToken(format!(
                "expected <string-token> or <ident-token> for attribute value, got {:?}",
                other
            )))
        }
    };
    stream.discard_whitespace();

    // §6.3 L2193-2264: optional attr-modifier `i` or `s`
    // (ASCII case-insensitive per L2227-2229).
    let modifier = match stream.next_token() {
        Token::Ident(ref m) if m.eq_ignore_ascii_case("i") => {
            stream.discard_token();
            Some(AttrModifier::CaseInsensitive)
        }
        Token::Ident(ref m) if m.eq_ignore_ascii_case("s") => {
            stream.discard_token();
            Some(AttrModifier::CaseSensitive)
        }
        _ => None,
    };
    stream.discard_whitespace();

    // §3 L4691-4693: must end with `]`.
    if !matches!(stream.consume_token(), Token::CloseBracket) {
        return Err(SelectorParseError::UnclosedBlock);
    }

    Ok(Some(AttributeSelector {
        name,
        matcher: Some(matcher),
        value: Some(value),
        modifier,
    }))
}

/// §3 L4693: Parse an `<attr-matcher>` = `[ '~' | '|' | '^' | '$' | '*' ]? '='`.
///
/// Pre-condition: the next token is one of `~`, `|`, `^`, `$`, `*`, or
/// `=`. Whitespace is forbidden between the prefix char and `=` (§3
/// L4720); this function does not skip whitespace between them.
///
/// Returns the matched [`AttrMatcher`] variant, or
/// `Err(UnexpectedToken)` if the input does not form a valid
/// `<attr-matcher>`.
fn parse_attr_matcher(stream: &mut TokenStream) -> Result<AttrMatcher, SelectorParseError> {
    match stream.consume_token() {
        // `[attr=value]` — §6.1 L2037-2054 exact match.
        Token::Delim('=') => Ok(AttrMatcher::Exact),
        // `[attr~=value]`, `[attr|=value]`, etc. — the prefix char must
        // be immediately followed by `=`.
        Token::Delim(prefix) => {
            let matcher = match prefix {
                '~' => AttrMatcher::Includes,
                '|' => AttrMatcher::DashMatch,
                '^' => AttrMatcher::Prefix,
                '$' => AttrMatcher::Suffix,
                '*' => AttrMatcher::Substring,
                other => {
                    return Err(SelectorParseError::UnexpectedToken(format!(
                        "expected '~', '|', '^', '$', or '*' before '=', got '{}'",
                        other
                    )))
                }
            };
            match stream.consume_token() {
                Token::Delim('=') => Ok(matcher),
                other => Err(SelectorParseError::UnexpectedToken(format!(
                    "expected '=' after attr-matcher prefix '{}', got {:?}",
                    prefix, other
                ))),
            }
        }
        other => Err(SelectorParseError::UnexpectedToken(format!(
            "expected <attr-matcher>, got {:?}",
            other
        ))),
    }
}

// ── Pseudo-class / pseudo-element parsing (SP-4) ───────────────────

/// §3 L1245-1306: Known pseudo-class names recognised by this parser.
///
/// Pseudo-class names are matched ASCII case-insensitively. Unknown
/// names produce [`SelectorParseError::UnknownPseudoClass`].
///
/// SP-4 scope: tree-structural (§13), :defined (§5.4), :scope (§8),
/// plus the user-action / resource / display / input / location
/// pseudo-classes whose matching is stubbed until SP-8.
const KNOWN_PSEUDO_CLASSES: &[&str] = &[
    // §13 tree-structural
    "root",
    "empty",
    "first-child",
    "last-child",
    "only-child",
    "first-of-type",
    "last-of-type",
    "only-of-type",
    "nth-child",
    "nth-last-child",
    "nth-of-type",
    "nth-last-of-type",
    // §5.4
    "defined",
    // §8
    "scope",
    // §9 user-action states (parse-only; matching stubbed)
    "hover",
    "active",
    "focus",
    "focus-visible",
    "focus-within",
    "playing",
    "paused",
    "seeking",
    "buffering",
    "stalled",
    "muted",
    "volume-locked",
    // §10 UI element states (parse-only; matching stubbed)
    "enabled",
    "disabled",
    "read-only",
    "read-write",
    "placeholder-shown",
    "default",
    "checked",
    "indeterminate",
    "valid",
    "invalid",
    "in-range",
    "out-of-range",
    "required",
    "optional",
    "blank",
    // §8 location pseudo-classes (parse-only; matching stubbed)
    "any-link",
    "link",
    "visited",
    "local-link",
    "target",
    "target-within",
    "current",
    "past",
    "future",
    "host",
    "host-context",
    // §4 logical combinations (parameterised; argument dispatch in
    // parse_pseudo_class_argument).
    "is",
    "not",
    "where",
    "has",
];

/// §14 L4476-4535: Pseudo-elements that accept the legacy single-colon
/// form for backwards compatibility. When the parser encounters
/// `:name` where `name` is in this list, it produces a
/// [`PseudoElement`] with `legacy = true` instead of treating it as a
/// pseudo-class.
const LEGACY_PSEUDO_ELEMENTS: &[&str] = &["before", "after", "first-line", "first-letter"];

/// §14: Known pseudo-element names. Unknown names produce
/// [`SelectorParseError::UnknownPseudoElement`].
fn is_known_pseudo_element(name: &str) -> bool {
    matches!(
        name,
        "before"
            | "after"
            | "first-line"
            | "first-letter"
            | "selection"
            | "placeholder"
            | "marker"
            | "backdrop"
            | "file-selector-button"
            | "spelling-error"
            | "grammar-error"
            | "target-text"
            | "view-transition"
            | "view-transition-group"
            | "view-transition-image-pair"
            | "view-transition-old"
            | "view-transition-new"
            | "cue"
            | "region"
    )
}

/// Outcome of attempting to parse a single-colon pseudo-construct.
///
/// `:` followed by an ident-token may be either a regular pseudo-class
/// or a legacy pseudo-element (one of `:before`, `:after`,
/// `:first-line`, `:first-letter`). The caller ([compound.rs]) uses
/// the discriminant to decide whether to push the result into
/// `subclasses` or `pseudo_compounds`.
#[derive(Debug, Clone, PartialEq)]
pub enum PseudoClassOrLegacy {
    /// The next token is not `:` — no pseudo-construct here.
    None,
    /// A regular pseudo-class (`:name` or `:name(args)`).
    Class(PseudoClass),
    /// A legacy pseudo-element written with a single colon (`:before`,
    /// `:after`, `:first-line`, `:first-letter`). The subclass loop
    /// terminates after this — pseudo-elements cannot be followed by
    /// further subclasses.
    LegacyElement(PseudoElement),
}

/// §3 L4684 + §13 + §14: Parse an optional pseudo-class or legacy
/// pseudo-element starting at the current position.
///
/// Handles the single-colon form (`:name` or `:name(args)`). The
/// double-colon form (`::name`) is handled by
/// [`parse_pseudo_element`]; this function returns
/// [`PseudoClassOrLegacy::None`] when it sees `::` so the caller can
/// try the modern-form parser.
///
/// # Returns
///
/// - `Ok(PseudoClassOrLegacy::None)` — the next token is not `:`, or
///   it is `::` (modern pseudo-element). The stream is left
///   unmodified.
/// - `Ok(PseudoClassOrLegacy::Class(pc))` — a regular pseudo-class was
///   parsed and consumed.
/// - `Ok(PseudoClassOrLegacy::LegacyElement(pe))` — a legacy
///   pseudo-element (`:before` / `:after` / `:first-line` /
///   `:first-letter`) was parsed and consumed.
/// - `Err(UnknownPseudoClass)` — `:` is followed by an ident-token
///   whose name is neither a known pseudo-class nor a legacy
///   pseudo-element.
/// - `Err(UnclosedBlock)` — `:name(` is not closed with `)`.
/// - `Err(InvalidAnPlusB)` — `:nth-child(...)` etc. has a malformed
///   An+B argument.
/// - `Err(UnexpectedToken)` — `:` is followed by something other than
///   an ident-token or function-token.
pub fn parse_pseudo_class_or_legacy(
    stream: &mut TokenStream,
) -> Result<PseudoClassOrLegacy, SelectorParseError> {
    // Must start with `:`.
    if !matches!(stream.next_token(), Token::Colon) {
        return Ok(PseudoClassOrLegacy::None);
    }

    // Use mark/restore so that on any failure path we rewind to before
    // the `:` was consumed.
    stream.mark();
    stream.discard_token(); // consume `:`

    // `::` indicates a modern pseudo-element; defer to
    // parse_pseudo_element. Restore so that function sees `::` from
    // the start.
    if matches!(stream.next_token(), Token::Colon) {
        stream.restore_mark();
        return Ok(PseudoClassOrLegacy::None);
    }

    let result = match stream.consume_token() {
        // `:ident` — simple value-less pseudo-class (or legacy
        // pseudo-element).
        Token::Ident(name) => {
            let lower = name.to_ascii_lowercase();
            // Legacy pseudo-element check takes precedence: `:before`
            // etc. must be redirected to the pseudo-element path.
            if LEGACY_PSEUDO_ELEMENTS.iter().any(|&p| p == lower) {
                stream.discard_mark();
                return Ok(PseudoClassOrLegacy::LegacyElement(PseudoElement {
                    name: lower,
                    legacy: true,
                }));
            }
            // Validate against the known pseudo-class whitelist.
            if !KNOWN_PSEUDO_CLASSES.iter().any(|&p| p == lower) {
                stream.restore_mark();
                return Err(SelectorParseError::UnknownPseudoClass(name));
            }
            PseudoClass {
                name: lower,
                argument: None,
            }
        }
        // `:function-token ... )` — parameterised pseudo-class.
        // The `(` was already consumed by the tokenizer when forming
        // the Function token.
        Token::Function(name) => {
            let lower = name.to_ascii_lowercase();
            // Validate against the known pseudo-class whitelist.
            if !KNOWN_PSEUDO_CLASSES.iter().any(|&p| p == lower) {
                stream.restore_mark();
                return Err(SelectorParseError::UnknownPseudoClass(name));
            }
            // Legacy pseudo-elements don't take arguments: `:before(...)`
            // is invalid.
            if LEGACY_PSEUDO_ELEMENTS.iter().any(|&p| p == lower) {
                stream.restore_mark();
                return Err(SelectorParseError::UnknownPseudoClass(name));
            }
            let argument = parse_pseudo_class_argument(stream, &lower)?;
            // Expect closing `)`.
            match stream.consume_token() {
                Token::CloseParen => {}
                other => {
                    stream.restore_mark();
                    return Err(SelectorParseError::UnexpectedToken(format!(
                        "expected ')' to close :{}(...), got {:?}",
                        lower, other
                    )));
                }
            }
            PseudoClass {
                name: lower,
                argument: Some(argument),
            }
        }
        other => {
            stream.restore_mark();
            return Err(SelectorParseError::UnexpectedToken(format!(
                "expected pseudo-class name after ':', got {:?}",
                other
            )));
        }
    };

    stream.discard_mark();
    Ok(PseudoClassOrLegacy::Class(result))
}

/// Parse the argument of a parameterised pseudo-class.
///
/// Dispatches based on the pseudo-class name:
/// - `nth-child`, `nth-last-child`, `nth-of-type`, `nth-last-of-type`
///   → parse an `<an-plus-b>` (§13.5).
/// - `is`, `where` → parse a `<forgiving-selector-list>` (§4.2/§4.4
///   L1497-1499 + L1617, §3 L4765-4813). Failed selectors are
///   silently dropped.
/// - `not` → parse a `<complex-selector-list>` (§4.3 L1564-1607).
///   Non-forgiving: any invalid selector fails the whole pseudo-class.
///   Level 4 allows complex selectors as the argument (Level 3 only
///   permitted simple selectors).
/// - `has` → parse a `<relative-selector-list>` (§4.5 L1700).
///   Non-forgiving. Each relative selector may begin with an optional
///   combinator (default descendant) and is anchored against an
///   implicit `:scope`.
/// - All other known pseudo-classes → preserve the raw token stream
///   until the closing `)`.
///
/// The closing `)` is left unconsumed for the caller
/// ([`parse_pseudo_class_or_legacy`]) to verify.
fn parse_pseudo_class_argument(
    stream: &mut TokenStream,
    name: &str,
) -> Result<PseudoClassArgument, SelectorParseError> {
    match name {
        "nth-child" | "nth-last-child" => {
            // §13.3 L3968 / §13.4 L4077: `An+B [of S]?`. Only nth-child and
            // nth-last-child accept the `of S` clause.
            let an_plus_b = parse_an_plus_b(stream)?;
            let of_s = parse_optional_of_selector_list(stream)?;
            Ok(PseudoClassArgument::AnPlusB(an_plus_b, of_s))
        }
        "nth-of-type" | "nth-last-of-type" => {
            // §13.6 / §13.7: no `of S` syntax. If the user wrote `of`, it's
            // an error — the trailing tokens will be caught by the closing
            // `)` verification step in `parse_pseudo_class_or_legacy`.
            let an_plus_b = parse_an_plus_b(stream)?;
            Ok(PseudoClassArgument::AnPlusB(an_plus_b, None))
        }
        // §4.2 L1497-1499 + §4.4 L1617: forgiving-selector-list. Each
        // complex selector parsed independently; failures silently
        // dropped (§3 L4765-4813).
        "is" | "where" => {
            let list = parse_forgiving_selector_list(stream)?;
            Ok(PseudoClassArgument::SelectorList(list))
        }
        // §4.3 L1564-1607: complex-selector-list, non-forgiving.
        // Level 4 permits complex selectors as the argument.
        "not" => {
            let list = parse_selector_list(stream)?;
            Ok(PseudoClassArgument::SelectorList(list))
        }
        // §4.5 L1700: relative-selector-list, non-forgiving. Each
        // relative selector may begin with an optional combinator
        // (default descendant) and is anchored against an implicit
        // :scope.
        "has" => {
            let list = parse_relative_selector_list(stream)?;
            Ok(PseudoClassArgument::SelectorList(list))
        }
        _ => {
            // Other parameterised pseudo-classes: capture raw
            // component values until the closing `)`. The closing `)`
            // is left unconsumed for the caller
            // ([`parse_pseudo_class_or_legacy`]) to verify.
            let mut tokens = Vec::new();
            loop {
                match stream.next_token() {
                    Token::CloseParen => break,
                    Token::Eof => return Err(SelectorParseError::UnclosedBlock),
                    t => {
                        tokens.push(t);
                        stream.discard_token();
                    }
                }
            }
            Ok(PseudoClassArgument::Raw(tokens))
        }
    }
}

/// §13.3 L3968 / §13.4 L4077: parse the optional `of S` clause of
/// `:nth-child(An+B of S)` / `:nth-last-child(An+B of S)`.
///
/// Pre-condition: An+B has already been consumed; the stream cursor
/// sits just past the B-part of An+B.
///
/// Returns:
/// - `Ok(Some(SelectorList))` — `of S` clause present and parsed.
/// - `Ok(None)` — no `of` keyword; the clause was omitted.
///
/// The closing `)` is left unconsumed for the caller to verify (same
/// convention as other pseudo-class argument parsers).
fn parse_optional_of_selector_list(
    stream: &mut TokenStream,
) -> Result<Option<SelectorList>, SelectorParseError> {
    stream.discard_whitespace();
    match stream.next_token() {
        // No `of` keyword — clause is absent. Leave the stream
        // positioned at the closing `)` (or whatever terminator
        // follows) for the caller.
        Token::CloseParen | Token::Eof => Ok(None),
        // `of` keyword (case-insensitive per CSS ident folding).
        Token::Ident(ref s) if s.eq_ignore_ascii_case("of") => {
            stream.discard_token();
            // §13.3 L3968: S is a <selector-list> (non-forgiving).
            // Reuse parse_selector_list from list.rs.
            let list = crate::parser::list::parse_selector_list(stream)?;
            Ok(Some(list))
        }
        // Anything else after An+B is a structural error.
        _ => Err(SelectorParseError::InvalidSelector(
            "expected `of` or `)` after An+B in :nth-child/:nth-last-child argument".to_string(),
        )),
    }
}

/// §3 L4671 + §14: Parse an optional modern pseudo-element (`::name`).
///
/// Handles the double-colon form. The legacy single-colon form
/// (`:before` etc.) is handled by [`parse_pseudo_class_or_legacy`].
///
/// # Returns
///
/// - `Ok(Some(PseudoElement))` — a modern pseudo-element was parsed
///   and consumed. The returned `PseudoElement` has `legacy = false`.
/// - `Ok(None)` — the next token is not `::`; the stream is left
///   unmodified.
/// - `Err(UnknownPseudoElement)` — `::` is followed by an ident-token
///   whose name is not a known pseudo-element.
/// - `Err(UnexpectedToken)` — `::` is followed by something other
///   than an ident-token.
pub fn parse_pseudo_element(
    stream: &mut TokenStream,
) -> Result<Option<PseudoElement>, SelectorParseError> {
    // Must start with `:` (the first of two).
    if !matches!(stream.next_token(), Token::Colon) {
        return Ok(None);
    }
    stream.mark();
    stream.discard_token(); // consume first `:`

    // Must be followed by another `:`.
    if !matches!(stream.next_token(), Token::Colon) {
        stream.restore_mark();
        return Ok(None);
    }
    stream.discard_token(); // consume second `:`

    match stream.consume_token() {
        Token::Ident(name) => {
            let lower = name.to_ascii_lowercase();
            if !is_known_pseudo_element(&lower) {
                stream.restore_mark();
                return Err(SelectorParseError::UnknownPseudoElement(name));
            }
            stream.discard_mark();
            Ok(Some(PseudoElement {
                name: lower,
                legacy: false,
            }))
        }
        other => {
            stream.restore_mark();
            Err(SelectorParseError::UnexpectedToken(format!(
                "expected pseudo-element name after '::', got {:?}",
                other
            )))
        }
    }
}
