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

use std::cell::RefCell;
use std::rc::Rc;

use muskitty_dom::{append_child, Attribute, Node, NodeKind};

use crate::error::ParseError;
use crate::tokenizer::{State, TagKind, Token, Tokenizer};

use super::helpers;
use super::insertion_mode::InsertionMode;
use super::{ActiveFormattingEntry, HtmlTreeConstructor};

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
        InsertionMode::AfterBody => handle_after_body(parser, token),
        InsertionMode::AfterAfterBody => handle_after_after_body(parser, token),
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
                || (dt.system_id.is_some()
                    && dt.system_id.as_deref() != Some("about:legacy-compat"))
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
            if tag.kind == TagKind::End
                && matches!(tag.name.as_str(), "head" | "body" | "html" | "br") =>
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
            if tag.kind == TagKind::End
                && matches!(tag.name.as_str(), "head" | "body" | "html" | "br") =>
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
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "template" => {
            let _ = tag;
            parser.errors.push(ParseError::Generic(
                "template not yet supported (Phase 3.5)",
            ));
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::End && tag.name == "template" => {
            let _ = tag;
            parser.errors.push(ParseError::Generic(
                "template end tag not yet supported (Phase 3.5)",
            ));
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
                    "base"
                        | "basefont"
                        | "bgsound"
                        | "link"
                        | "meta"
                        | "noframes"
                        | "script"
                        | "style"
                        | "template"
                        | "title"
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
            parser.errors.push(ParseError::Generic(
                "template end tag not yet supported (Phase 3.5)",
            ));
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

// ── In body insertion mode (§13.2.6.4.7) ──────────────────────
//
// Phase 3.2 implements block-level elements, headings, lists, forms,
// void elements, and basic end-tag handling. Formatting elements
// (`<b>`/`<i>`/`<a>`/etc.) and the adoption agency algorithm are
// deferred to Phase 3.3.

/// Tag names that the spec groups under "address/article/aside/...":
/// close a `<p>` if open, then insert a fresh HTML element.
const BLOCK_LEVEL_START_TAGS: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "center",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "header",
    "hgroup",
    "main",
    "menu",
    "nav",
    "ol",
    "p",
    "search",
    "section",
    "summary",
    "ul",
];

/// Same as BLOCK_LEVEL_START_TAGS, used by the "any other end tag" branch.
const BLOCK_LEVEL_END_TAGS: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "button",
    "center",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "header",
    "hgroup",
    "main",
    "menu",
    "nav",
    "ol",
    "p",
    "search",
    "section",
    "summary",
    "ul",
];

/// HTML void elements (§13.2.6.2) — inserted and immediately popped.
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "keygen", "link", "meta", "param",
    "source", "track", "wbr",
];

fn handle_in_body(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
    match token {
        Token::EOF => Step::Done,
        Token::Character(c) if is_whitespace(*c) => {
            helpers::reconstruct_active_formatting_elements(parser);
            helpers::insert_character(parser, *c);
            Step::Done
        }
        Token::Character(c) => {
            helpers::reconstruct_active_formatting_elements(parser);
            parser.frameset_ok = false;
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
                .push(ParseError::Generic("unexpected DOCTYPE in body"));
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::Start => handle_in_body_start_tag(parser, tag),
        Token::Tag(tag) if tag.kind == TagKind::End => handle_in_body_end_tag(parser, tag),
        _ => Step::Done,
    }
}

fn handle_in_body_start_tag(
    parser: &mut HtmlTreeConstructor,
    tag: &crate::tokenizer::TagToken,
) -> Step {
    let name = tag.name.as_str();

    // "html" — merge attributes onto the existing <html> element.
    if name == "html" {
        parser
            .errors
            .push(ParseError::Generic("unexpected <html> start tag in body"));
        // Merge attributes onto the html element (top of stack after document).
        if let Some(html) = parser.open_elements.first().cloned() {
            merge_attributes(&html, tag);
        }
        parser.frameset_ok = false;
        return Step::Done;
    }

    // Head-element start tags: process using the rules for "in head".
    if matches!(
        name,
        "base"
            | "basefont"
            | "bgsound"
            | "link"
            | "meta"
            | "noframes"
            | "script"
            | "style"
            | "template"
            | "title"
    ) {
        // Defer to the InHead handler. We cannot call it directly (it's a
        // private fn), so switch modes transiently. Simpler: replicate the
        // common void-element behaviour here.
        if matches!(name, "base" | "basefont" | "bgsound" | "link" | "meta") {
            helpers::insert_element(parser, tag);
            parser.open_elements.pop();
        } else {
            // title/style/script/noframes/template: defer to a future InHead
            // callback by signalling that this tag is not yet supported
            // inline. For Phase 3.2 we mark it as a parse error so callers
            // know it was ignored.
            parser.errors.push(ParseError::Generic(
                "head element in body not yet inline-handled",
            ));
        }
        return Step::Done;
    }

    // "body" — merge attributes onto the existing <body> element.
    if name == "body" {
        parser
            .errors
            .push(ParseError::Generic("unexpected <body> start tag in body"));
        if parser.open_elements.len() >= 2 {
            // open_elements[1] is typically the body.
            if let Some(body) = parser.open_elements.get(1).cloned() {
                merge_attributes(&body, tag);
            }
        }
        parser.frameset_ok = false;
        return Step::Done;
    }

    // "frameset" — deferred to Phase 3.5.
    if name == "frameset" {
        parser.errors.push(ParseError::Generic(
            "frameset in body not yet supported (Phase 3.5)",
        ));
        return Step::Done;
    }

    // Block-level: close <p> if in button scope, insert.
    if BLOCK_LEVEL_START_TAGS.contains(&name) {
        if helpers::has_element_in_button_scope(parser, "p") {
            helpers::close_p_element(parser);
        }
        helpers::insert_element(parser, tag);
        return Step::Done;
    }

    // Headings h1-h6.
    if matches!(name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
        if helpers::has_element_in_button_scope(parser, "p") {
            helpers::close_p_element(parser);
        }
        // If current node is a heading, parse error: close it.
        if let Some(top) = parser.open_elements.last() {
            let top_name = top.borrow().kind.as_element().map(|e| e.local_name.clone());
            if matches!(
                top_name.as_deref(),
                Some("h1" | "h2" | "h3" | "h4" | "h5" | "h6")
            ) {
                parser
                    .errors
                    .push(ParseError::Generic("heading nested in heading"));
                parser.open_elements.pop();
            }
        }
        helpers::insert_element(parser, tag);
        return Step::Done;
    }

    // pre / listing: close p, insert, skip a leading newline, frameset_ok=false.
    if matches!(name, "pre" | "listing") {
        if helpers::has_element_in_button_scope(parser, "p") {
            helpers::close_p_element(parser);
        }
        helpers::insert_element(parser, tag);
        // Skip a single leading U+000A (newline) per §13.2.6.4.7. The
        // tokenizer emits characters one at a time, so the next token must
        // be checked by the caller; for the skeleton, we rely on the
        // tokenizer emitting that character normally and treat this as
        // best-effort.
        parser.frameset_ok = false;
        return Step::Done;
    }

    // form: close p; if form_element is set, parse error; else insert and
    // set form_element.
    if name == "form" {
        if helpers::has_element_in_button_scope(parser, "p") {
            helpers::close_p_element(parser);
        }
        if parser.form_element.is_some() {
            parser
                .errors
                .push(ParseError::Generic("nested form element"));
            // Per spec, ignore the start tag entirely if form pointer set
            // AND template content exists. Skeleton ignores the second
            // condition and just drops the tag.
            return Step::Done;
        }
        let element = helpers::create_element_for_token(parser, tag);
        helpers::insert_node(parser, &element);
        parser.open_elements.push(element.clone());
        parser.form_element = Some(element);
        return Step::Done;
    }

    // li: close p; loop popping li if in list scope.
    if name == "li" {
        parser.frameset_ok = false;
        if helpers::has_element_in_list_scope(parser, "li") {
            helpers::generate_implied_end_tags(parser, Some("li"));
            // Pop until li is popped.
            while let Some(top) = parser.open_elements.last() {
                let is_li = top
                    .borrow()
                    .kind
                    .as_element()
                    .map(|e| e.local_name.as_str())
                    == Some("li");
                parser.open_elements.pop();
                if is_li {
                    break;
                }
            }
        } else {
            // No li in scope: just close p.
            if helpers::has_element_in_button_scope(parser, "p") {
                helpers::close_p_element(parser);
            }
        }
        helpers::insert_element(parser, tag);
        return Step::Done;
    }

    // dd / dt: similar to li.
    if matches!(name, "dd" | "dt") {
        parser.frameset_ok = false;
        if helpers::has_element_in_scope(parser, "dd")
            || helpers::has_element_in_scope(parser, "dt")
        {
            helpers::generate_implied_end_tags(parser, Some(name));
            // Pop until dd/dt popped.
            while let Some(top) = parser.open_elements.last() {
                let top_name = top.borrow().kind.as_element().map(|e| e.local_name.clone());
                let is_target = matches!(top_name.as_deref(), Some("dd") | Some("dt"));
                parser.open_elements.pop();
                if is_target {
                    break;
                }
            }
        } else if helpers::has_element_in_button_scope(parser, "p") {
            helpers::close_p_element(parser);
        }
        helpers::insert_element(parser, tag);
        return Step::Done;
    }

    // plaintext: insert, switch tokenizer to PLAINTEXT.
    if name == "plaintext" {
        helpers::insert_element(parser, tag);
        // Tokenizer state switch is done by the caller; mark it via a
        // side-channel for now. For Phase 3.2 we don't have access here;
        // left as a parse error.
        parser.errors.push(ParseError::Generic(
            "plaintext tokenizer switch not yet supported",
        ));
        return Step::Done;
    }

    // button: if button in scope, parse error, pop until button, reprocess.
    if name == "button" {
        if helpers::has_element_in_scope(parser, "button") {
            parser.errors.push(ParseError::Generic("nested button"));
            helpers::generate_implied_end_tags(parser, None);
            while let Some(top) = parser.open_elements.last() {
                let is_button = top
                    .borrow()
                    .kind
                    .as_element()
                    .map(|e| e.local_name.as_str())
                    == Some("button");
                parser.open_elements.pop();
                if is_button {
                    break;
                }
            }
            // Reprocess in InBody.
            return Step::Reprocess;
        }
        helpers::reconstruct_active_formatting_elements(parser);
        helpers::insert_element(parser, tag);
        parser.frameset_ok = false;
        return Step::Done;
    }

    // hr: close p, frameset_ok=false, insert, pop.
    if name == "hr" {
        if helpers::has_element_in_button_scope(parser, "p") {
            helpers::close_p_element(parser);
        }
        helpers::insert_element(parser, tag);
        parser.open_elements.pop();
        parser.frameset_ok = false;
        return Step::Done;
    }

    // Void elements (area/base/br/col/embed/img/input/keygen/link/meta/
    // param/source/track/wbr): reconstruct, insert, pop. img/keygen/wbr
    // additionally set frameset_ok=false.
    if VOID_ELEMENTS.contains(&name) {
        // image → img (parse error).
        if name == "image" {
            parser
                .errors
                .push(ParseError::Generic("image start tag treated as img"));
        }
        helpers::reconstruct_active_formatting_elements(parser);
        // Build the element from the (possibly renamed) tag.
        let effective_tag = if name == "image" {
            crate::tokenizer::TagToken {
                kind: tag.kind,
                name: "img".to_string(),
                attrs: tag.attrs.clone(),
                self_closing: tag.self_closing,
            }
        } else {
            tag.clone()
        };
        helpers::insert_element(parser, &effective_tag);
        parser.open_elements.pop();
        if matches!(effective_tag.name.as_str(), "img" | "keygen" | "wbr") {
            parser.frameset_ok = false;
        }
        // input: frameset_ok=false unless type=hidden.
        if effective_tag.name == "input" {
            let is_hidden = effective_tag
                .attrs
                .iter()
                .any(|(k, v)| k.eq_ignore_ascii_case("type") && v.eq_ignore_ascii_case("hidden"));
            if !is_hidden {
                parser.frameset_ok = false;
            }
        }
        return Step::Done;
    }

    // image: parse error, act as img (handled above via VOID_ELEMENTS).
    if name == "image" {
        // Already handled by the VOID_ELEMENTS branch above; this is a
        // safety net in case the const list is reordered.
        return Step::Done;
    }

    // Formatting elements (a/b/big/code/em/font/i/nobr/s/small/strike/
    // strong/tt/u) — full active formatting bookkeeping + adoption agency.
    if matches!(
        name,
        "a" | "b"
            | "big"
            | "code"
            | "em"
            | "font"
            | "i"
            | "nobr"
            | "s"
            | "small"
            | "strike"
            | "strong"
            | "tt"
            | "u"
    ) {
        helpers::reconstruct_active_formatting_elements(parser);
        // Special case for <a>: if there is an <a> in the active formatting
        // elements list, run adoption agency for "a", then remove any <a>
        // from the list (§13.2.6.4.7).
        if name == "a" {
            let has_a_in_afe = parser.active_formatting_elements.iter().any(|e| {
                matches!(e, ActiveFormattingEntry::Element(el) if {
                    let l = el.borrow();
                    l.kind.as_element().map(|e| e.local_name.as_str()) == Some("a")
                })
            });
            if has_a_in_afe {
                helpers::adoption_agency(parser, "a");
                // Remove any <a> elements from the active formatting list
                // and the open elements stack.
                parser.active_formatting_elements.retain(|e| {
                    !matches!(e, ActiveFormattingEntry::Element(el) if {
                        let l = el.borrow();
                        l.kind.as_element().map(|e| e.local_name.as_str()) == Some("a")
                    })
                });
                if let Some(pos) = parser.open_elements.iter().position(|n| {
                    let l = n.borrow();
                    l.kind.as_element().map(|e| e.local_name.as_str()) == Some("a")
                }) {
                    parser.open_elements.truncate(pos);
                }
            }
        }
        helpers::reconstruct_active_formatting_elements(parser);
        let element = helpers::create_element_for_token(parser, tag);
        helpers::insert_node(parser, &element);
        parser.open_elements.push(element.clone());
        helpers::push_formatting_element(parser, element);
        return Step::Done;
    }

    // Anything else (start tag): reconstruct active formatting, insert.
    helpers::reconstruct_active_formatting_elements(parser);
    helpers::insert_element(parser, tag);
    Step::Done
}

fn handle_in_body_end_tag(
    parser: &mut HtmlTreeConstructor,
    tag: &crate::tokenizer::TagToken,
) -> Step {
    let name = tag.name.as_str();

    // </p>: if no p in button scope, parse error, insert <p>, reprocess.
    // Else: generate implied end tags except p, pop until p.
    if name == "p" {
        if !helpers::has_element_in_button_scope(parser, "p") {
            parser
                .errors
                .push(ParseError::Generic("end tag p without open p"));
            let p = muskitty_dom::Node::new_element_html("p", vec![], &parser.document);
            helpers::insert_node(parser, &p);
            parser.open_elements.push(p);
        }
        helpers::close_p_element(parser);
        return Step::Done;
    }

    // </body>: if body not in scope, parse error, ignore. Else: switch to
    // AfterBody.
    if name == "body" {
        if !helpers::has_element_in_scope(parser, "body") {
            parser
                .errors
                .push(ParseError::Generic("end tag body without body in scope"));
            return Step::Done;
        }
        parser.insertion_mode = InsertionMode::AfterBody;
        return Step::Done;
    }

    // </html>: if body not in scope, parse error, ignore. Else: switch to
    // AfterBody, reprocess.
    if name == "html" {
        if !helpers::has_element_in_scope(parser, "body") {
            parser
                .errors
                .push(ParseError::Generic("end tag html without body in scope"));
            return Step::Done;
        }
        parser.insertion_mode = InsertionMode::AfterBody;
        return Step::Reprocess;
    }

    // Block-level end tags (address/article/aside/blockquote/...): if not in
    // scope, parse error, ignore; else: generate implied end tags, if current
    // is not target, parse error, pop until target.
    if BLOCK_LEVEL_END_TAGS.contains(&name) {
        if !helpers::has_element_in_scope(parser, name) {
            parser
                .errors
                .push(ParseError::UnexpectedEndTag(name.to_string()));
            return Step::Done;
        }
        helpers::generate_implied_end_tags(parser, None);
        // Pop until target.
        while let Some(top) = parser.open_elements.last() {
            let top_name = top.borrow().kind.as_element().map(|e| e.local_name.clone());
            let is_target = top_name.as_deref() == Some(name);
            parser.open_elements.pop();
            if is_target {
                break;
            }
        }
        return Step::Done;
    }

    // </form>: if form_element is None, parse error, ignore. Else: set
    // form_element to None; if form not in scope, parse error; else:
    // generate implied end tags, pop until form.
    if name == "form" {
        if parser.form_element.is_none() {
            parser
                .errors
                .push(ParseError::Generic("end tag form without open form"));
            return Step::Done;
        }
        // Per spec, the form on the stack may differ from the form pointer
        // when inside template content; skeleton ignores template subtlety.
        parser.form_element = None;
        if !helpers::has_element_in_scope(parser, "form") {
            parser
                .errors
                .push(ParseError::Generic("end tag form without form in scope"));
            return Step::Done;
        }
        helpers::generate_implied_end_tags(parser, None);
        // Pop until form.
        while let Some(top) = parser.open_elements.last() {
            let is_form = top
                .borrow()
                .kind
                .as_element()
                .map(|e| e.local_name.as_str())
                == Some("form");
            parser.open_elements.pop();
            if is_form {
                break;
            }
        }
        return Step::Done;
    }

    // </li>/</dd>/</dt>: if not in list/scope, parse error, ignore; else:
    // generate implied end tags except tag, if current != tag, parse error,
    // pop until tag.
    if matches!(name, "li") {
        if !helpers::has_element_in_list_scope(parser, "li") {
            parser
                .errors
                .push(ParseError::UnexpectedEndTag(name.to_string()));
            return Step::Done;
        }
        helpers::generate_implied_end_tags(parser, Some("li"));
        while let Some(top) = parser.open_elements.last() {
            let is_target = top
                .borrow()
                .kind
                .as_element()
                .map(|e| e.local_name.as_str())
                == Some("li");
            parser.open_elements.pop();
            if is_target {
                break;
            }
        }
        return Step::Done;
    }
    if matches!(name, "dd" | "dt") {
        if !helpers::has_element_in_scope(parser, name) {
            parser
                .errors
                .push(ParseError::UnexpectedEndTag(name.to_string()));
            return Step::Done;
        }
        helpers::generate_implied_end_tags(parser, Some(name));
        while let Some(top) = parser.open_elements.last() {
            let top_name = top.borrow().kind.as_element().map(|e| e.local_name.clone());
            let is_target = top_name.as_deref() == Some(name);
            parser.open_elements.pop();
            if is_target {
                break;
            }
        }
        return Step::Done;
    }

    // </h1>-</h6>: if no heading in scope, parse error; else: generate
    // implied end tags, if current is not heading, parse error, pop until
    // heading.
    if matches!(name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
        let heading_in_scope = ["h1", "h2", "h3", "h4", "h5", "h6"]
            .iter()
            .any(|h| helpers::has_element_in_scope(parser, h));
        if !heading_in_scope {
            parser
                .errors
                .push(ParseError::UnexpectedEndTag(name.to_string()));
            return Step::Done;
        }
        helpers::generate_implied_end_tags(parser, None);
        while let Some(top) = parser.open_elements.last() {
            let top_name = top.borrow().kind.as_element().map(|e| e.local_name.clone());
            let is_heading = matches!(
                top_name.as_deref(),
                Some("h1" | "h2" | "h3" | "h4" | "h5" | "h6")
            );
            parser.open_elements.pop();
            if is_heading {
                break;
            }
        }
        return Step::Done;
    }

    // Formatting end tags (a/b/i/em/strong/code/etc.) — run the adoption
    // agency algorithm (§13.2.6.4.7).
    if matches!(
        name,
        "a" | "b"
            | "big"
            | "code"
            | "em"
            | "font"
            | "i"
            | "nobr"
            | "s"
            | "small"
            | "strike"
            | "strong"
            | "tt"
            | "u"
    ) {
        helpers::adoption_agency(parser, name);
        return Step::Done;
    }

    // Any other end tag: walk the stack from top to bottom.
    // For each node: if name matches, generate implied end tags except name,
    // if current != name, parse error, pop until name, break.
    // Else if node is special (in default scope set), parse error, return.
    for (i, node) in parser.open_elements.iter().enumerate().rev() {
        let node_name = node
            .borrow()
            .kind
            .as_element()
            .map(|e| e.local_name.clone());
        if node_name.as_deref() == Some(name) {
            helpers::generate_implied_end_tags(parser, Some(name));
            // Pop until we've popped the matching node at index i.
            while parser.open_elements.len() > i {
                parser.open_elements.pop();
            }
            return Step::Done;
        }
        // Special element (in default scope list) blocks the search.
        if let Some(n) = node_name.as_deref() {
            if helpers::SPECIAL_ELEMENTS.contains(&n) {
                parser
                    .errors
                    .push(ParseError::UnexpectedEndTag(name.to_string()));
                return Step::Done;
            }
        }
    }

    // No match on the stack: parse error, ignore.
    parser
        .errors
        .push(ParseError::UnexpectedEndTag(name.to_string()));
    Step::Done
}

/// Merge the attributes from `tag` onto `element`, skipping any whose name
/// already exists on the element (per "adjust the attributes" §13.2.6.2).
fn merge_attributes(element: &Rc<RefCell<muskitty_dom::Node>>, tag: &crate::tokenizer::TagToken) {
    let mut e = element.borrow_mut();
    if let NodeKind::Element(ref mut data) = e.kind {
        for (name, value) in &tag.attrs {
            let exists = data
                .attributes
                .iter()
                .any(|a| a.local_name.eq_ignore_ascii_case(name));
            if !exists {
                data.attributes.push(Attribute::new(name, value));
            }
        }
    }
}

// ── After body insertion mode (§13.2.6.4.17) ──────────────────

fn handle_after_body(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
    match token {
        Token::Character(c) if is_whitespace(*c) => {
            // Process using the rules for "in body".
            handle_in_body(parser, token)
        }
        Token::Comment(data) => {
            // Insert a comment as the last child of the first element in the
            // open elements stack (the <html> element).
            let html = parser
                .open_elements
                .first()
                .cloned()
                .unwrap_or_else(|| parser.document.clone());
            helpers::insert_comment_at(&html, data, &parser.document);
            Step::Done
        }
        Token::Doctype(_) => {
            parser
                .errors
                .push(ParseError::Generic("unexpected DOCTYPE after body"));
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "html" => {
            // Process using the rules for "in body".
            handle_in_body(parser, token)
        }
        Token::Tag(tag) if tag.kind == TagKind::End && tag.name == "body" => {
            // If body not in scope, parse error, ignore. Otherwise switch to
            // "after after body".
            if !helpers::has_element_in_scope(parser, "body") {
                parser
                    .errors
                    .push(ParseError::Generic("end tag body without body in scope"));
                return Step::Done;
            }
            parser.insertion_mode = InsertionMode::AfterAfterBody;
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::End && tag.name == "html" => {
            // Process the token as if it were an end tag body token, then
            // switch to "after after body". Since `</body>` just switches
            // mode (above), we replicate that here.
            if !helpers::has_element_in_scope(parser, "body") {
                parser
                    .errors
                    .push(ParseError::Generic("end tag html without body in scope"));
                return Step::Done;
            }
            parser.insertion_mode = InsertionMode::AfterAfterBody;
            Step::Done
        }
        Token::EOF => Step::Done,
        _ => {
            // Parse error; switch to "in body", reprocess.
            parser
                .errors
                .push(ParseError::Generic("unexpected token after body"));
            parser.insertion_mode = InsertionMode::InBody;
            Step::Reprocess
        }
    }
}

// ── After after body insertion mode (§13.2.6.4.20) ────────────

fn handle_after_after_body(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
    match token {
        Token::Comment(data) => {
            // Insert a comment at the Document.
            helpers::insert_comment_at(&parser.document, data, &parser.document);
            Step::Done
        }
        Token::Doctype(_) => {
            // Process using the rules for "in body".
            handle_in_body(parser, token)
        }
        Token::Character(c) if is_whitespace(*c) => {
            // Process using the rules for "in body".
            handle_in_body(parser, token)
        }
        Token::EOF => Step::Done,
        _ => {
            // Parse error; switch to "in body", reprocess.
            parser
                .errors
                .push(ParseError::Generic("unexpected token after after body"));
            parser.insertion_mode = InsertionMode::InBody;
            Step::Reprocess
        }
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
