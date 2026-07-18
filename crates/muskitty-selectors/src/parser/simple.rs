//! Simple-selector parsing (type / universal / class / id / ns-prefix /
//! attribute).
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
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §5 L1805-1995
//! (elemental selectors + namespaces), §6.5 L2376-2462 (class), §6.6
//! L2463-2533 (id), §6.1 L2023-2135 + §6.2 L2137-2162 + §6.3
//! L2193-2264 + §6.4 L2266-2313 (attribute), §3 L4679-4699 (grammar).

use crate::error::SelectorParseError;
use crate::types::{
    AttrMatcher, AttrModifier, AttrValue, AttributeSelector, ClassSelector, IdSelector, NsPrefix,
    NsPrefixKind, TypeSelector, TypeSelectorName, WqName,
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
