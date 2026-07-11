//! Concrete [`HtmlTokenizer`] implementation of the [`Tokenizer`] trait.
//!
//! The tokenizer processes one code point per `next_token()` call,
//! following the state machine defined in WHATWG §13.2.5.

use super::trait_def::Tokenizer;
use super::types::{DoctypeToken, State, TagKind, TagToken, Token};

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
    /// The attribute name currently being accumulated (AttributeName state).
    current_attr_name: String,
    /// The attribute value currently being accumulated (attribute value states).
    current_attr_value: String,
    /// The DOCTYPE token currently being built (§13.2.5.53–§13.2.5.68).
    /// Reset when entering DOCTYPE states via MarkupDeclarationOpen.
    current_doctype: DoctypeToken,
    /// The tag name that can close the current RCDATA/RAWTEXT section.
    /// Set by tree construction before switching to RCDATA/RAWTEXT state.
    /// e.g. `Some("title")` when entering `<title>`, `Some("textarea")`
    /// when entering `<textarea>`.
    appropriate_end_tag_name: Option<String>,
    /// Temporary buffer for RCDATA/RAWTEXT end tag name accumulation.
    /// Stores original-case characters of the candidate end tag name.
    /// On match failure, these chars are emitted back as character tokens.
    temporary_buffer: String,
    /// Queue of tokens to emit before consuming the next input character.
    /// Used when a single state transition needs to emit multiple tokens
    /// (e.g. "anything else" in RCDATA/RAWTEXT end tag name).
    pending_tokens: Vec<Token>,
    /// The return state for character references (§13.2.5.72–§13.2.5.80).
    /// Set before entering CharacterReference state so the tokenizer knows
    /// where to return after resolving the reference.
    return_state: Option<State>,
    /// Accumulator for numeric character reference value
    /// (§13.2.5.78–§13.2.5.79).
    character_reference_code: u32,
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
            current_attr_name: String::new(),
            current_attr_value: String::new(),
            current_doctype: DoctypeToken {
                name: None,
                public_id: None,
                system_id: None,
                force_quirks: false,
            },
            appropriate_end_tag_name: None,
            temporary_buffer: String::new(),
            pending_tokens: Vec::new(),
            return_state: None,
            character_reference_code: 0,
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

    /// Set the appropriate end tag name for RCDATA/RAWTEXT content.
    ///
    /// The tree construction stage calls this before switching the tokenizer
    /// to [`State::RCDATA`] or [`State::RAWTEXT`]. In these content models,
    /// `</` followed by the appropriate end tag name closes the section and
    /// returns to [`State::Data`].
    ///
    /// Pass `None` to clear (e.g. when leaving RCDATA/RAWTEXT).
    ///
    /// # Examples
    ///
    /// - `<title>` → `set_appropriate_end_tag_name(Some("title"))`, then `set_state(State::RCDATA)`
    /// - `<style>` → `set_appropriate_end_tag_name(Some("style"))`, then `set_state(State::RAWTEXT)`
    pub fn set_appropriate_end_tag_name(&mut self, name: Option<&str>) {
        self.appropriate_end_tag_name = name.map(|s| s.to_string());
    }
}

impl Tokenizer for HtmlTokenizer {
    fn next_token(&mut self) -> Option<Token> {
        // After EOF has been emitted, the stream is exhausted.
        if self.eof_emitted {
            return None;
        }

        // Drain pending tokens (multi-token emission queue) first.
        if let Some(token) = self.pending_tokens.pop() {
            return Some(token);
        }

        let result = match self.state {
            State::Data => self.handle_data_state(),
            State::TagOpen => self.handle_tag_open_state(),
            State::EndTagOpen => self.handle_end_tag_open_state(),
            State::TagName => self.handle_tag_name_state(),
            State::SelfClosingStartTag => self.handle_self_closing_start_tag_state(),
            State::MarkupDeclarationOpen => self.handle_markup_declaration_open_state(),
            State::Doctype => self.handle_doctype_state(),
            State::BeforeDoctypeName => self.handle_before_doctype_name_state(),
            State::DoctypeName => self.handle_doctype_name_state(),
            State::AfterDoctypeName => self.handle_after_doctype_name_state(),
            State::AfterDoctypePublicKeyword => self.handle_after_doctype_public_keyword_state(),
            State::BeforeDoctypePublicId => self.handle_before_doctype_public_id_state(),
            State::DoctypePublicIdDoubleQuoted => self.handle_doctype_public_id_double_quoted_state(),
            State::DoctypePublicIdSingleQuoted => self.handle_doctype_public_id_single_quoted_state(),
            State::AfterDoctypePublicId => self.handle_after_doctype_public_id_state(),
            State::BetweenDoctypePublicAndSystemIds => self.handle_between_doctype_public_and_system_ids_state(),
            State::AfterDoctypeSystemKeyword => self.handle_after_doctype_system_keyword_state(),
            State::BeforeDoctypeSystemId => self.handle_before_doctype_system_id_state(),
            State::DoctypeSystemIdDoubleQuoted => self.handle_doctype_system_id_double_quoted_state(),
            State::DoctypeSystemIdSingleQuoted => self.handle_doctype_system_id_single_quoted_state(),
            State::AfterDoctypeSystemId => self.handle_after_doctype_system_id_state(),
            State::BogusDoctype => self.handle_bogus_doctype_state(),
            State::BogusComment => self.handle_bogus_comment_state(),
            State::CommentStart => self.handle_comment_start_state(),
            State::CommentStartDash => self.handle_comment_start_dash_state(),
            State::Comment => self.handle_comment_state(),
            State::CommentLessThanSign => self.handle_comment_less_than_sign_state(),
            State::CommentLessThanSignBang => self.handle_comment_less_than_sign_bang_state(),
            State::CommentLessThanSignBangDash => self.handle_comment_less_than_sign_bang_dash_state(),
            State::CommentLessThanSignBangDashDash => self.handle_comment_less_than_sign_bang_dash_dash_state(),
            State::CommentEndDash => self.handle_comment_end_dash_state(),
            State::CommentEnd => self.handle_comment_end_state(),
            State::CommentEndBang => self.handle_comment_end_bang_state(),
            State::BeforeAttributeName => self.handle_before_attribute_name_state(),
            State::AttributeName => self.handle_attribute_name_state(),
            State::AfterAttributeName => self.handle_after_attribute_name_state(),
            State::BeforeAttributeValue => self.handle_before_attribute_value_state(),
            State::AttributeValueDoubleQuoted => self.handle_attribute_value_double_quoted_state(),
            State::AttributeValueSingleQuoted => self.handle_attribute_value_single_quoted_state(),
            State::AttributeValueUnquoted => self.handle_attribute_value_unquoted_state(),
            State::AfterAttributeValueQuoted => self.handle_after_attribute_value_quoted_state(),
            State::RCDATA => self.handle_rcdata_state(),
            State::RAWTEXT => self.handle_rawtext_state(),
            State::PLAINTEXT => self.handle_plaintext_state(),
            State::RCDATALessThanSign => self.handle_rcdata_less_than_sign_state(),
            State::RCDATAEndTagOpen => self.handle_rcdata_end_tag_open_state(),
            State::RCDATAEndTagName => self.handle_rcdata_end_tag_name_state(),
            State::RAWTEXTLessThanSign => self.handle_rawtext_less_than_sign_state(),
            State::RAWTEXTEndTagOpen => self.handle_rawtext_end_tag_open_state(),
            State::RAWTEXTEndTagName => self.handle_rawtext_end_tag_name_state(),
            State::ScriptData => self.handle_script_data_state(),
            State::ScriptDataLessThanSign => self.handle_script_data_less_than_sign_state(),
            State::ScriptDataEndTagOpen => self.handle_script_data_end_tag_open_state(),
            State::ScriptDataEndTagName => self.handle_script_data_end_tag_name_state(),
            State::ScriptDataEscapeStart => self.handle_script_data_escape_start_state(),
            State::ScriptDataEscapeStartDash => self.handle_script_data_escape_start_dash_state(),
            State::ScriptDataEscaped => self.handle_script_data_escaped_state(),
            State::ScriptDataEscapedDash => self.handle_script_data_escaped_dash_state(),
            State::ScriptDataEscapedDashDash => self.handle_script_data_escaped_dash_dash_state(),
            State::ScriptDataEscapedLessThanSign => self.handle_script_data_escaped_less_than_sign_state(),
            State::ScriptDataEscapedEndTagOpen => self.handle_script_data_escaped_end_tag_open_state(),
            State::ScriptDataEscapedEndTagName => self.handle_script_data_escaped_end_tag_name_state(),
            State::ScriptDataDoubleEscapeStart => self.handle_script_data_double_escape_start_state(),
            State::ScriptDataDoubleEscaped => self.handle_script_data_double_escaped_state(),
            State::ScriptDataDoubleEscapedDash => self.handle_script_data_double_escaped_dash_state(),
            State::ScriptDataDoubleEscapedDashDash => self.handle_script_data_double_escaped_dash_dash_state(),
            State::ScriptDataDoubleEscapedLessThanSign => self.handle_script_data_double_escaped_less_than_sign_state(),
            State::ScriptDataDoubleEscapeEnd => self.handle_script_data_double_escape_end_state(),
            State::CharacterReference => self.handle_character_reference_state(),
            State::NumericCharacterReference => self.handle_numeric_character_reference_state(),
            State::HexCharacterReferenceStart => self.handle_hex_character_reference_start_state(),
            State::DecimalCharacterReferenceStart => self.handle_decimal_character_reference_start_state(),
            State::HexCharacterReference => self.handle_hex_character_reference_state(),
            State::DecimalCharacterReference => self.handle_decimal_character_reference_state(),
            State::NumericCharacterReferenceEnd => self.handle_numeric_character_reference_end_state(),
            State::NamedCharacterReference => self.handle_named_character_reference_state(),
            State::AmbiguousAmpersand => self.handle_ambiguous_ampersand_state(),
            _ => panic!(
                "State::{:?} is not yet implemented (TODO in types.rs)",
                self.state
            ),
        };
        result
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
        self.current_attr_name.clear();
        self.current_attr_value.clear();
        self.current_doctype = DoctypeToken {
            name: None,
            public_id: None,
            system_id: None,
            force_quirks: false,
        };
        self.appropriate_end_tag_name = None;
        self.temporary_buffer.clear();
        self.pending_tokens.clear();
        self.return_state = None;
        self.character_reference_code = 0;
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
                self.return_state = Some(State::Data);
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

    // ── Content model states (§13.2.5.2–§13.2.5.5) ──────────────────

    /// §13.2.5.2 RCDATA state
    ///
    /// Consume the next input character:
    /// - U+0026 AMPERSAND (&) → character reference state
    /// - U+003C LESS-THAN SIGN (<) → RCDATA less-than sign state
    /// - U+0000 NULL → parse error; emit U+FFFD character token
    /// - EOF → emit end-of-file token
    /// - Anything else → emit character token
    fn handle_rcdata_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('&') => {
                self.return_state = Some(State::RCDATA);
                self.state = State::CharacterReference;
                None
            }
            Some('<') => {
                self.state = State::RCDATALessThanSign;
                None
            }
            Some('\0') => {
                // TODO: record parse error (unexpected-null-character)
                Some(Token::Character('\u{FFFD}'))
            }
            Some(c) => Some(Token::Character(c)),
            None => {
                self.eof_emitted = true;
                Some(Token::EOF)
            }
        }
    }

    /// §13.2.5.3 RAWTEXT state
    ///
    /// Consume the next input character:
    /// - U+003C LESS-THAN SIGN (<) → RAWTEXT less-than sign state
    /// - U+0000 NULL → parse error; emit U+FFFD character token
    /// - EOF → emit end-of-file token
    /// - Anything else → emit character token
    ///
    /// Note: RAWTEXT does NOT handle `&` — character references are not resolved.
    fn handle_rawtext_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('<') => {
                self.state = State::RAWTEXTLessThanSign;
                None
            }
            Some('\0') => {
                // TODO: record parse error (unexpected-null-character)
                Some(Token::Character('\u{FFFD}'))
            }
            Some(c) => Some(Token::Character(c)),
            None => {
                self.eof_emitted = true;
                Some(Token::EOF)
            }
        }
    }

    /// §13.2.5.5 PLAINTEXT state
    ///
    /// Consume the next input character:
    /// - U+0000 NULL → parse error; emit U+FFFD character token
    /// - EOF → emit end-of-file token
    /// - Anything else → emit character token
    ///
    /// Note: PLAINTEXT treats EVERYTHING as literal text — even `<` and `&`.
    /// There is no end tag to close this state; it persists until EOF.
    fn handle_plaintext_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\0') => {
                // TODO: record parse error (unexpected-null-character)
                Some(Token::Character('\u{FFFD}'))
            }
            Some(c) => Some(Token::Character(c)),
            None => {
                self.eof_emitted = true;
                Some(Token::EOF)
            }
        }
    }

    // ── RCDATA tag detection states (§13.2.5.9–§13.2.5.11) ─────────

    /// §13.2.5.9 RCDATA less-than sign state
    ///
    /// Consume the next input character:
    /// - U+002F SOLIDUS (/) → clear temporary buffer, switch to RCDATA end tag open
    /// - Anything else → emit `<` character token, reconsume in RCDATA
    fn handle_rcdata_less_than_sign_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('/') => {
                self.temporary_buffer.clear();
                self.state = State::RCDATAEndTagOpen;
                None
            }
            Some(_c) => {
                self.state = State::RCDATA;
                self.reconsume = true;
                Some(Token::Character('<'))
            }
            None => {
                // EOF: emit `<` then return to RCDATA (next call emits EOF)
                self.state = State::RCDATA;
                Some(Token::Character('<'))
            }
        }
    }

    /// §13.2.5.10 RCDATA end tag open state
    ///
    /// Consume the next input character:
    /// - ASCII alpha → create end tag token (empty name), append lowercase,
    ///   append original to temporary buffer, switch to RCDATA end tag name
    /// - Anything else → emit `<` + `/` char tokens, reconsume in RCDATA
    fn handle_rcdata_end_tag_open_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some(c) if c.is_ascii_alphabetic() => {
                let mut name = String::new();
                name.push(c.to_ascii_lowercase());
                self.temporary_buffer.push(c);
                self.current_tag = Some(TagToken {
                    kind: TagKind::End,
                    name,
                    attrs: Vec::new(),
                    self_closing: false,
                });
                self.state = State::RCDATAEndTagName;
                None
            }
            Some(_c) => {
                // Emit `<` + `/`, reconsume in RCDATA
                // Push in reverse order so pop() yields correct sequence
                self.pending_tokens.push(Token::Character('/'));
                self.pending_tokens.push(Token::Character('<'));
                self.state = State::RCDATA;
                self.reconsume = true;
                None
            }
            None => {
                // Emit `<` + `/`, reconsume EOF in RCDATA
                self.pending_tokens.push(Token::Character('/'));
                self.pending_tokens.push(Token::Character('<'));
                self.state = State::RCDATA;
                self.reconsume = true;
                None
            }
        }
    }

    /// §13.2.5.11 RCDATA end tag name state
    ///
    /// Consume the next input character:
    /// - TAB/LF/FF/SPACE → if appropriate end tag, switch to BeforeAttributeName;
    ///   else "anything else"
    /// - `/` → if appropriate end tag, switch to SelfClosingStartTag;
    ///   else "anything else"
    /// - `>` → if appropriate end tag, emit tag token, switch to Data;
    ///   else "anything else"
    /// - ASCII upper alpha → append lowercase to tag name, append original to
    ///   temporary buffer
    /// - ASCII lower alpha → append to tag name, append to temporary buffer
    /// - Anything else → emit `<` + `/` + temporary buffer chars, reconsume in
    ///   RCDATA
    fn handle_rcdata_end_tag_name_state(&mut self) -> Option<Token> {
        match self.next_char() {
            // Whitespace → check if appropriate, then switch to BeforeAttributeName
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                if self.is_appropriate_end_tag() {
                    self.state = State::BeforeAttributeName;
                    None
                } else {
                    self.rcdata_end_tag_name_backout()
                }
            }
            Some('/') => {
                if self.is_appropriate_end_tag() {
                    self.state = State::SelfClosingStartTag;
                    None
                } else {
                    self.rcdata_end_tag_name_backout()
                }
            }
            Some('>') => {
                if self.is_appropriate_end_tag() {
                    let tag = self.current_tag.take().unwrap();
                    self.temporary_buffer.clear();
                    self.state = State::Data;
                    Some(Token::Tag(tag))
                } else {
                    self.rcdata_end_tag_name_backout()
                }
            }
            Some(c) if c.is_ascii_uppercase() => {
                if let Some(ref mut tag) = self.current_tag {
                    tag.name.push(c.to_ascii_lowercase());
                }
                self.temporary_buffer.push(c);
                None
            }
            Some(c) if c.is_ascii_lowercase() => {
                if let Some(ref mut tag) = self.current_tag {
                    tag.name.push(c);
                }
                self.temporary_buffer.push(c);
                None
            }
            Some(_c) => {
                // Non-alpha character → "anything else" backout
                self.rcdata_end_tag_name_backout()
            }
            None => {
                // EOF → "anything else" backout to RCDATA
                self.rcdata_end_tag_name_backout()
            }
        }
    }

    /// Helper: check if the current tag being built matches the appropriate end tag.
    fn is_appropriate_end_tag(&self) -> bool {
        match (&self.appropriate_end_tag_name, &self.current_tag) {
            (Some(expected), Some(tag)) => *expected == tag.name,
            _ => false,
        }
    }

    /// Helper: "anything else" backout for RCDATA end tag name state.
    ///
    /// Emits `<` + `/` + each char in the temporary buffer as character tokens,
    /// then reconsume the current character back in RCDATA state.
    fn rcdata_end_tag_name_backout(&mut self) -> Option<Token> {
        // Push tokens in reverse order (pop yields forward order)
        // temp buffer chars first (they come after `<` and `/`)
        for ch in self.temporary_buffer.chars().rev() {
            self.pending_tokens.push(Token::Character(ch));
        }
        self.pending_tokens.push(Token::Character('/'));
        self.pending_tokens.push(Token::Character('<'));
        // Discard the partial end tag
        self.current_tag = None;
        self.temporary_buffer.clear();
        self.state = State::RCDATA;
        self.reconsume = true;
        None
    }

    // ── RAWTEXT tag detection states (§13.2.5.12–§13.2.5.14) ──────

    /// §13.2.5.12 RAWTEXT less-than sign state
    ///
    /// Consume the next input character:
    /// - U+002F SOLIDUS (/) → clear temporary buffer, switch to RAWTEXT end tag open
    /// - Anything else → emit `<` character token, reconsume in RAWTEXT
    fn handle_rawtext_less_than_sign_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('/') => {
                self.temporary_buffer.clear();
                self.state = State::RAWTEXTEndTagOpen;
                None
            }
            Some(_c) => {
                self.state = State::RAWTEXT;
                self.reconsume = true;
                Some(Token::Character('<'))
            }
            None => {
                self.state = State::RAWTEXT;
                Some(Token::Character('<'))
            }
        }
    }

    /// §13.2.5.13 RAWTEXT end tag open state
    ///
    /// Consume the next input character:
    /// - ASCII alpha → create end tag token (empty name), append lowercase,
    ///   append original to temporary buffer, switch to RAWTEXT end tag name
    /// - Anything else → emit `<` + `/` char tokens, reconsume in RAWTEXT
    fn handle_rawtext_end_tag_open_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some(c) if c.is_ascii_alphabetic() => {
                let mut name = String::new();
                name.push(c.to_ascii_lowercase());
                self.temporary_buffer.push(c);
                self.current_tag = Some(TagToken {
                    kind: TagKind::End,
                    name,
                    attrs: Vec::new(),
                    self_closing: false,
                });
                self.state = State::RAWTEXTEndTagName;
                None
            }
            Some(_c) => {
                self.pending_tokens.push(Token::Character('/'));
                self.pending_tokens.push(Token::Character('<'));
                self.state = State::RAWTEXT;
                self.reconsume = true;
                None
            }
            None => {
                self.pending_tokens.push(Token::Character('/'));
                self.pending_tokens.push(Token::Character('<'));
                self.state = State::RAWTEXT;
                self.reconsume = true;
                None
            }
        }
    }

    /// §13.2.5.14 RAWTEXT end tag name state
    ///
    /// Consume the next input character:
    /// - TAB/LF/FF/SPACE → if appropriate end tag, switch to BeforeAttributeName;
    ///   else "anything else"
    /// - `/` → if appropriate end tag, switch to SelfClosingStartTag;
    ///   else "anything else"
    /// - `>` → if appropriate end tag, emit tag token, switch to Data;
    ///   else "anything else"
    /// - ASCII upper alpha → append lowercase to tag name, append original to
    ///   temporary buffer
    /// - ASCII lower alpha → append to tag name, append to temporary buffer
    /// - Anything else → emit `<` + `/` + temporary buffer chars, reconsume in
    ///   RAWTEXT
    /// - EOF → parse error; switch to Data; emit `<` + `/`; reconsume EOF
    fn handle_rawtext_end_tag_name_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                if self.is_appropriate_end_tag() {
                    self.state = State::BeforeAttributeName;
                    None
                } else {
                    self.rawtext_end_tag_name_backout()
                }
            }
            Some('/') => {
                if self.is_appropriate_end_tag() {
                    self.state = State::SelfClosingStartTag;
                    None
                } else {
                    self.rawtext_end_tag_name_backout()
                }
            }
            Some('>') => {
                if self.is_appropriate_end_tag() {
                    let tag = self.current_tag.take().unwrap();
                    self.temporary_buffer.clear();
                    self.state = State::Data;
                    Some(Token::Tag(tag))
                } else {
                    self.rawtext_end_tag_name_backout()
                }
            }
            Some(c) if c.is_ascii_uppercase() => {
                if let Some(ref mut tag) = self.current_tag {
                    tag.name.push(c.to_ascii_lowercase());
                }
                self.temporary_buffer.push(c);
                None
            }
            Some(c) if c.is_ascii_lowercase() => {
                if let Some(ref mut tag) = self.current_tag {
                    tag.name.push(c);
                }
                self.temporary_buffer.push(c);
                None
            }
            Some(_c) => self.rawtext_end_tag_name_backout(),
            None => {
                // §13.2.5.14: EOF → parse error; switch to Data; emit `<` + `/`;
                // reconsume the EOF token. Since we're at EOF, don't set
                // `reconsume` here — the next `next_char()` call will
                // naturally return `None`, and Data will emit EOF.
                // TODO: record parse error (eof-in-tag)
                self.pending_tokens.push(Token::Character('/'));
                self.pending_tokens.push(Token::Character('<'));
                self.current_tag = None;
                self.temporary_buffer.clear();
                self.state = State::Data;
                None
            }
        }
    }

    /// Helper: "anything else" backout for RAWTEXT end tag name state.
    ///
    /// Emits `<` + `/` + each char in the temporary buffer as character tokens,
    /// then reconsume the current character back in RAWTEXT state.
    fn rawtext_end_tag_name_backout(&mut self) -> Option<Token> {
        for ch in self.temporary_buffer.chars().rev() {
            self.pending_tokens.push(Token::Character(ch));
        }
        self.pending_tokens.push(Token::Character('/'));
        self.pending_tokens.push(Token::Character('<'));
        self.current_tag = None;
        self.temporary_buffer.clear();
        self.state = State::RAWTEXT;
        self.reconsume = true;
        None
    }

    // ── Script data states (§13.2.5.4, §13.2.5.15–§13.2.5.17) ────

    /// §13.2.5.4 Script data state
    ///
    /// Consume the next input character:
    /// - U+003C LESS-THAN SIGN (<) → ScriptDataLessThanSign
    /// - U+0000 NULL → parse error; emit U+FFFD character token
    /// - EOF → emit end-of-file token
    /// - Anything else → emit character token
    ///
    /// Note: ScriptData does NOT handle `&` — no character references.
    fn handle_script_data_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('<') => {
                self.state = State::ScriptDataLessThanSign;
                None
            }
            Some('\0') => {
                // TODO: record parse error (unexpected-null-character)
                Some(Token::Character('\u{FFFD}'))
            }
            Some(c) => Some(Token::Character(c)),
            None => {
                self.eof_emitted = true;
                Some(Token::EOF)
            }
        }
    }

    /// §13.2.5.15 Script data less-than sign state
    ///
    /// Consume the next input character:
    /// - U+002F SOLIDUS (/) → clear temporary buffer, ScriptDataEndTagOpen
    /// - U+0021 EXCLAMATION MARK (!) → ScriptDataEscapeStart
    /// - Anything else → emit `<` character token, reconsume in ScriptData
    fn handle_script_data_less_than_sign_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('/') => {
                self.temporary_buffer.clear();
                self.state = State::ScriptDataEndTagOpen;
                None
            }
            Some('!') => {
                self.state = State::ScriptDataEscapeStart;
                None
            }
            Some(_c) => {
                self.state = State::ScriptData;
                self.reconsume = true;
                Some(Token::Character('<'))
            }
            None => {
                self.state = State::ScriptData;
                Some(Token::Character('<'))
            }
        }
    }

    /// §13.2.5.16 Script data end tag open state
    ///
    /// Consume the next input character:
    /// - ASCII alpha → create end tag token (empty name), append lowercase,
    ///   append original to temporary buffer, ScriptDataEndTagName
    /// - Anything else → emit `<` + `/` char tokens, reconsume in ScriptData
    fn handle_script_data_end_tag_open_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some(c) if c.is_ascii_alphabetic() => {
                let mut name = String::new();
                name.push(c.to_ascii_lowercase());
                self.temporary_buffer.push(c);
                self.current_tag = Some(TagToken {
                    kind: TagKind::End,
                    name,
                    attrs: Vec::new(),
                    self_closing: false,
                });
                self.state = State::ScriptDataEndTagName;
                None
            }
            Some(_c) => {
                self.pending_tokens.push(Token::Character('/'));
                self.pending_tokens.push(Token::Character('<'));
                self.state = State::ScriptData;
                self.reconsume = true;
                None
            }
            None => {
                self.pending_tokens.push(Token::Character('/'));
                self.pending_tokens.push(Token::Character('<'));
                self.state = State::ScriptData;
                self.reconsume = true;
                None
            }
        }
    }

    /// §13.2.5.17 Script data end tag name state
    ///
    /// Consume the next input character:
    /// - TAB/LF/FF/SPACE → if appropriate end tag, BeforeAttributeName;
    ///   else "anything else"
    /// - `/` → if appropriate end tag, SelfClosingStartTag; else "anything else"
    /// - `>` → if appropriate end tag, emit tag, Data; else "anything else"
    /// - ASCII upper alpha → append lowercase to tag name, append original to
    ///   temporary buffer
    /// - ASCII lower alpha → append to tag name, append to temporary buffer
    /// - Anything else → emit `<` + `/` + temporary buffer chars, reconsume in
    ///   ScriptData
    fn handle_script_data_end_tag_name_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                if self.is_appropriate_end_tag() {
                    self.state = State::BeforeAttributeName;
                    None
                } else {
                    self.script_data_end_tag_name_backout()
                }
            }
            Some('/') => {
                if self.is_appropriate_end_tag() {
                    self.state = State::SelfClosingStartTag;
                    None
                } else {
                    self.script_data_end_tag_name_backout()
                }
            }
            Some('>') => {
                if self.is_appropriate_end_tag() {
                    let tag = self.current_tag.take().unwrap();
                    self.temporary_buffer.clear();
                    self.state = State::Data;
                    Some(Token::Tag(tag))
                } else {
                    self.script_data_end_tag_name_backout()
                }
            }
            Some(c) if c.is_ascii_uppercase() => {
                if let Some(ref mut tag) = self.current_tag {
                    tag.name.push(c.to_ascii_lowercase());
                }
                self.temporary_buffer.push(c);
                None
            }
            Some(c) if c.is_ascii_lowercase() => {
                if let Some(ref mut tag) = self.current_tag {
                    tag.name.push(c);
                }
                self.temporary_buffer.push(c);
                None
            }
            Some(_c) => self.script_data_end_tag_name_backout(),
            None => self.script_data_end_tag_name_backout(),
        }
    }

    /// §13.2.5.18 Script data escape start state
    ///
    /// Consume the next input character:
    /// - U+002D HYPHEN-MINUS (-) → ScriptDataEscapeStartDash
    /// - Anything else → reconsume in ScriptData
    fn handle_script_data_escape_start_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('-') => {
                self.state = State::ScriptDataEscapeStartDash;
                None
            }
            Some(_c) => {
                self.state = State::ScriptData;
                self.reconsume = true;
                None
            }
            None => {
                self.state = State::ScriptData;
                self.reconsume = true;
                None
            }
        }
    }

    /// §13.2.5.19 Script data escape start dash state
    ///
    /// Consume the next input character:
    /// - U+002D HYPHEN-MINUS (-) → ScriptDataEscapedDashDash
    /// - Anything else → reconsume in ScriptData
    fn handle_script_data_escape_start_dash_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('-') => {
                self.state = State::ScriptDataEscapedDashDash;
                None
            }
            Some(_c) => {
                self.state = State::ScriptData;
                self.reconsume = true;
                None
            }
            None => {
                self.state = State::ScriptData;
                self.reconsume = true;
                None
            }
        }
    }

    // ── Script data escaped states (§13.2.5.20–§13.2.5.25) ──────

    /// §13.2.5.20 Script data escaped state
    ///
    /// Consume the next input character:
    /// - U+002D HYPHEN-MINUS (-) → ScriptDataEscapedDash
    /// - U+003C LESS-THAN SIGN (<) → ScriptDataEscapedLessThanSign
    /// - U+0000 NULL → parse error; emit U+FFFD character token
    /// - EOF → parse error; emit end-of-file token
    /// - Anything else → emit character token
    fn handle_script_data_escaped_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('-') => {
                self.state = State::ScriptDataEscapedDash;
                None
            }
            Some('<') => {
                self.state = State::ScriptDataEscapedLessThanSign;
                None
            }
            Some('\0') => {
                // TODO: record parse error (unexpected-null-character)
                Some(Token::Character('\u{FFFD}'))
            }
            Some(c) => Some(Token::Character(c)),
            None => {
                // TODO: record parse error (eof-in-script-html-comment-like-text)
                self.eof_emitted = true;
                Some(Token::EOF)
            }
        }
    }

    /// §13.2.5.21 Script data escaped dash state
    ///
    /// Consume the next input character:
    /// - U+002D HYPHEN-MINUS (-) → ScriptDataEscapedDashDash
    /// - U+003C LESS-THAN SIGN (<) → ScriptDataEscapedLessThanSign
    /// - U+0000 NULL → parse error; emit U+FFFD character token
    /// - EOF → parse error; emit end-of-file token
    /// - Anything else → emit `-`, reconsume in ScriptDataEscaped
    fn handle_script_data_escaped_dash_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('-') => {
                self.state = State::ScriptDataEscapedDashDash;
                None
            }
            Some('<') => {
                self.state = State::ScriptDataEscapedLessThanSign;
                None
            }
            Some('\0') => {
                // TODO: record parse error (unexpected-null-character)
                Some(Token::Character('\u{FFFD}'))
            }
            Some(_c) => {
                self.state = State::ScriptDataEscaped;
                self.reconsume = true;
                Some(Token::Character('-'))
            }
            None => {
                // TODO: record parse error (eof-in-script-html-comment-like-text)
                self.eof_emitted = true;
                Some(Token::EOF)
            }
        }
    }

    /// §13.2.5.22 Script data escaped dash dash state
    ///
    /// Consume the next input character:
    /// - U+002D HYPHEN-MINUS (-) → emit `-`
    /// - U+003C LESS-THAN SIGN (<) → ScriptDataEscapedLessThanSign
    /// - U+003E GREATER-THAN SIGN (>) → emit `>`, switch to ScriptData
    /// - U+0000 NULL → parse error; emit U+FFFD character token
    /// - EOF → parse error; emit end-of-file token
    /// - Anything else → emit `--`, reconsume in ScriptDataEscaped
    fn handle_script_data_escaped_dash_dash_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('-') => Some(Token::Character('-')),
            Some('<') => {
                self.state = State::ScriptDataEscapedLessThanSign;
                None
            }
            Some('>') => {
                self.state = State::ScriptData;
                Some(Token::Character('>'))
            }
            Some('\0') => {
                // TODO: record parse error (unexpected-null-character)
                Some(Token::Character('\u{FFFD}'))
            }
            Some(c) => {
                self.state = State::ScriptDataEscaped;
                self.reconsume = true;
                // Emit "--" via pending tokens
                self.pending_tokens.push(Token::Character('-'));
                self.pending_tokens.push(Token::Character('-'));
                Some(Token::Character(c))
            }
            None => {
                // TODO: record parse error (eof-in-script-html-comment-like-text)
                self.eof_emitted = true;
                Some(Token::EOF)
            }
        }
    }

    /// §13.2.5.23 Script data escaped less-than sign state
    ///
    /// Consume the next input character:
    /// - U+002F SOLIDUS (/) → clear temporary buffer, ScriptDataEscapedEndTagOpen
    /// - ASCII alpha → clear temporary buffer, create end tag, append
    ///   lowercase + original to temp, ScriptDataDoubleEscapeStart
    /// - Anything else → emit `<`, reconsume in ScriptDataEscaped
    fn handle_script_data_escaped_less_than_sign_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('/') => {
                self.temporary_buffer.clear();
                self.state = State::ScriptDataEscapedEndTagOpen;
                None
            }
            Some(c) if c.is_ascii_alphabetic() => {
                self.temporary_buffer.clear();
                let mut name = String::new();
                name.push(c.to_ascii_lowercase());
                self.temporary_buffer.push(c);
                self.current_tag = Some(TagToken {
                    kind: TagKind::End,
                    name,
                    attrs: Vec::new(),
                    self_closing: false,
                });
                self.state = State::ScriptDataDoubleEscapeStart;
                None
            }
            Some(_c) => {
                self.state = State::ScriptDataEscaped;
                self.reconsume = true;
                Some(Token::Character('<'))
            }
            None => {
                self.state = State::ScriptDataEscaped;
                Some(Token::Character('<'))
            }
        }
    }

    /// §13.2.5.24 Script data escaped end tag open state
    ///
    /// Consume the next input character:
    /// - ASCII alpha → create end tag (empty name), append lowercase + original
    ///   to temp, ScriptDataEscapedEndTagName
    /// - Anything else → emit `<` + `/`, reconsume in ScriptDataEscaped
    fn handle_script_data_escaped_end_tag_open_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some(c) if c.is_ascii_alphabetic() => {
                let mut name = String::new();
                name.push(c.to_ascii_lowercase());
                self.temporary_buffer.push(c);
                self.current_tag = Some(TagToken {
                    kind: TagKind::End,
                    name,
                    attrs: Vec::new(),
                    self_closing: false,
                });
                self.state = State::ScriptDataEscapedEndTagName;
                None
            }
            Some(_c) => {
                self.pending_tokens.push(Token::Character('/'));
                self.pending_tokens.push(Token::Character('<'));
                self.state = State::ScriptDataEscaped;
                self.reconsume = true;
                None
            }
            None => {
                self.pending_tokens.push(Token::Character('/'));
                self.pending_tokens.push(Token::Character('<'));
                self.state = State::ScriptDataEscaped;
                self.reconsume = true;
                None
            }
        }
    }

    /// §13.2.5.25 Script data escaped end tag name state
    ///
    /// Same pattern as other end tag name states but returns to
    /// ScriptDataEscaped on backout.
    fn handle_script_data_escaped_end_tag_name_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                if self.is_appropriate_end_tag() {
                    self.state = State::BeforeAttributeName;
                    None
                } else {
                    self.script_data_escaped_end_tag_name_backout()
                }
            }
            Some('/') => {
                if self.is_appropriate_end_tag() {
                    self.state = State::SelfClosingStartTag;
                    None
                } else {
                    self.script_data_escaped_end_tag_name_backout()
                }
            }
            Some('>') => {
                if self.is_appropriate_end_tag() {
                    let tag = self.current_tag.take().unwrap();
                    self.temporary_buffer.clear();
                    self.state = State::Data;
                    Some(Token::Tag(tag))
                } else {
                    self.script_data_escaped_end_tag_name_backout()
                }
            }
            Some(c) if c.is_ascii_uppercase() => {
                if let Some(ref mut tag) = self.current_tag {
                    tag.name.push(c.to_ascii_lowercase());
                }
                self.temporary_buffer.push(c);
                None
            }
            Some(c) if c.is_ascii_lowercase() => {
                if let Some(ref mut tag) = self.current_tag {
                    tag.name.push(c);
                }
                self.temporary_buffer.push(c);
                None
            }
            Some(_c) => self.script_data_escaped_end_tag_name_backout(),
            None => self.script_data_escaped_end_tag_name_backout(),
        }
    }

    // ── Script data double escaped states (§13.2.5.26–§13.2.5.31) ─

    /// §13.2.5.26 Script data double escape start state
    ///
    /// Checks whether the accumulated temporary buffer matches "script".
    /// If so, enters double-escaped mode. Otherwise returns to escaped.
    ///
    /// - TAB/LF/FF/SPACE, `/`, or `>` → if temp buffer is "script",
    ///   ScriptDataDoubleEscaped; else ScriptDataEscaped
    /// - ASCII upper alpha → append lowercase to tag name, append original
    ///   to temp buffer
    /// - ASCII lower alpha → append to tag name, append to temp buffer
    /// - Anything else → reconsume in ScriptDataEscaped
    fn handle_script_data_double_escape_start_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ')
            | Some('/') | Some('>') => {
                // Check if temp buffer is exactly "script"
                if self.temporary_buffer == "script" {
                    // Tag persists into DoubleEscaped for later emission in
                    // DoubleEscapeEnd (§13.2.5.31).
                    self.temporary_buffer.clear();
                    self.state = State::ScriptDataDoubleEscaped;
                } else {
                    self.current_tag = None;
                    self.temporary_buffer.clear();
                    self.state = State::ScriptDataEscaped;
                }
                None
            }
            Some(c) if c.is_ascii_uppercase() => {
                if let Some(ref mut tag) = self.current_tag {
                    tag.name.push(c.to_ascii_lowercase());
                }
                self.temporary_buffer.push(c);
                None
            }
            Some(c) if c.is_ascii_lowercase() => {
                if let Some(ref mut tag) = self.current_tag {
                    tag.name.push(c);
                }
                self.temporary_buffer.push(c);
                None
            }
            Some(_c) => {
                self.current_tag = None;
                self.temporary_buffer.clear();
                self.state = State::ScriptDataEscaped;
                self.reconsume = true;
                None
            }
            None => {
                self.current_tag = None;
                self.temporary_buffer.clear();
                self.state = State::ScriptDataEscaped;
                self.reconsume = true;
                None
            }
        }
    }

    /// §13.2.5.27 Script data double escaped state
    ///
    /// Consume the next input character:
    /// - U+002D HYPHEN-MINUS (-) → ScriptDataDoubleEscapedDash, emit `-`
    /// - U+003C LESS-THAN SIGN (<) → ScriptDataDoubleEscapedLessThanSign,
    ///   emit `<`
    /// - U+0000 NULL → parse error; emit U+FFFD character token
    /// - EOF → parse error; emit end-of-file token
    /// - Anything else → emit character token
    fn handle_script_data_double_escaped_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('-') => {
                self.state = State::ScriptDataDoubleEscapedDash;
                Some(Token::Character('-'))
            }
            Some('<') => {
                self.state = State::ScriptDataDoubleEscapedLessThanSign;
                Some(Token::Character('<'))
            }
            Some('\0') => {
                // TODO: record parse error (unexpected-null-character)
                Some(Token::Character('\u{FFFD}'))
            }
            Some(c) => Some(Token::Character(c)),
            None => {
                // TODO: record parse error (eof-in-script-html-comment-like-text)
                self.eof_emitted = true;
                Some(Token::EOF)
            }
        }
    }

    /// §13.2.5.28 Script data double escaped dash state
    ///
    /// Consume the next input character:
    /// - U+002D HYPHEN-MINUS (-) → ScriptDataDoubleEscapedDashDash, emit `-`
    /// - U+003C LESS-THAN SIGN (<) → ScriptDataDoubleEscapedLessThanSign,
    ///   emit `<`
    /// - U+0000 NULL → parse error; emit U+FFFD character token
    /// - EOF → parse error; emit end-of-file token
    /// - Anything else → emit `-`, reconsume in ScriptDataDoubleEscaped
    fn handle_script_data_double_escaped_dash_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('-') => {
                self.state = State::ScriptDataDoubleEscapedDashDash;
                Some(Token::Character('-'))
            }
            Some('<') => {
                self.state = State::ScriptDataDoubleEscapedLessThanSign;
                Some(Token::Character('<'))
            }
            Some('\0') => {
                // TODO: record parse error (unexpected-null-character)
                Some(Token::Character('\u{FFFD}'))
            }
            Some(_c) => {
                self.state = State::ScriptDataDoubleEscaped;
                self.reconsume = true;
                Some(Token::Character('-'))
            }
            None => {
                // TODO: record parse error (eof-in-script-html-comment-like-text)
                self.eof_emitted = true;
                Some(Token::EOF)
            }
        }
    }

    /// §13.2.5.29 Script data double escaped dash dash state
    ///
    /// Consume the next input character:
    /// - U+002D HYPHEN-MINUS (-) → emit `-`
    /// - U+003C LESS-THAN SIGN (<) → ScriptDataDoubleEscapedLessThanSign,
    ///   emit `<`
    /// - U+003E GREATER-THAN SIGN (>) → emit `>`, switch to ScriptData
    /// - U+0000 NULL → parse error; emit U+FFFD character token
    /// - EOF → parse error; emit end-of-file token
    /// - Anything else → emit `--`, reconsume in ScriptDataDoubleEscaped
    fn handle_script_data_double_escaped_dash_dash_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('-') => Some(Token::Character('-')),
            Some('<') => {
                self.state = State::ScriptDataDoubleEscapedLessThanSign;
                Some(Token::Character('<'))
            }
            Some('>') => {
                self.state = State::ScriptData;
                Some(Token::Character('>'))
            }
            Some('\0') => {
                // TODO: record parse error (unexpected-null-character)
                Some(Token::Character('\u{FFFD}'))
            }
            Some(c) => {
                self.state = State::ScriptDataDoubleEscaped;
                self.reconsume = true;
                self.pending_tokens.push(Token::Character('-'));
                self.pending_tokens.push(Token::Character('-'));
                Some(Token::Character(c))
            }
            None => {
                // TODO: record parse error (eof-in-script-html-comment-like-text)
                self.eof_emitted = true;
                Some(Token::EOF)
            }
        }
    }

    /// §13.2.5.30 Script data double escaped less-than sign state
    ///
    /// Consume the next input character:
    /// - U+002F SOLIDUS (/) → clear temporary buffer, emit `/`,
    ///   ScriptDataDoubleEscapeEnd
    /// - Anything else → emit `<`, reconsume in ScriptDataDoubleEscaped
    fn handle_script_data_double_escaped_less_than_sign_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('/') => {
                self.temporary_buffer.clear();
                self.state = State::ScriptDataDoubleEscapeEnd;
                // Emit `/` via pending — the spec says "Emit a U+002F SOLIDUS
                // character token."
                self.pending_tokens.push(Token::Character('/'));
                None
            }
            Some(_c) => {
                self.state = State::ScriptDataDoubleEscaped;
                self.reconsume = true;
                Some(Token::Character('<'))
            }
            None => {
                self.state = State::ScriptDataDoubleEscaped;
                Some(Token::Character('<'))
            }
        }
    }

    /// §13.2.5.31 Script data double escape end state
    ///
    /// Checks whether the accumulated temporary buffer matches "script".
    /// If so, exits double-escaped mode back to escaped. Otherwise backout.
    ///
    /// - TAB/LF/FF/SPACE, `/`, or `>` → if temp buffer is "script", emit the
    ///   current tag token, switch to ScriptDataEscaped; else emit `<` + `/` +
    ///   temp buffer chars, reconsume in ScriptDataDoubleEscaped
    /// - ASCII upper alpha → append lowercase to tag name, append original
    ///   to temp buffer
    /// - ASCII lower alpha → append to tag name, append to temp buffer
    /// - Anything else → emit `<` + `/` + temp buffer chars, reconsume in
    ///   ScriptDataDoubleEscaped
    fn handle_script_data_double_escape_end_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ')
            | Some('/') | Some('>') => {
                if self.temporary_buffer == "script" {
                    // Exit double-escaped: emit the tag token, return to escaped
                    if let Some(tag) = self.current_tag.take() {
                        self.temporary_buffer.clear();
                        self.state = State::ScriptDataEscaped;
                        Some(Token::Tag(tag))
                    } else {
                        self.temporary_buffer.clear();
                        self.state = State::ScriptDataEscaped;
                        None
                    }
                } else {
                    self.script_data_double_escaped_end_tag_name_backout()
                }
            }
            Some(c) if c.is_ascii_uppercase() => {
                if let Some(ref mut tag) = self.current_tag {
                    tag.name.push(c.to_ascii_lowercase());
                }
                self.temporary_buffer.push(c);
                None
            }
            Some(c) if c.is_ascii_lowercase() => {
                if let Some(ref mut tag) = self.current_tag {
                    tag.name.push(c);
                }
                self.temporary_buffer.push(c);
                None
            }
            Some(_c) => self.script_data_double_escaped_end_tag_name_backout(),
            None => self.script_data_double_escaped_end_tag_name_backout(),
        }
    }

    // ── Script data backout helpers ─────────────────────────────

    /// Helper: "anything else" backout for ScriptData end tag name state.
    fn script_data_end_tag_name_backout(&mut self) -> Option<Token> {
        for ch in self.temporary_buffer.chars().rev() {
            self.pending_tokens.push(Token::Character(ch));
        }
        self.pending_tokens.push(Token::Character('/'));
        self.pending_tokens.push(Token::Character('<'));
        self.current_tag = None;
        self.temporary_buffer.clear();
        self.state = State::ScriptData;
        self.reconsume = true;
        None
    }

    /// Helper: "anything else" backout for ScriptDataEscaped end tag name.
    fn script_data_escaped_end_tag_name_backout(&mut self) -> Option<Token> {
        for ch in self.temporary_buffer.chars().rev() {
            self.pending_tokens.push(Token::Character(ch));
        }
        self.pending_tokens.push(Token::Character('/'));
        self.pending_tokens.push(Token::Character('<'));
        self.current_tag = None;
        self.temporary_buffer.clear();
        self.state = State::ScriptDataEscaped;
        self.reconsume = true;
        None
    }

    /// Helper: "anything else" backout for ScriptDataDoubleEscapeEnd.
    fn script_data_double_escaped_end_tag_name_backout(&mut self) -> Option<Token> {
        for ch in self.temporary_buffer.chars().rev() {
            self.pending_tokens.push(Token::Character(ch));
        }
        self.pending_tokens.push(Token::Character('/'));
        self.pending_tokens.push(Token::Character('<'));
        self.current_tag = None;
        self.temporary_buffer.clear();
        self.state = State::ScriptDataDoubleEscaped;
        self.reconsume = true;
        None
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

    // ── Character reference states (§13.2.5.72–§13.2.5.80) ─────－

    /// §13.2.5.72 Character reference state
    ///
    /// Dispatches the character following `&`:
    /// - U+0009 TAB → emit `&`, reconsume in return_state
    /// - U+000A LF → emit `&`, reconsume in return_state
    /// - U+000C FF → emit `&`, reconsume in return_state
    /// - U+0020 SPACE → emit `&`, reconsume in return_state
    /// - U+003C LESS-THAN SIGN (<) → emit `&`, reconsume in return_state
    /// - U+0026 AMPERSAND (&) → emit `&`, reconsume in return_state
    /// - EOF → parse error; emit `&`, reconsume EOF
    /// - U+0023 NUMBER SIGN (#) → NumericCharacterReference
    /// - ASCII alphanumeric → NamedCharacterReference
    /// - Anything else → emit `&`, reconsume in return_state
    fn handle_character_reference_state(&mut self) -> Option<Token> {
        match self.next_char() {
            // Characters that cause immediate fallback (not a valid char ref)
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ')
            | Some('<') | Some('&') => {
                self.emit_ampersand_and_return()
            }
            None => {
                // TODO: record parse error (eof-in-tag — actually not in a tag,
                // but spec says this is an error)
                self.emit_ampersand_and_return()
            }
            Some('#') => {
                self.state = State::NumericCharacterReference;
                None
            }
            Some(c) if c.is_ascii_alphanumeric() => {
                self.state = State::NamedCharacterReference;
                // The character was already consumed — we'll handle it
                // in NamedCharacterReference. For the spec-compliant approach,
                // we should reconsume it rather than consume it.
                // But since NamedCharacterReference needs to build up the name,
                // we reconsume so it can consume this char.
                self.reconsume = true;
                None
            }
            Some(_c) => {
                // Non-alphanumeric, non-# → not a character reference
                self.emit_ampersand_and_return()
            }
        }
    }

    /// §13.2.5.75 Numeric character reference state
    ///
    /// Consume the next input character:
    /// - U+0078 x / U+0058 X → HexCharacterReferenceStart
    /// - ASCII digit → reconsume in DecimalCharacterReferenceStart
    /// - Anything else → parse error; emit `&#`, reconsume
    fn handle_numeric_character_reference_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('x') | Some('X') => {
                self.character_reference_code = 0;
                self.state = State::HexCharacterReferenceStart;
                None
            }
            Some(c) if c.is_ascii_digit() => {
                self.character_reference_code = 0;
                self.state = State::DecimalCharacterReferenceStart;
                self.reconsume = true;
                None
            }
            Some(_c) => {
                // TODO: record parse error (absence-of-digits-in-numeric-character-reference)
                self.emit_ampersand_hash_and_return()
            }
            None => {
                // TODO: record parse error
                self.emit_ampersand_hash_and_return()
            }
        }
    }

    /// §13.2.5.76 Hexadecimal character reference start state
    ///
    /// Consume the next input character:
    /// - ASCII hex digit → reconsume in HexCharacterReference
    /// - Anything else → parse error; emit `&#x`, reconsume
    fn handle_hex_character_reference_start_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some(c) if c.is_ascii_hexdigit() => {
                self.state = State::HexCharacterReference;
                self.reconsume = true;
                None
            }
            Some(_c) => {
                // TODO: record parse error (absence-of-digits-in-numeric-character-reference)
                self.emit_ampersand_hash_x_and_return()
            }
            None => {
                self.emit_ampersand_hash_x_and_return()
            }
        }
    }

    /// §13.2.5.77 Decimal character reference start state
    ///
    /// Consume the next input character:
    /// - ASCII digit → reconsume in DecimalCharacterReference
    /// - Anything else → parse error; emit `&#`, reconsume
    fn handle_decimal_character_reference_start_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some(c) if c.is_ascii_digit() => {
                self.state = State::DecimalCharacterReference;
                self.reconsume = true;
                None
            }
            Some(_c) => {
                // TODO: record parse error
                self.emit_ampersand_hash_and_return()
            }
            None => {
                self.emit_ampersand_hash_and_return()
            }
        }
    }

    /// §13.2.5.78 Hexadecimal character reference state
    ///
    /// Consume the next input character:
    /// - ASCII hex digit → multiply code by 16, add digit value, stay
    /// - U+003B SEMICOLON (;) → switch to NumericCharacterReferenceEnd
    /// - Anything else → parse error; reconsume in NumericCharacterReferenceEnd
    fn handle_hex_character_reference_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some(c) if c.is_ascii_hexdigit() => {
                let digit = match c {
                    '0'..='9' => c as u32 - '0' as u32,
                    'a'..='f' => c as u32 - 'a' as u32 + 10,
                    'A'..='F' => c as u32 - 'A' as u32 + 10,
                    _ => unreachable!(),
                };
                self.character_reference_code =
                    self.character_reference_code.wrapping_mul(16).wrapping_add(digit);
                None
            }
            Some(';') => {
                self.state = State::NumericCharacterReferenceEnd;
                None
            }
            Some(_c) => {
                // TODO: record parse error (missing-semicolon-after-character-reference)
                self.state = State::NumericCharacterReferenceEnd;
                self.reconsume = true;
                None
            }
            None => {
                // TODO: record parse error
                self.state = State::NumericCharacterReferenceEnd;
                self.reconsume = true;
                None
            }
        }
    }

    /// §13.2.5.79 Decimal character reference state
    ///
    /// Consume the next input character:
    /// - ASCII digit → multiply code by 10, add digit value, stay
    /// - U+003B SEMICOLON (;) → switch to NumericCharacterReferenceEnd
    /// - Anything else → parse error; reconsume in NumericCharacterReferenceEnd
    fn handle_decimal_character_reference_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some(c) if c.is_ascii_digit() => {
                let digit = c as u32 - '0' as u32;
                self.character_reference_code =
                    self.character_reference_code.wrapping_mul(10).wrapping_add(digit);
                None
            }
            Some(';') => {
                self.state = State::NumericCharacterReferenceEnd;
                None
            }
            Some(_c) => {
                // TODO: record parse error (missing-semicolon-after-character-reference)
                self.state = State::NumericCharacterReferenceEnd;
                self.reconsume = true;
                None
            }
            None => {
                self.state = State::NumericCharacterReferenceEnd;
                self.reconsume = true;
                None
            }
        }
    }

    /// §13.2.5.80 Numeric character reference end state
    ///
    /// Validate the accumulated code point and emit:
    /// - 0x00 → parse error; emit U+FFFD
    /// - 0x80–0x9F → replace with Windows-1252 equivalent (0x80 → €, etc.)
    /// - Surrogate (0xD800–0xDFFF) → parse error; emit U+FFFD
    /// - > 0x10FFFF → parse error; emit U+FFFD
    /// - Valid → emit the resolved character, return to return_state
    fn handle_numeric_character_reference_end_state(&mut self) -> Option<Token> {
        let code = self.character_reference_code;
        self.character_reference_code = 0;

        let ch = match code {
            0x00 => {
                // TODO: record parse error (null-character-reference)
                '\u{FFFD}'
            }
            // Surrogate range
            0xD800..=0xDFFF => {
                // TODO: record parse error
                '\u{FFFD}'
            }
            // Beyond Unicode
            c if c > 0x10FFFF => {
                // TODO: record parse error
                '\u{FFFD}'
            }
            // Windows-1252 replacement range (0x80–0x9F, except some)
            0x80 => '\u{20AC}', // €
            0x82 => '\u{201A}', // ‚
            0x83 => '\u{0192}', // ƒ
            0x84 => '\u{201E}', // „
            0x85 => '\u{2026}', // …
            0x86 => '\u{2020}', // †
            0x87 => '\u{2021}', // ‡
            0x88 => '\u{02C6}', // ˆ
            0x89 => '\u{2030}', // ‰
            0x8A => '\u{0160}', // Š
            0x8B => '\u{2039}', // ‹
            0x8C => '\u{0152}', // Œ
            0x8E => '\u{017D}', // Ž
            0x91 => '\u{2018}', // '
            0x92 => '\u{2019}', // '
            0x93 => '\u{201C}', // "
            0x94 => '\u{201D}', // "
            0x95 => '\u{2022}', // •
            0x96 => '\u{2013}', // –
            0x97 => '\u{2014}', // —
            0x98 => '\u{02DC}', // ˜
            0x99 => '\u{2122}', // ™
            0x9A => '\u{0161}', // š
            0x9B => '\u{203A}', // ›
            0x9C => '\u{0153}', // œ
            0x9E => '\u{017E}', // ž
            0x9F => '\u{0178}', // Ÿ
            // Valid Unicode
            _ => {
                // Safety: we already checked surrogates and >0x10FFFF
                char::from_u32(code).unwrap_or('\u{FFFD}')
            }
        };

        let state = self.return_state.take().unwrap_or(State::Data);
        // If returning to an attribute value state, append to attr value
        match state {
            State::AttributeValueDoubleQuoted
            | State::AttributeValueSingleQuoted
            | State::AttributeValueUnquoted => {
                self.current_attr_value.push(ch);
            }
            _ => {}
        }
        self.state = state;
        Some(Token::Character(ch))
    }

    // ── Named entity table ────────────────────────────────────

    /// Resolve a named character reference to its Unicode character(s).
    /// Returns `Some(char)` if the entity is known, `None` otherwise.
    ///
    /// The entity name is case-sensitive and does NOT include the leading `&`
    /// or trailing `;`.
    ///
    /// Currently covers the ~30 most common HTML entities.
    /// Full WHATWG entity table (~2200 entities) is a future step.
    fn resolve_named_entity(name: &str) -> Option<char> {
        // Keep entries sorted alphabetically for binary search
        match name {
            "AElig" => Some('\u{00C6}'),
            "Aacute" => Some('\u{00C1}'),
            "Acirc" => Some('\u{00C2}'),
            "Agrave" => Some('\u{00C0}'),
            "Aring" => Some('\u{00C5}'),
            "Atilde" => Some('\u{00C3}'),
            "Auml" => Some('\u{00C4}'),
            "Ccedil" => Some('\u{00C7}'),
            "Eacute" => Some('\u{00C9}'),
            "Egrave" => Some('\u{00C8}'),
            "Iacute" => Some('\u{00CD}'),
            "Icirc" => Some('\u{00CE}'),
            "Igrave" => Some('\u{00CC}'),
            "Ntilde" => Some('\u{00D1}'),
            "Oacute" => Some('\u{00D3}'),
            "Ocirc" => Some('\u{00D4}'),
            "Ograve" => Some('\u{00D2}'),
            "Oslash" => Some('\u{00D8}'),
            "Otilde" => Some('\u{00D5}'),
            "Ouml" => Some('\u{00D6}'),
            "Uacute" => Some('\u{00DA}'),
            "Ugrave" => Some('\u{00D9}'),
            "aacute" => Some('\u{00E1}'),
            "acirc" => Some('\u{00E2}'),
            "acute" => Some('\u{00B4}'),
            "aelig" => Some('\u{00E6}'),
            "agrave" => Some('\u{00E0}'),
            "amp" => Some('\u{0026}'),
            "apos" => Some('\u{0027}'),
            "aring" => Some('\u{00E5}'),
            "atilde" => Some('\u{00E3}'),
            "auml" => Some('\u{00E4}'),
            "brvbar" => Some('\u{00A6}'),
            "ccedil" => Some('\u{00E7}'),
            "cedil" => Some('\u{00B8}'),
            "cent" => Some('\u{00A2}'),
            "copy" => Some('\u{00A9}'),
            "curren" => Some('\u{00A4}'),
            "deg" => Some('\u{00B0}'),
            "divide" => Some('\u{00F7}'),
            "eacute" => Some('\u{00E9}'),
            "ecirc" => Some('\u{00EA}'),
            "egrave" => Some('\u{00E8}'),
            "eth" => Some('\u{00F0}'),
            "euml" => Some('\u{00EB}'),
            "frac12" => Some('\u{00BD}'),
            "frac14" => Some('\u{00BC}'),
            "frac34" => Some('\u{00BE}'),
            "gt" => Some('\u{003E}'),
            "hellip" => Some('\u{2026}'),
            "iacute" => Some('\u{00ED}'),
            "icirc" => Some('\u{00EE}'),
            "iexcl" => Some('\u{00A1}'),
            "igrave" => Some('\u{00EC}'),
            "iquest" => Some('\u{00BF}'),
            "iuml" => Some('\u{00EF}'),
            "laquo" => Some('\u{00AB}'),
            "ldquo" => Some('\u{201C}'),
            "lsquo" => Some('\u{2018}'),
            "lt" => Some('\u{003C}'),
            "macr" => Some('\u{00AF}'),
            "mdash" => Some('\u{2014}'),
            "micro" => Some('\u{00B5}'),
            "middot" => Some('\u{00B7}'),
            "nbsp" => Some('\u{00A0}'),
            "ndash" => Some('\u{2013}'),
            "not" => Some('\u{00AC}'),
            "ntilde" => Some('\u{00F1}'),
            "oacute" => Some('\u{00F3}'),
            "ocirc" => Some('\u{00F4}'),
            "ograve" => Some('\u{00F2}'),
            "ordf" => Some('\u{00AA}'),
            "ordm" => Some('\u{00BA}'),
            "oslash" => Some('\u{00F8}'),
            "otilde" => Some('\u{00F5}'),
            "ouml" => Some('\u{00F6}'),
            "para" => Some('\u{00B6}'),
            "plusmn" => Some('\u{00B1}'),
            "pound" => Some('\u{00A3}'),
            "quot" => Some('\u{0022}'),
            "raquo" => Some('\u{00BB}'),
            "rdquo" => Some('\u{201D}'),
            "reg" => Some('\u{00AE}'),
            "rsquo" => Some('\u{2019}'),
            "sect" => Some('\u{00A7}'),
            "shy" => Some('\u{00AD}'),
            "sup1" => Some('\u{00B9}'),
            "sup2" => Some('\u{00B2}'),
            "sup3" => Some('\u{00B3}'),
            "szlig" => Some('\u{00DF}'),
            "thorn" => Some('\u{00FE}'),
            "times" => Some('\u{00D7}'),
            "trade" => Some('\u{2122}'),
            "uacute" => Some('\u{00FA}'),
            "ucirc" => Some('\u{00FB}'),
            "ugrave" => Some('\u{00F9}'),
            "uml" => Some('\u{00A8}'),
            "uuml" => Some('\u{00FC}'),
            "yacute" => Some('\u{00FD}'),
            "yen" => Some('\u{00A5}'),
            "yuml" => Some('\u{00FF}'),
            _ => None,
        }
    }

    /// §13.2.5.73 Named character reference state
    ///
    /// Consumes ASCII alphanumeric characters to build an entity name,
    /// then looks it up in the entity table.
    ///
    /// - On `;` → lookup match: if found → emit resolved char; else → ambiguous
    /// - On ASCII alphanum → append to buffer, stay
    /// - On anything else → lookup without `;`: if found → emit resolved char +
    ///   reconsume; else → ambiguous ampersand
    /// - EOF → ambiguous ampersand
    fn handle_named_character_reference_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some(';') => {
                // End of entity name — look up accumulated chars
                let entity_char = Self::resolve_named_entity(&self.temporary_buffer);
                match entity_char {
                    Some(ch) => {
                        self.temporary_buffer.clear();
                        let state = self.return_state.take().unwrap_or(State::Data);
                        self.emit_char_to_attr(ch, state);
                        self.state = state;
                        Some(Token::Character(ch))
                    }
                    None => {
                        // Unknown entity: push buffered chars to pending,
                        // reconsume `;` so AmbiguousAmpersand sees it.
                        for ch in self.temporary_buffer.chars().rev() {
                            self.pending_tokens.push(Token::Character(ch));
                        }
                        self.temporary_buffer.clear();
                        self.state = State::AmbiguousAmpersand;
                        self.reconsume = true;
                        None
                    }
                }
            }
            Some(c) if c.is_ascii_alphanumeric() => {
                self.temporary_buffer.push(c);
                None
            }
            Some(_c) => {
                // Non-alphanumeric, non-`;`: try lookup without semicolon
                let entity_char = Self::resolve_named_entity(&self.temporary_buffer);
                self.temporary_buffer.clear();
                match entity_char {
                    Some(ch) => {
                        let state = self.return_state.take().unwrap_or(State::Data);
                        self.emit_char_to_attr(ch, state);
                        self.state = state;
                        self.reconsume = true;
                        Some(Token::Character(ch))
                    }
                    None => {
                        self.state = State::AmbiguousAmpersand;
                        self.reconsume = true;
                        None
                    }
                }
            }
            None => {
                // EOF after starting a named reference → ambiguous
                self.state = State::AmbiguousAmpersand;
                None
            }
        }
    }

    /// §13.2.5.74 Ambiguous ampersand state
    ///
    /// Consume the next input character:
    /// - ASCII alphanumeric → emit the character, stay
    /// - U+003B SEMICOLON (;) → emit `;`, switch to return_state
    /// - Anything else → reconsume in return_state
    fn handle_ambiguous_ampersand_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some(c) if c.is_ascii_alphanumeric() => {
                Some(Token::Character(c))
            }
            Some(';') => {
                let state = self.return_state.take().unwrap_or(State::Data);
                self.state = state;
                Some(Token::Character(';'))
            }
            Some(_c) => {
                let state = self.return_state.take().unwrap_or(State::Data);
                self.state = state;
                self.reconsume = true;
                None
            }
            None => {
                let state = self.return_state.take().unwrap_or(State::Data);
                self.state = state;
                None
            }
        }
    }

    /// Helper: emit a character to current_attr_value if the return state
    /// is an attribute value state.
    fn emit_char_to_attr(&mut self, ch: char, return_state: State) {
        match return_state {
            State::AttributeValueDoubleQuoted
            | State::AttributeValueSingleQuoted
            | State::AttributeValueUnquoted => {
                self.current_attr_value.push(ch);
            }
            _ => {}
        }
    }

    // ── Character reference fallback helpers ──────────────────

    /// Emit `&` and return to the return_state.
    /// Used when an `&` is not followed by a valid character reference.
    /// Per spec §13.2.5.72, the current input character is re-consumed
    /// in the return state.
    fn emit_ampersand_and_return(&mut self) -> Option<Token> {
        let state = self.return_state.take().unwrap_or(State::Data);
        self.state = state;
        self.reconsume = true;
        self.append_char_to_attr_if_needed('&', state);
        Some(Token::Character('&'))
    }

    /// Emit `&#` and return via pending tokens.
    fn emit_ampersand_hash_and_return(&mut self) -> Option<Token> {
        self.pending_tokens.push(Token::Character('#'));
        self.pending_tokens.push(Token::Character('&'));
        let state = self.return_state.take().unwrap_or(State::Data);
        self.state = state;
        self.reconsume = true;
        None
    }

    /// Emit `&#x` and return via pending tokens.
    fn emit_ampersand_hash_x_and_return(&mut self) -> Option<Token> {
        self.pending_tokens.push(Token::Character('x'));
        self.pending_tokens.push(Token::Character('#'));
        self.pending_tokens.push(Token::Character('&'));
        let state = self.return_state.take().unwrap_or(State::Data);
        self.state = state;
        self.reconsume = true;
        None
    }

    /// Append a character to `current_attr_value` if the state is
    /// an attribute value state.
    fn append_char_to_attr_if_needed(&mut self, ch: char, state: State) {
        match state {
            State::AttributeValueDoubleQuoted
            | State::AttributeValueSingleQuoted
            | State::AttributeValueUnquoted => {
                self.current_attr_value.push(ch);
            }
            _ => {}
        }
    }

    // ── Attribute state helpers ───────────────────────────────────

    /// Push the currently accumulated attribute (name + value) into the tag.
    fn emit_current_attribute(&mut self) {
        if let Some(ref mut tag) = self.current_tag {
            let name = std::mem::take(&mut self.current_attr_name);
            let value = std::mem::take(&mut self.current_attr_value);
            tag.attrs.push((name, value));
        }
    }

    // ── Markup declaration (§13.2.5.42) ──────────────────────────

    /// §13.2.5.42 Markup declaration open state
    ///
    /// Dispatches `<!` to comment, DOCTYPE, CDATA, or bogus comment.
    fn handle_markup_declaration_open_state(&mut self) -> Option<Token> {
        // 检查 "--"（注释起始，2 字符）
        if self.pos + 1 < self.input.len()
            && self.input[self.pos] == '-'
            && self.input[self.pos + 1] == '-'
        {
            self.pos += 2;
            self.current_comment.clear();
            self.state = State::CommentStart;
            return None;
        }

        // 检查 "DOCTYPE"（大小写不敏感，7 字符）
        if self.pos + 6 < self.input.len() {
            let slice: String = self.input[self.pos..self.pos + 7].iter().collect();
            if slice.eq_ignore_ascii_case("DOCTYPE") {
                self.pos += 7;
                // TODO: Step 1.4 — DOCTYPE 状态尚未实现
                self.state = State::Doctype;
                return None;
            }
        }

        // 检查 "[CDATA["（7 字符）
        if self.pos + 6 < self.input.len() {
            let slice: String = self.input[self.pos..self.pos + 7].iter().collect();
            if slice == "[CDATA[" {
                self.pos += 7;
                // TODO: Step 1.8 — CDATA 状态尚未实现
                self.state = State::CDATASection;
                return None;
            }
        }

        // 都不匹配 → parse error → BogusComment
        // 注意：不消费任何字符，BogusComment 会自行消费
        self.state = State::BogusComment;
        None
    }

    // ── DOCTYPE helpers ───────────────────────────────────────────

    /// Emit the accumulated `current_doctype` as `Token::Doctype`，
    /// replace with a fresh default, and switch to Data state.
    fn emit_current_doctype(&mut self) -> Token {
        let doctype = std::mem::replace(
            &mut self.current_doctype,
            DoctypeToken {
                name: None,
                public_id: None,
                system_id: None,
                force_quirks: false,
            },
        );
        self.state = State::Data;
        Token::Doctype(doctype)
    }

    // ── DOCTYPE state handlers (§13.2.5.53–§13.2.5.68) ───────────

    /// §13.2.5.53 DOCTYPE state
    ///
    /// 入口：MarkupDeclarationOpen 识别 "DOCTYPE" 后。跳过空白，非空白 reconsume
    /// 到 BeforeDoctypeName。
    fn handle_doctype_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                // 跳过空白
                None
            }
            Some(_c) => {
                // 非空白字符 → reconsume 到 BeforeDoctypeName
                self.state = State::BeforeDoctypeName;
                self.reconsume = true;
                None
            }
            None => {
                // TODO: parse error (eof-in-doctype)
                self.current_doctype.force_quirks = true;
                let token = self.emit_current_doctype();
                Some(token)
            }
        }
    }

    /// §13.2.5.54 Before DOCTYPE name state
    fn handle_before_doctype_name_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                // 跳过空白
                None
            }
            Some('\0') => {
                // TODO: parse error (unexpected-null-character)
                self.current_doctype.name = Some(String::from("\u{FFFD}"));
                self.state = State::DoctypeName;
                None
            }
            Some('>') => {
                // TODO: parse error (missing-doctype-name)
                self.current_doctype.force_quirks = true;
                let token = self.emit_current_doctype();
                Some(token)
            }
            None => {
                // TODO: parse error (eof-in-doctype)
                self.current_doctype.force_quirks = true;
                let token = self.emit_current_doctype();
                Some(token)
            }
            Some(c) => {
                // 创建 DOCTYPE name（ASCII 大写→小写）
                let ch = if c.is_ascii_uppercase() {
                    c.to_ascii_lowercase()
                } else {
                    c
                };
                self.current_doctype.name = Some(String::from(ch));
                self.state = State::DoctypeName;
                None
            }
        }
    }

    /// §13.2.5.55 DOCTYPE name state
    fn handle_doctype_name_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                self.state = State::AfterDoctypeName;
                None
            }
            Some('>') => {
                let token = self.emit_current_doctype();
                Some(token)
            }
            Some('\0') => {
                // TODO: parse error (unexpected-null-character)
                if let Some(ref mut name) = self.current_doctype.name {
                    name.push('\u{FFFD}');
                }
                None
            }
            None => {
                // TODO: parse error (eof-in-doctype)
                self.current_doctype.force_quirks = true;
                let token = self.emit_current_doctype();
                Some(token)
            }
            Some(c) => {
                let ch = if c.is_ascii_uppercase() {
                    c.to_ascii_lowercase()
                } else {
                    c
                };
                if let Some(ref mut name) = self.current_doctype.name {
                    name.push(ch);
                }
                None
            }
        }
    }

    /// §13.2.5.56 After DOCTYPE name state
    fn handle_after_doctype_name_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                None // stay in AfterDoctypeName
            }
            Some('>') => {
                let token = self.emit_current_doctype();
                Some(token)
            }
            None => {
                // TODO: parse error (eof-in-doctype)
                self.current_doctype.force_quirks = true;
                let token = self.emit_current_doctype();
                Some(token)
            }
            Some(_c) => {
                // 尝试匹配 "PUBLIC" 或 "SYSTEM"（大小写不敏感，6 字符）
                // 注意：_c 已经被 next_char() 消费，需从 pos-1 开始比较
                if self.pos + 5 <= self.input.len() {
                    let start = self.pos - 1;
                    let slice: String = self.input[start..start + 6].iter().collect();
                    if slice.eq_ignore_ascii_case("PUBLIC") {
                        self.pos = start + 6;
                        self.state = State::AfterDoctypePublicKeyword;
                        return None;
                    }
                    if slice.eq_ignore_ascii_case("SYSTEM") {
                        self.pos = start + 6;
                        self.state = State::AfterDoctypeSystemKeyword;
                        return None;
                    }
                }
                // 不匹配 → BogusDoctype（force_quirks=true）
                self.current_doctype.force_quirks = true;
                self.state = State::BogusDoctype;
                self.reconsume = true;
                None
            }
        }
    }

    /// §13.2.5.68 Bogus DOCTYPE state
    fn handle_bogus_doctype_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('>') => {
                let token = self.emit_current_doctype();
                Some(token)
            }
            Some('\0') => {
                // TODO: parse error (unexpected-null-character)
                // 忽略字符，不追加
                None
            }
            None => {
                let token = self.emit_current_doctype();
                Some(token)
            }
            Some(_) => {
                // 忽略其他字符
                None
            }
        }
    }

    // ── DOCTYPE PUBLIC 标识符路径 (§13.2.5.57–§13.2.5.62) ───────

    /// §13.2.5.57 After DOCTYPE public keyword state
    fn handle_after_doctype_public_keyword_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                self.state = State::BeforeDoctypePublicId;
                None
            }
            Some('"') | Some('\'') | Some('>') => {
                // TODO: parse error
                self.current_doctype.force_quirks = true;
                let token = self.emit_current_doctype();
                Some(token)
            }
            None => {
                // TODO: parse error (eof-in-doctype)
                self.current_doctype.force_quirks = true;
                let token = self.emit_current_doctype();
                Some(token)
            }
            Some(_) => {
                self.current_doctype.force_quirks = true;
                self.state = State::BogusDoctype;
                None
            }
        }
    }

    /// §13.2.5.58 Before DOCTYPE public identifier state
    fn handle_before_doctype_public_id_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                None // skip whitespace
            }
            Some('"') => {
                self.current_doctype.public_id = Some(String::new());
                self.state = State::DoctypePublicIdDoubleQuoted;
                None
            }
            Some('\'') => {
                self.current_doctype.public_id = Some(String::new());
                self.state = State::DoctypePublicIdSingleQuoted;
                None
            }
            Some('>') | None => {
                // TODO: parse error
                self.current_doctype.force_quirks = true;
                let token = self.emit_current_doctype();
                Some(token)
            }
            Some(_) => {
                // TODO: parse error
                self.current_doctype.force_quirks = true;
                self.state = State::BogusDoctype;
                None
            }
        }
    }

    /// §13.2.5.59 DOCTYPE public identifier (double-quoted) state
    fn handle_doctype_public_id_double_quoted_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('"') => {
                self.state = State::AfterDoctypePublicId;
                None
            }
            Some('>') | None => {
                // TODO: parse error (eof-in-doctype / abrupt-doctype-public-identifier)
                self.current_doctype.force_quirks = true;
                let token = self.emit_current_doctype();
                Some(token)
            }
            Some('\0') => {
                // TODO: parse error (unexpected-null-character)
                if let Some(ref mut id) = self.current_doctype.public_id {
                    id.push('\u{FFFD}');
                }
                None
            }
            Some(c) => {
                if let Some(ref mut id) = self.current_doctype.public_id {
                    id.push(c);
                }
                None
            }
        }
    }

    /// §13.2.5.60 DOCTYPE public identifier (single-quoted) state
    fn handle_doctype_public_id_single_quoted_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\'') => {
                self.state = State::AfterDoctypePublicId;
                None
            }
            Some('>') | None => {
                self.current_doctype.force_quirks = true;
                let token = self.emit_current_doctype();
                Some(token)
            }
            Some('\0') => {
                if let Some(ref mut id) = self.current_doctype.public_id {
                    id.push('\u{FFFD}');
                }
                None
            }
            Some(c) => {
                if let Some(ref mut id) = self.current_doctype.public_id {
                    id.push(c);
                }
                None
            }
        }
    }

    /// §13.2.5.61 After DOCTYPE public identifier state
    fn handle_after_doctype_public_id_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                self.state = State::BetweenDoctypePublicAndSystemIds;
                None
            }
            Some('>') => {
                let token = self.emit_current_doctype();
                Some(token)
            }
            Some('"') | Some('\'') => {
                // TODO: parse error (unexpected-quote-before-doctype-system-id)
                self.current_doctype.force_quirks = true;
                self.state = State::BogusDoctype;
                None
            }
            None => {
                self.current_doctype.force_quirks = true;
                let token = self.emit_current_doctype();
                Some(token)
            }
            Some(_) => {
                self.current_doctype.force_quirks = true;
                self.state = State::BogusDoctype;
                None
            }
        }
    }

    /// §13.2.5.62 Between DOCTYPE public and system identifiers state
    fn handle_between_doctype_public_and_system_ids_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                None
            }
            Some('>') => {
                let token = self.emit_current_doctype();
                Some(token)
            }
            Some('"') => {
                self.current_doctype.system_id = Some(String::new());
                self.state = State::DoctypeSystemIdDoubleQuoted;
                None
            }
            Some('\'') => {
                self.current_doctype.system_id = Some(String::new());
                self.state = State::DoctypeSystemIdSingleQuoted;
                None
            }
            None => {
                self.current_doctype.force_quirks = true;
                let token = self.emit_current_doctype();
                Some(token)
            }
            Some(_c) => {
                // 尝试匹配 "SYSTEM"
                if self.pos + 5 <= self.input.len() {
                    let start = self.pos - 1;
                    let slice: String = self.input[start..start + 6].iter().collect();
                    if slice.eq_ignore_ascii_case("SYSTEM") {
                        self.pos = start + 6;
                        self.state = State::AfterDoctypeSystemKeyword;
                        return None;
                    }
                }
                self.current_doctype.force_quirks = true;
                self.state = State::BogusDoctype;
                self.reconsume = true;
                None
            }
        }
    }

    // ── DOCTYPE SYSTEM 标识符路径 (§13.2.5.63–§13.2.5.67) ───────

    /// §13.2.5.63 After DOCTYPE system keyword state
    fn handle_after_doctype_system_keyword_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                self.state = State::BeforeDoctypeSystemId;
                None
            }
            Some('"') | Some('\'') | Some('>') => {
                self.current_doctype.force_quirks = true;
                let token = self.emit_current_doctype();
                Some(token)
            }
            None => {
                self.current_doctype.force_quirks = true;
                let token = self.emit_current_doctype();
                Some(token)
            }
            Some(_) => {
                self.current_doctype.force_quirks = true;
                self.state = State::BogusDoctype;
                None
            }
        }
    }

    /// §13.2.5.64 Before DOCTYPE system identifier state
    fn handle_before_doctype_system_id_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                None
            }
            Some('"') => {
                self.current_doctype.system_id = Some(String::new());
                self.state = State::DoctypeSystemIdDoubleQuoted;
                None
            }
            Some('\'') => {
                self.current_doctype.system_id = Some(String::new());
                self.state = State::DoctypeSystemIdSingleQuoted;
                None
            }
            Some('>') | None => {
                self.current_doctype.force_quirks = true;
                let token = self.emit_current_doctype();
                Some(token)
            }
            Some(_) => {
                self.current_doctype.force_quirks = true;
                self.state = State::BogusDoctype;
                None
            }
        }
    }

    /// §13.2.5.65 DOCTYPE system identifier (double-quoted) state
    fn handle_doctype_system_id_double_quoted_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('"') => {
                self.state = State::AfterDoctypeSystemId;
                None
            }
            Some('>') | None => {
                self.current_doctype.force_quirks = true;
                let token = self.emit_current_doctype();
                Some(token)
            }
            Some('\0') => {
                if let Some(ref mut id) = self.current_doctype.system_id {
                    id.push('\u{FFFD}');
                }
                None
            }
            Some(c) => {
                if let Some(ref mut id) = self.current_doctype.system_id {
                    id.push(c);
                }
                None
            }
        }
    }

    /// §13.2.5.66 DOCTYPE system identifier (single-quoted) state
    fn handle_doctype_system_id_single_quoted_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\'') => {
                self.state = State::AfterDoctypeSystemId;
                None
            }
            Some('>') | None => {
                self.current_doctype.force_quirks = true;
                let token = self.emit_current_doctype();
                Some(token)
            }
            Some('\0') => {
                if let Some(ref mut id) = self.current_doctype.system_id {
                    id.push('\u{FFFD}');
                }
                None
            }
            Some(c) => {
                if let Some(ref mut id) = self.current_doctype.system_id {
                    id.push(c);
                }
                None
            }
        }
    }

    /// §13.2.5.67 After DOCTYPE system identifier state
    fn handle_after_doctype_system_id_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                None
            }
            Some('>') => {
                let token = self.emit_current_doctype();
                Some(token)
            }
            None => {
                self.current_doctype.force_quirks = true;
                let token = self.emit_current_doctype();
                Some(token)
            }
            Some(_) => {
                // TODO: parse error (unexpected-character-after-doctype-system-identifier)
                self.state = State::BogusDoctype;
                None
            }
        }
    }

    /// §13.2.5.41 Bogus comment state
    ///
    /// 累积字符直到 `>`，作为 `Token::Comment` 发出。
    /// 入口：TagOpen 遇到 `?`，或 MarkupDeclarationOpen 无法匹配。
    fn handle_bogus_comment_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('>') => {
                let comment = std::mem::take(&mut self.current_comment);
                self.state = State::Data;
                Some(Token::Comment(comment))
            }
            Some('\0') => {
                // TODO: parse error (unexpected-null-character)
                self.current_comment.push('\u{FFFD}');
                None
            }
            None => {
                // 发出 Comment，切换到 Data 让 Data 状态在下一次调用时发出 EOF
                let comment = std::mem::take(&mut self.current_comment);
                self.state = State::Data;
                Some(Token::Comment(comment))
            }
            Some(c) => {
                self.current_comment.push(c);
                None
            }
        }
    }

    // ── Comment state handlers (§13.2.5.43–§13.2.5.45) ───────────

    /// §13.2.5.43 Comment start state
    ///
    /// 进入时机：`<!--` 已消费，当前字符为注释内容第一个字符。
    fn handle_comment_start_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('-') => {
                self.state = State::CommentStartDash;
                None
            }
            Some('>') => {
                // 空注释
                self.current_comment.clear();
                self.state = State::Data;
                Some(Token::Comment(String::new()))
            }
            Some('<') => {
                self.state = State::CommentLessThanSign;
                None
            }
            Some('\0') => {
                // TODO: parse error (unexpected-null-character)
                self.current_comment.push('\u{FFFD}');
                self.state = State::Comment;
                None
            }
            None => {
                // TODO: parse error (eof-in-comment)
                self.current_comment.clear();
                self.state = State::Data;
                Some(Token::Comment(String::new()))
            }
            Some(c) => {
                self.current_comment.push(c);
                self.state = State::Comment;
                None
            }
        }
    }

    /// §13.2.5.44 Comment start dash state
    fn handle_comment_start_dash_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('-') => {
                self.state = State::CommentEnd;
                None
            }
            Some('>') => {
                // TODO: parse error (abrupt-closing-of-empty-comment)
                self.current_comment.clear();
                self.state = State::Data;
                Some(Token::Comment(String::new()))
            }
            None => {
                // TODO: parse error (eof-in-comment)
                self.current_comment.clear();
                self.state = State::Data;
                Some(Token::Comment(String::new()))
            }
            Some(c) => {
                self.current_comment.push('-');
                self.current_comment.push(c);
                self.state = State::Comment;
                None
            }
        }
    }

    /// §13.2.5.45 Comment state
    fn handle_comment_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('<') => {
                self.state = State::CommentLessThanSign;
                None
            }
            Some('-') => {
                self.state = State::CommentEndDash;
                None
            }
            Some('\0') => {
                // TODO: parse error (unexpected-null-character)
                self.current_comment.push('\u{FFFD}');
                None
            }
            None => {
                // TODO: parse error (eof-in-comment)
                let comment = std::mem::take(&mut self.current_comment);
                self.state = State::Data;
                Some(Token::Comment(comment))
            }
            Some(c) => {
                self.current_comment.push(c);
                None
            }
        }
    }

    // ── Comment < 系列 (§13.2.5.46–§13.2.5.49) ─────────────────

    /// §13.2.5.46 Comment less-than sign state
    fn handle_comment_less_than_sign_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('!') => {
                self.state = State::CommentLessThanSignBang;
                None
            }
            Some('<') => {
                self.current_comment.push('<');
                None
            }
            Some(_c) => {
                self.reconsume = true;
                self.state = State::Comment;
                None
            }
            None => {
                // TODO: parse error (eof-in-comment)
                let comment = std::mem::take(&mut self.current_comment);
                self.state = State::Data;
                Some(Token::Comment(comment))
            }
        }
    }

    /// §13.2.5.47 Comment less-than sign bang state
    fn handle_comment_less_than_sign_bang_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('-') => {
                self.state = State::CommentLessThanSignBangDash;
                None
            }
            Some(_c) => {
                self.reconsume = true;
                self.state = State::Comment;
                None
            }
            None => {
                // TODO: parse error (eof-in-comment)
                let comment = std::mem::take(&mut self.current_comment);
                self.state = State::Data;
                Some(Token::Comment(comment))
            }
        }
    }

    /// §13.2.5.48 Comment less-than sign bang dash state
    fn handle_comment_less_than_sign_bang_dash_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('-') => {
                self.state = State::CommentLessThanSignBangDashDash;
                None
            }
            Some(_c) => {
                self.reconsume = true;
                self.state = State::Comment;
                None
            }
            None => {
                // TODO: parse error (eof-in-comment)
                let comment = std::mem::take(&mut self.current_comment);
                self.state = State::Data;
                Some(Token::Comment(comment))
            }
        }
    }

    /// §13.2.5.49 Comment less-than sign bang dash dash state
    fn handle_comment_less_than_sign_bang_dash_dash_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('>') => {
                self.state = State::Comment;
                None
            }
            Some(_c) => {
                self.reconsume = true;
                self.state = State::Comment;
                None
            }
            None => {
                // TODO: parse error (eof-in-comment)
                let comment = std::mem::take(&mut self.current_comment);
                self.state = State::Data;
                Some(Token::Comment(comment))
            }
        }
    }

    // ── Comment end 系列 (§13.2.5.50–§13.2.5.52) ────────────────

    /// §13.2.5.50 Comment end dash state
    fn handle_comment_end_dash_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('-') => {
                self.state = State::CommentEnd;
                None
            }
            Some(_c) => {
                self.current_comment.push('-');
                self.reconsume = true;
                self.state = State::Comment;
                None
            }
            None => {
                // TODO: parse error (eof-in-comment)
                let comment = std::mem::take(&mut self.current_comment);
                self.state = State::Data;
                Some(Token::Comment(comment))
            }
        }
    }

    /// §13.2.5.51 Comment end state
    fn handle_comment_end_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('>') => {
                let comment = std::mem::take(&mut self.current_comment);
                self.state = State::Data;
                Some(Token::Comment(comment))
            }
            Some('!') => {
                self.state = State::CommentEndBang;
                None
            }
            Some('-') => {
                // 吃掉多余的 '-'
                self.current_comment.push('-');
                None
            }
            Some(_c) => {
                self.current_comment.push_str("--");
                self.reconsume = true;
                self.state = State::Comment;
                None
            }
            None => {
                // TODO: parse error (eof-in-comment)
                let comment = std::mem::take(&mut self.current_comment);
                self.state = State::Data;
                Some(Token::Comment(comment))
            }
        }
    }

    /// §13.2.5.52 Comment end bang state
    fn handle_comment_end_bang_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('-') => {
                self.current_comment.push_str("--!");
                self.state = State::CommentEnd;
                None
            }
            Some('>') => {
                let comment = std::mem::take(&mut self.current_comment);
                self.state = State::Data;
                Some(Token::Comment(comment))
            }
            Some(_c) => {
                self.current_comment.push_str("--!");
                self.reconsume = true;
                self.state = State::Comment;
                None
            }
            None => {
                // TODO: parse error (eof-in-comment)
                let comment = std::mem::take(&mut self.current_comment);
                self.state = State::Data;
                Some(Token::Comment(comment))
            }
        }
    }

    // ── Attribute state handlers (§13.2.5.32–§13.2.5.39) ────────

    /// §13.2.5.32 Before attribute name state
    fn handle_before_attribute_name_state(&mut self) -> Option<Token> {
        match self.next_char() {
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
                self.current_tag = None;
                self.eof_emitted = true;
                Some(Token::EOF)
            }
            Some(_c) => {
                self.state = State::AttributeName;
                self.reconsume = true;
                None
            }
        }
    }

    /// §13.2.5.33 Attribute name state
    fn handle_attribute_name_state(&mut self) -> Option<Token> {
        let ch = self.next_char();
        let result = match ch {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                self.state = State::AfterAttributeName;
                None
            }
            Some('/') => {
                self.emit_current_attribute();
                self.state = State::SelfClosingStartTag;
                None
            }
            Some('=') => {
                self.state = State::BeforeAttributeValue;
                None
            }
            Some('>') => {
                self.emit_current_attribute();
                let tag = self.current_tag.take().unwrap();
                self.state = State::Data;
                Some(Token::Tag(tag))
            }
            Some('"') => {
                self.emit_current_attribute();
                self.current_attr_value.clear();
                self.state = State::AttributeValueDoubleQuoted;
                None
            }
            Some('\'') => {
                self.emit_current_attribute();
                self.current_attr_value.clear();
                self.state = State::AttributeValueSingleQuoted;
                None
            }
            Some('\0') => {
                self.current_attr_name.push('\u{FFFD}');
                self.state = State::AttributeName;
                None
            }
            None => {
                self.current_tag = None;
                self.eof_emitted = true;
                Some(Token::EOF)
            }
            Some(c) => {
                // §13.2.5.33: ASCII upper-alpha → lowercase
                if c.is_ascii_uppercase() {
                    self.current_attr_name.push(c.to_ascii_lowercase());
                } else {
                    self.current_attr_name.push(c);
                }
                self.state = State::AttributeName;
                None
            }
        };
        result
    }

    /// §13.2.5.34 After attribute name state
    fn handle_after_attribute_name_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                self.state = State::AfterAttributeName;
                None
            }
            Some('/') => {
                self.emit_current_attribute();
                self.state = State::SelfClosingStartTag;
                None
            }
            Some('=') => {
                self.state = State::BeforeAttributeValue;
                None
            }
            Some('>') => {
                self.emit_current_attribute();
                let tag = self.current_tag.take().unwrap();
                self.state = State::Data;
                Some(Token::Tag(tag))
            }
            None => {
                self.current_tag = None;
                self.eof_emitted = true;
                Some(Token::EOF)
            }
            Some(_c) => {
                // Unexpected-character-after-attribute-name parse error
                self.state = State::AttributeName;
                self.reconsume = true;
                None
            }
        }
    }

    /// §13.2.5.35 Before attribute value state
    fn handle_before_attribute_value_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                self.state = State::BeforeAttributeValue;
                None
            }
            Some('"') => {
                self.current_attr_value.clear();
                self.state = State::AttributeValueDoubleQuoted;
                None
            }
            Some('\'') => {
                self.current_attr_value.clear();
                self.state = State::AttributeValueSingleQuoted;
                None
            }
            Some('>') => {
                // missing-attribute-value parse error: emit attr with empty value
                self.emit_current_attribute();
                let tag = self.current_tag.take().unwrap();
                self.state = State::Data;
                Some(Token::Tag(tag))
            }
            None => {
                self.current_tag = None;
                self.eof_emitted = true;
                Some(Token::EOF)
            }
            Some(_c) => {
                self.current_attr_value.clear();
                self.state = State::AttributeValueUnquoted;
                self.reconsume = true;
                None
            }
        }
    }

    /// §13.2.5.36 Attribute value (double-quoted) state
    fn handle_attribute_value_double_quoted_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('"') => {
                self.state = State::AfterAttributeValueQuoted;
                None
            }
            Some('&') => {
                self.return_state = Some(State::AttributeValueDoubleQuoted);
                self.state = State::CharacterReference;
                None
            }
            Some('\0') => {
                // unexpected-null-character parse error
                self.current_attr_value.push('\u{FFFD}');
                self.state = State::AttributeValueDoubleQuoted;
                None
            }
            None => {
                self.current_tag = None;
                self.eof_emitted = true;
                Some(Token::EOF)
            }
            Some(c) => {
                self.current_attr_value.push(c);
                self.state = State::AttributeValueDoubleQuoted;
                None
            }
        }
    }

    /// §13.2.5.37 Attribute value (single-quoted) state
    fn handle_attribute_value_single_quoted_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\'') => {
                self.state = State::AfterAttributeValueQuoted;
                None
            }
            Some('&') => {
                self.return_state = Some(State::AttributeValueSingleQuoted);
                self.state = State::CharacterReference;
                None
            }
            Some('\0') => {
                self.current_attr_value.push('\u{FFFD}');
                self.state = State::AttributeValueSingleQuoted;
                None
            }
            None => {
                self.current_tag = None;
                self.eof_emitted = true;
                Some(Token::EOF)
            }
            Some(c) => {
                self.current_attr_value.push(c);
                self.state = State::AttributeValueSingleQuoted;
                None
            }
        }
    }

    /// §13.2.5.38 Attribute value (unquoted) state
    fn handle_attribute_value_unquoted_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                self.emit_current_attribute();
                self.state = State::BeforeAttributeName;
                None
            }
            Some('&') => {
                self.return_state = Some(State::AttributeValueUnquoted);
                self.state = State::CharacterReference;
                None
            }
            Some('>') => {
                self.emit_current_attribute();
                let tag = self.current_tag.take().unwrap();
                self.state = State::Data;
                Some(Token::Tag(tag))
            }
            Some('\0') => {
                self.current_attr_value.push('\u{FFFD}');
                self.state = State::AttributeValueUnquoted;
                None
            }
            None => {
                self.current_tag = None;
                self.eof_emitted = true;
                Some(Token::EOF)
            }
            Some(c) => {
                self.current_attr_value.push(c);
                self.state = State::AttributeValueUnquoted;
                None
            }
        }
    }

    /// §13.2.5.39 After attribute value (quoted) state
    fn handle_after_attribute_value_quoted_state(&mut self) -> Option<Token> {
        match self.next_char() {
            Some('\t') | Some('\n') | Some('\u{000C}') | Some(' ') => {
                self.emit_current_attribute();
                self.state = State::BeforeAttributeName;
                None
            }
            Some('/') => {
                self.emit_current_attribute();
                self.state = State::SelfClosingStartTag;
                None
            }
            Some('>') => {
                self.emit_current_attribute();
                let tag = self.current_tag.take().unwrap();
                self.state = State::Data;
                Some(Token::Tag(tag))
            }
            None => {
                self.current_tag = None;
                self.eof_emitted = true;
                Some(Token::EOF)
            }
            Some(_c) => {
                // Unexpected-character-after-quoted-attribute-value parse error
                self.emit_current_attribute();
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

    // ── Markup declaration tests (§13.2.5.42) ─────────────────────

    #[test]
    fn markup_declaration_open_dash_dash_to_comment_start() {
        // `<!--` → MarkupDeclarationOpen → CommentStart
        let mut t = HtmlTokenizer::new("<!--");
        assert_eq!(t.next_token(), None); // Data → TagOpen ('<')
        assert_eq!(t.next_token(), None); // TagOpen → MarkupDeclarationOpen ('!')
        assert_eq!(t.state(), State::MarkupDeclarationOpen);
        assert_eq!(t.next_token(), None); // MarkupDeclarationOpen → CommentStart ("--")
        assert_eq!(t.state(), State::CommentStart);
    }

    #[test]
    fn markup_declaration_open_doctype() {
        // `<!DOCTYPE` → MarkupDeclarationOpen → Doctype → EOF → emit force_quirks Doctype
        let mut t = HtmlTokenizer::new("<!DOCTYPE");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → MarkupDeclarationOpen
        assert_eq!(t.state(), State::MarkupDeclarationOpen);
        assert_eq!(t.next_token(), None); // MarkupDeclarationOpen → Doctype
        // Doctype 遇到 EOF：force_quirks=true, emit
        assert_eq!(
            t.next_token(),
            Some(Token::Doctype(DoctypeToken {
                name: None,
                public_id: None,
                system_id: None,
                force_quirks: true,
            }))
        );
    }

    // ── DOCTYPE 测试 (§13.2.5.53) ─────────────────────────────────

    /// 辅助：推进到 Doctype 状态（!DOCTYPE 已消费）
    fn enter_doctype(input: &str) -> HtmlTokenizer {
        let mut t = HtmlTokenizer::new(input);
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → MarkupDeclarationOpen
        assert_eq!(t.next_token(), None); // MarkupDeclarationOpen → Doctype
        assert_eq!(t.state(), State::Doctype);
        t
    }

    #[test]
    fn doctype_entry_skips_whitespace() {
        // `<!DOCTYPE html>` → 跳过 Doctype 中的空白，进入 BeforeDoctypeName
        let mut t = enter_doctype("<!DOCTYPE html>");
        // ' ' → stay in Doctype
        assert_eq!(t.next_token(), None);
        assert_eq!(t.state(), State::Doctype);
        // 'h' → reconsume → BeforeDoctypeName
        assert_eq!(t.next_token(), None);
        assert_eq!(t.state(), State::BeforeDoctypeName);
    }

    #[test]
    fn doctype_entry_non_whitespace_immediate() {
        // `<!DOCTYPEhtml>` → 直接进入 BeforeDoctypeName（reconsume）
        let mut t = enter_doctype("<!DOCTYPEhtml>");
        // 'h' → reconsume → BeforeDoctypeName
        assert_eq!(t.next_token(), None);
        assert_eq!(t.state(), State::BeforeDoctypeName);
    }

    // ── DOCTYPE 名称测试 (§13.2.5.54–§13.2.5.55) ─────────────────

    /// 辅助：推进到 BeforeDoctypeName（Doctype 已跳过空白）
    fn enter_before_doctype_name(input: &str) -> HtmlTokenizer {
        let mut t = enter_doctype(input);
        while t.state() == State::Doctype {
            assert_eq!(t.next_token(), None);
        }
        assert_eq!(t.state(), State::BeforeDoctypeName);
        t
    }

    #[test]
    fn doctype_name_simple_html() {
        let mut t = enter_before_doctype_name("<!DOCTYPE html>");
        assert_eq!(t.next_token(), None); // 'h' → DoctypeName
        assert_eq!(t.state(), State::DoctypeName);
        for _ in 0..3 { assert_eq!(t.next_token(), None); } // 't','m','l'
        assert_eq!(
            t.next_token(),
            Some(Token::Doctype(DoctypeToken {
                name: Some("html".into()),
                public_id: None,
                system_id: None,
                force_quirks: false,
            }))
        );
    }

    #[test]
    fn doctype_name_uppercase() {
        let mut t = enter_before_doctype_name("<!DOCTYPE HTML>");
        assert_eq!(t.next_token(), None); // 'H'→'h'
        assert_eq!(t.next_token(), None); // 'T'→'t'
        assert_eq!(t.next_token(), None); // 'M'→'m'
        assert_eq!(t.next_token(), None); // 'L'→'l'
        assert_eq!(
            t.next_token(),
            Some(Token::Doctype(DoctypeToken {
                name: Some("html".into()),
                public_id: None,
                system_id: None,
                force_quirks: false,
            }))
        );
    }

    #[test]
    fn doctype_name_null_char() {
        let mut t = enter_before_doctype_name("<!DOCTYPE html\0x>");
        for _ in 0..4 { assert_eq!(t.next_token(), None); } // 'h','t','m','l'
        assert_eq!(t.next_token(), None); // '\0' → U+FFFD
        assert_eq!(t.next_token(), None); // 'x'
        assert_eq!(
            t.next_token(),
            Some(Token::Doctype(DoctypeToken {
                name: Some("html\u{FFFD}x".into()),
                public_id: None,
                system_id: None,
                force_quirks: false,
            }))
        );
    }

    #[test]
    fn doctype_before_name_empty_gt() {
        let mut t = enter_doctype("<!DOCTYPE >");
        assert_eq!(t.next_token(), None); // ' ' → stay Doctype
        assert_eq!(t.next_token(), None); // '>' → reconsume, BeforeDoctypeName
        assert_eq!(
            t.next_token(), // BeforeDoctypeName 处理 '>' → force_quirks emit
            Some(Token::Doctype(DoctypeToken {
                name: None,
                public_id: None,
                system_id: None,
                force_quirks: true,
            }))
        );
    }

    #[test]
    fn doctype_name_eof() {
        let mut t = enter_before_doctype_name("<!DOCTYPE html");
        for _ in 0..4 { assert_eq!(t.next_token(), None); }
        assert_eq!(
            t.next_token(), // EOF → force_quirks emit
            Some(Token::Doctype(DoctypeToken {
                name: Some("html".into()),
                public_id: None,
                system_id: None,
                force_quirks: true,
            }))
        );
    }

    // ── AfterDoctypeName + BogusDoctype 测试 (§13.2.5.56, §13.2.5.68) ─

    #[test]
    fn doctype_after_name_public_keyword() {
        let mut t = enter_before_doctype_name("<!DOCTYPE html PUBLIC \"-//EN\">");
        // 'h','t','m','l'
        for _ in 0..4 { assert_eq!(t.next_token(), None); }
        // ' ' → AfterDoctypeName
        assert_eq!(t.next_token(), None); // skip whitespace in AfterDoctypeName
        assert_eq!(t.state(), State::AfterDoctypeName);
        // 'P' → matches "PUBLIC"
        assert_eq!(t.next_token(), None);
        assert_eq!(t.state(), State::AfterDoctypePublicKeyword);
    }

    #[test]
    fn doctype_after_name_system_keyword() {
        let mut t = enter_before_doctype_name("<!DOCTYPE html SYSTEM \"about:\">");
        for _ in 0..4 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), None); // ' ' → AfterDoctypeName
        assert_eq!(t.state(), State::AfterDoctypeName);
        assert_eq!(t.next_token(), None); // 'S' → matches "SYSTEM"
        assert_eq!(t.state(), State::AfterDoctypeSystemKeyword);
    }

    #[test]
    fn doctype_after_name_gt_emits() {
        let mut t = enter_before_doctype_name("<!DOCTYPE html>");
        for _ in 0..4 { assert_eq!(t.next_token(), None); }
        assert_eq!(
            t.next_token(),
            Some(Token::Doctype(DoctypeToken {
                name: Some("html".into()),
                public_id: None,
                system_id: None,
                force_quirks: false,
            }))
        );
    }

    #[test]
    fn doctype_after_name_unknown_to_bogus() {
        // `<!DOCTYPE html x>` → 'x' 不匹配 PUBLIC/SYSTEM → BogusDoctype
        let mut t = enter_before_doctype_name("<!DOCTYPE html x>");
        for _ in 0..4 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), None); // ' ' → AfterDoctypeName
        assert_eq!(t.next_token(), None); // 'x' → BogusDoctype (reconsume)
        assert_eq!(t.state(), State::BogusDoctype);
        // BogusDoctype: ignore 'x' (reconsumed) → '>' emit
        assert_eq!(t.next_token(), None); // 'x' ignored by BogusDoctype
        assert_eq!(
            t.next_token(), // '>' emit
            Some(Token::Doctype(DoctypeToken {
                name: Some("html".into()),
                public_id: None,
                system_id: None,
                force_quirks: true,
            }))
        );
    }

    #[test]
    fn doctype_bogus_ignores_chars() {
        let mut t = enter_before_doctype_name("<!DOCTYPE html foo>");
        for _ in 0..4 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), None); // ' ' → AfterDoctypeName
        assert_eq!(t.next_token(), None); // 'f' → BogusDoctype (reconsume)
        // BogusDoctype 忽略 'f','o','o'
        for _ in 0..3 { assert_eq!(t.next_token(), None); }
        assert_eq!(
            t.next_token(), // '>' emit
            Some(Token::Doctype(DoctypeToken {
                name: Some("html".into()),
                public_id: None,
                system_id: None,
                force_quirks: true,
            }))
        );
    }

    // ── DOCTYPE PUBLIC/SYSTEM 标识符集成测试 ──────────────────────

    #[test]
    fn doctype_public_id_double_quoted() {
        let mut t = HtmlTokenizer::new(
            "<!DOCTYPE html PUBLIC \"-//W3C//DTD HTML 4.01//EN\">"
        );
        assert_eq!(
            next_real_token(&mut t),
            Token::Doctype(DoctypeToken {
                name: Some("html".into()),
                public_id: Some("-//W3C//DTD HTML 4.01//EN".into()),
                system_id: None,
                force_quirks: false,
            })
        );
    }

    #[test]
    fn doctype_system_id_double_quoted() {
        let mut t = HtmlTokenizer::new(
            "<!DOCTYPE html SYSTEM \"about:legacy-compat\">"
        );
        assert_eq!(
            next_real_token(&mut t),
            Token::Doctype(DoctypeToken {
                name: Some("html".into()),
                public_id: None,
                system_id: Some("about:legacy-compat".into()),
                force_quirks: false,
            })
        );
    }

    #[test]
    fn doctype_full_public_and_system() {
        let mut t = HtmlTokenizer::new(
            "<!DOCTYPE html PUBLIC \"-//W3C//DTD HTML 4.01//EN\" \
             \"http://www.w3.org/TR/html4/strict.dtd\">"
        );
        assert_eq!(
            next_real_token(&mut t),
            Token::Doctype(DoctypeToken {
                name: Some("html".into()),
                public_id: Some("-//W3C//DTD HTML 4.01//EN".into()),
                system_id: Some("http://www.w3.org/TR/html4/strict.dtd".into()),
                force_quirks: false,
            })
        );
    }

    #[test]
    fn doctype_public_id_single_quoted() {
        let mut t = HtmlTokenizer::new(
            "<!DOCTYPE html PUBLIC '-//W3C//DTD XHTML 1.0//EN'>"
        );
        assert_eq!(
            next_real_token(&mut t),
            Token::Doctype(DoctypeToken {
                name: Some("html".into()),
                public_id: Some("-//W3C//DTD XHTML 1.0//EN".into()),
                system_id: None,
                force_quirks: false,
            })
        );
    }

    #[test]
    fn doctype_system_id_single_quoted() {
        let mut t = HtmlTokenizer::new(
            "<!DOCTYPE html SYSTEM 'about:legacy-compat'>"
        );
        assert_eq!(
            next_real_token(&mut t),
            Token::Doctype(DoctypeToken {
                name: Some("html".into()),
                public_id: None,
                system_id: Some("about:legacy-compat".into()),
                force_quirks: false,
            })
        );
    }

    #[test]
    fn markup_declaration_open_bogus() {
        // `<!foo` → MarkupDeclarationOpen → BogusComment
        let mut t = HtmlTokenizer::new("<!foo");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → MarkupDeclarationOpen
        assert_eq!(t.next_token(), None); // → BogusComment
        assert_eq!(t.state(), State::BogusComment);
    }

    // ── Bogus comment tests (§13.2.5.41) ──────────────────────────

    #[test]
    fn bogus_comment_emits_on_greater_than() {
        // `<?xml>` → Token::Comment("?xml")
        let mut t = HtmlTokenizer::new("<?xml>");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → BogusComment ('?')
        // 'x', 'm', 'l'
        for _ in 0..3 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), Some(Token::Comment("?xml".into())));
    }

    #[test]
    fn bogus_comment_handles_null() {
        // `<?a\0b>` → Token::Comment("?a\u{FFFD}b")
        let mut t = HtmlTokenizer::new("<?a\0b>");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → BogusComment ('?')
        assert_eq!(t.next_token(), None); // 'a'
        assert_eq!(t.next_token(), None); // '\0' → U+FFFD
        assert_eq!(t.next_token(), None); // 'b'
        assert_eq!(
            t.next_token(),
            Some(Token::Comment("?a\u{FFFD}b".into()))
        );
    }

    #[test]
    fn bogus_comment_eof() {
        // `<?x` + EOF → Token::Comment("?x") + EOF
        let mut t = HtmlTokenizer::new("<?x");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → BogusComment
        assert_eq!(t.next_token(), None); // 'x'
        assert_eq!(t.next_token(), Some(Token::Comment("?x".into())));
        assert_eq!(t.next_token(), Some(Token::EOF));
    }

    #[test]
    fn bogus_comment_from_bang() {
        // `<!foo>` → MarkupDeclarationOpen → BogusComment → Token::Comment("foo")
        let mut t = HtmlTokenizer::new("<!foo>");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → MarkupDeclarationOpen
        assert_eq!(t.next_token(), None); // MarkupDeclarationOpen → BogusComment
        // 'f', 'o', 'o'
        for _ in 0..3 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), Some(Token::Comment("foo".into())));
    }

    // ── Comment state tests (§13.2.5.43–§13.2.5.45) ──────────────

    /// 辅助：推进 tokenizer 到 MarkupDeclarationOpen 的 '!' 之后
    /// 调用后 pos 在 '!' 之后，state = TagOpen 刚设置 MarkupDeclarationOpen 但还未执行
    fn enter_markup_declaration(t: &mut HtmlTokenizer) {
        // Data → TagOpen → MarkupDeclarationOpen (doesn't consume yet)
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → MarkupDeclarationOpen (sees '!')
    }

    #[test]
    fn comment_start_dash_to_comment_start_dash() {
        let mut t = HtmlTokenizer::new("<!---");
        enter_markup_declaration(&mut t);
        assert_eq!(t.next_token(), None); // MarkupDeclarationOpen → CommentStart ("--")
        assert_eq!(t.next_token(), None); // CommentStart → CommentStartDash ('-')
        assert_eq!(t.state(), State::CommentStartDash);
    }

    #[test]
    fn comment_start_empty_comment_on_gt() {
        let mut t = HtmlTokenizer::new("<!-->");
        enter_markup_declaration(&mut t);
        assert_eq!(t.next_token(), None); // MarkupDeclarationOpen → CommentStart
        assert_eq!(t.next_token(), Some(Token::Comment("".into()))); // '>' → emit empty
    }

    #[test]
    fn comment_start_lt_to_comment_lt_sign() {
        let mut t = HtmlTokenizer::new("<!--<");
        enter_markup_declaration(&mut t);
        assert_eq!(t.next_token(), None); // MarkupDeclarationOpen → CommentStart
        assert_eq!(t.next_token(), None); // '<' → CommentLessThanSign
        assert_eq!(t.state(), State::CommentLessThanSign);
    }

    #[test]
    fn comment_start_null_to_comment() {
        let mut t = HtmlTokenizer::new("<!--\0");
        enter_markup_declaration(&mut t);
        assert_eq!(t.next_token(), None); // MarkupDeclarationOpen → CommentStart
        assert_eq!(t.next_token(), None); // '\0' → Comment (with U+FFFD)
        assert_eq!(t.state(), State::Comment);
    }

    #[test]
    fn comment_start_dash_gt_emits_empty() {
        let mut t = HtmlTokenizer::new("<!--->");
        enter_markup_declaration(&mut t);
        assert_eq!(t.next_token(), None); // MarkupDeclarationOpen → CommentStart
        assert_eq!(t.next_token(), None); // '-' → CommentStartDash
        assert_eq!(t.next_token(), Some(Token::Comment("".into()))); // '>' → emit
    }

    #[test]
    fn comment_start_dash_other_to_comment() {
        let mut t = HtmlTokenizer::new("<!---a");
        enter_markup_declaration(&mut t);
        assert_eq!(t.next_token(), None); // MarkupDeclarationOpen → CommentStart
        assert_eq!(t.next_token(), None); // '-' → CommentStartDash
        assert_eq!(t.next_token(), None); // 'a' → Comment (appends "-a")
        assert_eq!(t.state(), State::Comment);
    }

    #[test]
    fn comment_state_appends_chars() {
        let mut t = HtmlTokenizer::new("<!--abc-->");
        enter_markup_declaration(&mut t);
        assert_eq!(t.next_token(), None); // MarkupDeclarationOpen → CommentStart
        assert_eq!(t.next_token(), None); // 'a' → Comment
        assert_eq!(t.next_token(), None); // 'b'
        assert_eq!(t.next_token(), None); // 'c'
        // '-->' closing
        assert_eq!(t.next_token(), None); // '-' → CommentEndDash
        assert_eq!(t.next_token(), None); // '-' → CommentEnd
        assert_eq!(t.next_token(), Some(Token::Comment("abc".into()))); // '>' emit
    }

    #[test]
    fn comment_state_lt_switches() {
        let mut t = HtmlTokenizer::new("<!--a<");
        enter_markup_declaration(&mut t);
        assert_eq!(t.next_token(), None); // MarkupDeclarationOpen → CommentStart
        assert_eq!(t.next_token(), None); // 'a' → Comment
        assert_eq!(t.next_token(), None); // '<' → CommentLessThanSign
        assert_eq!(t.state(), State::CommentLessThanSign);
    }

    #[test]
    fn comment_state_dash_switches() {
        let mut t = HtmlTokenizer::new("<!--a-");
        enter_markup_declaration(&mut t);
        assert_eq!(t.next_token(), None); // MarkupDeclarationOpen → CommentStart
        assert_eq!(t.next_token(), None); // 'a' → Comment
        assert_eq!(t.next_token(), None); // '-' → CommentEndDash
        assert_eq!(t.state(), State::CommentEndDash);
    }

    #[test]
    fn comment_state_null_handles() {
        let mut t = HtmlTokenizer::new("<!--a\0b-->");
        enter_markup_declaration(&mut t);
        assert_eq!(t.next_token(), None); // MarkupDeclarationOpen → CommentStart
        assert_eq!(t.next_token(), None); // 'a' → Comment
        assert_eq!(t.next_token(), None); // '\0' → U+FFFD
        assert_eq!(t.next_token(), None); // 'b'
        // '-->'
        assert_eq!(t.next_token(), None); // '-' → CommentEndDash
        assert_eq!(t.next_token(), None); // '-' → CommentEnd
        assert_eq!(
            t.next_token(),
            Some(Token::Comment("a\u{FFFD}b".into()))
        );
    }

    // ── CommentLessThanSign 系列测试 (§13.2.5.46–§13.2.5.49) ────

    #[test]
    fn comment_lt_sign_excl_to_bang() {
        let mut t = HtmlTokenizer::new("<!--a<!--b-->");
        enter_markup_declaration(&mut t);
        assert_eq!(t.next_token(), None); // MarkupDeclarationOpen → CommentStart
        assert_eq!(t.next_token(), None); // CommentStart → Comment ('a')
        assert_eq!(t.next_token(), None); // Comment → CommentLessThanSign ('<')
        assert_eq!(t.next_token(), None); // CommentLessThanSign → Bang ('!')
        assert_eq!(t.state(), State::CommentLessThanSignBang);
    }

    #[test]
    fn comment_lt_sign_lt_stays() {
        let mut t = HtmlTokenizer::new("<!--a<<b-->");
        enter_markup_declaration(&mut t);
        assert_eq!(t.next_token(), None); // MarkupDeclarationOpen → CommentStart
        assert_eq!(t.next_token(), None); // CommentStart → Comment ('a')
        assert_eq!(t.next_token(), None); // Comment → CommentLessThanSign ('<')
        assert_eq!(t.next_token(), None); // LessThanSign → stay, append '<'
        assert_eq!(t.state(), State::CommentLessThanSign);
    }

    #[test]
    fn comment_lt_bang_dash_chain() {
        let mut t = HtmlTokenizer::new("<!--a<!-b-->");
        enter_markup_declaration(&mut t);
        assert_eq!(t.next_token(), None); // MarkupDeclarationOpen → CommentStart
        assert_eq!(t.next_token(), None); // CommentStart → Comment ('a')
        assert_eq!(t.next_token(), None); // Comment → CommentLessThanSign ('<')
        assert_eq!(t.next_token(), None); // LessThanSign → Bang ('!')
        assert_eq!(t.next_token(), None); // Bang → BangDash ('-')
        assert_eq!(t.state(), State::CommentLessThanSignBangDash);
    }

    #[test]
    fn comment_lt_bang_dash_dash_to_dashdash() {
        let mut t = HtmlTokenizer::new("<!--<!--b-->");
        enter_markup_declaration(&mut t);
        assert_eq!(t.next_token(), None); // MarkupDeclarationOpen → CommentStart
        assert_eq!(t.next_token(), None); // CommentStart → Comment (no chars before '<')
        assert_eq!(t.next_token(), None); // Comment → CommentLessThanSign ('<')
        assert_eq!(t.next_token(), None); // LessThanSign → Bang ('!')
        assert_eq!(t.next_token(), None); // Bang → BangDash ('-')
        assert_eq!(t.next_token(), None); // BangDash → BangDashDash ('-')
        // BangDashDash 总是回到 Comment（任意字符 reconsume）
        assert_eq!(t.state(), State::Comment);
    }

    #[test]
    fn comment_nested_open_not_close() {
        // `<!-- a<!--> b -->` → comment 内容为 " a<!--> b "
        let mut t = HtmlTokenizer::new("<!-- a<!--> b -->");
        enter_markup_declaration(&mut t);
        assert_eq!(t.next_token(), None); // MarkupDeclarationOpen → CommentStart
        assert_eq!(t.next_token(), None); // ' ' → Comment
        assert_eq!(t.next_token(), None); // 'a'
        // '<', '!', '-', '-' → LessThanSign → Bang → BangDash → BangDashDash
        assert_eq!(t.next_token(), None); // '<' → LessThanSign
        assert_eq!(t.next_token(), None); // '!' → Bang
        assert_eq!(t.next_token(), None); // '-' → BangDash
        assert_eq!(t.next_token(), None); // '-' → BangDashDash
        assert_eq!(t.next_token(), None); // '>' → back to Comment
        assert_eq!(t.state(), State::Comment);
        // ' b '
        assert_eq!(t.next_token(), None); // ' '
        assert_eq!(t.next_token(), None); // 'b'
        assert_eq!(t.next_token(), None); // ' '
        // '-->' closing
        assert_eq!(t.next_token(), None); // '-' → CommentEndDash
        assert_eq!(t.next_token(), None); // '-' → CommentEnd
        assert_eq!(
            t.next_token(),
            // 注：`<!-->` 在注释内部被 silently consumed（规范 §13.2.5.46–49），不追加到内容
            Some(Token::Comment(" a b ".into()))
        );
    }

    // ── CommentEnd 系列测试 (§13.2.5.50–§13.2.5.52) ────────────

    #[test]
    fn comment_end_gt_emits() {
        let mut t = HtmlTokenizer::new("<!--hello-->");
        enter_markup_declaration(&mut t);
        // CommentStart + 'h','e','l','l','o' = 1 + 5 = 6 calls
        for _ in 0..6 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), None); // '-' → CommentEndDash
        assert_eq!(t.next_token(), None); // '-' → CommentEnd
        assert_eq!(t.next_token(), Some(Token::Comment("hello".into()))); // '>'
    }

    #[test]
    fn comment_end_bang_gt_emits() {
        // `<!--hello--!>` → emit "hello"
        let mut t = HtmlTokenizer::new("<!--hello--!>");
        enter_markup_declaration(&mut t);
        for _ in 0..6 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), None); // '-' → CommentEndDash
        assert_eq!(t.next_token(), None); // '-' → CommentEnd
        assert_eq!(t.next_token(), None); // '!' → CommentEndBang
        assert_eq!(t.next_token(), Some(Token::Comment("hello".into()))); // '>'
    }

    #[test]
    fn comment_end_bang_dash_to_end() {
        // `<!--hello--!->` → CommentEndBang appends '--!', '-' → CommentEnd
        let mut t = HtmlTokenizer::new("<!--hello--!->");
        enter_markup_declaration(&mut t);
        for _ in 0..6 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), None); // '-' → CommentEndDash
        assert_eq!(t.next_token(), None); // '-' → CommentEnd
        assert_eq!(t.next_token(), None); // '!' → CommentEndBang
        assert_eq!(t.next_token(), None); // '-' → CommentEnd (appended '--!')
        assert_eq!(t.next_token(), Some(Token::Comment("hello--!".into()))); // '>' emit
    }

    #[test]
    fn comment_extra_dashes() {
        // `<!--hello---->` → 多余的 '-'
        let mut t = HtmlTokenizer::new("<!--hello---->");
        enter_markup_declaration(&mut t);
        for _ in 0..6 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), None); // '-' → CommentEndDash (1st)
        assert_eq!(t.next_token(), None); // '-' → CommentEnd (2nd)
        assert_eq!(t.next_token(), None); // '-' → stay, append '-' (3rd)
        assert_eq!(t.next_token(), None); // '-' → stay, append '-' (4th)
        assert_eq!(t.next_token(), Some(Token::Comment("hello--".into()))); // '>'
    }

    #[test]
    fn comment_state_eof() {
        let mut t = HtmlTokenizer::new("<!--abc");
        enter_markup_declaration(&mut t);
        assert_eq!(t.next_token(), None); // MarkupDeclarationOpen → CommentStart
        assert_eq!(t.next_token(), None); // 'a' → Comment
        assert_eq!(t.next_token(), None); // 'b'
        assert_eq!(t.next_token(), None); // 'c'
        assert_eq!(t.next_token(), Some(Token::Comment("abc".into()))); // EOF → emit
    }

    // ── Comment 集成测试 ──────────────────────────────────────────

    /// 辅助：跳过 None，直达下一个 Some(token)（常用于集成测试）
    fn next_real_token(t: &mut HtmlTokenizer) -> Token {
        loop {
            match t.next_token() {
                Some(token) => return token,
                None => continue,
            }
        }
    }

    #[test]
    fn comment_e2e_simple() {
        // `<!-- hello world -->`
        let mut t = HtmlTokenizer::new("<!-- hello world -->");
        assert_eq!(
            next_real_token(&mut t),
            Token::Comment(" hello world ".into())
        );
    }

    #[test]
    fn comment_e2e_empty() {
        // `<!---->` → 空注释
        let mut t = HtmlTokenizer::new("<!---->");
        assert_eq!(
            next_real_token(&mut t),
            Token::Comment("".into())
        );
    }

    #[test]
    fn comment_e2e_followed_by_tag() {
        // `<!-- comment --><div>` → Comment + Tag
        let mut t = HtmlTokenizer::new("<!-- comment --><div>");
        assert_eq!(
            next_real_token(&mut t),
            Token::Comment(" comment ".into())
        );
        assert_eq!(
            next_real_token(&mut t),
            Token::Tag(TagToken {
                kind: TagKind::Start,
                name: "div".into(),
                attrs: vec![],
                self_closing: false,
            })
        );
    }

    #[test]
    fn comment_e2e_nested() {
        // `<!-- <!-- nested --> -->`
        // 内部 `<!--` 按规范 silently consumed，内容仅为 " nested "
        let mut t = HtmlTokenizer::new("<!-- <!-- nested --> -->");
        assert_eq!(
            next_real_token(&mut t),
            Token::Comment("  nested ".into())
        );
    }

    // ── Attribute tests (§13.2.5.32–§13.2.5.39) ──────────────────

    #[test]
    fn attr_single_double_quoted_value() {
        // `<div class="x">` → start tag with one double-quoted attribute
        let mut t = HtmlTokenizer::new("<div class=\"x\">");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → TagName, name="d"
        assert_eq!(t.next_token(), None); // 'i'
        assert_eq!(t.next_token(), None); // 'v'
        assert_eq!(t.next_token(), None); // ' ' → BeforeAttributeName
        assert_eq!(t.state(), State::BeforeAttributeName);
        assert_eq!(t.next_token(), None); // 'c' → BA reconsume (call 6)
        assert_eq!(t.next_token(), None); // 'l' (call 7, reconsume 'c')
        assert_eq!(t.next_token(), None); // 'a' (call 8)
        assert_eq!(t.next_token(), None); // 's' (call 9)
        assert_eq!(t.next_token(), None); // 's' (call 10, last char)
        assert_eq!(t.next_token(), None); // 's' (call 11 — 第二个 's')
        assert_eq!(t.next_token(), None); // '=' → BeforeAttributeValue (call 12)
        assert_eq!(t.state(), State::BeforeAttributeValue);
        assert_eq!(t.next_token(), None); // '"' → AttributeValueDoubleQuoted (call 13)
        assert_eq!(t.state(), State::AttributeValueDoubleQuoted);
        assert_eq!(t.next_token(), None); // 'x' → append
        assert_eq!(t.state(), State::AttributeValueDoubleQuoted);
        assert_eq!(t.next_token(), None); // '"' → AfterAttributeValueQuoted, emit attr
        assert_eq!(t.state(), State::AfterAttributeValueQuoted);
        assert_eq!(
            t.next_token(),
            Some(Token::Tag(TagToken {
                kind: TagKind::Start,
                name: "div".into(),
                attrs: vec![("class".into(), "x".into())],
                self_closing: false,
            }))
        );
        assert_eq!(t.state(), State::Data);
    }

    #[test]
    fn attr_single_quoted_value() {
        // `<input type='text'>` → single-quoted attribute
        let mut t = HtmlTokenizer::new("<input type='text'>");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → TagName, name="i"
        // 'n', 'p', 'u', 't'
        for _ in 0..4 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), None); // ' ' → BeforeAttributeName
        // 't', 'y', 'p', 'e' — 需要 5 次调用（BeforeAttributeName reconsume 占用 1 次）
        // BeforeAttributeName → reconsume 't' → AttributeName → 'y','p','e'
        for _ in 0..5 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), None); // '=' → BeforeAttributeValue
        assert_eq!(t.state(), State::BeforeAttributeValue);
        assert_eq!(t.next_token(), None); // '\'' → AttributeValueSingleQuoted
        assert_eq!(t.state(), State::AttributeValueSingleQuoted);
        // 't', 'e', 'x', 't'
        for _ in 0..4 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), None); // '\'' → AfterAttributeValueQuoted, emit attr
        assert_eq!(
            t.next_token(),
            Some(Token::Tag(TagToken {
                kind: TagKind::Start,
                name: "input".into(),
                attrs: vec![("type".into(), "text".into())],
                self_closing: false,
            }))
        );
    }

    #[test]
    fn attr_unquoted_value() {
        // `<a href=x>` → unquoted attribute value
        let mut t = HtmlTokenizer::new("<a href=x>");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → TagName, name="a"
        assert_eq!(t.next_token(), None); // ' ' → BeforeAttributeName
        assert_eq!(t.state(), State::BeforeAttributeName);
        // 'h', 'r', 'e', 'f' — 需要 5 次（BA reconsume + 'h' reconsume + 3 剩余字符）
        for _ in 0..5 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), None); // '=' → BeforeAttributeValue
        assert_eq!(t.state(), State::BeforeAttributeValue);
        assert_eq!(t.next_token(), None); // 'x' → AttributeValueUnquoted
        assert_eq!(t.state(), State::AttributeValueUnquoted);
        assert_eq!(t.next_token(), None); // '>' → emit attr, emit tag
        assert_eq!(
            t.next_token(),
            Some(Token::Tag(TagToken {
                kind: TagKind::Start,
                name: "a".into(),
                attrs: vec![("href".into(), "x".into())],
                self_closing: false,
            }))
        );
    }

    #[test]
    fn attr_multiple_attributes() {
        // `<div id="a" class="b">` → two attributes
        let mut t = HtmlTokenizer::new("<div id=\"a\" class=\"b\">");
        // Skip to tag name done: `<` → TagOpen, `d` → name, `i`, `v`
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → TagName, name="d"
        assert_eq!(t.next_token(), None); // 'i'
        assert_eq!(t.next_token(), None); // 'v'
        // ' ' → BeforeAttributeName
        assert_eq!(t.next_token(), None);
        assert_eq!(t.state(), State::BeforeAttributeName);
        // 'i', 'd' — 需要 3 次（BA reconsume + 'i' reconsume + 'd'）
        assert_eq!(t.next_token(), None); // BA → reconsume 'i'
        assert_eq!(t.next_token(), None); // reconsume 'i'
        assert_eq!(t.next_token(), None); // 'd'
        // '=' → BeforeAttributeValue
        assert_eq!(t.next_token(), None);
        assert_eq!(t.state(), State::BeforeAttributeValue);
        // '"' → AttributeValueDoubleQuoted
        assert_eq!(t.next_token(), None);
        assert_eq!(t.state(), State::AttributeValueDoubleQuoted);
        // 'a'
        assert_eq!(t.next_token(), None);
        // '"' → AfterAttributeValueQuoted, emit attr("id","a")
        assert_eq!(t.next_token(), None);
        assert_eq!(t.state(), State::AfterAttributeValueQuoted);
        // ' ' → BeforeAttributeName
        assert_eq!(t.next_token(), None);
        assert_eq!(t.state(), State::BeforeAttributeName);
        // 'c', 'l', 'a', 's', 's' — 需要 6 次（BA reconsume + 'c' reconsume + 4 剩余字符）
        for _ in 0..6 { assert_eq!(t.next_token(), None); }
        // '=' → BeforeAttributeValue
        assert_eq!(t.next_token(), None);
        assert_eq!(t.state(), State::BeforeAttributeValue);
        // '"' → AttributeValueDoubleQuoted
        assert_eq!(t.next_token(), None);
        assert_eq!(t.state(), State::AttributeValueDoubleQuoted);
        // 'b'
        assert_eq!(t.next_token(), None);
        // '"' → AfterAttributeValueQuoted, emit attr("class","b")
        assert_eq!(t.next_token(), None);
        assert_eq!(t.state(), State::AfterAttributeValueQuoted);
        // '>' → emit tag
        let token = t.next_token();
        assert_eq!(
            token,
            Some(Token::Tag(TagToken {
                kind: TagKind::Start,
                name: "div".into(),
                attrs: vec![("id".into(), "a".into()), ("class".into(), "b".into())],
                self_closing: false,
            }))
        );
        assert_eq!(t.state(), State::Data);
    }

    #[test]
    fn attr_boolean_attribute() {
        let mut t = HtmlTokenizer::new("<input disabled>");
        // Skip to tag name done
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → TagName, name="i"
        // 'n', 'p', 'u', 't'
        for _ in 0..4 { assert_eq!(t.next_token(), None); }
        // ' ' → BeforeAttributeName
        assert_eq!(t.next_token(), None);
        // 'd', 'i', 's', 'a', 'b', 'l', 'e', 'd' — 需要 9 次（BA reconsume + 'd' reconsume + 7 剩余字符）
        for _ in 0..9 { assert_eq!(t.next_token(), None); }
        // '>' → AfterAttributeName → emit attr, emit tag
        let token = t.next_token();
        assert_eq!(
            token,
            Some(Token::Tag(TagToken {
                kind: TagKind::Start,
                name: "input".into(),
                attrs: vec![("disabled".into(), "".into())],
                self_closing: false,
            }))
        );
    }

    #[test]
    fn attr_name_lowercases_ascii_upper() {
        // §13.2.5.33: ASCII uppercase → lowercase in attribute names
        let mut t = HtmlTokenizer::new("<div CLASS=\"x\">");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → TagName 'd'
        assert_eq!(t.next_token(), None); // 'i'
        assert_eq!(t.next_token(), None); // 'v'
        assert_eq!(t.next_token(), None); // ' ' → BeforeAttributeName
        // 'C','L','A','S','S' — 6 calls (BA reconsume + 5 chars)
        for _ in 0..6 {
            assert_eq!(t.next_token(), None);
        }
        assert_eq!(t.next_token(), None); // '=' → BeforeAttributeValue
        assert_eq!(t.next_token(), None); // '"' → AttributeValueDoubleQuoted
        assert_eq!(t.next_token(), None); // 'x'
        assert_eq!(t.next_token(), None); // '"' → AfterAttributeValueQuoted
        let token = t.next_token(); // '>'
        assert_eq!(
            token,
            Some(Token::Tag(TagToken {
                kind: TagKind::Start,
                name: "div".into(),
                attrs: vec![("class".into(), "x".into())],
                self_closing: false,
            }))
        );
    }

    #[test]
    fn attr_name_preserves_non_ascii() {
        // Non-ASCII chars in attribute names should be preserved as-is
        let mut t = HtmlTokenizer::new("<div café=\"oui\">");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → TagName 'd'
        assert_eq!(t.next_token(), None); // 'i'
        assert_eq!(t.next_token(), None); // 'v'
        assert_eq!(t.next_token(), None); // ' ' → BeforeAttributeName
        // 'c','a','f','é' — 5 calls (BA reconsume + 4 chars)
        for _ in 0..5 {
            assert_eq!(t.next_token(), None);
        }
        assert_eq!(t.next_token(), None); // '=' → BeforeAttributeValue
        assert_eq!(t.next_token(), None); // '"' → AttributeValueDoubleQuoted
        assert_eq!(t.next_token(), None); // 'o'
        assert_eq!(t.next_token(), None); // 'u'
        assert_eq!(t.next_token(), None); // 'i'
        assert_eq!(t.next_token(), None); // '"' → AfterAttributeValueQuoted
        let token = t.next_token(); // '>'
        assert_eq!(
            token,
            Some(Token::Tag(TagToken {
                kind: TagKind::Start,
                name: "div".into(),
                attrs: vec![("café".into(), "oui".into())],
                self_closing: false,
            }))
        );
    }

    #[test]
    fn e2e_attr_and_self_closing() {
        // `<input type='text'/>` → attribute + self-closing
        let mut t = HtmlTokenizer::new("<input type='text'/>");
        assert_eq!(t.next_token(), None); // Data → TagOpen
        assert_eq!(t.next_token(), None); // TagOpen → TagName, name="i"
        // 'n', 'p', 'u', 't'
        for _ in 0..4 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), None); // ' ' → BeforeAttributeName
        // 't', 'y', 'p', 'e' — 需要 5 次（BA reconsume + 't' reconsume + 3 剩余字符）
        for _ in 0..5 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), None); // '=' → BeforeAttributeValue
        assert_eq!(t.next_token(), None); // '\'' → AttributeValueSingleQuoted
        // 't', 'e', 'x', 't'
        for _ in 0..4 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), None); // '\'' → AfterAttributeValueQuoted
        assert_eq!(t.next_token(), None); // '/' → SelfClosingStartTag
        assert_eq!(
            t.next_token(),
            Some(Token::Tag(TagToken {
                kind: TagKind::Start,
                name: "input".into(),
                attrs: vec![("type".into(), "text".into())],
                self_closing: true,
            }))
        );
        assert_eq!(t.next_token(), Some(Token::EOF));
    }

    // ── Content model tests (§13.2.5.2–§13.2.5.5) ────────────────

    /// Helper: create a tokenizer in a specific content model state.
    fn enter_content_model(input: &str, state: State, end_tag: Option<&str>) -> HtmlTokenizer {
        let mut t = HtmlTokenizer::new(input);
        t.set_state(state);
        t.set_appropriate_end_tag_name(end_tag);
        t
    }

    #[test]
    fn rcdata_emits_characters() {
        let mut t = enter_content_model("hello", State::RCDATA, Some("title"));
        assert_eq!(t.next_token(), Some(Token::Character('h')));
        assert_eq!(t.next_token(), Some(Token::Character('e')));
        assert_eq!(t.next_token(), Some(Token::Character('l')));
        assert_eq!(t.next_token(), Some(Token::Character('l')));
        assert_eq!(t.next_token(), Some(Token::Character('o')));
    }

    #[test]
    fn rcdata_lt_switches_to_less_than_sign() {
        let mut t = enter_content_model("<", State::RCDATA, Some("title"));
        assert_eq!(t.next_token(), None); // '<' → RCDATALessThanSign
        assert_eq!(t.state(), State::RCDATALessThanSign);
    }

    #[test]
    fn rcdata_ampersand_switches_to_charref() {
        let mut t = enter_content_model("&", State::RCDATA, Some("title"));
        assert_eq!(t.next_token(), None); // '&' → CharacterReference
        assert_eq!(t.state(), State::CharacterReference);
    }

    #[test]
    fn rcdata_null_emits_replacement_char() {
        let mut t = enter_content_model("\0", State::RCDATA, Some("title"));
        assert_eq!(t.next_token(), Some(Token::Character('\u{FFFD}')));
    }

    #[test]
    fn rcdata_eof_emits_eof() {
        let mut t = enter_content_model("", State::RCDATA, Some("title"));
        assert_eq!(t.next_token(), Some(Token::EOF));
    }

    #[test]
    fn rcdata_end_tag_match() {
        // `</title>` in RCDATA with appropriate_end_tag_name = "title"
        let mut t = enter_content_model("</title>", State::RCDATA, Some("title"));
        // '<' → RCDATALessThanSign
        assert_eq!(t.next_token(), None);
        // '/' → RCDATAEndTagOpen
        assert_eq!(t.next_token(), None);
        // 't' → RCDATAEndTagName (create end tag "t")
        assert_eq!(t.next_token(), None);
        // 'i' → append to end tag "ti"
        assert_eq!(t.next_token(), None);
        // 't' → append to end tag "tit"
        assert_eq!(t.next_token(), None);
        // 'l' → append to end tag "titl"
        assert_eq!(t.next_token(), None);
        // 'e' → append to end tag "title"
        assert_eq!(t.next_token(), None);
        // '>' → matches appropriate end tag → emit tag, switch to Data
        assert_eq!(
            t.next_token(),
            Some(Token::Tag(TagToken {
                kind: TagKind::End,
                name: "title".into(),
                attrs: vec![],
                self_closing: false,
            }))
        );
        assert_eq!(t.state(), State::Data);
    }

    #[test]
    fn rcdata_end_tag_no_match_backout() {
        // `</div>` in RCDATA with appropriate_end_tag_name = "title" — does not match
        let mut t = enter_content_model("</div>x", State::RCDATA, Some("title"));
        // '<' → RCDATALessThanSign
        assert_eq!(t.next_token(), None);
        // '/' → RCDATAEndTagOpen
        assert_eq!(t.next_token(), None);
        // 'd' → RCDATAEndTagName (create end tag, name="d", buf="d")
        assert_eq!(t.next_token(), None);
        // 'i' → append
        assert_eq!(t.next_token(), None);
        // 'v' → append
        assert_eq!(t.next_token(), None);
        // '>' → not appropriate (name="div" != expected="title") → backout
        // Handler returns None but pushes pending tokens
        assert_eq!(t.next_token(), None);
        // Pending tokens drained: '<' '/' 'd' 'i' 'v' (forward order)
        assert_eq!(t.next_token(), Some(Token::Character('<')));
        assert_eq!(t.next_token(), Some(Token::Character('/')));
        assert_eq!(t.next_token(), Some(Token::Character('d')));
        assert_eq!(t.next_token(), Some(Token::Character('i')));
        assert_eq!(t.next_token(), Some(Token::Character('v')));
        // '>' is re-consumed in RCDATA as character token
        assert_eq!(t.next_token(), Some(Token::Character('>')));
        // Then 'x'
        assert_eq!(t.state(), State::RCDATA);
        assert_eq!(t.next_token(), Some(Token::Character('x')));
    }

    #[test]
    fn rawtext_emits_characters() {
        let mut t = enter_content_model("hello", State::RAWTEXT, Some("style"));
        assert_eq!(t.next_token(), Some(Token::Character('h')));
        assert_eq!(t.next_token(), Some(Token::Character('e')));
        assert_eq!(t.next_token(), Some(Token::Character('l')));
        assert_eq!(t.next_token(), Some(Token::Character('l')));
        assert_eq!(t.next_token(), Some(Token::Character('o')));
    }

    #[test]
    fn rawtext_ampersand_is_literal() {
        // RAWTEXT does NOT handle `&` — it's emitted as a regular character
        let mut t = enter_content_model("&amp;", State::RAWTEXT, Some("style"));
        assert_eq!(t.next_token(), Some(Token::Character('&')));
        assert_eq!(t.next_token(), Some(Token::Character('a')));
        assert_eq!(t.next_token(), Some(Token::Character('m')));
        assert_eq!(t.next_token(), Some(Token::Character('p')));
        assert_eq!(t.next_token(), Some(Token::Character(';')));
    }

    #[test]
    fn rawtext_lt_switches_to_less_than_sign() {
        let mut t = enter_content_model("<", State::RAWTEXT, Some("style"));
        assert_eq!(t.next_token(), None); // '<' → RAWTEXTLessThanSign
        assert_eq!(t.state(), State::RAWTEXTLessThanSign);
    }

    #[test]
    fn rawtext_end_tag_match() {
        // `</style>` in RAWTEXT with appropriate_end_tag_name = "style"
        let mut t = enter_content_model("</style>", State::RAWTEXT, Some("style"));
        // '<' → RAWTEXTLessThanSign
        assert_eq!(t.next_token(), None);
        // '/' → RAWTEXTEndTagOpen
        assert_eq!(t.next_token(), None);
        // 's','t','y','l','e' → RAWTEXTEndTagName
        for _ in 0..5 { assert_eq!(t.next_token(), None); }
        // '>' → matches → emit tag, switch to Data
        assert_eq!(
            t.next_token(),
            Some(Token::Tag(TagToken {
                kind: TagKind::End,
                name: "style".into(),
                attrs: vec![],
                self_closing: false,
            }))
        );
        assert_eq!(t.state(), State::Data);
    }

    #[test]
    fn rawtext_end_tag_no_match_backout() {
        // `</div>x` in RAWTEXT with expected "style" — should back out
        let mut t = enter_content_model("</div>x", State::RAWTEXT, Some("style"));
        // '<' → RAWTEXTLessThanSign
        assert_eq!(t.next_token(), None);
        // '/' → RAWTEXTEndTagOpen
        assert_eq!(t.next_token(), None);
        // 'd','i','v' → RAWTEXTEndTagName
        for _ in 0..3 { assert_eq!(t.next_token(), None); }
        // '>' → not appropriate → backout (returns None, pushes pending)
        assert_eq!(t.next_token(), None);
        // Pending tokens drained
        assert_eq!(t.next_token(), Some(Token::Character('<')));
        assert_eq!(t.next_token(), Some(Token::Character('/')));
        assert_eq!(t.next_token(), Some(Token::Character('d')));
        assert_eq!(t.next_token(), Some(Token::Character('i')));
        assert_eq!(t.next_token(), Some(Token::Character('v')));
        // '>' re-consumed in RAWTEXT
        assert_eq!(t.next_token(), Some(Token::Character('>')));
        assert_eq!(t.state(), State::RAWTEXT);
        assert_eq!(t.next_token(), Some(Token::Character('x')));
    }

    #[test]
    fn rawtext_end_tag_no_appropriate_end_tag() {
        // No appropriate_end_tag_name set — nothing matches
        let mut t = enter_content_model("</style>x", State::RAWTEXT, None);
        // '<' → RAWTEXTLessThanSign
        assert_eq!(t.next_token(), None);
        // '/' → RAWTEXTEndTagOpen
        assert_eq!(t.next_token(), None);
        // 's','t','y','l','e' → RAWTEXTEndTagName
        for _ in 0..5 { assert_eq!(t.next_token(), None); }
        // '>' → not appropriate (None is not "style") → backout (returns None)
        assert_eq!(t.next_token(), None);
        // Pending tokens drained
        assert_eq!(t.next_token(), Some(Token::Character('<')));
        assert_eq!(t.next_token(), Some(Token::Character('/')));
        assert_eq!(t.next_token(), Some(Token::Character('s')));
        assert_eq!(t.next_token(), Some(Token::Character('t')));
        assert_eq!(t.next_token(), Some(Token::Character('y')));
        assert_eq!(t.next_token(), Some(Token::Character('l')));
        assert_eq!(t.next_token(), Some(Token::Character('e')));
    }

    #[test]
    fn rawtext_end_tag_name_eof() {
        // RAWTEXT end tag name on EOF emits `</` + switch to Data
        let mut t = enter_content_model("</sty", State::RAWTEXT, Some("style"));
        // '<' → RAWTEXTLessThanSign
        assert_eq!(t.next_token(), None);
        // '/' → RAWTEXTEndTagOpen
        assert_eq!(t.next_token(), None);
        // 's','t','y' → RAWTEXTEndTagName
        for _ in 0..3 { assert_eq!(t.next_token(), None); }
        // EOF → handler pushes pending '<' '/' and returns None
        assert_eq!(t.next_token(), None);
        // Pending tokens drained
        assert_eq!(t.next_token(), Some(Token::Character('<')));
        assert_eq!(t.next_token(), Some(Token::Character('/')));
        // Next call: state is Data, naturally at EOF → Data emits EOF
        assert_eq!(t.state(), State::Data);
        assert_eq!(t.next_token(), Some(Token::EOF));
    }

    #[test]
    fn plaintext_emits_everything_literally() {
        // PLAINTEXT treats `<`, `&`, everything as literal characters
        let mut t = enter_content_model("<div>&amp;</div>", State::PLAINTEXT, None);
        for expected in "<div>&amp;</div>".chars() {
            assert_eq!(t.next_token(), Some(Token::Character(expected)));
        }
    }

    #[test]
    fn plaintext_null_emits_replacement_char() {
        let mut t = enter_content_model("\0x", State::PLAINTEXT, None);
        assert_eq!(t.next_token(), Some(Token::Character('\u{FFFD}')));
        assert_eq!(t.next_token(), Some(Token::Character('x')));
    }

    #[test]
    fn plaintext_eof_emits_eof() {
        let mut t = enter_content_model("", State::PLAINTEXT, None);
        assert_eq!(t.next_token(), Some(Token::EOF));
    }

    // ── Script data tests (§13.2.5.4, §13.2.5.15–§13.2.5.31) ────

    #[test]
    fn script_data_emits_characters() {
        let mut t = enter_content_model("abc", State::ScriptData, Some("script"));
        assert_eq!(t.next_token(), Some(Token::Character('a')));
        assert_eq!(t.next_token(), Some(Token::Character('b')));
        assert_eq!(t.next_token(), Some(Token::Character('c')));
    }

    #[test]
    fn script_data_null_emits_replacement_char() {
        let mut t = enter_content_model("\0", State::ScriptData, Some("script"));
        assert_eq!(t.next_token(), Some(Token::Character('\u{FFFD}')));
    }

    #[test]
    fn script_data_ampersand_is_literal() {
        // ScriptData does NOT handle `&` — no character references
        let mut t = enter_content_model("&amp;", State::ScriptData, Some("script"));
        assert_eq!(t.next_token(), Some(Token::Character('&')));
        assert_eq!(t.next_token(), Some(Token::Character('a')));
    }

    #[test]
    fn script_data_lt_switches_to_less_than_sign() {
        let mut t = enter_content_model("<", State::ScriptData, Some("script"));
        assert_eq!(t.next_token(), None); // '<' → ScriptDataLessThanSign
        assert_eq!(t.state(), State::ScriptDataLessThanSign);
    }

    #[test]
    fn script_data_lt_excl_to_escape_start() {
        // `<!` in ScriptData: '<' → LessThanSign, '!' → EscapeStart
        let mut t = enter_content_model("<!", State::ScriptData, Some("script"));
        assert_eq!(t.next_token(), None); // '<' → LessThanSign
        assert_eq!(t.next_token(), None); // '!' → EscapeStart
        assert_eq!(t.state(), State::ScriptDataEscapeStart);
    }

    #[test]
    fn script_data_escape_start_dash_chain() {
        // `<!--` in ScriptData → escape start chain
        let mut t = enter_content_model("<!--", State::ScriptData, Some("script"));
        assert_eq!(t.next_token(), None); // '<' → LessThanSign
        assert_eq!(t.next_token(), None); // '!' → EscapeStart
        assert_eq!(t.next_token(), None); // '-' → EscapeStartDash
        assert_eq!(t.next_token(), None); // '-' → EscapedDashDash
        assert_eq!(t.state(), State::ScriptDataEscapedDashDash);
    }

    #[test]
    fn script_data_end_tag_match() {
        // `</script>` in ScriptData matches appropriate end tag
        let mut t = enter_content_model("</script>", State::ScriptData, Some("script"));
        // '<' → LessThanSign
        assert_eq!(t.next_token(), None);
        // '/' → EndTagOpen
        assert_eq!(t.next_token(), None);
        // 's','c','r','i','p','t' → EndTagName
        for _ in 0..6 { assert_eq!(t.next_token(), None); }
        // '>' → match → emit tag, Data
        assert_eq!(
            t.next_token(),
            Some(Token::Tag(TagToken {
                kind: TagKind::End,
                name: "script".into(),
                attrs: vec![],
                self_closing: false,
            }))
        );
        assert_eq!(t.state(), State::Data);
    }

    #[test]
    fn script_data_end_tag_no_match_backout() {
        // `</div>x` in ScriptData — not appropriate, backout
        let mut t = enter_content_model("</div>x", State::ScriptData, Some("script"));
        assert_eq!(t.next_token(), None); // '<' → LessThanSign
        assert_eq!(t.next_token(), None); // '/' → EndTagOpen
        for _ in 0..3 { assert_eq!(t.next_token(), None); } // 'd','i','v'
        assert_eq!(t.next_token(), None); // '>' → not appropriate → backout
        // Pending tokens drained
        assert_eq!(t.next_token(), Some(Token::Character('<')));
        assert_eq!(t.next_token(), Some(Token::Character('/')));
        assert_eq!(t.next_token(), Some(Token::Character('d')));
        assert_eq!(t.next_token(), Some(Token::Character('i')));
        assert_eq!(t.next_token(), Some(Token::Character('v')));
        // '>' re-consumed in ScriptData
        assert_eq!(t.next_token(), Some(Token::Character('>')));
        assert_eq!(t.state(), State::ScriptData);
    }

    #[test]
    fn script_data_escaped_emits_chars() {
        let mut t = enter_content_model("x", State::ScriptDataEscaped, Some("script"));
        assert_eq!(t.next_token(), Some(Token::Character('x')));
    }

    #[test]
    fn script_data_escaped_dash_dash_gt_exits_escape() {
        // `-->` in escaped mode: dash dash dash → emit '>', back to ScriptData
        let mut t = enter_content_model("-->", State::ScriptDataEscaped, Some("script"));
        assert_eq!(t.next_token(), None); // '-' → EscapedDash
        assert_eq!(t.next_token(), None); // '-' → EscapedDashDash
        assert_eq!(t.next_token(), Some(Token::Character('>'))); // '>' → emit, back to ScriptData
        assert_eq!(t.state(), State::ScriptData);
    }

    #[test]
    fn script_data_escaped_lt_alpha_to_double_escape_start() {
        // `<s` in escaped: '<' → LessThanSign, 's' → DoubleEscapeStart
        let mut t = enter_content_model("<s", State::ScriptDataEscaped, Some("script"));
        assert_eq!(t.next_token(), None); // '<' → EscapedLessThanSign
        assert_eq!(t.next_token(), None); // 's' → DoubleEscapeStart (alpha)
        assert_eq!(t.state(), State::ScriptDataDoubleEscapeStart);
    }

    #[test]
    fn script_data_double_escape_start_script_match() {
        // In DoubleEscapeStart, `script` + `/` → enter DoubleEscaped
        let mut t = enter_content_model("script/", State::ScriptDataDoubleEscapeStart, Some("script"));
        for _ in 0..6 { assert_eq!(t.next_token(), None); } // 's','c','r','i','p','t' → accumulate
        assert_eq!(t.next_token(), None); // '/' → temp buffer is "script" → DoubleEscaped
        assert_eq!(t.state(), State::ScriptDataDoubleEscaped);
    }

    #[test]
    fn script_data_double_escape_start_not_script() {
        // In DoubleEscapeStart, `foo ` → not "script" → back to Escaped
        let mut t = enter_content_model("foo ", State::ScriptDataDoubleEscapeStart, Some("script"));
        for _ in 0..3 { assert_eq!(t.next_token(), None); } // 'f','o','o'
        assert_eq!(t.next_token(), None); // ' ' → not "script" → Escaped
        assert_eq!(t.state(), State::ScriptDataEscaped);
    }

    #[test]
    fn script_data_double_escaped_emits_chars() {
        let mut t = enter_content_model("hi", State::ScriptDataDoubleEscaped, Some("script"));
        assert_eq!(t.next_token(), Some(Token::Character('h')));
        assert_eq!(t.next_token(), Some(Token::Character('i')));
    }

    #[test]
    fn script_data_double_escaped_lt_emits_lt() {
        // In DoubleEscaped, '<' emits '<' immediately, switches to DoubleEscapedLessThanSign
        let mut t = enter_content_model("<", State::ScriptDataDoubleEscaped, Some("script"));
        assert_eq!(t.next_token(), Some(Token::Character('<')));
        assert_eq!(t.state(), State::ScriptDataDoubleEscapedLessThanSign);
    }

    #[test]
    fn script_data_double_escaped_dash_dash_gt_to_script_data() {
        // `-->` in double escaped → exit to ScriptData
        let mut t = enter_content_model("-->", State::ScriptDataDoubleEscaped, Some("script"));
        assert_eq!(t.next_token(), Some(Token::Character('-'))); // '-' → DoubleEscapedDash, emit '-'
        assert_eq!(t.next_token(), Some(Token::Character('-'))); // '-' → DoubleEscapedDashDash, emit '-'
        assert_eq!(t.next_token(), Some(Token::Character('>'))); // '>' → emit '>', ScriptData
        assert_eq!(t.state(), State::ScriptData);
    }

    #[test]
    fn script_data_double_escape_end_script_match() {
        // Simulate entering DoubleEscapeEnd with `/` path.
        // In DoubleEscaped, `</` → pend `/`, switch to DoubleEscapeEnd.
        // Then `script/` → temp buffer is "script" → exit to Escaped.
        // Since we entered via `/` path (no alpha tag creation),
        // no tag token is emitted — just state transition.
        let mut t = enter_content_model("</script/", State::ScriptDataDoubleEscaped, Some("script"));
        // '<' → emit '<', switch to DoubleEscapedLessThanSign
        assert_eq!(t.next_token(), Some(Token::Character('<')));
        assert_eq!(t.state(), State::ScriptDataDoubleEscapedLessThanSign);
        // '/' → push '/', switch to DoubleEscapeEnd
        assert_eq!(t.next_token(), None);
        // pending pops '/'
        assert_eq!(t.next_token(), Some(Token::Character('/')));
        assert_eq!(t.state(), State::ScriptDataDoubleEscapeEnd);
        // 's','c','r','i','p','t' → accumulate
        for _ in 0..6 { assert_eq!(t.next_token(), None); }
        // '/' → temp buffer == "script" → no tag (entered via / path) →
        // just switch to Escaped
        assert_eq!(t.next_token(), None);
        assert_eq!(t.state(), State::ScriptDataEscaped);
    }

    // ── Character reference tests (§13.2.5.72–§13.2.5.80) ──────

    #[test]
    fn char_ref_named_amp() {
        // `&amp;` → `&`: '&' 'a' 'm' 'p' ';' (5 Nones → Some)
        let mut t = HtmlTokenizer::new("&amp;");
        for _ in 0..5 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), Some(Token::Character('&')));
    }

    #[test]
    fn char_ref_named_lt() {
        // `&lt;` → `<`: '&' 'l' 't' ';' (3 Nones → Some)
        let mut t = HtmlTokenizer::new("&lt;");
        for _ in 0..4 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), Some(Token::Character('<')));
    }

    #[test]
    fn char_ref_named_gt() {
        // `&gt;` → `>`: '&' 'g' 't' ';' (4 Nones → Some)
        let mut t = HtmlTokenizer::new("&gt;");
        for _ in 0..4 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), Some(Token::Character('>')));
    }

    #[test]
    fn char_ref_named_quot() {
        // `&quot;` → `"`: '&' 'q' 'u' 'o' 't' ';' (5 Nones → Some)
        let mut t = HtmlTokenizer::new("&quot;");
        for _ in 0..6 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), Some(Token::Character('"')));
    }

    #[test]
    fn char_ref_numeric_decimal() {
        let mut t = HtmlTokenizer::new("&#60;");
        for _ in 0..7 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), Some(Token::Character('<')));
    }

    #[test]
    fn char_ref_numeric_hex() {
        let mut t = HtmlTokenizer::new("&#x3C;");
        for _ in 0..7 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), Some(Token::Character('<')));
    }

    #[test]
    fn char_ref_not_a_ref_space() {
        // `& a` → emit `&`, then ` ` in Data
        let mut t = HtmlTokenizer::new("& a");
        assert_eq!(t.next_token(), None);  // '&' → CharacterReference
        assert_eq!(t.next_token(), Some(Token::Character('&'))); // ' ' → fallback
        assert_eq!(t.next_token(), Some(Token::Character(' '))); // reconsume ' '
        assert_eq!(t.next_token(), Some(Token::Character('a')));
    }

    #[test]
    fn char_ref_ampersand_not_ref() {
        // `&&` → emit `&`, reconsume `&`
        let mut t = HtmlTokenizer::new("&&");
        assert_eq!(t.next_token(), None); // '&' → CharacterReference
        assert_eq!(t.next_token(), Some(Token::Character('&'))); // '&' → fallback
        assert_eq!(t.next_token(), None); // reconsume '&' → CharacterReference
        assert_eq!(t.next_token(), Some(Token::Character('&'))); // at EOF → fallback
    }

    #[test]
    fn char_ref_numeric_null() {
        // `&#x00;` → U+FFFD: '&' '#' 'x' '0' '0' ';' (5 Nones → Some)
        let mut t = HtmlTokenizer::new("&#x00;");
        for _ in 0..7 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), Some(Token::Character('\u{FFFD}')));
    }

    #[test]
    fn char_ref_numeric_win1252() {
        let mut t = HtmlTokenizer::new("&#x80;");
        for _ in 0..7 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), Some(Token::Character('\u{20AC}')));
    }

    #[test]
    fn char_ref_unknown_entity_ambiguous() {
        // `&notin;` — `notin` not in minimal table → ambiguous ampersand
        let mut t = HtmlTokenizer::new("&notin;");
        // 8 Nones then pending tokens + AmbigAmpersand emit
        for _ in 0..8 { assert_eq!(t.next_token(), None); }
        assert_eq!(t.next_token(), Some(Token::Character('n')));
        assert_eq!(t.next_token(), Some(Token::Character('o')));
        assert_eq!(t.next_token(), Some(Token::Character('t')));
        assert_eq!(t.next_token(), Some(Token::Character('i')));
        assert_eq!(t.next_token(), Some(Token::Character('n')));
        assert_eq!(t.next_token(), Some(Token::Character(';')));
    }

    #[test]
    fn char_ref_e2e_data_state() {
        // `&lt;a&gt;` → `<a>`
        let mut t = HtmlTokenizer::new("&lt;a&gt;");
        for _ in 0..4 { assert_eq!(t.next_token(), None); } // &lt; resolved
        assert_eq!(t.next_token(), Some(Token::Character('<')));
        assert_eq!(t.next_token(), Some(Token::Character('a')));
        for _ in 0..4 { assert_eq!(t.next_token(), None); } // &gt; resolved
        assert_eq!(t.next_token(), Some(Token::Character('>')));
        assert_eq!(t.next_token(), Some(Token::EOF));
    }
}
