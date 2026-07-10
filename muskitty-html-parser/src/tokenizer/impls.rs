//! Concrete [`HtmlTokenizer`] implementation of the [`Tokenizer`] trait.
//!
//! The tokenizer processes one code point per `next_token()` call,
//! following the state machine defined in WHATWG §13.2.5.

use super::trait_def::Tokenizer;
use super::types::{State, TagKind, TagToken, Token};

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
    /// When true, the next call to `next_char()` returns the current character
    /// without advancing `pos`. Used by states that "reconsume" the character
    /// in a different state (§13.2.5 convention).
    reconsume: bool,
    /// The tag token currently being built, if any.
    /// Set when entering TagName state, emitted when tag is complete.
    current_tag: Option<TagToken>,
    /// The comment data currently being accumulated.
    current_comment: String,
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
            reconsume: false,
            current_tag: None,
            current_comment: String::new(),
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

    /// Consume and return the current input character, advancing `pos`.
    fn consume(&mut self) -> Option<char> {
        let c = self.current_char();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    /// Return the next input character, respecting the reconsume flag.
    ///
    /// If [`reconsume`](Self::reconsume) is true, returns the previously
    /// consumed character (at `pos - 1`) and clears the flag without
    /// advancing. Otherwise, consumes and advances as usual.
    ///
    /// This implements the "reconsume the current input character" convention
    /// used throughout §13.2.5.
    fn next_char(&mut self) -> Option<char> {
        if self.reconsume {
            self.reconsume = false;
            // The character to reconsume was already consumed — it's at pos-1.
            if self.pos > 0 && self.pos <= self.input.len() {
                Some(self.input[self.pos - 1])
            } else {
                None
            }
        } else {
            self.consume()
        }
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
            State::TagOpen => self.handle_tag_open_state(),
            State::EndTagOpen => self.handle_end_tag_open_state(),
            State::TagName => self.handle_tag_name_state(),
            State::SelfClosingStartTag => self.handle_self_closing_start_tag_state(),
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
        self.reconsume = false;
        self.current_tag = None;
        self.current_comment.clear();
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
        match self.next_char() {
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

    /// §13.2.5.6 Tag open state
    ///
    /// Consume the next input character:
    /// - `!` → switch to markup declaration open state
    /// - `/` → switch to end tag open state
    /// - ASCII alpha → create a new start tag token with the current
    ///   character as its tag name, switch to tag name state
    /// - `?` → parse error; create a comment token (data = "?"), switch
    ///   to bogus comment state
    /// - EOF → parse error; emit `<` character token + EOF
    /// - Anything else → parse error; emit `<` character token, reconsume
    ///   the current character in the data state
    fn handle_tag_open_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('!') => {
                self.state = State::MarkupDeclarationOpen;
                None
            }
            Some('/') => {
                self.state = State::EndTagOpen;
                None
            }
            Some(c) if c.is_ascii_alphabetic() => {
                let mut name = String::new();
                name.push(c.to_ascii_lowercase());
                self.current_tag = Some(TagToken {
                    kind: TagKind::Start,
                    name,
                    attrs: Vec::new(),
                    self_closing: false,
                });
                self.state = State::TagName;
                None
            }
            Some('?') => {
                self.current_comment.clear();
                self.current_comment.push('?');
                self.state = State::BogusComment;
                None
            }
            Some(_c) => {
                self.state = State::Data;
                self.reconsume = true;
                Some(Token::Character('<'))
            }
            None => {
                self.state = State::Data;
                Some(Token::Character('<'))
            }
        }
    }

    /// §13.2.5.7 End tag open state
    ///
    /// Consume the next input character:
    /// - ASCII alpha → create a new end tag token, set tag name to empty string,
    ///   append lowercased char to name, switch to tag name state
    /// - Anything else → switch to data state (don't emit, don't reconsume)
    /// - EOF → emit `<` character token + EOF, switch to data state
    fn handle_end_tag_open_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some(c) if c.is_ascii_alphabetic() => {
                let mut name = String::new();
                name.push(c.to_ascii_lowercase());
                self.current_tag = Some(TagToken {
                    kind: TagKind::End,
                    name,
                    attrs: Vec::new(),
                    self_closing: false,
                });
                self.state = State::TagName;
                None
            }
            None => {
                // EOF: emit `<` then return to Data (next call emits EOF)
                self.state = State::Data;
                Some(Token::Character('<'))
            }
            Some(_c) => {
                // Consume the character, switch to Data, don't emit anything.
                self.state = State::Data;
                None
            }
        }
    }

    /// §13.2.5.8 Tag name state
    ///
    /// Consume the next input character:
    /// - ASCII alpha/upper → append lowercase to tag name, stay in TagName
    /// - NULL → append U+FFFD, stay in TagName
    /// - TAB/LF/FF/SPACE → switch to BeforeAttributeName
    /// - `/` → switch to SelfClosingStartTag
    /// - `>` → emit current tag token, switch to Data
    /// - EOF → discard tag, emit EOF
    /// - Anything else → append to tag name, stay in TagName
    fn handle_tag_name_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some(c) if c.is_ascii_alphabetic() => {
                if let Some(ref mut tag) = self.current_tag {
                    tag.name.push(c.to_ascii_lowercase());
                }
                self.state = State::TagName;
                None
            }
            Some('\0') => {
                // unexpected-null-character parse error
                if let Some(ref mut tag) = self.current_tag {
                    tag.name.push('\u{FFFD}');
                }
                self.state = State::TagName;
                None
            }
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                self.state = State::BeforeAttributeName;
                None
            }
            Some('/') => {
                self.state = State::SelfClosingStartTag;
                None
            }
            Some('>') => {
                let tag = self.current_tag.take().unwrap();
                self.state = State::Data;
                Some(Token::Tag(tag))
            }
            None => {
                // EOF: discard incomplete tag, emit EOF
                self.current_tag = None;
                self.eof_emitted = true;
                Some(Token::EOF)
            }
            Some(c) => {
                if let Some(ref mut tag) = self.current_tag {
                    tag.name.push(c);
                }
                self.state = State::TagName;
                None
            }
        }
    }

    /// §13.2.5.40 Self-closing start tag state
    ///
    /// Consume the next input character:
    /// - `>` → set self_closing flag, emit current tag token, switch to Data
    /// - Anything else → parse error, switch to BeforeAttributeName, reconsume
    /// - EOF → discard tag, emit EOF
    fn handle_self_closing_start_tag_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('>') => {
                let mut tag = self.current_tag.take().unwrap();
                tag.self_closing = true;
                self.state = State::Data;
                Some(Token::Tag(tag))
            }
            None => {
                // EOF: discard incomplete tag, emit EOF
                self.current_tag = None;
                self.eof_emitted = true;
                Some(Token::EOF)
            }
            Some(_c) => {
                // unexpected-solidus-in-tag parse error
                self.state = State::BeforeAttributeName;
                self.reconsume = true;
                None
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

    // ── TagName tests (§13.2.5.8) ──────────────────────────────

    /// Helper: create a tokenizer in TagName state with a start tag already built.
    fn enter_tag_name(input: &str) -> HtmlTokenizer {
        let mut t = HtmlTokenizer::new(input);
        // Data → TagOpen (on `<`)
        assert_eq!(t.next_token(), None);
        assert_eq!(t.state(), State::TagOpen);
        // TagOpen → creates start tag + TagName (on first alpha)
        assert_eq!(t.next_token(), None);
        assert_eq!(t.state(), State::TagName);
        // Verify the tag name was initialized with the first char
        // (we can't check current_tag directly since it's private, but we
        // trust TagOpen's existing test covers this)
        t
    }

    #[test]
    fn tag_name_emits_start_tag_on_greater_than() {
        // `<a>` should emit a start tag token with name "a"
        let mut t = enter_tag_name("<a>");
        let token = t.next_token();
        assert_eq!(
            token,
            Some(Token::Tag(TagToken {
                kind: TagKind::Start,
                name: "a".into(),
                attrs: Vec::new(),
                self_closing: false,
            }))
        );
        assert_eq!(t.state(), State::Data);
    }

    #[test]
    fn tag_name_emits_tag_with_lowercased_name() {
        // `<DIV>` → tag name should be "div"
        let mut t = HtmlTokenizer::new("<DIV>");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.state(), State::TagOpen);
        assert_eq!(t.next_token(), None); // TagOpen → TagName, name="d"
        assert_eq!(t.state(), State::TagName);
        assert_eq!(t.next_token(), None); // 'I' → append 'i'
        assert_eq!(t.state(), State::TagName);
        assert_eq!(t.next_token(), None); // 'V' → append 'v'
        assert_eq!(t.state(), State::TagName);
        // '>' → emit tag
        let token = t.next_token();
        assert_eq!(
            token,
            Some(Token::Tag(TagToken {
                kind: TagKind::Start,
                name: "div".into(),
                attrs: Vec::new(),
                self_closing: false,
            }))
        );
        assert_eq!(t.state(), State::Data);
    }

    #[test]
    fn tag_name_appends_lowercased_uppercase_chars() {
        // `<AbC>` → tag name should be "abc"
        let mut t = HtmlTokenizer::new("<AbC>");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.state(), State::TagOpen);
        assert_eq!(t.next_token(), None); // TagOpen → TagName, name="a"
        assert_eq!(t.state(), State::TagName);
        assert_eq!(t.next_token(), None); // 'b' → append, still TagName
        assert_eq!(t.state(), State::TagName);
        assert_eq!(t.next_token(), None); // 'C' → append 'c', still TagName
        assert_eq!(t.state(), State::TagName);
        // '>' → emit tag
        let token = t.next_token();
        assert_eq!(
            token,
            Some(Token::Tag(TagToken {
                kind: TagKind::Start,
                name: "abc".into(),
                attrs: Vec::new(),
                self_closing: false,
            }))
        );
    }

    #[test]
    fn tag_name_switches_to_self_closing_on_solidus() {
        // `<br/>` → after "br", '/' switches to SelfClosingStartTag
        let mut t = HtmlTokenizer::new("<br/>");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.state(), State::TagOpen);
        assert_eq!(t.next_token(), None); // TagOpen → TagName, name="b"
        assert_eq!(t.state(), State::TagName);
        assert_eq!(t.next_token(), None); // 'r' appended, still TagName
        assert_eq!(t.state(), State::TagName);
        assert_eq!(t.next_token(), None); // '/' → SelfClosingStartTag
        assert_eq!(t.state(), State::SelfClosingStartTag);
        // Now '/' is consumed, next char is '>'
        let token = t.next_token();
        assert_eq!(
            token,
            Some(Token::Tag(TagToken {
                kind: TagKind::Start,
                name: "br".into(),
                attrs: Vec::new(),
                self_closing: true,
            }))
        );
        assert_eq!(t.state(), State::Data);
    }

    // ── EndTagOpen tests (§13.2.5.7) ─────────────────────────────

    #[test]
    fn end_tag_open_creates_end_tag_on_alpha() {
        // `</div>`: `<` → TagOpen, `/` → EndTagOpen, `d` → creates end tag + TagName
        let mut t = HtmlTokenizer::new("</div>");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.state(), State::TagOpen);
        assert_eq!(t.next_token(), None); // TagOpen → EndTagOpen
        assert_eq!(t.state(), State::EndTagOpen);
        assert_eq!(t.next_token(), None); // EndTagOpen → TagName, name="d"
        assert_eq!(t.state(), State::TagName);
        assert_eq!(t.next_token(), None); // 'i' → append
        assert_eq!(t.next_token(), None); // 'v' → append
        let token = t.next_token(); // '>' → emit
        assert_eq!(
            token,
            Some(Token::Tag(TagToken {
                kind: TagKind::End,
                name: "div".into(),
                attrs: Vec::new(),
                self_closing: false,
            }))
        );
        assert_eq!(t.state(), State::Data);
    }

    #[test]
    fn end_tag_open_lowercases_tag_name() {
        let mut t = HtmlTokenizer::new("</DIV>");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → EndTagOpen
        assert_eq!(t.next_token(), None); // EndTagOpen → TagName, name="d"
        assert_eq!(t.next_token(), None); // 'i'
        assert_eq!(t.next_token(), None); // 'v'
        let token = t.next_token(); // '>' → emit
        assert_eq!(
            token,
            Some(Token::Tag(TagToken {
                kind: TagKind::End,
                name: "div".into(),
                attrs: Vec::new(),
                self_closing: false,
            }))
        );
        assert_eq!(t.state(), State::Data);
    }

    #[test]
    fn end_tag_open_non_alpha_switches_to_data() {
        // `</>` → not alpha, switch to Data, don't emit anything
        let mut t = HtmlTokenizer::new("</>");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → EndTagOpen
        // EndTagOpen sees `>`: not alpha → switch to Data, no emit
        let token = t.next_token();
        assert_eq!(token, None); // nothing emitted
        assert_eq!(t.state(), State::Data);
        // The '>' was consumed by EndTagOpen (not re-consumed), so next char is EOF
        let token2 = t.next_token();
        assert_eq!(token2, Some(Token::EOF));
    }

    #[test]
    fn end_tag_open_eof_emits_lt_then_eof() {
        // `</` + EOF → emit `<`, return to Data, then emit EOF
        let mut t = HtmlTokenizer::new("</");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → EndTagOpen
        let token = t.next_token();
        assert_eq!(token, Some(Token::Character('<')));
        assert_eq!(t.state(), State::Data);
        let token2 = t.next_token();
        assert_eq!(token2, Some(Token::EOF));
    }

    // ── End-to-end integration tests ─────────────────────────────

    #[test]
    fn e2e_simple_open_close_tag() {
        // `<p>hello</p>` → start tag, chars, end tag, EOF
        let mut t = HtmlTokenizer::new("<p>hello</p>");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → TagName, name="p"
        assert_eq!(
            t.next_token(),
            Some(Token::Tag(TagToken { kind: TagKind::Start, name: "p".into(), attrs: Vec::new(), self_closing: false }))
        ); // '>' → emit
        assert_eq!(t.next_token(), Some(Token::Character('h')));
        assert_eq!(t.next_token(), Some(Token::Character('e')));
        assert_eq!(t.next_token(), Some(Token::Character('l')));
        assert_eq!(t.next_token(), Some(Token::Character('l')));
        assert_eq!(t.next_token(), Some(Token::Character('o')));
        assert_eq!(t.next_token(), None); // '<' → TagOpen
        assert_eq!(t.next_token(), None); // '/' → EndTagOpen
        assert_eq!(t.next_token(), None); // 'p' → TagName, name="p"
        assert_eq!(
            t.next_token(),
            Some(Token::Tag(TagToken { kind: TagKind::End, name: "p".into(), attrs: Vec::new(), self_closing: false }))
        ); // '>' → emit
        assert_eq!(t.next_token(), Some(Token::EOF));
    }

    #[test]
    fn e2e_self_closing_tag() {
        // `<br/>` → self-closing start tag, EOF
        let mut t = HtmlTokenizer::new("<br/>");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → TagName, name="b"
        assert_eq!(t.next_token(), None); // 'r' → append
        assert_eq!(t.next_token(), None); // '/' → SelfClosingStartTag
        assert_eq!(
            t.next_token(),
            Some(Token::Tag(TagToken { kind: TagKind::Start, name: "br".into(), attrs: Vec::new(), self_closing: true }))
        ); // '>' → emit
        assert_eq!(t.next_token(), Some(Token::EOF));
    }

    #[test]
    fn e2e_tag_space_then_chars() {
        // `<div>text</div>` → start tag, text chars, end tag, EOF
        let mut t = HtmlTokenizer::new("<div>text</div>");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → TagName, name="d"
        assert_eq!(t.next_token(), None); // 'i'
        assert_eq!(t.next_token(), None); // 'v'
        assert_eq!(
            t.next_token(),
            Some(Token::Tag(TagToken { kind: TagKind::Start, name: "div".into(), attrs: Vec::new(), self_closing: false }))
        ); // '>' → emit
        assert_eq!(t.next_token(), Some(Token::Character('t')));
        assert_eq!(t.next_token(), Some(Token::Character('e')));
        assert_eq!(t.next_token(), Some(Token::Character('x')));
        assert_eq!(t.next_token(), Some(Token::Character('t')));
        assert_eq!(t.next_token(), None); // '<' → TagOpen
        assert_eq!(t.next_token(), None); // '/' → EndTagOpen
        assert_eq!(t.next_token(), None); // 'd' → TagName, name="d"
        assert_eq!(t.next_token(), None); // 'i'
        assert_eq!(t.next_token(), None); // 'v'
        assert_eq!(
            t.next_token(),
            Some(Token::Tag(TagToken { kind: TagKind::End, name: "div".into(), attrs: Vec::new(), self_closing: false }))
        ); // '>' → emit
        assert_eq!(t.next_token(), Some(Token::EOF));
    }

    #[test]
    fn tag_name_switches_to_before_attribute_name_on_space() {
        // `<div class="x">` → space switches to BeforeAttributeName
        let mut t = HtmlTokenizer::new("<div class=\"x\">");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.state(), State::TagOpen);
        assert_eq!(t.next_token(), None); // TagOpen → TagName, name="d"
        assert_eq!(t.state(), State::TagName);
        assert_eq!(t.next_token(), None); // 'i' → append
        assert_eq!(t.state(), State::TagName);
        assert_eq!(t.next_token(), None); // 'v' → append
        assert_eq!(t.state(), State::TagName);
        assert_eq!(t.next_token(), None); // ' ' → BeforeAttributeName
        assert_eq!(t.state(), State::BeforeAttributeName);
    }

    #[test]
    fn tag_name_appends_non_ascii_chars() {
        // Non-ASCII characters in TagOpen fall through to Data (correct per spec).
        // `<日本語>` → '<' is emitted as text, then Japanese chars are character tokens.
        let mut t = HtmlTokenizer::new("<日本語>");
        assert_eq!(t.next_token(), None); // Data → TagOpen (no emit)
        assert_eq!(t.state(), State::TagOpen);
        assert_eq!(t.next_token(), Some(Token::Character('<'))); // TagOpen: not alpha, emit '<', reconsume
        assert_eq!(t.state(), State::Data);
        assert_eq!(t.next_token(), Some(Token::Character('日'))); // re-consumed in Data
        assert_eq!(t.next_token(), Some(Token::Character('本')));
        assert_eq!(t.next_token(), Some(Token::Character('語')));
        assert_eq!(t.next_token(), Some(Token::Character('>')));
    }

    #[test]
    fn tag_name_appends_non_ascii_after_entering_tag_name() {
        // Non-ASCII chars ARE appended to the tag name once we're in TagName state.
        // `<a日本語>`: 'a' enters TagName, then '日', '本', '語' are appended.
        let mut t = HtmlTokenizer::new("<a日本語>");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.state(), State::TagOpen);
        assert_eq!(t.next_token(), None); // TagOpen → TagName, name="a"
        assert_eq!(t.state(), State::TagName);
        assert_eq!(t.next_token(), None); // '日' → append
        assert_eq!(t.state(), State::TagName);
        assert_eq!(t.next_token(), None); // '本' → append
        assert_eq!(t.state(), State::TagName);
        assert_eq!(t.next_token(), None); // '語' → append
        assert_eq!(t.state(), State::TagName);
        // '>' → emit tag with name "a日本語"
        let token = t.next_token();
        assert_eq!(
            token,
            Some(Token::Tag(TagToken {
                kind: TagKind::Start,
                name: "a日本語".into(),
                attrs: Vec::new(),
                self_closing: false,
            }))
        );
        assert_eq!(t.state(), State::Data);
    }

    #[test]
    fn tag_name_handles_null_character() {
        // `<a\x00>` → NULL in TagName should append U+FFFD, then '>' emits tag
        let mut t = HtmlTokenizer::new("<a\x00>");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.state(), State::TagOpen);
        assert_eq!(t.next_token(), None); // TagOpen → TagName, name="a"
        assert_eq!(t.state(), State::TagName);
        // '\0' in TagName: append U+FFFD (parse error), stay in TagName
        assert_eq!(t.next_token(), None);
        assert_eq!(t.state(), State::TagName);
        // '>' → emit tag with name "a\u{FFFD}"
        let token = t.next_token();
        assert_eq!(
            token,
            Some(Token::Tag(TagToken {
                kind: TagKind::Start,
                name: "a\u{FFFD}".into(),
                attrs: Vec::new(),
                self_closing: false,
            }))
        );
        assert_eq!(t.state(), State::Data);
    }
}
