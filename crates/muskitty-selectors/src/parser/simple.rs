//! Simple-selector parsing (type / universal / class / id / ns-prefix).
//!
//! Implements the §3 grammar productions for the basic building blocks
//! of a compound selector. Per §3 L4679-4689:
//!
//! ```text
//! <wq-name>        = <ns-prefix>? <ident-token>
//! <ns-prefix>      = [ <ident-token> | '*' ]? '|'
//! <type-selector>  = <wq-name> | <ns-prefix>? '*'
//! <id-selector>    = <hash-token>            (value must be an identifier)
//! <class-selector> = '.' <ident-token>
//! ```
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §5 L1805-1995
//! (elemental selectors + namespaces), §6.5 L2376-2462 (class), §6.6
//! L2463-2533 (id), §3 L4679-4689 (grammar).

use crate::error::SelectorParseError;
use crate::types::{
    ClassSelector, IdSelector, NsPrefix, NsPrefixKind, TypeSelector, TypeSelectorName,
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
pub fn parse_ns_prefix(stream: &mut TokenStream) -> Result<Option<NsPrefix>, SelectorParseError> {
    // Look ahead at most two tokens without committing: we need either
    // `<ident>|` / `*|` (two-token form) or just `|` (one-token form).
    stream.mark();
    let first = stream.consume_token();
    match first {
        Token::Ident(name) => {
            if matches!(stream.next_token(), Token::Delim('|')) {
                stream.discard_token(); // consume '|'
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
