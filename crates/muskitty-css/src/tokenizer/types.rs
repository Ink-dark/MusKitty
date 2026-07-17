//! Token types for the CSS Syntax tokenizer.
//!
//! Implements the token definitions of CSS Syntax Module Level 3 §4.1
//! (Token Railroad Diagrams) and §5.2 (CSS Parsing Results).
//!
//! Every token type enumerated in §4.1 has a corresponding variant in
//! [`Token`]. The compiler enforces exhaustive `match` arms, so no token
//! kind can be forgotten by the parser.

use std::fmt;

/// A CSS token (§4.1).
///
/// Variants correspond to the token types defined by the CSS Syntax
/// Module §4.1 railroad diagrams. Each variant's doc comment cites the
/// relevant normative section.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// `<ident-token>` (§4.1). An identifier such as `color`, `auto`,
    /// `-webkit-flex`. The value is the decoded ident sequence (escapes
    /// resolved, §4.3.12).
    Ident(String),

    /// `<function-token>` (§4.1). An identifier immediately followed by
    /// `(`, e.g. `translate(`, `var(`. The value is the function name
    /// (without the parenthesis).
    Function(String),

    /// `<at-keyword-token>` (§4.1). `@` followed by an ident sequence,
    /// e.g. `@media`, `@import`. The value is the keyword (without `@`).
    AtKeyword(String),

    /// `<hash-token>` (§4.1). `#` followed by an ident sequence or hex
    /// digits, e.g. `#main`, `#ff00aa`. The [`HashType`] flag indicates
    /// whether the hash is an "id" hash (would-start-an-ident-sequence),
    /// an "unrestricted" hash (hex digits that don't form an ident), or
    /// "unknown" for hashes that don't match either pattern.
    Hash(String, HashType),

    /// `<string-token>` (§4.1). A quoted string (`"..."` or `'...'`) with
    /// escapes resolved. The value is the string's contents without the
    /// surrounding quotes.
    String(String),

    /// `<bad-string-token>` (§4.1). Emitted when a string contains an
    /// unescaped newline (§4.3.5).
    BadString,

    /// `<url-token>` (§4.1). The `url(...)` form with an unquoted or
    /// quoted URL. The value is the decoded URL contents.
    Url(String),

    /// `<bad-url-token>` (§4.1). Emitted when a URL contains an invalid
    /// escape or unterminated quoted segment (§4.3.6).
    BadUrl,

    /// `<delim-token>` (§4.1). A single code point not consumed by any
    /// other token type, e.g. `>`, `+`, `~` (when not part of `~=`),
    /// `!` (when not part of `!=`).
    Delim(char),

    /// `<number-token>` (§4.1). A numeric value, possibly signed, possibly
    /// with a fractional part or scientific notation. The [`Numeric`]
    /// carries the parsed value and integer/number flag.
    Number(Numeric),

    /// `<percentage-token>` (§4.1). A number immediately followed by `%`.
    Percentage(Numeric),

    /// `<dimension-token>` (§4.1). A number immediately followed by a unit
    /// (an ident sequence), e.g. `10px`, `1.5em`, `-30deg`. The value
    /// carries the [`Numeric`] and the unit string.
    Dimension(Numeric, String),

    /// `<unicode-range-token>` (§4.1). A range of Unicode code points in
    /// the `U+` form, e.g. `U+1234`, `U+12-34FF`, `U+12??`. The two
    /// `Option<u32>` values are the start and end of the range. Either
    /// may be `None` only in malformed inputs that the spec still accepts
    /// as a unicode-range; in practice both are `Some` for well-formed
    /// ranges.
    UnicodeRange(Option<u32>, Option<u32>),

    /// `<whitespace-token>` (§4.1). A run of one or more whitespace code
    /// points (space, tab, newline, §4.2 definition of whitespace).
    Whitespace,

    /// `<comment-token>` (§4.1). The contents of a `/* ... */` comment
    /// (without the delimiters). Comment tokens are only emitted in
    /// "tokenizer-friendly" mode; the default stylesheet tokenizer
    /// discards them. This implementation emits them so callers can
    /// choose.
    Comment(String),

    /// `<colon-token>` (§4.1). `:`.
    Colon,

    /// `<semicolon-token>` (§4.1). `;`.
    Semicolon,

    /// `<comma-token>` (§4.1). `,`.
    Comma,

    /// `<[-token>` (§4.1). `[`.
    OpenBracket,

    /// `<]-token>` (§4.1). `]`.
    CloseBracket,

    /// `<(-token>` (§4.1). `(`.
    OpenParen,

    /// `<)-token>` (§4.1). `)`.
    CloseParen,

    /// `<{-token>` (§4.1). `{`.
    OpenBrace,

    /// `<}-token>` (§4.1). `}`.
    CloseBrace,

    /// `<CDO-token>` (§4.1). The literal `<!--` (Comment Declaration
    /// Open). Kept for backwards compatibility with CSS 1/2.1's practice
    /// of wrapping stylesheets in `<!--` ... `-->` to hide them from
    /// ancient browsers. At the tokenizer level this is a single token
    /// spanning all four code points.
    Cdo,

    /// `<CDC-token>` (§4.1). The literal `-->` (Comment Declaration
    /// Close). See [`Token::Cdo`].
    Cdc,

    /// `<EOF-token>` (§5.3). Emitted once when the input stream is
    /// exhausted. [`Tokenizer::next_token`] returns `None` after this.
    Eof,
}

/// The type flag on a `<hash-token>` (§4.1, §4.3.4).
///
/// Per §4.3.4, when consuming an ident-like token starting with `#`:
/// - If the following code points would start an ident sequence (§4.3.9),
///   the hash type is "id".
/// - Otherwise, if the following code points are hex digits, the hash
///   type is "unrestricted".
/// - Otherwise (no ident sequence, no hex digits), the hash is still
///   "unrestricted" with an empty value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashType {
    /// `#` followed by an ident sequence. Matches the ID selector syntax.
    Id,
    /// `#` followed by something that isn't an ident sequence (e.g. hex
    /// digits, or empty).
    Unrestricted,
}

/// A numeric value carried by `<number-token>`, `<percentage-token>`,
/// and `<dimension-token>` (§4.3.13).
///
/// Per §4.3.13 "Consume a number", the numeric value is parsed as either
/// an integer or a floating-point value, and a flag records which kind
/// was found. The sign is included in the value.
#[derive(Debug, Clone, PartialEq)]
pub struct Numeric {
    /// The parsed numeric value.
    pub value: f64,
    /// Whether the source representation had a `.` or scientific-notation
    /// exponent, making it a "number" rather than an "integer". Per
    /// §4.3.13 this flag is set when the number's representation includes
    /// a fractional component or an `e`/`E` exponent.
    pub is_integer: bool,
}

impl fmt::Display for Numeric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_integer {
            write!(f, "{}", self.value as i64)
        } else {
            write!(f, "{}", self.value)
        }
    }
}

/// The current state of the CSS tokenizer.
///
/// Unlike the HTML tokenizer (which has ~80 explicit states per WHATWG
/// §13.2.5), the CSS tokenizer is a recursive-descent algorithm: the
/// main entry point `consume_a_token` (§4.3.1) dispatches to
/// sub-algorithms without a persistent state machine. [`State`] is
/// therefore minimal — it only tracks the top-level position (consuming
/// tokens vs. EOF emitted) for the [`Tokenizer::next_token`] iterator
/// protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// The tokenizer is at the top level, ready to consume the next token
    /// via §4.3.1.
    Data,
    /// The tokenizer has emitted `<EOF-token>`; subsequent
    /// `next_token()` calls return `None`.
    Eof,
}

impl Default for State {
    fn default() -> Self {
        State::Data
    }
}
