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
                // TODO: Switch to character reference state with return state
                self.current_attr_value.push('&');
                self.state = State::AttributeValueDoubleQuoted;
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
                // TODO: Switch to character reference state with return state
                self.current_attr_value.push('&');
                self.state = State::AttributeValueSingleQuoted;
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
                // TODO: Switch to character reference state with return state
                self.current_attr_value.push('&');
                self.state = State::AttributeValueUnquoted;
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
}
