//! Tree construction mode dispatcher.
//!
//! Each insertion mode has a handler function that receives a token and
//! returns a [`Step`] indicating whether the token was consumed or needs
//! to be reprocessed in the new insertion mode.
//!
//! Phase 3.1 implements the prelude chain (§13.2.6.4.1–§13.2.6.4.6):
//! Initial → BeforeHtml → BeforeHead → InHead → AfterHead → InBody,
//! plus a minimal Text mode to absorb the contents of `<title>`/`<style>`/
//! `<script>` etc. Full InBody handling and remaining modes come in
//! Phase 3.2+.

use muskitty_dom::{append_child, Node};

use crate::error::ParseError;
use crate::tokenizer::{State, TagKind, Token, Tokenizer};

use super::helpers;
use super::insertion_mode::InsertionMode;
use super::HtmlTreeConstructor;

/// Result of a tree construction step.
pub enum Step {
    /// Token was consumed; get the next token.
    Done,
    /// Switch insertion mode and reprocess the same token.
    Reprocess,
}

/// Dispatch a token to the handler for the parser's current insertion mode.
///
/// The `tokenizer` is passed so handlers can switch the tokenizer's content
/// model (e.g. RCDATA for `<title>`, RAWTEXT for `<style>`, ScriptData for
/// `<script>`, per §13.2.6.4.4).
pub fn dispatch(
    parser: &mut HtmlTreeConstructor,
    token: &Token,
    tokenizer: &mut dyn Tokenizer,
) -> Step {
    match parser.insertion_mode {
        InsertionMode::Initial => handle_initial(parser, token),
        InsertionMode::BeforeHtml => handle_before_html(parser, token),
        InsertionMode::BeforeHead => handle_before_head(parser, token),
        InsertionMode::InHead => handle_in_head(parser, token, tokenizer),
        InsertionMode::AfterHead => handle_after_head(parser, token, tokenizer),
        InsertionMode::InBody => handle_in_body(parser, token),
        InsertionMode::Text => handle_text(parser, token, tokenizer),
        // All other modes are stubs until later phases.
        _ => handle_stub(parser, token),
    }
}

/// Check if a character is a WHATWG whitespace character (§13.2.6.4.1).
fn is_whitespace(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{000C}' | '\r' | ' ')
}

/// Create an element with the given tag name, append it to the current node,
/// and push it onto the open elements stack.
fn create_and_push(parser: &mut HtmlTreeConstructor, name: &str) {
    let element = Node::new_element_html(name, vec![], &parser.document);
    let current = parser.current_node();
    let _ = append_child(&current, element.clone());
    parser.open_elements.push(element);
}

// ── Initial insertion mode (§13.2.6.4.1) ──────────────────────

fn handle_initial(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
    match token {
        Token::Character(c) if is_whitespace(*c) => Step::Done,
        Token::Comment(data) => {
            helpers::insert_comment_at(&parser.document, data, &parser.document);
            Step::Done
        }
        Token::Doctype(dt) => {
            // Validate DOCTYPE: name must be "html", public ID must be absent,
            // system ID must be absent or "about:legacy-compat".
            if dt.name.as_deref() != Some("html")
                || dt.public_id.is_some()
                || (dt.system_id.is_some() && dt.system_id.as_deref() != Some("about:legacy-compat"))
            {
                parser.errors.push(ParseError::InvalidDoctype);
            }
            let doctype_node = Node::new_document_type(
                dt.name.as_deref().unwrap_or(""),
                dt.public_id.as_deref().unwrap_or(""),
                dt.system_id.as_deref().unwrap_or(""),
                &parser.document,
            );
            let _ = append_child(&parser.document, doctype_node);
            Step::Done
        }
        _ => {
            parser.insertion_mode = InsertionMode::BeforeHtml;
            Step::Reprocess
        }
    }
}

// ── Before html insertion mode (§13.2.6.4.2) ──────────────────

fn handle_before_html(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
    match token {
        Token::Doctype(_) => {
            parser
                .errors
                .push(ParseError::Generic("unexpected DOCTYPE in before html"));
            Step::Done
        }
        Token::Comment(data) => {
            helpers::insert_comment_at(&parser.document, data, &parser.document);
            Step::Done
        }
        Token::Character(c) if is_whitespace(*c) => Step::Done,
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "html" => {
            let element = helpers::create_element_for_token(parser, tag);
            let _ = append_child(&parser.document, element.clone());
            parser.open_elements.push(element);
            parser.insertion_mode = InsertionMode::BeforeHead;
            Step::Done
        }
        Token::Tag(tag)
            if tag.kind == TagKind::End && matches!(tag.name.as_str(), "head" | "body" | "html" | "br") =>
        {
            // Act as anything-else: create html, switch to BeforeHead, reprocess.
            create_and_push(parser, "html");
            parser.insertion_mode = InsertionMode::BeforeHead;
            Step::Reprocess
        }
        _ => {
            create_and_push(parser, "html");
            parser.insertion_mode = InsertionMode::BeforeHead;
            Step::Reprocess
        }
    }
}

// ── Before head insertion mode (§13.2.6.4.3) ──────────────────

fn handle_before_head(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
    match token {
        Token::Character(c) if is_whitespace(*c) => Step::Done,
        Token::Comment(data) => {
            helpers::insert_comment(parser, data);
            Step::Done
        }
        Token::Doctype(_) => {
            parser
                .errors
                .push(ParseError::Generic("unexpected DOCTYPE in before head"));
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "html" => {
            // Process using in body rules — skeleton ignores it (Phase 3.2).
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "head" => {
            let element = helpers::create_element_for_token(parser, tag);
            let current = parser.current_node();
            let _ = append_child(&current, element.clone());
            parser.open_elements.push(element.clone());
            parser.head_element = Some(element);
            parser.insertion_mode = InsertionMode::InHead;
            Step::Done
        }
        Token::Tag(tag)
            if tag.kind == TagKind::End && matches!(tag.name.as_str(), "head" | "body" | "html" | "br") =>
        {
            // Act as anything-else: create head, switch to InHead, reprocess.
            create_and_push(parser, "head");
            parser.head_element = parser.open_elements.last().cloned();
            parser.insertion_mode = InsertionMode::InHead;
            Step::Reprocess
        }
        _ => {
            create_and_push(parser, "head");
            parser.head_element = parser.open_elements.last().cloned();
            parser.insertion_mode = InsertionMode::InHead;
            Step::Reprocess
        }
    }
}

// ── In head insertion mode (§13.2.6.4.4) ──────────────────────

fn handle_in_head(
    parser: &mut HtmlTreeConstructor,
    token: &Token,
    tokenizer: &mut dyn Tokenizer,
) -> Step {
    match token {
        Token::Character(c) if is_whitespace(*c) => {
            helpers::insert_character(parser, *c);
            Step::Done
        }
        Token::Comment(data) => {
            helpers::insert_comment(parser, data);
            Step::Done
        }
        Token::Doctype(_) => {
            parser
                .errors
                .push(ParseError::Generic("unexpected DOCTYPE in head"));
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "html" => {
            // Process using in body rules — skeleton ignores it (Phase 3.2).
            Step::Done
        }
        // base / basefont / bgsound / link: insert element, immediately pop.
        Token::Tag(tag)
            if tag.kind == TagKind::Start
                && matches!(tag.name.as_str(), "base" | "basefont" | "bgsound" | "link") =>
        {
            helpers::insert_element(parser, tag);
            parser.open_elements.pop();
            Step::Done
        }
        // meta: insert element, immediately pop. (Charset/pragma processing
        // deferred — skeleton just creates the node.)
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "meta" => {
            helpers::insert_element(parser, tag);
            parser.open_elements.pop();
            Step::Done
        }
        // title: switch tokenizer to RCDATA, insert element, switch to Text.
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "title" => {
            tokenizer.set_appropriate_end_tag_name(Some(&tag.name));
            tokenizer.set_state(State::RCDATA);
            helpers::insert_element(parser, tag);
            parser.original_insertion_mode = Some(parser.insertion_mode);
            parser.insertion_mode = InsertionMode::Text;
            Step::Done
        }
        // noframes / style: switch tokenizer to RAWTEXT, insert element, Text.
        Token::Tag(tag)
            if tag.kind == TagKind::Start && matches!(tag.name.as_str(), "noframes" | "style") =>
        {
            tokenizer.set_appropriate_end_tag_name(Some(&tag.name));
            tokenizer.set_state(State::RAWTEXT);
            helpers::insert_element(parser, tag);
            parser.original_insertion_mode = Some(parser.insertion_mode);
            parser.insertion_mode = InsertionMode::Text;
            Step::Done
        }
        // noscript with scripting disabled: insert element, switch to
        // InHeadNoscript. (Scripting-enabled branch uses RAWTEXT; since the
        // skeleton's scripting_flag defaults to false, only the disabled
        // branch is implemented here. Phase 3.5 will add scripting support.)
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "noscript" => {
            if !parser.scripting_flag {
                helpers::insert_element(parser, tag);
                parser.insertion_mode = InsertionMode::InHeadNoscript;
                Step::Done
            } else {
                tokenizer.set_appropriate_end_tag_name(Some(&tag.name));
                tokenizer.set_state(State::RAWTEXT);
                helpers::insert_element(parser, tag);
                parser.original_insertion_mode = Some(parser.insertion_mode);
                parser.insertion_mode = InsertionMode::Text;
                Step::Done
            }
        }
        // script: switch tokenizer to ScriptData, insert element, Text.
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "script" => {
            tokenizer.set_appropriate_end_tag_name(Some(&tag.name));
            tokenizer.set_state(State::ScriptData);
            helpers::insert_element(parser, tag);
            parser.original_insertion_mode = Some(parser.insertion_mode);
            parser.insertion_mode = InsertionMode::Text;
            Step::Done
        }
        // template: complex (active formatting elements + template content
        // stack). Deferred to Phase 3.5.
        Token::Tag(tag)
            if tag.kind == TagKind::Start && tag.name == "template" =>
        {
            let _ = tag;
            parser
                .errors
                .push(ParseError::Generic("template not yet supported (Phase 3.5)"));
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::End && tag.name == "template" => {
            let _ = tag;
            parser
                .errors
                .push(ParseError::Generic("template end tag not yet supported (Phase 3.5)"));
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "head" => {
            parser
                .errors
                .push(ParseError::Generic("duplicate head start tag"));
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::End && tag.name == "head" => {
            parser.open_elements.pop();
            parser.insertion_mode = InsertionMode::AfterHead;
            Step::Done
        }
        Token::Tag(tag)
            if tag.kind == TagKind::End && matches!(tag.name.as_str(), "body" | "html" | "br") =>
        {
            // Act as anything-else: pop head, switch to AfterHead, reprocess.
            parser.open_elements.pop();
            parser.insertion_mode = InsertionMode::AfterHead;
            Step::Reprocess
        }
        // Any other start tag → anything-else.
        Token::Tag(tag) if tag.kind == TagKind::Start => {
            let _ = tag;
            parser.open_elements.pop();
            parser.insertion_mode = InsertionMode::AfterHead;
            Step::Reprocess
        }
        // Any other end tag → parse error, ignore.
        Token::Tag(tag) if tag.kind == TagKind::End => {
            parser
                .errors
                .push(ParseError::UnexpectedEndTag(tag.name.clone()));
            Step::Done
        }
        _ => {
            // Anything else: pop head, switch to AfterHead, reprocess.
            parser.open_elements.pop();
            parser.insertion_mode = InsertionMode::AfterHead;
            Step::Reprocess
        }
    }
}

// ── After head insertion mode (§13.2.6.4.6) ───────────────────

fn handle_after_head(
    parser: &mut HtmlTreeConstructor,
    token: &Token,
    tokenizer: &mut dyn Tokenizer,
) -> Step {
    match token {
        Token::Character(c) if is_whitespace(*c) => {
            helpers::insert_character(parser, *c);
            Step::Done
        }
        Token::Comment(data) => {
            helpers::insert_comment(parser, data);
            Step::Done
        }
        Token::Doctype(_) => {
            parser
                .errors
                .push(ParseError::Generic("unexpected DOCTYPE after head"));
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "html" => {
            // Process using in body rules — skeleton ignores it (Phase 3.2).
            let _ = tag;
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "body" => {
            let element = helpers::create_element_for_token(parser, tag);
            let current = parser.current_node();
            let _ = append_child(&current, element.clone());
            parser.open_elements.push(element);
            parser.frameset_ok = false;
            parser.insertion_mode = InsertionMode::InBody;
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "frameset" => {
            let element = helpers::create_element_for_token(parser, tag);
            let current = parser.current_node();
            let _ = append_child(&current, element.clone());
            parser.open_elements.push(element);
            parser.insertion_mode = InsertionMode::InFrameset;
            Step::Done
        }
        // base/basefont/bgsound/link/meta/noframes/script/style/template/title:
        // parse error. Push the head element back onto the stack, process the
        // token using the "in head" rules, then remove the head element again.
        // Simplified: reprocess in InHead with head temporarily pushed.
        Token::Tag(tag)
            if tag.kind == TagKind::Start
                && matches!(
                    tag.name.as_str(),
                    "base" | "basefont" | "bgsound" | "link" | "meta" | "noframes"
                        | "script" | "style" | "template" | "title"
                ) =>
        {
            parser
                .errors
                .push(ParseError::UnexpectedStartTag(tag.name.clone()));
            if let Some(head) = parser.head_element.clone() {
                parser.open_elements.push(head);
                // Process in InHead.
                parser.insertion_mode = InsertionMode::InHead;
                // After InHead pops back, we need to remove the head and
                // return to AfterHead. For the skeleton, reprocess in InHead;
                // the InHead handler's anything-else / head-end-tag will pop
                // and switch to AfterHead, which is close enough for the
                // common cases (e.g. <meta> after </head>).
                Step::Reprocess
            } else {
                Step::Done
            }
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "head" => {
            parser
                .errors
                .push(ParseError::Generic("unexpected head start tag after head"));
            Step::Done
        }
        Token::Tag(tag)
            if tag.kind == TagKind::End && matches!(tag.name.as_str(), "body" | "html" | "br") =>
        {
            // Act as anything-else: create body, switch to InBody, reprocess.
            create_and_push(parser, "body");
            parser.frameset_ok = false;
            parser.insertion_mode = InsertionMode::InBody;
            Step::Reprocess
        }
        // template end tag: process using in head rules.
        Token::Tag(tag) if tag.kind == TagKind::End && tag.name == "template" => {
            let _ = (tag, tokenizer);
            parser
                .errors
                .push(ParseError::Generic("template end tag not yet supported (Phase 3.5)"));
            Step::Done
        }
        _ => {
            // Anything else: create body, switch to InBody, reprocess.
            create_and_push(parser, "body");
            parser.frameset_ok = false;
            parser.insertion_mode = InsertionMode::InBody;
            Step::Reprocess
        }
    }
}

// ── Text insertion mode (§13.2.6.5) — minimal ────────────────
//
// Entered after a `<title>`/`<style>`/`<script>`/etc. start tag. Absorbs
// the element's character content until the matching end tag, then pops the
// element and restores the original insertion mode.

fn handle_text(
    parser: &mut HtmlTreeConstructor,
    token: &Token,
    _tokenizer: &mut dyn Tokenizer,
) -> Step {
    match token {
        Token::Character(c) => {
            helpers::insert_character(parser, *c);
            Step::Done
        }
        Token::EOF => {
            parser
                .errors
                .push(ParseError::Generic("unexpected EOF in text mode"));
            // Pop the open element and reprocess EOF in the original mode.
            parser.open_elements.pop();
            if let Some(orig) = parser.original_insertion_mode.take() {
                parser.insertion_mode = orig;
            }
            Step::Reprocess
        }
        Token::Tag(tag) if tag.kind == TagKind::End => {
            let _ = tag;
            // Pop the current element (the title/style/script/etc.).
            parser.open_elements.pop();
            // Restore the original insertion mode.
            if let Some(orig) = parser.original_insertion_mode.take() {
                parser.insertion_mode = orig;
            }
            // Reset tokenizer to Data state and clear the appropriate end tag
            // name so subsequent `</...>` sequences are parsed as normal tags.
            _tokenizer.set_state(State::Data);
            _tokenizer.set_appropriate_end_tag_name(None);
            Step::Done
        }
        // Any other token (start tags, comments, doctype) is a parse error
        // in Text mode; skeleton ignores them for now.
        _ => {
            parser
                .errors
                .push(ParseError::Generic("unexpected token in text mode"));
            Step::Done
        }
    }
}

// ── In body insertion mode (§13.2.6.4.7) — minimal skeleton ──

fn handle_in_body(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
    match token {
        Token::EOF => Step::Done,
        Token::Character(c) if is_whitespace(*c) => {
            helpers::insert_character(parser, *c);
            Step::Done
        }
        Token::Character(c) => {
            helpers::insert_character(parser, *c);
            Step::Done
        }
        Token::Comment(data) => {
            helpers::insert_comment(parser, data);
            Step::Done
        }
        // All other token types are deferred to Phase 3.2.
        _ => Step::Done,
    }
}

// ── Stub for unimplemented modes ────────────────────────────────

fn handle_stub(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
    match token {
        Token::EOF => Step::Done,
        _ => {
            let _ = parser;
            todo!("insertion mode not yet implemented — Phase 3");
        }
    }
}
