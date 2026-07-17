//! CSS Syntax tokenizer implementation.
//!
//! Implements the "Tokenizer Algorithms" of CSS Syntax Module Level 3 §4.3.
//! The tokenizer is recursive-descent: [`CssTokenizer::next_token`] calls
//! [`consume_a_token`](CssTokenizer::consume_a_token) (§4.3.1), which
//! dispatches to sub-algorithms based on the current input code point.
//!
//! # Current coverage
//!
//! - §4.3.1 Consume a token (Data-state dispatch) — partial, simple tokens only
//! - §4.3.2 Consume comments — not yet
//! - §4.3.3 Consume a numeric token — not yet
//! - §4.3.4 Consume an ident-like token — not yet
//! - §4.3.5 Consume a string token — not yet
//! - §4.3.6 Consume a url token — not yet
//! - §4.3.7 Consume an escaped code point — not yet
//! - §4.3.8 Check if two code points are a valid escape — not yet
//! - §4.3.9 Check if three code points would start an ident sequence — not yet
//! - §4.3.10 Check if three code points would start a number — not yet
//! - §4.3.11 Check if three code points would start a unicode-range — not yet
//! - §4.3.12 Consume an ident sequence — not yet
//! - §4.3.13 Consume a number — not yet
//! - §4.3.14 Consume a unicode-range token — not yet
//! - §4.3.15 Consume the remnants of a bad url — not yet

use super::trait_def::Tokenizer;
use super::types::{State, Token};

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
    /// through §4.3.6) or emits a simple token directly.
    ///
    /// **Current coverage** (C-0 + C-1): whitespace, colon, semicolon,
    /// comma, brackets, EOF, comments (§4.3.2), legacy `<!--` / `-->`
    /// comment forms. Other code points dispatch to sub-algorithm stubs
    /// (C-2 onwards) or emit `<delim-token>`.
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
                    } else {
                        // would-start-an-ident-sequence handling is C-2.
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

    /// §4.3.5 Consume a string token.
    ///
    /// `quote` is the opening quote (`"` or `'`).
    fn consume_a_string_token(&mut self, quote: char) -> Token {
        // C-3: full implementation pending. For now consume until
        // matching quote / newline / EOF.
        let mut value = String::new();
        loop {
            match self.consume() {
                None => {
                    // EOF in string: parse error, return string token
                    // (§4.3.5 step 4).
                    return Token::String(value);
                }
                Some('\n') => {
                    // Unescaped newline: parse error, return <bad-string-token>
                    // (§4.3.5 step 3).
                    return Token::BadString;
                }
                Some(c) if c == quote => {
                    return Token::String(value);
                }
                Some('\\') => {
                    // §4.3.5 step: escape handling. C-3 pending.
                    // For now, just consume the next char literally.
                    if let Some(next) = self.consume() {
                        if next == '\n' {
                            // escaped newline: continue (line continuation)
                        } else {
                            value.push(next);
                        }
                    }
                }
                Some(c) => value.push(c),
            }
        }
    }

    /// §4.3.4 (hash branch) Consume a hash token.
    fn consume_a_hash_token(&mut self) -> Token {
        todo!("C-2: hash token")
    }

    /// §4.3.3 Consume a numeric token.
    fn consume_a_numeric_token(&mut self) -> Token {
        todo!("C-4: numeric token")
    }

    /// §4.3.4 Consume an ident-like token.
    fn consume_an_ident_like_token(&mut self) -> Token {
        todo!("C-2: ident-like token")
    }

    /// §4.3.4 (at-keyword branch) Consume an at-keyword token.
    fn consume_an_at_keyword_token(&mut self) -> Token {
        todo!("C-2: at-keyword token")
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

    /// §4.3.1 (u/U branch): consume an ident-like token, or if it matches
    /// the `U+` form, consume a unicode-range token (§4.3.14).
    fn consume_u_or_unicode_range(&mut self) -> Token {
        // C-6: full unicode-range detection pending. For now treat as
        // ident-like.
        self.reconsume();
        self.consume_an_ident_like_token()
    }

    // ── Predicates (stubs returning false; implemented in C-2/C-4/C-6) ─

    /// §4.3.10 Check if three code points would start a number.
    ///
    /// `first` is the code point already consumed (the caller passes it
    /// so the predicate can examine the next two without reconsume).
    fn starts_with_number(&self, _first: char) -> bool {
        false
    }

    /// §4.3.8 Check if the next code point starts a valid escape.
    ///
    /// The current code point is `\`; this returns true if the escape is
    /// valid (i.e. the next code point is not a newline and not EOF).
    fn is_valid_escape_next(&self) -> bool {
        // §4.3.8: "If the next input code point is ... EOF, return false.
        // Otherwise, return true." (Newline after `\` is not a valid
        // escape.)
        match self.peek(0) {
            None => false,
            Some('\n') => false,
            _ => true,
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

/// Allow `is_ident_code_point` to be used by future sub-algorithms.
#[allow(dead_code)]
fn _ident_code_point_used(c: char) -> bool {
    is_ident_code_point(c)
}

/// §4.2 Whether `c` is a digit (ASCII 0-9).
fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

/// §4.2 Whether `c` is a hex digit (ASCII 0-9, A-F, a-f).
fn is_hex_digit(c: char) -> bool {
    c.is_ascii_hexdigit()
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

// Allow helpers to be referenced from later commits without warnings.
#[allow(dead_code)]
fn _helpers_used() {
    let _ = (is_digit, is_hex_digit, _ident_code_point_used);
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
}
