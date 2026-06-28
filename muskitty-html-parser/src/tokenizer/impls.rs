//! Concrete [`HtmlTokenizer`] implementation of the [`Tokenizer`] trait.
//!
//! The tokenizer processes one code point per `next_token()` call,
//! following the state machine defined in WHATWG §13.2.5.

use super::trait_def::Tokenizer;
use super::types::{State, Token};

/// A concrete HTML tokenizer.
///
/// Consumes a sequence of Unicode code points and emits [`Token`]s
/// according to the WHATWG tokenization state machine (§13.2.5).
///
/// # Usage
///
/// ```ignore
/// let mut t = HtmlTokenizer::new("<p>hello</p>");
/// while let Some(token) = t.next_token() {
///     // process token
/// }
/// ```
///
/// After `Token::EOF` is emitted, subsequent calls return `None`.
pub struct HtmlTokenizer {
    /// Input code points.
    input: Vec<char>,
    /// Current position in `input`.
    pos: usize,
    /// Current tokenizer state (§13.2.5).
    state: State,
    /// Whether `Token::EOF` has been emitted.
    eof_emitted: bool,
}

impl HtmlTokenizer {
    /// Create a new tokenizer from a string input.
    ///
    /// The tokenizer starts in [`State::Data`] (§13.2.5.1).
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
            state: State::Data,
            eof_emitted: false,
        }
    }

    /// Peek at the current input character without consuming it.
    fn current_char(&self) -> Option<char> {
        if self.pos < self.input.len() {
            Some(self.input[self.pos])
        } else {
            None
        }
    }

    /// Consume and return the current input character.
    fn consume(&mut self) -> Option<char> {
        let c = self.current_char();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }
}

impl Tokenizer for HtmlTokenizer {
    fn next_token(&mut self) -> Option<Token> {
        // After EOF has been emitted, the stream is exhausted.
        if self.eof_emitted {
            return None;
        }

        match self.state {
            State::Data => self.handle_data_state(),
            _ => panic!(
                "State::{:?} is not yet implemented (TODO in types.rs)",
                self.state
            ),
        }
    }

    fn set_state(&mut self, state: State) {
        self.state = state;
    }

    fn state(&self) -> State {
        self.state
    }

    fn reset(&mut self) {
        self.pos = 0;
        self.state = State::Data;
        self.eof_emitted = false;
    }
}

// ── State handlers ────────────────────────────────────────────────

impl HtmlTokenizer {
    /// §13.2.5.1 Data state
    ///
    /// Consume the next input character:
    /// - U+0026 AMPERSAND (&) → switch to character reference state
    /// - U+003C LESS-THAN SIGN (<) → switch to tag open state
    /// - U+0000 NULL → parse error (unexpected-null-character);
    ///   emit the current input character as a character token
    /// - EOF → emit an end-of-file token
    /// - Anything else → emit the current input character as a character token
    fn handle_data_state(&mut self) -> Option<Token> {
        match self.consume() {
            Some('&') => {
                // TODO: set return state to Data, then switch to CharacterReference
                self.state = State::CharacterReference;
                None // no token emitted yet — CharacterReference will emit one
            }
            Some('<') => {
                self.state = State::TagOpen;
                None // no token emitted yet — TagOpen will emit one
            }
            Some('\0') => {
                // §13.2.5.1: unexpected-null-character parse error.
                // Emit the current input character as a character token.
                // TODO: record parse error (unexpected-null-character)
                Some(Token::Character('\0'))
            }
            Some(c) => {
                // Any other character: emit as a character token.
                Some(Token::Character(c))
            }
            None => {
                // EOF: emit end-of-file token.
                self.eof_emitted = true;
                Some(Token::EOF)
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_state_emits_character_for_letter() {
        let mut t = HtmlTokenizer::new("a");
        let token = t.next_token();
        assert_eq!(token, Some(Token::Character('a')));
        assert_eq!(t.state(), State::Data);
    }

    #[test]
    fn data_state_switches_to_tag_open_on_less_than() {
        let mut t = HtmlTokenizer::new("<");
        let token = t.next_token();
        assert_eq!(token, None); // no token emitted — state changed
        assert_eq!(t.state(), State::TagOpen);
    }

    #[test]
    fn data_state_switches_to_character_reference_on_ampersand() {
        let mut t = HtmlTokenizer::new("&");
        let token = t.next_token();
        assert_eq!(token, None); // no token emitted — state changed
        assert_eq!(t.state(), State::CharacterReference);
    }

    #[test]
    fn data_state_emits_eof_on_empty_input() {
        let mut t = HtmlTokenizer::new("");
        let token = t.next_token();
        assert_eq!(token, Some(Token::EOF));

        // Subsequent call → None (stream exhausted)
        let token2 = t.next_token();
        assert_eq!(token2, None);
    }

    #[test]
    fn data_state_emits_eof_after_last_char() {
        let mut t = HtmlTokenizer::new("x");
        let first = t.next_token();
        assert_eq!(first, Some(Token::Character('x')));
        assert_eq!(t.state(), State::Data);

        let second = t.next_token();
        assert_eq!(second, Some(Token::EOF));
    }

    #[test]
    fn data_state_handles_null_character() {
        let mut t = HtmlTokenizer::new("\0");
        let token = t.next_token();
        // §13.2.5.1: U+0000 NULL emits the current input character as a char token
        assert_eq!(token, Some(Token::Character('\0')));
        assert_eq!(t.state(), State::Data);
    }

    #[test]
    fn data_state_emits_multiple_characters() {
        let mut t = HtmlTokenizer::new("abc");
        assert_eq!(t.next_token(), Some(Token::Character('a')));
        assert_eq!(t.next_token(), Some(Token::Character('b')));
        assert_eq!(t.next_token(), Some(Token::Character('c')));
        assert_eq!(t.next_token(), Some(Token::EOF));
        assert_eq!(t.next_token(), None); // stream done
    }
}
