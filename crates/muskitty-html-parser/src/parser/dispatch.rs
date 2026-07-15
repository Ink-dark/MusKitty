//! Tree construction mode dispatcher.
//!
//! Each insertion mode has a handler function that receives a token and
//! returns a [`Step`] indicating whether the token was consumed or needs
//! to be reprocessed in the new insertion mode.
//!
//! The skeleton implements the Initial → BeforeHtml → BeforeHead → InHead →
//! AfterHead → InBody chain (§13.2.6.2–§13.2.6.7) with minimal logic.
//! Full token handling for each mode is implemented in Phase 3.

use muskitty_dom::{append_child, Node};

use crate::error::ParseError;
use crate::tokenizer::{TagKind, Token};

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
pub fn dispatch(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
    match parser.insertion_mode {
        InsertionMode::Initial => handle_initial(parser, token),
        InsertionMode::BeforeHtml => handle_before_html(parser, token),
        InsertionMode::BeforeHead => handle_before_head(parser, token),
        InsertionMode::InHead => handle_in_head(parser, token),
        InsertionMode::AfterHead => handle_after_head(parser, token),
        InsertionMode::InBody => handle_in_body(parser, token),
        // All other modes are stubs until Phase 3.
        _ => handle_stub(parser, token),
    }
}

/// Check if a character is a WHATWG whitespace character (§13.2.6.2).
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

// ── Initial insertion mode (§13.2.6.2) ──────────────────────────

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

// ── Before html insertion mode (§13.2.6.3) ──────────────────────

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

// ── Before head insertion mode (§13.2.6.4) ──────────────────────

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
            // Process using in body rules — skeleton ignores it.
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

// ── In head insertion mode (§13.2.6.5) — minimal ───────────────

fn handle_in_head(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
    match token {
        Token::Character(c) if is_whitespace(*c) => Step::Done,
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
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "head" => {
            parser
                .errors
                .push(ParseError::Generic("duplicate head start tag"));
            Step::Done
        }
        // title/style/script/link/meta/base: skeleton ignores them.
        // Phase 3 will implement content model switching and element creation.
        Token::Tag(tag) if tag.kind == TagKind::Start => {
            let _ = tag; // acknowledged
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
        _ => {
            // Pop head, switch to AfterHead, reprocess.
            parser.open_elements.pop();
            parser.insertion_mode = InsertionMode::AfterHead;
            Step::Reprocess
        }
    }
}

// ── After head insertion mode (§13.2.6.7) — minimal ────────────

fn handle_after_head(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
    match token {
        Token::Character(c) if is_whitespace(*c) => Step::Done,
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
            // Process using in body rules — skeleton ignores it.
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
            let _ = tag;
            todo!("frameset handling in after head — Phase 3.5");
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
        _ => {
            create_and_push(parser, "body");
            parser.frameset_ok = false;
            parser.insertion_mode = InsertionMode::InBody;
            Step::Reprocess
        }
    }
}

// ── In body insertion mode (§13.2.6.4) — minimal skeleton ──────

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
