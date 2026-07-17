//! CSS Syntax tokenizer implementation.
//!
//! Implements the "Tokenizer Algorithms" of CSS Syntax Module Level 3 §4.3.
//! The tokenizer is recursive-descent: [`CssTokenizer::next_token`] calls
//! [`consume_a_token`](CssTokenizer::consume_a_token) (§4.3.1), which
//! dispatches to sub-algorithms based on the current input code point.
//!
//! # Coverage
//!
//! All §4.3 sub-algorithms are implemented:
//! - §4.3.1 Consume a token (full dispatch, incl. `unicode_ranges_allowed`)
//! - §4.3.2 Consume comments
//! - §4.3.3 Consume a numeric token
//! - §4.3.4 Consume an ident-like token (incl. `url(` special case)
//! - §4.3.5 Consume a string token
//! - §4.3.6 Consume a url token
//! - §4.3.7 Consume an escaped code point
//! - §4.3.8 Check if two code points are a valid escape
//! - §4.3.9 Check if three code points would start an ident sequence
//! - §4.3.10 Check if three code points would start a number
//! - §4.3.11 Check if three code points would start a unicode-range
//! - §4.3.12 Consume an ident sequence
//! - §4.3.13 Consume a number
//! - §4.3.14 Consume a unicode-range token
//! - §4.3.15 Consume the remnants of a bad url

use super::trait_def::Tokenizer;
use super::types::{HashType, Numeric, State, Token};

/// The CSS tokenizer (§4.3).
///
/// Holds the preprocessed input stream (§5.3 normalization applied at
/// construction), the current position, and EOF tracking. Sub-algorithms
/// are implemented as methods on this struct so they share the input
/// stream and position.
pub struct CssTokenizer {
    /// Input code points after §5.3 preprocessing (CR/LF/FF normalized to LF).
    input: Vec<char>,
    /// Current position in `input` (0-based index of the next code point
    /// to consume). `pos == input.len()` means the stream is exhausted.
    pos: usize,
    /// Top-level state (§4.3.1 dispatch vs. EOF).
    state: State,
    /// Whether `<EOF-token>` has been emitted. After this is `true`,
    /// [`next_token`](Tokenizer::next_token) returns `None`.
    eof_emitted: bool,
    /// §4.3.1 L782-783: `unicode_ranges_allowed` flag, default `false`.
    /// When `true`, the `U+`/`u+` branch of §4.3.1 produces a
    /// `<unicode-range-token>` (§4.3.14); otherwise `U`/`u` is tokenized
    /// as an ident-like token.
    unicode_ranges_allowed: bool,
}

impl CssTokenizer {
    /// Construct a new tokenizer over `input`.
    ///
    /// Applies §5.3 input preprocessing: per the "input stream"
    /// definition, U+000D CR followed by U+000A LF is collapsed to a
    /// single LF, lone CR is replaced with LF, and U+000C FF is replaced
    /// with LF. The tokenizer operates on this normalized stream.
    pub fn new(input: &str) -> Self {
        let input = preprocess_input(input);
        Self {
            input,
            pos: 0,
            state: State::Data,
            eof_emitted: false,
            unicode_ranges_allowed: false,
        }
    }

    /// Peek at the code point at `offset` from the current position
    /// without consuming it. Returns `None` if past end of input.
    ///
    /// Used by §4.3.8 / §4.3.9 / §4.3.10 / §4.3.11 "check if N code
    /// points..." predicates, which must look ahead without advancing.
    fn peek(&self, offset: usize) -> Option<char> {
        self.input.get(self.pos + offset).copied()
    }

    /// Consume and return the next code point, advancing `pos`. Returns
    /// `None` if at end of input.
    ///
    /// Implements the "consume the next input code point" primitive used
    /// throughout §4.3.
    fn consume(&mut self) -> Option<char> {
        let c = self.input.get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    /// Re-consume the current code point: step `pos` back by one.
    ///
    /// Per §4.3.1 "reconsume the current input code point": the next
    /// call to [`consume`](Self::consume) will return the same code point
    /// that was just consumed. No-op if `pos == 0`.
    fn reconsume(&mut self) {
        if self.pos > 0 {
            self.pos -= 1;
        }
    }

    /// §4.3.1 Consume a token.
    ///
    /// This is the main entry point of the tokenizer. Dispatches based on
    /// the current input code point to one of the sub-algorithms (§4.3.2
    /// through §4.3.6) or emits a simple token directly. The optional
    /// `unicode_ranges_allowed` flag (§4.3.1 L782-783, default `false`)
    /// gates the `U+`/`u+` unicode-range branch.
    fn consume_a_token(&mut self) -> Token {
        // §4.3.2: If the next two input code points are U+002F SOLIDUS
        // (`/`) and U+002A ASTERISK (`*`), consume comments and
        // recursively consume the next token. We implement this as a
        // loop: any leading run of `/* ... */` comments is skipped before
        // producing the next real token.
        loop {
            let Some(c) = self.consume() else {
                // §5.3: end of stream → <EOF-token>
                self.eof_emitted = true;
                self.state = State::Eof;
                return Token::Eof;
            };

            // §4.3.2 comment dispatch: `/` followed by `*`.
            if c == '/' && self.peek(0) == Some('*') {
                self.consume(); // consume `*`
                self.consume_comments_body();
                // Loop back to consume the next token (which may itself
                // be preceded by another comment).
                continue;
            }

            return match c {
                // §4.3.1: whitespace → <whitespace-token>
                ' ' | '\t' | '\n' | '\r' | '\u{000C}' => {
                    // Consume all consecutive whitespace.
                    while let Some(next) = self.peek(0) {
                        if matches!(next, ' ' | '\t' | '\n' | '\r' | '\u{000C}') {
                            self.consume();
                        } else {
                            break;
                        }
                    }
                    Token::Whitespace
                }
                // §4.3.1: `"` or `'` → consume a string token (§4.3.5)
                '"' | '\'' => self.consume_a_string_token(c),
                // §4.3.1: `#` → consume an ident-like token (§4.3.4) for hash
                '#' => self.consume_a_hash_token(),
                // §4.3.1: `(` → <(-token>
                '(' => Token::OpenParen,
                // §4.3.1: `)` → <)-token>
                ')' => Token::CloseParen,
                // §4.3.1: `+` or `-` → if would-start-a-number, consume a
                // numeric token (§4.3.3); else <delim-token>.
                // §4.3.1: `-` additionally: if next two are `-` and `>`
                // (i.e. `-->`), consume them and return <CDC-token>.
                // §4.3.1: `.` → if would-start-a-number, consume a numeric
                // token; else <delim-token>.
                '-' => {
                    if self.starts_with_number(c) {
                        self.reconsume();
                        self.consume_a_numeric_token()
                    } else if self.peek(0) == Some('-') && self.peek(1) == Some('>') {
                        // §4.3.1: `-->` → <CDC-token>
                        self.consume(); // consume second `-`
                        self.consume(); // consume `>`
                        Token::Cdc
                    } else if self.would_start_ident_sequence_at(0) {
                        // §4.3.1: `-` starting an ident sequence → reconsume
                        // and consume an ident-like token.
                        self.reconsume();
                        self.consume_an_ident_like_token()
                    } else {
                        Token::Delim(c)
                    }
                }
                '+' | '.' => {
                    if self.starts_with_number(c) {
                        self.reconsume();
                        self.consume_a_numeric_token()
                    } else {
                        Token::Delim(c)
                    }
                }
                // §4.3.1: `<` → if next three are `!`, `-`, `-` (i.e.
                // `<!--`), consume them and return <CDO-token>. Otherwise
                // emit <delim-token>.
                '<' => {
                    if self.peek(0) == Some('!')
                        && self.peek(1) == Some('-')
                        && self.peek(2) == Some('-')
                    {
                        // Consume `!`, `-`, `-`.
                        self.consume();
                        self.consume();
                        self.consume();
                        Token::Cdo
                    } else {
                        Token::Delim(c)
                    }
                }
                // §4.3.1: `@` → if next would-start-an-ident-sequence,
                // consume an ident-like token (§4.3.4) for at-keyword.
                '@' => self.consume_an_at_keyword_token(),
                // §4.3.1: `[` → <[-token>
                '[' => Token::OpenBracket,
                // §4.3.1: `\` → if would-start-an-escape (§4.3.8), reconsume
                // and consume an ident-like token (§4.3.4); else parse error,
                // <delim-token>.
                '\\' => self.consume_ident_or_delim(),
                // §4.3.1: `]` → <]-token>
                ']' => Token::CloseBracket,
                // §4.3.1: `{` → <{-token>
                '{' => Token::OpenBrace,
                // §4.3.1: `}` → <}-token>
                '}' => Token::CloseBrace,
                // §4.3.1: digit → reconsume and consume a numeric token (§4.3.3)
                '0'..='9' => {
                    self.reconsume();
                    self.consume_a_numeric_token()
                }
                // §4.3.1: `U+` or `u+` → if would-start-a-unicode-range (§4.3.11),
                // consume a unicode-range token (§4.3.14).
                'u' | 'U' => self.consume_u_or_unicode_range(),
                // §4.3.1: ident-start code point → reconsume and consume an
                // ident-like token (§4.3.4)
                _ if is_ident_start_code_point(c) => {
                    self.reconsume();
                    self.consume_an_ident_like_token()
                }
                // §4.3.1: `:` → <colon-token>
                ':' => Token::Colon,
                // §4.3.1: `;` → <semicolon-token>
                ';' => Token::Semicolon,
                // §4.3.1: `,` → <comma-token>
                ',' => Token::Comma,
                // §4.3.1: anything else → <delim-token>
                _ => Token::Delim(c),
            };
        }
    }

    /// §4.3.2 Consume comments — body only.
    ///
    /// Precondition: the opening `/*` has already been consumed. Consume
    /// code points until the closing `*/` or EOF. Per §4.3.2, an
    /// unterminated comment (EOF before `*/`) is a parse error; we
    /// silently consume to EOF in that case (no error reporting yet).
    fn consume_comments_body(&mut self) {
        loop {
            match self.consume() {
                None => return, // EOF in comment: parse error (unreported)
                Some('*') => {
                    if self.peek(0) == Some('/') {
                        self.consume(); // consume `/`
                        return;
                    }
                }
                Some(_) => {}
            }
        }
    }

    // ── Sub-algorithm stubs (implemented in subsequent commits) ───────

    /// §4.3.5 (L1081-1128) Consume a string token.
    ///
    /// `quote` is the ending code point (the opening quote `"` or `'`).
    /// Returns a `string-token` (normal or EOF-terminated) or a
    /// `bad-string-token` (unescaped newline).
    ///
    /// Per §4.3.5:
    /// - ending code point (L1097-1099) → return string-token.
    /// - EOF (L1101-1104) → parse error, return string-token.
    /// - newline (L1106-1110) → parse error, reconsume, return bad-string.
    /// - `\` (L1112-1124):
    ///   - next EOF → do nothing (loop; next consume hits EOF → string).
    ///   - next newline → consume it (line continuation).
    ///   - otherwise (valid escape) → consume an escaped code point, append.
    /// - anything else (L1126-1128) → append.
    fn consume_a_string_token(&mut self, quote: char) -> Token {
        let mut value = String::new();
        loop {
            match self.consume() {
                None => {
                    // §4.3.5 L1101-1104: EOF → parse error, return string-token.
                    return Token::String(value);
                }
                Some(c) if c == quote => {
                    // §4.3.5 L1097-1099: ending code point → return string-token.
                    return Token::String(value);
                }
                Some('\n') => {
                    // §4.3.5 L1106-1110: newline → parse error, reconsume, return bad-string.
                    self.reconsume();
                    return Token::BadString;
                }
                Some('\\') => {
                    // §4.3.5 L1112-1124: escape handling.
                    match self.peek(0) {
                        None => {
                            // §4.3.5 L1114-1115: next is EOF, do nothing.
                            // Loop continues; next consume() returns None → String.
                        }
                        Some('\n') => {
                            // §4.3.5 L1117-1119: next is newline, consume it (line continuation).
                            self.consume();
                        }
                        Some(_) => {
                            // §4.3.5 L1121-1124: valid escape, consume escaped code point.
                            let escaped = self.consume_an_escaped_code_point();
                            value.push(escaped);
                        }
                    }
                }
                Some(c) => {
                    // §4.3.5 L1126-1128: anything else → append.
                    value.push(c);
                }
            }
        }
    }

    /// §4.3.1 (L801-826) `#` branch: Consume a hash token.
    ///
    /// Precondition: `#` has been consumed. Per §4.3.1 L801-826:
    /// - If the next code point is an ident code point OR the next two
    ///   are a valid escape, then create a `hash-token` (type "id" if
    ///   would-start-ident-sequence, else "unrestricted"), consume an
    ///   ident sequence for the value, and return it.
    /// - Otherwise, return a `delim-token` with value `#`.
    fn consume_a_hash_token(&mut self) -> Token {
        // §4.3.1 L803-805: next is ident code point OR next two are valid escape.
        let next_is_ident = self.peek(0).is_some_and(is_ident_code_point);
        let next_is_valid_escape = self.is_valid_escape_at(0);
        if !next_is_ident && !next_is_valid_escape {
            // §4.3.1 L824-826: return delim-token with value `#`.
            return Token::Delim('#');
        }
        let hash_type = if self.would_start_ident_sequence_at(0) {
            HashType::Id
        } else {
            HashType::Unrestricted
        };
        let name = self.consume_an_ident_sequence();
        Token::Hash(name, hash_type)
    }

    /// §4.3.3 (L1011-1042) Consume a numeric token.
    ///
    /// Returns a `number-token`, `percentage-token`, or `dimension-token`.
    /// Per §4.3.3:
    /// 1. Consume a number (§4.3.13).
    /// 2. If next 3 would start an ident sequence → dimension-token (consume
    ///    an ident sequence for the unit).
    /// 3. Else if next is `%` → percentage-token (consume `%`).
    /// 4. Else → number-token.
    fn consume_a_numeric_token(&mut self) -> Token {
        let (value, is_integer) = self.consume_a_number();
        let numeric = Numeric { value, is_integer };
        // §4.3.3 L1019-1029: dimension
        if self.would_start_ident_sequence_at(0) {
            let unit = self.consume_an_ident_sequence();
            return Token::Dimension(numeric, unit);
        }
        // §4.3.3 L1032-1036: percentage
        if self.peek(0) == Some('%') {
            self.consume();
            return Token::Percentage(numeric);
        }
        // §4.3.3 L1040-1042: number
        Token::Number(numeric)
    }

    /// §4.3.13 (L1415-1483) Consume a number.
    ///
    /// Returns `(value, is_integer)` where `is_integer` is true when the
    /// source representation had no fractional part and no exponent (type
    /// "integer"), false when it had either (type "number"). The sign is
    /// included in `value`.
    ///
    /// Per §4.3.13:
    /// 1. type = "integer"; number_part = ""; exponent_part = "".
    /// 2. If next is `+`/`-`, consume, append to number_part.
    /// 3. While next is digit, consume, append to number_part.
    /// 4. If next 2 are `.` + digit: consume `.`, consume digits; type = "number".
    /// 5. If next 2/3 are `e`/`E` + optional `+`/`-` + digit: consume `e`/`E`,
    ///    optional sign, digits into exponent_part; type = "number".
    /// 6. value = parse number_part; if exponent_part non-empty, value *= 10^exp.
    /// 7. Return value, type, sign.
    fn consume_a_number(&mut self) -> (f64, bool) {
        let mut type_is_number = false;
        let mut number_part = String::new();
        let mut exponent_part = String::new();

        // §4.3.13 L1436-1440: sign
        if self.peek(0) == Some('+') || self.peek(0) == Some('-') {
            number_part.push(self.consume().unwrap());
        }

        // §4.3.13 L1442-1444: integer digits
        while let Some(d) = self.peek(0) {
            if is_digit(d) {
                number_part.push(d);
                self.consume();
            } else {
                break;
            }
        }

        // §4.3.13 L1446-1454: fraction part
        if self.peek(0) == Some('.') && matches!(self.peek(1), Some(d) if is_digit(d)) {
            number_part.push(self.consume().unwrap()); // consume `.`
            type_is_number = true;
            while let Some(d) = self.peek(0) {
                if is_digit(d) {
                    number_part.push(d);
                    self.consume();
                } else {
                    break;
                }
            }
        }

        // §4.3.13 L1457-1469: exponent part
        if matches!(self.peek(0), Some('e') | Some('E')) {
            let p1 = self.peek(1);
            let p2 = self.peek(2);
            let valid_exp = match p1 {
                Some('+') | Some('-') => matches!(p2, Some(d) if is_digit(d)),
                Some(d) if is_digit(d) => true,
                _ => false,
            };
            if valid_exp {
                self.consume(); // consume `e`/`E`
                if self.peek(0) == Some('+') || self.peek(0) == Some('-') {
                    exponent_part.push(self.consume().unwrap());
                }
                while let Some(d) = self.peek(0) {
                    if is_digit(d) {
                        exponent_part.push(d);
                        self.consume();
                    } else {
                        break;
                    }
                }
                type_is_number = true;
            }
        }

        // §4.3.13 L1472-1480: compute value
        let mut value: f64 = number_part.parse().unwrap_or(0.0);
        if !exponent_part.is_empty() {
            let exp: i32 = exponent_part.parse().unwrap_or(0);
            value *= 10.0f64.powi(exp);
        }

        (value, !type_is_number)
    }

    /// §4.3.4 (L1045-1078) Consume an ident-like token.
    ///
    /// Precondition: the current position is at the start of an ident
    /// sequence (the caller has not consumed anything; or has reconsumed
    /// the first code point). Returns an `ident-token`, `function-token`,
    /// `url-token`, or `bad-url-token`.
    ///
    /// Per §4.3.4:
    /// 1. Consume an ident sequence (§4.3.12), yielding `name`.
    /// 2. If `name` is ASCII-case-insensitive "url" and next is `(`:
    ///    consume `(`, collapse leading whitespace pairs, then if the
    ///    next 1-2 code points are a quote (or whitespace+quote) return
    ///    a `function-token`; otherwise consume a url token (§4.3.6).
    /// 3. Else if next is `(`: consume it, return `function-token`.
    /// 4. Else return `ident-token`.
    fn consume_an_ident_like_token(&mut self) -> Token {
        let name = self.consume_an_ident_sequence();
        // §4.3.4 L1053-1066: url( special case
        if name.eq_ignore_ascii_case("url") && self.peek(0) == Some('(') {
            self.consume(); // consume `(`
            // §4.3.4 L1056-1057: while next two are whitespace, consume one
            while self.peek(0).is_some_and(is_whitespace)
                && self.peek(1).is_some_and(is_whitespace)
            {
                self.consume();
            }
            // §4.3.4 L1058-1063: if next 1-2 are " / ' / ws+" / ws+' → Function
            let p0 = self.peek(0);
            let p1 = self.peek(1);
            let is_quote_case = matches!(p0, Some('"') | Some('\''))
                || (p0.is_some_and(is_whitespace) && matches!(p1, Some('"') | Some('\'')));
            if is_quote_case {
                return Token::Function(name);
            }
            // §4.3.4 L1064-1066: otherwise consume a url token
            return self.consume_a_url_token();
        }
        // §4.3.4 L1068-1073: ordinary function
        if self.peek(0) == Some('(') {
            self.consume();
            return Token::Function(name);
        }
        // §4.3.4 L1075-1078: ident
        Token::Ident(name)
    }

    /// §4.3.4 (at-keyword branch) Consume an at-keyword token.
    ///
    /// Precondition: `@` has been consumed. Per §4.3.1, if the next code
    /// points would start an ident sequence, consume it and return an
    /// `<at-keyword-token>`; otherwise return a `<delim-token>` with `@`.
    fn consume_an_at_keyword_token(&mut self) -> Token {
        if self.would_start_ident_sequence_at(0) {
            let name = self.consume_an_ident_sequence();
            Token::AtKeyword(name)
        } else {
            Token::Delim('@')
        }
    }

    /// §4.3.1 (backslash branch): if would-start-an-escape, consume an
    /// ident-like token; else emit `<delim-token>`.
    fn consume_ident_or_delim(&mut self) -> Token {
        if self.is_valid_escape_next() {
            self.reconsume();
            self.consume_an_ident_like_token()
        } else {
            Token::Delim('\\')
        }
    }

    /// §4.3.6 (L1132-1203) Consume a url token.
    ///
    /// Precondition: `url(` has been consumed by the caller (§4.3.4
    /// L1053-1066). This algorithm consumes the "unquoted" url body and
    /// returns a `url-token` or `bad-url-token`. A quoted value like
    /// `url("foo")` is handled by §4.3.4 as a `function-token` and never
    /// reaches here.
    ///
    /// Per §4.3.6:
    /// - `)` → return url-token.
    /// - EOF → parse error, return url-token.
    /// - whitespace → consume trailing whitespace; if next is `)` or EOF
    ///   return url-token, else bad-url.
    /// - `"` / `'` / `(` / non-printable → parse error, bad-url.
    /// - `\` → if valid escape, consume escaped and append; else bad-url.
    /// - anything else → append.
    fn consume_a_url_token(&mut self) -> Token {
        let mut value = String::new();
        // §4.3.6 L1151: consume as much whitespace as possible
        while self.peek(0).is_some_and(is_whitespace) {
            self.consume();
        }
        loop {
            match self.consume() {
                // §4.3.6 L1157-1159: `)` → return url-token
                Some(')') => return Token::Url(value),
                // §4.3.6 L1161-1164: EOF → parse error, return url-token
                None => return Token::Url(value),
                // §4.3.6 L1166-1175: whitespace → consume trailing ws, then check
                Some(c) if is_whitespace(c) => {
                    while self.peek(0).is_some_and(is_whitespace) {
                        self.consume();
                    }
                    match self.peek(0) {
                        Some(')') => {
                            self.consume();
                            return Token::Url(value);
                        }
                        None => return Token::Url(value), // parse error
                        _ => {
                            self.consume_the_remnants_of_a_bad_url();
                            return Token::BadUrl;
                        }
                    }
                }
                // §4.3.6 L1177-1185: " / ' / ( / non-printable → bad url
                Some(c) if c == '"' || c == '\'' || c == '(' || is_non_printable(c) => {
                    self.consume_the_remnants_of_a_bad_url();
                    return Token::BadUrl;
                }
                // §4.3.6 L1187-1197: `\`
                Some('\\') => {
                    if self.is_valid_escape_next() {
                        let escaped = self.consume_an_escaped_code_point();
                        value.push(escaped);
                    } else {
                        self.consume_the_remnants_of_a_bad_url();
                        return Token::BadUrl;
                    }
                }
                // §4.3.6 L1199-1202: anything else → append
                Some(c) => value.push(c),
            }
        }
    }

    /// §4.3.15 (L1551-1577) Consume the remnants of a bad url.
    ///
    /// Consumes code points until `)` or EOF, allowing an escaped `)`
    /// (`\)`) to be consumed without ending the bad-url. Per §4.3.15:
    /// - `)` or EOF → return.
    /// - valid escape (stream starts with `\` + non-newline non-EOF) →
    ///   consume an escaped code point.
    /// - anything else → do nothing.
    fn consume_the_remnants_of_a_bad_url(&mut self) {
        loop {
            match self.consume() {
                // §4.3.15 L1563-1566: `)` or EOF → return
                Some(')') | None => return,
                // §4.3.15 L1568-1572: valid escape → consume escaped code point
                Some('\\') if self.is_valid_escape_next() => {
                    let _ = self.consume_an_escaped_code_point();
                }
                // §4.3.15 L1574-1576: anything else → do nothing
                Some(_) => {}
            }
        }
    }

    /// §4.3.1 (L960-972) U/u branch: if `unicode_ranges_allowed` is true
    /// and the stream would start a unicode-range (§4.3.11), consume a
    /// unicode-range token (§4.3.14); otherwise consume an ident-like token.
    ///
    /// Precondition: `u`/`U` has been consumed, `pos` points just past it.
    /// This method reconsumes back to `u` before dispatching.
    fn consume_u_or_unicode_range(&mut self) -> Token {
        self.reconsume(); // pos back to `u`/`U`
        // §4.3.1 L963-967: unicode_ranges_allowed && would-start-unicode-range
        if self.unicode_ranges_allowed && self.would_start_unicode_range_at(0) {
            return self.consume_a_unicode_range_token();
        }
        // §4.3.1 L969-972: otherwise ident-like
        self.consume_an_ident_like_token()
    }

    /// §4.3.11 (L1356-1378) Check if three code points would start a
    /// unicode-range, examining code points starting at `offset` from the
    /// current position.
    ///
    /// Per §4.3.11: true iff first is `U`/`u`, second is `+`, third is `?`
    /// or a hex digit.
    fn would_start_unicode_range_at(&self, offset: usize) -> bool {
        let first = match self.peek(offset) {
            Some(c) => c,
            None => return false,
        };
        if !matches!(first, 'U' | 'u') {
            return false;
        }
        if self.peek(offset + 1) != Some('+') {
            return false;
        }
        match self.peek(offset + 2) {
            Some('?') => true,
            Some(c) if is_hex_digit(c) => true,
            _ => false,
        }
    }

    /// §4.3.14 (L1487-1548) Consume a unicode-range token.
    ///
    /// Precondition: the stream starts with `u`/`U` + `+` + (`?` or hex
    /// digit) (i.e. [`would_start_unicode_range_at`](Self::would_start_unicode_range_at)
    /// returns true at the current position).
    ///
    /// Returns `UnicodeRange(Some(start), Some(end))`.
    ///
    /// Per §4.3.14:
    /// 1. Consume and discard `u`/`U` and `+`.
    /// 2. Consume up to 6 hex digits; if fewer than 6, consume `?` up to a
    ///    total of 6 → `first_segment`.
    /// 3. If `first_segment` contains `?`: start = `?`→`0`, end = `?`→`F`.
    /// 4. Else start = hex value of `first_segment`.
    /// 5. If next two are `-` + hex digit: consume `-`, consume up to 6 hex
    ///    → end.
    /// 6. Else end = start.
    fn consume_a_unicode_range_token(&mut self) -> Token {
        // §4.3.14 step 1: consume and discard `u`/`U` + `+`
        self.consume(); // `u`/`U`
        self.consume(); // `+`

        // §4.3.14 step 2: consume up to 6 hex digits
        let mut first_segment = String::new();
        while first_segment.len() < 6 {
            match self.peek(0) {
                Some(c) if is_hex_digit(c) => {
                    first_segment.push(c);
                    self.consume();
                }
                _ => break,
            }
        }
        // then consume `?` up to a total of 6
        while first_segment.len() < 6 {
            match self.peek(0) {
                Some('?') => {
                    first_segment.push('?');
                    self.consume();
                }
                _ => break,
            }
        }

        // §4.3.14 step 3: if first_segment contains `?`
        if first_segment.contains('?') {
            let start_str: String = first_segment
                .chars()
                .map(|c| if c == '?' { '0' } else { c })
                .collect();
            let start = u32::from_str_radix(&start_str, 16).unwrap_or(0);
            let end_str: String = first_segment
                .chars()
                .map(|c| if c == '?' { 'F' } else { c })
                .collect();
            let end = u32::from_str_radix(&end_str, 16).unwrap_or(0);
            return Token::UnicodeRange(Some(start), Some(end));
        }

        // §4.3.14 step 4: first_segment as hex → start
        let start = u32::from_str_radix(&first_segment, 16).unwrap_or(0);

        // §4.3.14 step 5: if next two are `-` + hex digit
        if self.peek(0) == Some('-') && self.peek(1).is_some_and(is_hex_digit) {
            self.consume(); // consume `-`
            let mut end_segment = String::new();
            while end_segment.len() < 6 {
                match self.peek(0) {
                    Some(c) if is_hex_digit(c) => {
                        end_segment.push(c);
                        self.consume();
                    }
                    _ => break,
                }
            }
            let end = u32::from_str_radix(&end_segment, 16).unwrap_or(0);
            return Token::UnicodeRange(Some(start), Some(end));
        }

        // §4.3.14 step 6: otherwise start == end
        Token::UnicodeRange(Some(start), Some(start))
    }

    // ── §4.3.12 Consume an ident sequence ────────────────────────────

    /// §4.3.12 Consume an ident sequence.
    ///
    /// Repeatedly consume code points as long as they form an ident
    /// sequence: ident code points, valid escapes (consumed via
    /// §4.3.7), or `\`-`-` (for `--`-prefixed names). Returns the
    /// decoded name with escapes resolved.
    ///
    /// Per §4.3.12, the loop terminates when:
    /// - EOF is reached, or
    /// - the next code point is not an ident code point, not a valid
    ///   escape, and not part of a `\`-`-` continuation.
    fn consume_an_ident_sequence(&mut self) -> String {
        let mut result = String::new();
        loop {
            let Some(c) = self.consume() else {
                return result;
            };
            if is_ident_code_point(c) {
                result.push(c);
            } else if c == '\\' && self.is_valid_escape_next() {
                // §4.3.12: valid escape → consume the escaped code point
                // and append it.
                let escaped = self.consume_an_escaped_code_point();
                result.push(escaped);
            } else {
                // Not part of the ident sequence: reconsume and stop.
                self.reconsume();
                return result;
            }
        }
    }

    // ── §4.3.7 Consume an escaped code point ────────────────────────

    /// §4.3.7 Consume an escaped code point.
    ///
    /// Precondition: the `\` has been consumed. Returns the decoded code
    /// point.
    ///
    /// Per §4.3.7 (L1207-1240):
    /// - EOF (L1233-1236) → parse error, return U+FFFD REPLACEMENT CHARACTER.
    /// - newline → parse error, reconsume (invalid escape; the caller
    ///   should have checked §4.3.8 first, but we handle defensively).
    /// - hex digit(s) (L1219-1231) → consume 1-6 hex digits, optionally
    ///   followed by whitespace, and return the code point with that
    ///   value. U+0000 NULL or out-of-range → replacement character U+FFFD.
    /// - anything else (L1238-1240) → return the consumed code point.
    fn consume_an_escaped_code_point(&mut self) -> char {
        let Some(c) = self.consume() else {
            // §4.3.7 L1233-1236: EOF → parse error, return U+FFFD.
            return '\u{FFFD}';
        };
        if is_hex_digit(c) {
            // §4.3.7 step 4: consume up to 5 more hex digits (total ≤ 6).
            let mut hex = String::new();
            hex.push(c);
            while hex.len() < 6 {
                match self.peek(0) {
                    Some(next) if is_hex_digit(next) => {
                        hex.push(next);
                        self.consume();
                    }
                    _ => break,
                }
            }
            // §4.3.7 step 4: consume one trailing whitespace.
            if self.peek(0) == Some(' ') || self.peek(0) == Some('\t') || self.peek(0) == Some('\n')
            {
                self.consume();
            }
            // Parse the hex value.
            let value = u32::from_str_radix(&hex, 16).unwrap_or(0);
            // §4.3.7 step 4: 0 or > 0x10FFFF → U+FFFD. Also surrogates
            // (0xD800–0xDFFF) → U+FFFD.
            if value == 0 || value > 0x10FFFF || (0xD800..=0xDFFF).contains(&value) {
                '\u{FFFD}'
            } else {
                char::from_u32(value).unwrap_or('\u{FFFD}')
            }
        } else if c == '\n' {
            // §4.3.7 step 3: newline after `\` is a parse error. This
            // should not happen if the caller checked §4.3.8, but handle
            // defensively by returning `\` and reconsuming the newline.
            self.reconsume();
            '\\'
        } else {
            // §4.3.7 step 5: any other code point → return it.
            c
        }
    }

    // ── §4.3.8 / §4.3.9 Predicates ──────────────────────────────────

    /// §4.3.8 Check if two code points are a valid escape.
    ///
    /// Precondition: the `\` has been consumed. Returns true if the next
    /// code point forms a valid escape with the consumed `\` (i.e. the
    /// next code point is not a newline and not EOF).
    fn is_valid_escape_next(&self) -> bool {
        match self.peek(0) {
            None => false,
            Some('\n') => false,
            Some(_) => true,
        }
    }

    /// §4.3.9 Check if three code points would start an ident sequence,
    /// examining code points starting at `offset` from the current
    /// position.
    ///
    /// Per §4.3.9:
    /// - U+002D HYPHEN-MINUS (`-`):
    ///   - If the next is also `-`, true (start of `--...`).
    ///   - If the next is an ident-start code point (or a valid escape),
    ///     true.
    ///   - Else false.
    /// - U+005C REVERSE SOLIDUS (`\`): true if valid escape.
    /// - ident-start code point: true.
    /// - Else false.
    fn would_start_ident_sequence_at(&self, offset: usize) -> bool {
        let first = match self.peek(offset) {
            Some(c) => c,
            None => return false,
        };
        match first {
            '-' => {
                // §4.3.9: if next is `-`, true.
                match self.peek(offset + 1) {
                    Some('-') => true,
                    Some(next) if is_ident_start_code_point(next) => true,
                    Some('\\') => {
                        // §4.3.9: if `\` and the following forms a valid escape.
                        self.is_valid_escape_at(offset + 1)
                    }
                    _ => false,
                }
            }
            '\\' => self.is_valid_escape_at(offset),
            _ if is_ident_start_code_point(first) => true,
            _ => false,
        }
    }

    /// §4.3.8 Check if the code point at `offset` starts a valid escape
    /// (i.e. `offset` points at `\` and `offset+1` is not newline/EOF).
    fn is_valid_escape_at(&self, offset: usize) -> bool {
        if self.peek(offset) != Some('\\') {
            return false;
        }
        match self.peek(offset + 1) {
            None => false,
            Some('\n') => false,
            Some(_) => true,
        }
    }

    // ── Predicates (stubs returning false; implemented in C-4/C-6) ──

    /// §4.3.10 (L1307-1352) Check if three code points would start a number.
    ///
    /// `first` is the code point already consumed (the caller passes it
    /// so the predicate can examine the next two without reconsume). The
    /// three code points examined are `first`, `peek(0)`, `peek(1)`.
    ///
    /// Per §4.3.10:
    /// - `+`/`-`: second is digit → true; second is `.` and third is digit
    ///   → true; else false.
    /// - `.`: second is digit → true; else false.
    /// - digit: true.
    /// - anything else: false.
    fn starts_with_number(&self, first: char) -> bool {
        match first {
            '+' | '-' => match self.peek(0) {
                Some(d) if is_digit(d) => true,
                Some('.') => matches!(self.peek(1), Some(d) if is_digit(d)),
                _ => false,
            },
            '.' => matches!(self.peek(0), Some(d) if is_digit(d)),
            d if is_digit(d) => true,
            _ => false,
        }
    }
}

// ── Free functions (§4.2 code point classes) ──────────────────────────

/// §4.2 Whether `c` is an ident-start code point.
///
/// An ident-start code point is an ASCII letter, a non-ASCII code point
/// (U+0080 or higher), or U+005F LOW LINE (`_`).
fn is_ident_start_code_point(c: char) -> bool {
    matches!(c, 'A'..='Z' | 'a'..='z' | '_' | '\u{0080}'..=char::MAX)
}

/// §4.2 Whether `c` is an ident code point (allowed after the first).
///
/// An ident code point is an ident-start code point, or an ASCII digit,
/// or U+002D HYPHEN-MINUS (`-`).
fn is_ident_code_point(c: char) -> bool {
    matches!(c, 'A'..='Z' | 'a'..='z' | '0'..='9' | '_' | '-' | '\u{0080}'..=char::MAX)
}

/// §4.2 Whether `c` is a digit (ASCII 0-9).
fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

/// §4.2 Whether `c` is a hex digit (ASCII 0-9, A-F, a-f).
fn is_hex_digit(c: char) -> bool {
    c.is_ascii_hexdigit()
}

/// §4.2 Whether `c` is a whitespace code point.
///
/// Per §4.2: U+0009 TAB, U+000A LF, U+000C FF, U+000D CR, U+0020 SPACE.
/// (After §5.3 preprocessing, U+000C FF and lone U+000D CR have been
/// normalized to U+000A LF, but the helper follows the full §4.2
/// definition for correctness when called on arbitrary input.)
fn is_whitespace(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{000C}' | '\r' | ' ')
}

/// §4.2 Whether `c` is a non-printable code point.
///
/// Per §4.2: U+0000-U+0008, U+000B, U+000E-U+001F, U+007F-U+009F.
fn is_non_printable(c: char) -> bool {
    matches!(
        c,
        '\u{0000}'..='\u{0008}' | '\u{000B}' | '\u{000E}'..='\u{001F}' | '\u{007F}'..='\u{009F}'
    )
}

/// §5.3 input preprocessing.
///
/// Per the "input stream" definition in §5.3:
/// - U+000D CR followed by U+000A LF → single U+000A LF
/// - lone U+000D CR → U+000A LF
/// - U+000C FF → U+000A LF
///
/// The tokenizer operates on this normalized stream.
fn preprocess_input(s: &str) -> Vec<char> {
    let mut out = Vec::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push('\n');
            }
            '\u{000C}' => out.push('\n'),
            _ => out.push(c),
        }
    }
    out
}

impl Tokenizer for CssTokenizer {
    fn next_token(&mut self) -> Option<Token> {
        if self.eof_emitted {
            return None;
        }
        Some(self.consume_a_token())
    }

    fn state(&self) -> State {
        self.state
    }

    fn set_state(&mut self, state: State) {
        self.state = state;
        if state == State::Data {
            // Resetting to Data after EOF re-enables the iterator; useful
            // for `reset()` callers that want to re-tokenize.
            self.eof_emitted = false;
        }
    }

    fn reset(&mut self) {
        self.pos = 0;
        self.state = State::Data;
        self.eof_emitted = false;
    }

    fn set_unicode_ranges_allowed(&mut self, allowed: bool) {
        self.unicode_ranges_allowed = allowed;
    }
}

// ── Test helpers exposed for unit tests in this file ──────────────────

#[cfg(test)]
impl CssTokenizer {
    /// Tokenize the entire input and return all tokens except the
    /// trailing `<EOF-token>`. Test-only convenience.
    fn collect(input: &str) -> Vec<Token> {
        let mut tz = Self::new(input);
        let mut out = Vec::new();
        while let Some(t) = tz.next_token() {
            if matches!(t, Token::Eof) {
                break;
            }
            out.push(t);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_emits_eof_only() {
        let mut tz = CssTokenizer::new("");
        assert!(matches!(tz.next_token(), Some(Token::Eof)));
        assert!(tz.next_token().is_none());
    }

    #[test]
    fn whitespace_collapses_runs() {
        let tokens = CssTokenizer::collect("   \t\n  ");
        assert_eq!(tokens.len(), 1, "whitespace run → single token");
        assert!(matches!(tokens[0], Token::Whitespace));
    }

    #[test]
    fn simple_punctuation_tokens() {
        let tokens = CssTokenizer::collect(":;,.{}[]()");
        assert_eq!(tokens.len(), 10);
        assert!(matches!(tokens[0], Token::Colon));
        assert!(matches!(tokens[1], Token::Semicolon));
        assert!(matches!(tokens[2], Token::Comma));
        assert!(matches!(tokens[3], Token::Delim('.')));
        assert!(matches!(tokens[4], Token::OpenBrace));
        assert!(matches!(tokens[5], Token::CloseBrace));
        assert!(matches!(tokens[6], Token::OpenBracket));
        assert!(matches!(tokens[7], Token::CloseBracket));
        assert!(matches!(tokens[8], Token::OpenParen));
        assert!(matches!(tokens[9], Token::CloseParen));
    }

    #[test]
    fn close_paren_is_emitted() {
        let tokens = CssTokenizer::collect(")");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0], Token::CloseParen));
    }

    #[test]
    fn unknown_delim_chars() {
        let tokens = CssTokenizer::collect("^~`>");
        assert_eq!(tokens.len(), 4);
        assert!(matches!(tokens[0], Token::Delim('^')));
        assert!(matches!(tokens[1], Token::Delim('~')));
        assert!(matches!(tokens[2], Token::Delim('`')));
        assert!(matches!(tokens[3], Token::Delim('>')));
    }

    #[test]
    fn preprocessing_normalizes_cr_lf_ff() {
        // CR LF → single LF
        let tz = CssTokenizer::new("a\r\nb");
        assert_eq!(tz.input, vec!['a', '\n', 'b']);
        // Lone CR → LF
        let tz = CssTokenizer::new("a\rb");
        assert_eq!(tz.input, vec!['a', '\n', 'b']);
        // FF → LF
        let tz = CssTokenizer::new("a\u{000C}b");
        assert_eq!(tz.input, vec!['a', '\n', 'b']);
    }

    #[test]
    fn eof_emitted_once() {
        // C-1: use a non-ident-start, non-digit char to avoid the
        // ident-like / numeric branches (which are stubbed with todo!()
        // until C-2 / C-4).
        let mut tz = CssTokenizer::new(",");
        let t1 = tz.next_token();
        let t2 = tz.next_token();
        let t3 = tz.next_token();
        assert!(matches!(t1, Some(Token::Comma)));
        assert!(matches!(t2, Some(Token::Eof)));
        assert!(t3.is_none());
    }

    #[test]
    fn block_comment_is_skipped() {
        // §4.3.2: `/* ... */` is consumed and no token is emitted.
        let tokens = CssTokenizer::collect("/* hello */:/*x*/;");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0], Token::Colon));
        assert!(matches!(tokens[1], Token::Semicolon));
    }

    #[test]
    fn unterminated_comment_consumes_to_eof() {
        // §4.3.2: EOF before `*/` is a parse error; we consume to EOF.
        let tokens = CssTokenizer::collect("/* unfinished");
        assert_eq!(tokens.len(), 0, "unterminated comment → no tokens");
    }

    #[test]
    fn comment_between_tokens() {
        // C-2 not yet: `a`/`b` would be ident tokens (todo!()). Verify
        // comment is consumed without affecting surrounding simple tokens
        // by using only punctuation.
        let tokens = CssTokenizer::collect(":/*c*/;");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0], Token::Colon));
        assert!(matches!(tokens[1], Token::Semicolon));
    }

    #[test]
    fn cdo_token_emitted() {
        // §4.3.1: `<!--` → <CDO-token>
        let tokens = CssTokenizer::collect("<!--");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0], Token::Cdo));
    }

    #[test]
    fn cdc_token_emitted() {
        // §4.3.1: `-->` → <CDC-token>
        let tokens = CssTokenizer::collect("-->");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0], Token::Cdc));
    }

    #[test]
    fn cdo_cdc_wrapping_stylesheet() {
        // Legacy CSS 1/2.1 stylesheet wrapping.
        // `<!-- : ; -->`
        // tokens: Cdo, WS, Colon, WS, Semicolon, WS, Cdc = 7
        let tokens = CssTokenizer::collect("<!-- : ; -->");
        assert_eq!(tokens.len(), 7);
        assert!(matches!(tokens[0], Token::Cdo));
        assert!(matches!(tokens[1], Token::Whitespace));
        assert!(matches!(tokens[2], Token::Colon));
        assert!(matches!(tokens[3], Token::Whitespace));
        assert!(matches!(tokens[4], Token::Semicolon));
        assert!(matches!(tokens[5], Token::Whitespace));
        assert!(matches!(tokens[6], Token::Cdc));
    }

    #[test]
    fn less_than_alone_is_delim() {
        // §4.3.1: `<` not followed by `!-` → <delim-token>
        let tokens = CssTokenizer::collect("< ");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0], Token::Delim('<')));
        assert!(matches!(tokens[1], Token::Whitespace));
    }

    #[test]
    fn hyphen_alone_is_delim() {
        // §4.3.1: `-` not followed by number/ident/`->` → <delim-token>
        let tokens = CssTokenizer::collect("- ");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0], Token::Delim('-')));
        assert!(matches!(tokens[1], Token::Whitespace));
    }

    // ── C-2 tests: ident / function / at-keyword / hash ──────────────

    #[test]
    fn simple_ident() {
        let tokens = CssTokenizer::collect("color");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::Ident(s) if s == "color"));
    }

    #[test]
    fn ident_with_hyphen_prefix() {
        let tokens = CssTokenizer::collect("-webkit-flex");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::Ident(s) if s == "-webkit-flex"));
    }

    #[test]
    fn ident_with_double_hyphen() {
        // §4.3.9: `--` starts an ident sequence (custom properties).
        let tokens = CssTokenizer::collect("--my-var");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::Ident(s) if s == "--my-var"));
    }

    #[test]
    fn ident_with_underscore() {
        let tokens = CssTokenizer::collect("_private");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::Ident(s) if s == "_private"));
    }

    #[test]
    fn ident_with_escape() {
        // §4.3.7: `\26` → '&' (U+0026)
        let tokens = CssTokenizer::collect("color\\26 B");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::Ident(s) if s == "color&B"));
    }

    #[test]
    fn function_token() {
        let tokens = CssTokenizer::collect("translate(");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::Function(s) if s == "translate"));
    }

    #[test]
    fn at_keyword_token() {
        let tokens = CssTokenizer::collect("@media");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::AtKeyword(s) if s == "media"));
    }

    #[test]
    fn at_keyword_with_hyphen() {
        let tokens = CssTokenizer::collect("@-webkit-keyframes");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::AtKeyword(s) if s == "-webkit-keyframes"));
    }

    #[test]
    fn at_sign_alone_is_delim() {
        // §4.3.1: `@` not followed by ident-start → <delim-token>
        let tokens = CssTokenizer::collect("@ ");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0], Token::Delim('@')));
        assert!(matches!(tokens[1], Token::Whitespace));
    }

    #[test]
    fn hash_id_type() {
        // §4.3.1 / §4.3.4: `#main` → <hash-token> with type "id"
        let tokens = CssTokenizer::collect("#main");
        assert_eq!(tokens.len(), 1);
        match &tokens[0] {
            Token::Hash(s, t) => {
                assert_eq!(s, "main");
                assert_eq!(*t, crate::tokenizer::types::HashType::Id);
            }
            _ => panic!("expected Hash, got {:?}", tokens[0]),
        }
    }

    #[test]
    fn hash_unrestricted_type() {
        // §4.3.1 L801-826 + §4.3.4: `#123` → next `1` is an ident code
        // point (digits are ident code points per §4.2), so a hash-token
        // is created. `123` does not would-start-an-ident-sequence (digit
        // is not ident-start), so type is "unrestricted". The value is
        // "123" (consume_an_ident_sequence consumes all ident code points).
        let tokens = CssTokenizer::collect("#123");
        assert_eq!(tokens.len(), 1);
        match &tokens[0] {
            Token::Hash(s, t) => {
                assert_eq!(s, "123");
                assert_eq!(*t, crate::tokenizer::types::HashType::Unrestricted);
            }
            _ => panic!("expected Hash, got {:?}", tokens[0]),
        }
    }

    #[test]
    fn hash_hex_color_is_unrestricted() {
        // §4.3.4: `#ff00aa` → starts with `f` which IS ident-start, so
        // this is actually type "id" (the hex digits form a valid ident).
        let tokens = CssTokenizer::collect("#ff00aa");
        assert_eq!(tokens.len(), 1);
        match &tokens[0] {
            Token::Hash(s, t) => {
                assert_eq!(s, "ff00aa");
                assert_eq!(*t, crate::tokenizer::types::HashType::Id);
            }
            _ => panic!("expected Hash, got {:?}", tokens[0]),
        }
    }

    #[test]
    fn backslash_escape_in_ident() {
        // §4.3.7: `\` followed by non-hex → literal char. Use `z` (not
        // `b`, which is a hex digit and would decode to U+000B).
        let tokens = CssTokenizer::collect("a\\z");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::Ident(s) if s == "az"));
    }

    #[test]
    fn backslash_alone_is_delim() {
        // §4.3.1: `\` not followed by valid escape (EOF or newline) →
        // <delim-token>
        let tokens = CssTokenizer::collect("\\");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0], Token::Delim('\\')));
    }

    #[test]
    fn backslash_newline_is_delim() {
        // §4.3.8: `\` followed by newline is not a valid escape.
        let tokens = CssTokenizer::collect("\\\n");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0], Token::Delim('\\')));
        assert!(matches!(tokens[1], Token::Whitespace));
    }

    #[test]
    fn declaration_with_ident_and_value() {
        let tokens = CssTokenizer::collect("color: red");
        assert_eq!(tokens.len(), 4);
        assert!(matches!(&tokens[0], Token::Ident(s) if s == "color"));
        assert!(matches!(tokens[1], Token::Colon));
        assert!(matches!(&tokens[2], Token::Whitespace));
        assert!(matches!(&tokens[3], Token::Ident(s) if s == "red"));
    }

    #[test]
    fn unicode_escape_in_ident() {
        // §4.3.7 L1223-1225: `\000026` → '&' (U+0026); after 6 hex digits,
        // if next is whitespace, consume it. So `\000026 B` → the space is
        // consumed as trailing whitespace, `B` joins the ident sequence →
        // single token Ident("&B").
        let tokens = CssTokenizer::collect("\\000026 B");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::Ident(s) if s == "&B"));
    }

    #[test]
    fn null_escape_becomes_replacement_char() {
        // §4.3.7: `\0` → U+FFFD (NULL is not allowed)
        let tokens = CssTokenizer::collect("\\0");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::Ident(s) if s == "\u{FFFD}"));
    }

    #[test]
    fn surrogate_escape_becomes_replacement_char() {
        // §4.3.7: `\D800` → U+FFFD (surrogates not allowed)
        let tokens = CssTokenizer::collect("\\D800");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::Ident(s) if s == "\u{FFFD}"));
    }

    // ── C-2 Bug 1 回归测试: # Delim 回退 (§4.3.1 L824-826) ────────────

    #[test]
    fn hash_alone_is_delim() {
        // §4.3.1 L824-826: `#` followed by EOF → next is not ident code
        // point, not valid escape → return delim-token with value `#`.
        let tokens = CssTokenizer::collect("#");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0], Token::Delim('#')));
    }

    #[test]
    fn hash_followed_by_space_is_delim() {
        // §4.3.1 L824-826: `#` followed by space (not ident code point,
        // not valid escape) → delim-token with value `#`.
        let tokens = CssTokenizer::collect("# ");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0], Token::Delim('#')));
        assert!(matches!(tokens[1], Token::Whitespace));
    }

    #[test]
    fn hash_followed_by_at_is_delim() {
        // §4.3.1 L824-826: `#` followed by `@` (not ident code point,
        // not valid escape) → delim-token with value `#`.
        let tokens = CssTokenizer::collect("#@");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0], Token::Delim('#')));
        assert!(matches!(tokens[1], Token::Delim('@')));
    }

    // ── C-3 tests: §4.3.5 string + bad-string ────────────────────────

    #[test]
    fn string_double_quoted() {
        // §4.3.5: "hello" → String("hello")
        let tokens = CssTokenizer::collect("\"hello\"");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::String(s) if s == "hello"));
    }

    #[test]
    fn string_single_quoted() {
        // §4.3.5: 'hello' → String("hello")
        let tokens = CssTokenizer::collect("'hello'");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::String(s) if s == "hello"));
    }

    #[test]
    fn string_with_escape() {
        // §4.3.5 + §4.3.7: "a\z" → String("az") (z is non-hex → literal)
        let tokens = CssTokenizer::collect("\"a\\z\"");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::String(s) if s == "az"));
    }

    #[test]
    fn string_unterminated_eof() {
        // §4.3.5 L1101-1104: EOF before closing quote → parse error,
        // return string-token with value accumulated so far.
        let tokens = CssTokenizer::collect("\"hello");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::String(s) if s == "hello"));
    }

    #[test]
    fn string_unescaped_newline_is_bad_string() {
        // §4.3.5 L1106-1110: unescaped newline → parse error, reconsume,
        // return bad-string. The reconsumed newline is then tokenized as
        // whitespace by the next consume_a_token call.
        let tokens = CssTokenizer::collect("\"\n");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0], Token::BadString));
        assert!(matches!(tokens[1], Token::Whitespace));
    }

    #[test]
    fn string_line_continuation() {
        // §4.3.5 L1117-1119: backslash followed by newline → line
        // continuation (newline consumed, no char appended).
        let tokens = CssTokenizer::collect("\"a\\\nb\"");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::String(s) if s == "ab"));
    }

    #[test]
    fn string_hex_escape() {
        // §4.3.5 + §4.3.7: "\26" → String("&") (U+0026)
        let tokens = CssTokenizer::collect("\"\\26\"");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::String(s) if s == "&"));
    }

    // ── C-4 tests: §4.3.3/§4.3.10/§4.3.13 number/percentage/dimension ─

    #[test]
    fn number_integer() {
        let tokens = CssTokenizer::collect("42");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Number(Numeric { value: 42.0, is_integer: true }));
    }

    #[test]
    fn number_decimal() {
        let tokens = CssTokenizer::collect("3.5");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Number(Numeric { value: 3.5, is_integer: false }));
    }

    #[test]
    fn number_signed() {
        let tokens = CssTokenizer::collect("-5");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Number(Numeric { value: -5.0, is_integer: true }));
    }

    #[test]
    fn number_exponent() {
        // §4.3.13: 1e3 → 1.0 * 10^3 = 1000.0, type "number"
        let tokens = CssTokenizer::collect("1e3");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Number(Numeric { value: 1000.0, is_integer: false }));
    }

    #[test]
    fn number_decimal_exponent() {
        // §4.3.13: 1.5e2 → 1.5 * 10^2 = 150.0, type "number"
        let tokens = CssTokenizer::collect("1.5e2");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Number(Numeric { value: 150.0, is_integer: false }));
    }

    #[test]
    fn percentage_token() {
        let tokens = CssTokenizer::collect("50%");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Percentage(Numeric { value: 50.0, is_integer: true }));
    }

    #[test]
    fn dimension_px() {
        let tokens = CssTokenizer::collect("10px");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Dimension(
            Numeric { value: 10.0, is_integer: true },
            "px".to_string(),
        ));
    }

    #[test]
    fn dimension_em() {
        let tokens = CssTokenizer::collect("1.5em");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Dimension(
            Numeric { value: 1.5, is_integer: false },
            "em".to_string(),
        ));
    }

    #[test]
    fn dimension_signed() {
        let tokens = CssTokenizer::collect("-30deg");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Dimension(
            Numeric { value: -30.0, is_integer: true },
            "deg".to_string(),
        ));
    }

    #[test]
    fn plus_sign_number() {
        let tokens = CssTokenizer::collect("+5");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Number(Numeric { value: 5.0, is_integer: true }));
    }

    #[test]
    fn dot_starts_number() {
        let tokens = CssTokenizer::collect(".5");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Number(Numeric { value: 0.5, is_integer: false }));
    }

    // ── C-5 tests: §4.3.4 url( special case + §4.3.6 Url + §4.3.15 BadUrl ─

    #[test]
    fn url_unquoted_simple() {
        // §4.3.6: url(foo) → Url("foo")
        let tokens = CssTokenizer::collect("url(foo)");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Url("foo".to_string()));
    }

    #[test]
    fn url_unquoted_with_spaces() {
        // §4.3.6: leading/trailing whitespace consumed; url( foo ) → Url("foo")
        let tokens = CssTokenizer::collect("url( foo )");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Url("foo".to_string()));
    }

    #[test]
    fn url_empty() {
        // §4.3.6: url() → Url("")
        let tokens = CssTokenizer::collect("url()");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Url(String::new()));
    }

    #[test]
    fn url_eof_unterminated() {
        // §4.3.6 L1161-1164: EOF → parse error, return url-token with value so far
        let tokens = CssTokenizer::collect("url(foo");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Url("foo".to_string()));
    }

    #[test]
    fn url_quoted_is_function() {
        // §4.3.4 L1058-1063: url("foo") → Function("url"); the body
        // `"foo")` remains in the stream → String("foo") + CloseParen.
        let tokens = CssTokenizer::collect("url(\"foo\")");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0], Token::Function("url".to_string()));
        assert_eq!(tokens[1], Token::String("foo".to_string()));
        assert_eq!(tokens[2], Token::CloseParen);
    }

    #[test]
    fn url_single_quoted_is_function() {
        // §4.3.4 L1058-1063: url('foo') → Function("url") + String + CloseParen
        let tokens = CssTokenizer::collect("url('foo')");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0], Token::Function("url".to_string()));
        assert_eq!(tokens[1], Token::String("foo".to_string()));
        assert_eq!(tokens[2], Token::CloseParen);
    }

    #[test]
    fn url_ws_then_quote_is_function() {
        // §4.3.4 L1058-1063: url( "foo") → whitespace+quote → Function.
        // L1056-1057 only collapses when *two* consecutive whitespace
        // follow `(`; here only one space precedes `"`, so it is NOT
        // consumed and becomes a Whitespace token.
        let tokens = CssTokenizer::collect("url( \"foo\")");
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0], Token::Function("url".to_string()));
        assert_eq!(tokens[1], Token::Whitespace);
        assert_eq!(tokens[2], Token::String("foo".to_string()));
        assert_eq!(tokens[3], Token::CloseParen);
    }

    #[test]
    fn url_with_escape() {
        // §4.3.6 + §4.3.7: url(foo\29 bar) → \29 consumes trailing space,
        // decodes to `)`, then `bar` appended → Url("foo)bar")
        let tokens = CssTokenizer::collect("url(foo\\29 bar)");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Url("foo)bar".to_string()));
    }

    #[test]
    fn url_bad_paren_in_unquoted() {
        // §4.3.6 L1177-1185: `(` in unquoted url → BadUrl
        let tokens = CssTokenizer::collect("url(foo(bar)");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::BadUrl);
    }

    #[test]
    fn url_bad_quote_in_unquoted() {
        // §4.3.6 L1177-1185: `"` in unquoted url → BadUrl
        let tokens = CssTokenizer::collect("url(foo\"bar)");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::BadUrl);
    }

    // ── C-6 tests: §4.3.1 unicode_ranges_allowed + §4.3.11 + §4.3.14 ──

    #[test]
    fn unicode_range_disabled_by_default() {
        // §4.3.1 L782-783: default unicode_ranges_allowed=false.
        // `U+1234` → `U` ident-start → Ident("U") (`+` stops ident seq);
        // then `+1234` → starts_with_number (`+`+digit) → Number(1234).
        let tokens = CssTokenizer::collect("U+1234");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], Token::Ident("U".to_string()));
        assert_eq!(
            tokens[1],
            Token::Number(Numeric { value: 1234.0, is_integer: true })
        );
    }

    #[test]
    fn unicode_range_simple() {
        // §4.3.14: U+1234 → UnicodeRange(0x1234, 0x1234)
        let mut tz = CssTokenizer::new("U+1234");
        tz.set_unicode_ranges_allowed(true);
        let t = tz.next_token().unwrap();
        assert_eq!(t, Token::UnicodeRange(Some(0x1234), Some(0x1234)));
    }

    #[test]
    fn unicode_range_question_marks() {
        // §4.3.14 step 3: U+12?? → start=0x1200 (?→0), end=0x12FF (?→F)
        let mut tz = CssTokenizer::new("U+12??");
        tz.set_unicode_ranges_allowed(true);
        let t = tz.next_token().unwrap();
        assert_eq!(t, Token::UnicodeRange(Some(0x1200), Some(0x12FF)));
    }

    #[test]
    fn unicode_range_range() {
        // §4.3.14 step 5: U+12-34FF → start=0x12, end=0x34FF
        let mut tz = CssTokenizer::new("U+12-34FF");
        tz.set_unicode_ranges_allowed(true);
        let t = tz.next_token().unwrap();
        assert_eq!(t, Token::UnicodeRange(Some(0x12), Some(0x34FF)));
    }

    #[test]
    fn unicode_range_max_hex() {
        // §4.3.14: U+10FFFF → UnicodeRange(0x10FFFF, 0x10FFFF)
        let mut tz = CssTokenizer::new("U+10FFFF");
        tz.set_unicode_ranges_allowed(true);
        let t = tz.next_token().unwrap();
        assert_eq!(t, Token::UnicodeRange(Some(0x10FFFF), Some(0x10FFFF)));
    }

    #[test]
    fn unicode_range_lowercase_u() {
        // §4.3.11: lowercase u also triggers; u+abc → UnicodeRange(0xABC, 0xABC)
        let mut tz = CssTokenizer::new("u+abc");
        tz.set_unicode_ranges_allowed(true);
        let t = tz.next_token().unwrap();
        assert_eq!(t, Token::UnicodeRange(Some(0xABC), Some(0xABC)));
    }
}
