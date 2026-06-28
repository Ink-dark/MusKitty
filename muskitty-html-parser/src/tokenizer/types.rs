//! Token and state types for the HTML tokenizer.
//!
//! Defined per WHATWG HTML Spec §13.2.5 Tokenization.

/// A token emitted by the tokenizer.
///
/// WHATWG §13.2.5 defines six token kinds: DOCTYPE, start tag, end tag,
/// comment, character, and end-of-file.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A DOCTYPE token (§13.2.5.53–§13.2.5.68).
    Doctype(DoctypeToken),
    /// A start or end tag token, discriminated by [`TagToken::kind`].
    Tag(TagToken),
    /// A comment token. The String is the comment content.
    Comment(String),
    /// A character token carrying a single Unicode code point.
    Character(char),
    /// End-of-file token. Emitted when the input stream is exhausted.
    EOF,
}

/// Distinguishes start tags from end tags.
///
/// WHATWG §13.2.5: start tags and end tags share the same token structure but
/// differ in how tree construction handles them — end tag attributes are
/// ignored (parse error `end-tag-with-attributes`), and the self-closing flag
/// on an end tag is meaningless.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagKind {
    /// `<tag>`
    Start,
    /// `</tag>`
    End,
}

/// A tag token (used for both start and end tags, distinguished by [`TagKind`]).
///
/// WHATWG §13.2.5: start tags have a tag name, a self-closing flag (set when
/// the tag ends with `/>`), and a list of attributes. End tags have the same
/// structure, though attributes on end tags are a parse error and the
/// self-closing flag is ignored in tree construction.
#[derive(Debug, Clone, PartialEq)]
pub struct TagToken {
    /// Whether this is a start tag or end tag.
    pub kind: TagKind,
    /// The tag name (lowercased by the tokenizer).
    pub name: String,
    /// Attribute name-value pairs, in source order.
    pub attrs: Vec<(String, String)>,
    /// Whether the tag ends with `/>` (the self-closing solidus).
    /// Only meaningful for start tags on void elements (§13.2.6).
    pub self_closing: bool,
}

/// A DOCTYPE token.
///
/// WHATWG §13.2.5.53–§13.2.5.68.
#[derive(Debug, Clone, PartialEq)]
pub struct DoctypeToken {
    /// The DOCTYPE name (e.g. "html"), or None if absent.
    pub name: Option<String>,
    /// The public identifier, or None if absent.
    pub public_id: Option<String>,
    /// The system identifier, or None if absent.
    pub system_id: Option<String>,
    /// Whether force-quirks was set during DOCTYPE parsing.
    pub force_quirks: bool,
}

/// Tokenizer states.
///
/// WHATWG §13.2.5 defines 80 states. Every state is present in this enum so
/// the compiler enforces exhaustive `match` arms — no state can be forgotten.
///
/// States marked `TODO: not yet implemented` are reserved; their variant
/// exists but the tokenizer will panic if it transitions into one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    // ── Content model states ──────────────────────────────
    /// §13.2.5.1 Data state
    Data,
    /// §13.2.5.2 RCDATA state
    ///
    /// TODO: not yet implemented
    RCDATA,
    /// §13.2.5.3 RAWTEXT state
    ///
    /// TODO: not yet implemented
    RAWTEXT,
    /// §13.2.5.4 Script data state
    ///
    /// TODO: not yet implemented
    ScriptData,
    /// §13.2.5.5 PLAINTEXT state
    ///
    /// TODO: not yet implemented
    PLAINTEXT,

    // ── Tag open / close states ───────────────────────────
    /// §13.2.5.6 Tag open state
    ///
    /// TODO: not yet implemented
    TagOpen,
    /// §13.2.5.7 End tag open state
    ///
    /// TODO: not yet implemented
    EndTagOpen,
    /// §13.2.5.8 Tag name state
    ///
    /// TODO: not yet implemented
    TagName,

    // ── RCDATA states ─────────────────────────────────────
    /// §13.2.5.9 RCDATA less-than sign state
    ///
    /// TODO: not yet implemented
    RCDATALessThanSign,
    /// §13.2.5.10 RCDATA end tag open state
    ///
    /// TODO: not yet implemented
    RCDATAEndTagOpen,
    /// §13.2.5.11 RCDATA end tag name state
    ///
    /// TODO: not yet implemented
    RCDATAEndTagName,

    // ── RAWTEXT states ────────────────────────────────────
    /// §13.2.5.12 RAWTEXT less-than sign state
    ///
    /// TODO: not yet implemented
    RAWTEXTLessThanSign,
    /// §13.2.5.13 RAWTEXT end tag open state
    ///
    /// TODO: not yet implemented
    RAWTEXTEndTagOpen,
    /// §13.2.5.14 RAWTEXT end tag name state
    ///
    /// TODO: not yet implemented
    RAWTEXTEndTagName,

    // ── Script data states ────────────────────────────────
    /// §13.2.5.15 Script data less-than sign state
    ///
    /// TODO: not yet implemented
    ScriptDataLessThanSign,
    /// §13.2.5.16 Script data end tag open state
    ///
    /// TODO: not yet implemented
    ScriptDataEndTagOpen,
    /// §13.2.5.17 Script data end tag name state
    ///
    /// TODO: not yet implemented
    ScriptDataEndTagName,
    /// §13.2.5.18 Script data escape start state
    ///
    /// TODO: not yet implemented
    ScriptDataEscapeStart,
    /// §13.2.5.19 Script data escape start dash state
    ///
    /// TODO: not yet implemented
    ScriptDataEscapeStartDash,
    /// §13.2.5.20 Script data escaped state
    ///
    /// TODO: not yet implemented
    ScriptDataEscaped,
    /// §13.2.5.21 Script data escaped dash state
    ///
    /// TODO: not yet implemented
    ScriptDataEscapedDash,
    /// §13.2.5.22 Script data escaped dash dash state
    ///
    /// TODO: not yet implemented
    ScriptDataEscapedDashDash,
    /// §13.2.5.23 Script data escaped less-than sign state
    ///
    /// TODO: not yet implemented
    ScriptDataEscapedLessThanSign,
    /// §13.2.5.24 Script data escaped end tag open state
    ///
    /// TODO: not yet implemented
    ScriptDataEscapedEndTagOpen,
    /// §13.2.5.25 Script data escaped end tag name state
    ///
    /// TODO: not yet implemented
    ScriptDataEscapedEndTagName,
    /// §13.2.5.26 Script data double escape start state
    ///
    /// TODO: not yet implemented
    ScriptDataDoubleEscapeStart,
    /// §13.2.5.27 Script data double escaped state
    ///
    /// TODO: not yet implemented
    ScriptDataDoubleEscaped,
    /// §13.2.5.28 Script data double escaped dash state
    ///
    /// TODO: not yet implemented
    ScriptDataDoubleEscapedDash,
    /// §13.2.5.29 Script data double escaped dash dash state
    ///
    /// TODO: not yet implemented
    ScriptDataDoubleEscapedDashDash,
    /// §13.2.5.30 Script data double escaped less-than sign state
    ///
    /// TODO: not yet implemented
    ScriptDataDoubleEscapedLessThanSign,
    /// §13.2.5.31 Script data double escape end state
    ///
    /// TODO: not yet implemented
    ScriptDataDoubleEscapeEnd,

    // ── Attribute states ──────────────────────────────────
    /// §13.2.5.32 Before attribute name state
    ///
    /// TODO: not yet implemented
    BeforeAttributeName,
    /// §13.2.5.33 Attribute name state
    ///
    /// TODO: not yet implemented
    AttributeName,
    /// §13.2.5.34 After attribute name state
    ///
    /// TODO: not yet implemented
    AfterAttributeName,
    /// §13.2.5.35 Before attribute value state
    ///
    /// TODO: not yet implemented
    BeforeAttributeValue,
    /// §13.2.5.36 Attribute value (double-quoted) state
    ///
    /// TODO: not yet implemented
    AttributeValueDoubleQuoted,
    /// §13.2.5.37 Attribute value (single-quoted) state
    ///
    /// TODO: not yet implemented
    AttributeValueSingleQuoted,
    /// §13.2.5.38 Attribute value (unquoted) state
    ///
    /// TODO: not yet implemented
    AttributeValueUnquoted,
    /// §13.2.5.39 After attribute value (quoted) state
    ///
    /// TODO: not yet implemented
    AfterAttributeValueQuoted,
    /// §13.2.5.40 Self-closing start tag state
    ///
    /// TODO: not yet implemented
    SelfClosingStartTag,

    // ── Comment states ────────────────────────────────────
    /// §13.2.5.41 Bogus comment state
    ///
    /// TODO: not yet implemented
    BogusComment,
    /// §13.2.5.42 Markup declaration open state
    ///
    /// TODO: not yet implemented
    MarkupDeclarationOpen,
    /// §13.2.5.43 Comment start state
    ///
    /// TODO: not yet implemented
    CommentStart,
    /// §13.2.5.44 Comment start dash state
    ///
    /// TODO: not yet implemented
    CommentStartDash,
    /// §13.2.5.45 Comment state
    ///
    /// TODO: not yet implemented
    Comment,
    /// §13.2.5.46 Comment less-than sign state
    ///
    /// TODO: not yet implemented
    CommentLessThanSign,
    /// §13.2.5.47 Comment less-than sign bang state
    ///
    /// TODO: not yet implemented
    CommentLessThanSignBang,
    /// §13.2.5.48 Comment less-than sign bang dash state
    ///
    /// TODO: not yet implemented
    CommentLessThanSignBangDash,
    /// §13.2.5.49 Comment less-than sign bang dash dash state
    ///
    /// TODO: not yet implemented
    CommentLessThanSignBangDashDash,
    /// §13.2.5.50 Comment end dash state
    ///
    /// TODO: not yet implemented
    CommentEndDash,
    /// §13.2.5.51 Comment end state
    ///
    /// TODO: not yet implemented
    CommentEnd,
    /// §13.2.5.52 Comment end bang state
    ///
    /// TODO: not yet implemented
    CommentEndBang,

    // ── DOCTYPE states ────────────────────────────────────
    /// §13.2.5.53 DOCTYPE state
    ///
    /// TODO: not yet implemented
    Doctype,
    /// §13.2.5.54 Before DOCTYPE name state
    ///
    /// TODO: not yet implemented
    BeforeDoctypeName,
    /// §13.2.5.55 DOCTYPE name state
    ///
    /// TODO: not yet implemented
    DoctypeName,
    /// §13.2.5.56 After DOCTYPE name state
    ///
    /// TODO: not yet implemented
    AfterDoctypeName,
    /// §13.2.5.57 After DOCTYPE public keyword state
    ///
    /// TODO: not yet implemented
    AfterDoctypePublicKeyword,
    /// §13.2.5.58 Before DOCTYPE public identifier state
    ///
    /// TODO: not yet implemented
    BeforeDoctypePublicId,
    /// §13.2.5.59 DOCTYPE public identifier (double-quoted) state
    ///
    /// TODO: not yet implemented
    DoctypePublicIdDoubleQuoted,
    /// §13.2.5.60 DOCTYPE public identifier (single-quoted) state
    ///
    /// TODO: not yet implemented
    DoctypePublicIdSingleQuoted,
    /// §13.2.5.61 After DOCTYPE public identifier state
    ///
    /// TODO: not yet implemented
    AfterDoctypePublicId,
    /// §13.2.5.62 Between DOCTYPE public and system identifiers state
    ///
    /// TODO: not yet implemented
    BetweenDoctypePublicAndSystemIds,
    /// §13.2.5.63 After DOCTYPE system keyword state
    ///
    /// TODO: not yet implemented
    AfterDoctypeSystemKeyword,
    /// §13.2.5.64 Before DOCTYPE system identifier state
    ///
    /// TODO: not yet implemented
    BeforeDoctypeSystemId,
    /// §13.2.5.65 DOCTYPE system identifier (double-quoted) state
    ///
    /// TODO: not yet implemented
    DoctypeSystemIdDoubleQuoted,
    /// §13.2.5.66 DOCTYPE system identifier (single-quoted) state
    ///
    /// TODO: not yet implemented
    DoctypeSystemIdSingleQuoted,
    /// §13.2.5.67 After DOCTYPE system identifier state
    ///
    /// TODO: not yet implemented
    AfterDoctypeSystemId,
    /// §13.2.5.68 Bogus DOCTYPE state
    ///
    /// TODO: not yet implemented
    BogusDoctype,

    // ── CDATA section states ──────────────────────────────
    /// §13.2.5.69 CDATA section state
    ///
    /// TODO: not yet implemented
    CDATASection,
    /// §13.2.5.70 CDATA section bracket state
    ///
    /// TODO: not yet implemented
    CDATASectionBracket,
    /// §13.2.5.71 CDATA section end state
    ///
    /// TODO: not yet implemented
    CDATASectionEnd,

    // ── Character reference states ────────────────────────
    /// §13.2.5.72 Character reference state
    ///
    /// TODO: not yet implemented
    CharacterReference,
    /// §13.2.5.73 Named character reference state
    ///
    /// TODO: not yet implemented
    NamedCharacterReference,
    /// §13.2.5.74 Ambiguous ampersand state
    ///
    /// TODO: not yet implemented
    AmbiguousAmpersand,
    /// §13.2.5.75 Numeric character reference state
    ///
    /// TODO: not yet implemented
    NumericCharacterReference,
    /// §13.2.5.76 Hexadecimal character reference start state
    ///
    /// TODO: not yet implemented
    HexCharacterReferenceStart,
    /// §13.2.5.77 Decimal character reference start state
    ///
    /// TODO: not yet implemented
    DecimalCharacterReferenceStart,
    /// §13.2.5.78 Hexadecimal character reference state
    ///
    /// TODO: not yet implemented
    HexCharacterReference,
    /// §13.2.5.79 Decimal character reference state
    ///
    /// TODO: not yet implemented
    DecimalCharacterReference,
    /// §13.2.5.80 Numeric character reference end state
    ///
    /// TODO: not yet implemented
    NumericCharacterReferenceEnd,
}
