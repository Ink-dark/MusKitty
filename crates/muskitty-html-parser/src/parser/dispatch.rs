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
        InsertionMode::InTable => handle_in_table(parser, token, tokenizer),
        InsertionMode::InTableText => handle_in_table_text(parser, token),
        InsertionMode::InCaption => handle_in_caption(parser, token),
        InsertionMode::InColumnGroup => handle_in_column_group(parser, token, tokenizer),
        InsertionMode::InTableBody => handle_in_table_body(parser, token),
        InsertionMode::InRow => handle_in_row(parser, token),
        InsertionMode::InCell => handle_in_cell(parser, token),
        InsertionMode::InHeadNoscript => handle_in_head_noscript(parser, token, tokenizer),
        InsertionMode::InSelect => handle_in_select(parser, token),
        InsertionMode::InSelectInTable => handle_in_select_in_table(parser, token),
        InsertionMode::InTemplate => handle_in_template(parser, token, tokenizer),
        InsertionMode::InFrameset => handle_in_frameset(parser, token),
        InsertionMode::AfterFrameset => handle_after_frameset(parser, token),
        InsertionMode::AfterAfterFrameset => handle_after_after_frameset(parser, token),
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
        // template (§13.2.6.4.5): add marker to active formatting,
        // insert frame element, push to template insertion mode stack,
        // switch to InTemplate.
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "template" => {
            helpers::add_formatting_marker(parser);
            helpers::insert_element(parser, tag);
            parser.frameset_ok = false;
            parser
                .template_insertion_modes
                .push(InsertionMode::InTemplate);
            parser.insertion_mode = InsertionMode::InTemplate;
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::End && tag.name == "template" => {
            if !helpers::has_element_in_stack(parser, "template") {
                parser.errors.push(ParseError::Generic(
                    "end template without template in stack",
                ));
                return Step::Done;
            }
            // Generate implied end tags.
            helpers::generate_implied_end_tags(parser, None);
            // Pop until a template element is popped.
            while let Some(top) = parser.open_elements.pop() {
                let is_template = top
                    .borrow()
                    .kind
                    .as_element()
                    .map(|e| e.local_name == "template")
                    .unwrap_or(false);
                if is_template {
                    break;
                }
            }
            helpers::clear_active_formatting_to_last_marker(parser);
            // Pop the template insertion mode stack.
            parser.template_insertion_modes.pop();
            // Reset insertion mode.
            reset_insertion_mode(parser);
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

    // table (§13.2.6.4.7): close <p> if in button scope, insert <table>,
    // switch to InTable, set frameset_ok=false.
    if name == "table" {
        if helpers::has_element_in_button_scope(parser, "p") {
            helpers::close_p_element(parser);
        }
        helpers::insert_element(parser, tag);
        parser.insertion_mode = InsertionMode::InTable;
        parser.frameset_ok = false;
        return Step::Done;
    }

    // select (§13.2.6.4.7): reconstruct, insert, switch to InSelect,
    // frameset_ok=false.
    if name == "select" {
        helpers::reconstruct_active_formatting_elements(parser);
        helpers::insert_element(parser, tag);
        parser.insertion_mode = InsertionMode::InSelect;
        parser.frameset_ok = false;
        return Step::Done;
    }

    // optgroup/option (§13.2.6.4.7): if current node is an option, pop it
    // (implied end tag); then insert the element.
    if matches!(name, "optgroup" | "option") {
        if let Some(top) = parser.open_elements.last() {
            let is_option = top
                .borrow()
                .kind
                .as_element()
                .map(|e| e.local_name == "option")
                .unwrap_or(false);
            if is_option && name == "option" {
                parser.open_elements.pop();
            }
        }
        if name == "optgroup" {
            if let Some(top) = parser.open_elements.last() {
                let is_optgroup = top
                    .borrow()
                    .kind
                    .as_element()
                    .map(|e| e.local_name == "optgroup")
                    .unwrap_or(false);
                if is_optgroup {
                    parser.open_elements.pop();
                }
            }
        }
        helpers::insert_element(parser, tag);
        return Step::Done;
    }

    // frameset (§13.2.6.4.7): if frameset_ok is false OR current node is
    // not html/body, parse error and ignore. Otherwise, replace the body
    // with a frameset and switch to InFrameset.
    if name == "frameset" {
        if !parser.frameset_ok {
            parser
                .errors
                .push(ParseError::Generic("frameset after non-frameset-ok token"));
            return Step::Done;
        }
        // Check that current node is html or body (simplified: if the
        // second element on the stack is body).
        if parser.open_elements.len() < 2 {
            parser
                .errors
                .push(ParseError::Generic("frameset without body"));
            return Step::Done;
        }
        let second_is_body = parser
            .open_elements
            .get(1)
            .and_then(|n| n.borrow().kind.as_element().map(|e| e.local_name == "body"))
            .unwrap_or(false);
        if !second_is_body {
            parser
                .errors
                .push(ParseError::Generic("frameset with non-body current node"));
            return Step::Done;
        }
        // Remove the body element from the stack and from its parent.
        let body = parser.open_elements.remove(1);
        if let Some(parent) = parser.open_elements.first() {
            // Detach body from its parent (html).
            let body_ptr = Rc::as_ptr(&body);
            parent
                .borrow_mut()
                .children
                .retain(|c| Rc::as_ptr(c) != body_ptr);
        }
        // Insert the frameset element and switch to InFrameset.
        helpers::insert_element(parser, tag);
        parser.insertion_mode = InsertionMode::InFrameset;
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

// ── Table insertion modes (§13.2.6.4.9–§13.2.6.4.15) ──────────

/// Pop elements from the open elements stack until a table-context
/// element is current. The table-context elements are: `table`,
/// `tbody`, `tfoot`, `thead`, `tr`, `caption`, `colgroup`, `html`,
/// `template` (§13.2.6.4.9 "clear the stack back to a table context").
fn clear_stack_to_table_context(parser: &mut HtmlTreeConstructor) {
    const TABLE_CONTEXT: &[&str] = &[
        "table", "tbody", "tfoot", "thead", "tr", "caption", "colgroup", "html", "template",
    ];
    while let Some(top) = parser.open_elements.last() {
        let is_ctx = top
            .borrow()
            .kind
            .as_element()
            .map(|e| TABLE_CONTEXT.contains(&e.local_name.as_str()))
            .unwrap_or(false);
        if is_ctx {
            break;
        }
        parser.open_elements.pop();
    }
}

/// Pop elements from the open elements stack until a table row context
/// element is current (§13.2.6.4.13 "clear the stack back to a table
/// body context"). Row-context elements: `tbody`, `tfoot`, `thead`,
/// `html`, `template`.
fn clear_stack_to_table_body_context(parser: &mut HtmlTreeConstructor) {
    const BODY_CONTEXT: &[&str] = &["tbody", "tfoot", "thead", "html", "template"];
    while let Some(top) = parser.open_elements.last() {
        let is_ctx = top
            .borrow()
            .kind
            .as_element()
            .map(|e| BODY_CONTEXT.contains(&e.local_name.as_str()))
            .unwrap_or(false);
        if is_ctx {
            break;
        }
        parser.open_elements.pop();
    }
}

/// Pop elements from the open elements stack until a table row element
/// is current (§13.2.6.4.14 "clear the stack back to a table row
/// context"). Row elements: `tr`, `html`, `template`.
fn clear_stack_to_row_context(parser: &mut HtmlTreeConstructor) {
    const ROW_CONTEXT: &[&str] = &["tr", "html", "template"];
    while let Some(top) = parser.open_elements.last() {
        let is_ctx = top
            .borrow()
            .kind
            .as_element()
            .map(|e| ROW_CONTEXT.contains(&e.local_name.as_str()))
            .unwrap_or(false);
        if is_ctx {
            break;
        }
        parser.open_elements.pop();
    }
}

// ── InTable insertion mode (§13.2.6.4.9) ────────────────────────

fn handle_in_table(
    parser: &mut HtmlTreeConstructor,
    token: &Token,
    tokenizer: &mut dyn Tokenizer,
) -> Step {
    match token {
        Token::Character(c) if is_whitespace(*c) => {
            // Per §13.2.6.4.9, whitespace in InTable goes to InTableText.
            parser.pending_table_text.push(*c);
            parser.insertion_mode = InsertionMode::InTableText;
            parser.original_insertion_mode = Some(InsertionMode::InTable);
            Step::Done
        }
        Token::Comment(data) => {
            helpers::insert_comment(parser, data);
            Step::Done
        }
        Token::Doctype(_) => {
            parser
                .errors
                .push(ParseError::Generic("unexpected DOCTYPE in table"));
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::Start => {
            let name = tag.name.as_str();
            match name {
                "caption" => {
                    clear_stack_to_table_context(parser);
                    helpers::add_formatting_marker(parser);
                    helpers::insert_element(parser, tag);
                    parser.insertion_mode = InsertionMode::InCaption;
                    Step::Done
                }
                "colgroup" => {
                    clear_stack_to_table_context(parser);
                    helpers::insert_element(parser, tag);
                    parser.insertion_mode = InsertionMode::InColumnGroup;
                    Step::Done
                }
                "col" => {
                    clear_stack_to_table_context(parser);
                    create_and_push(parser, "colgroup");
                    parser.insertion_mode = InsertionMode::InColumnGroup;
                    Step::Reprocess
                }
                "tbody" | "tfoot" | "thead" => {
                    clear_stack_to_table_context(parser);
                    helpers::insert_element(parser, tag);
                    parser.insertion_mode = InsertionMode::InTableBody;
                    Step::Done
                }
                "td" | "th" | "tr" => {
                    clear_stack_to_table_context(parser);
                    create_and_push(parser, "tbody");
                    parser.insertion_mode = InsertionMode::InTableBody;
                    Step::Reprocess
                }
                "style" | "script" | "template" => {
                    // Process using the rules for "in head" (§13.2.6.4.9).
                    handle_in_head(parser, token, tokenizer)
                }
                _ => foster_parent_in_body(parser, token),
            }
        }
        Token::Tag(tag) if tag.kind == TagKind::End => match tag.name.as_str() {
            "table" => {
                // Pop until a table element is popped (§13.2.6.4.9).
                while let Some(top) = parser.open_elements.pop() {
                    let is_table = top
                        .borrow()
                        .kind
                        .as_element()
                        .map(|e| e.local_name == "table")
                        .unwrap_or(false);
                    if is_table {
                        break;
                    }
                }
                parser.insertion_mode = InsertionMode::InBody;
                Step::Done
            }
            "body" | "caption" | "col" | "colgroup" | "html" | "tbody" | "td" | "tfoot" | "th"
            | "thead" | "tr" => {
                parser
                    .errors
                    .push(ParseError::Generic("unexpected end tag in table"));
                Step::Done
            }
            "template" => handle_in_head(parser, token, tokenizer),
            _ => foster_parent_in_body(parser, token),
        },
        Token::EOF => {
            // Reprocess in InBody for EOF handling (template/fragment checks).
            parser.insertion_mode = InsertionMode::InBody;
            Step::Reprocess
        }
        _ => foster_parent_in_body(parser, token),
    }
}

/// Foster-parent a token by enabling foster parenting, processing it as
/// InBody, then disabling foster parenting (§13.2.6.4.9 "anything else").
fn foster_parent_in_body(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
    parser
        .errors
        .push(ParseError::Generic("foster parenting in table"));
    parser.foster_parenting = true;
    let step = handle_in_body(parser, token);
    parser.foster_parenting = false;
    step
}

// ── InTableText insertion mode (§13.2.6.4.10) ───────────────────

fn handle_in_table_text(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
    match token {
        Token::Character('\0') => {
            parser
                .errors
                .push(ParseError::Generic("unexpected null in table text"));
            Step::Done
        }
        Token::Character(c) => {
            parser.pending_table_text.push(*c);
            Step::Done
        }
        _ => {
            // Process the pending table character tokens: if any is
            // non-whitespace, foster-parent the whole run as InBody
            // characters; otherwise discard them (§13.2.6.4.10).
            let pending = std::mem::take(&mut parser.pending_table_text);
            let has_non_ws = pending.chars().any(|c| !is_whitespace(c));
            if has_non_ws {
                parser.foster_parenting = true;
                for c in pending.chars() {
                    handle_in_body(parser, &Token::Character(c));
                }
                parser.foster_parenting = false;
            }
            // Restore original insertion mode (InTable) and reprocess.
            parser.insertion_mode = parser
                .original_insertion_mode
                .take()
                .unwrap_or(InsertionMode::InTable);
            Step::Reprocess
        }
    }
}

// ── InCaption insertion mode (§13.2.6.4.11) ─────────────────────

fn handle_in_caption(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
    match token {
        Token::Tag(tag) if tag.kind == TagKind::End && tag.name == "caption" => {
            if !helpers::has_element_in_scope(parser, "caption") {
                parser
                    .errors
                    .push(ParseError::Generic("end caption without caption in scope"));
                return Step::Done;
            }
            helpers::generate_implied_end_tags(parser, None);
            // Pop until a caption element is popped.
            while let Some(top) = parser.open_elements.pop() {
                let is_caption = top
                    .borrow()
                    .kind
                    .as_element()
                    .map(|e| e.local_name == "caption")
                    .unwrap_or(false);
                if is_caption {
                    break;
                }
            }
            helpers::clear_active_formatting_to_last_marker(parser);
            parser.insertion_mode = InsertionMode::InTable;
            Step::Done
        }
        Token::Tag(tag)
            if tag.kind == TagKind::Start
                && matches!(
                    tag.name.as_str(),
                    "caption"
                        | "col"
                        | "colgroup"
                        | "tbody"
                        | "td"
                        | "tfoot"
                        | "th"
                        | "thead"
                        | "tr"
                ) =>
        {
            // Parse error; act as if </caption> was seen, then reprocess.
            parser
                .errors
                .push(ParseError::Generic("unexpected table start tag in caption"));
            close_caption(parser);
            Step::Reprocess
        }
        Token::Tag(tag)
            if tag.kind == TagKind::End
                && matches!(
                    tag.name.as_str(),
                    "table" | "tbody" | "tfoot" | "thead" | "tr" | "td" | "th"
                ) =>
        {
            parser
                .errors
                .push(ParseError::Generic("unexpected table end tag in caption"));
            close_caption(parser);
            Step::Reprocess
        }
        _ => {
            // Anything else: process using the rules for InBody.
            handle_in_body(parser, token)
        }
    }
}

/// Close the current caption (used by InCaption's "act as </caption>").
fn close_caption(parser: &mut HtmlTreeConstructor) {
    if helpers::has_element_in_scope(parser, "caption") {
        helpers::generate_implied_end_tags(parser, None);
        while let Some(top) = parser.open_elements.pop() {
            let is_caption = top
                .borrow()
                .kind
                .as_element()
                .map(|e| e.local_name == "caption")
                .unwrap_or(false);
            if is_caption {
                break;
            }
        }
        helpers::clear_active_formatting_to_last_marker(parser);
        parser.insertion_mode = InsertionMode::InTable;
    }
}

// ── InColumnGroup insertion mode (§13.2.6.4.12) ─────────────────

fn handle_in_column_group(
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
                .push(ParseError::Generic("unexpected DOCTYPE in colgroup"));
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::Start => match tag.name.as_str() {
            "html" => handle_in_body(parser, token),
            "col" => {
                helpers::insert_element(parser, tag);
                parser.open_elements.pop();
                Step::Done
            }
            "template" => handle_in_head(parser, token, tokenizer),
            _ => close_colgroup_and_reprocess(parser),
        },
        Token::Tag(tag) if tag.kind == TagKind::End => match tag.name.as_str() {
            "colgroup" => {
                if !helpers::has_element_in_scope(parser, "colgroup") {
                    parser.errors.push(ParseError::Generic(
                        "end colgroup without colgroup in scope",
                    ));
                    return Step::Done;
                }
                parser.open_elements.pop();
                parser.insertion_mode = InsertionMode::InTable;
                Step::Done
            }
            "col" => {
                parser.errors.push(ParseError::Generic("end col; ignored"));
                Step::Done
            }
            "template" => handle_in_head(parser, token, tokenizer),
            _ => close_colgroup_and_reprocess(parser),
        },
        Token::EOF => handle_in_head(parser, token, tokenizer),
        _ => close_colgroup_and_reprocess(parser),
    }
}

/// Close the colgroup (if present) and reprocess in InTable.
fn close_colgroup_and_reprocess(parser: &mut HtmlTreeConstructor) -> Step {
    if helpers::has_element_in_scope(parser, "colgroup") {
        parser.open_elements.pop();
        parser.insertion_mode = InsertionMode::InTable;
        Step::Reprocess
    } else {
        // No colgroup in scope: parse error, ignore the token.
        parser.errors.push(ParseError::Generic(
            "unexpected token; no colgroup in scope",
        ));
        Step::Done
    }
}

// ── InTableBody insertion mode (§13.2.6.4.13) ───────────────────

fn handle_in_table_body(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
    match token {
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "tr" => {
            clear_stack_to_table_body_context(parser);
            helpers::insert_element(parser, tag);
            parser.insertion_mode = InsertionMode::InRow;
            Step::Done
        }
        Token::Tag(tag)
            if tag.kind == TagKind::Start && matches!(tag.name.as_str(), "td" | "th") =>
        {
            clear_stack_to_table_body_context(parser);
            create_and_push(parser, "tr");
            parser.insertion_mode = InsertionMode::InRow;
            Step::Reprocess
        }
        Token::Tag(tag)
            if tag.kind == TagKind::End
                && matches!(tag.name.as_str(), "tbody" | "tfoot" | "thead") =>
        {
            let name = tag.name.clone();
            if !helpers::has_element_in_table_scope(parser, &name) {
                parser.errors.push(ParseError::Generic(
                    "end tag without element in table scope",
                ));
                return Step::Done;
            }
            clear_stack_to_table_body_context(parser);
            parser.open_elements.pop();
            parser.insertion_mode = InsertionMode::InTable;
            Step::Done
        }
        Token::Tag(tag)
            if (tag.kind == TagKind::Start
                && matches!(
                    tag.name.as_str(),
                    "caption" | "col" | "colgroup" | "tbody" | "tfoot" | "thead"
                ))
                || (tag.kind == TagKind::End
                    && matches!(tag.name.as_str(), "table" | "tbody" | "tfoot" | "thead")) =>
        {
            // Act as if </tbody> (or </tfoot>/</thead>) was seen, then
            // reprocess. If no tbody/tfoot/thead is in table scope, ignore.
            if !helpers::has_element_in_table_scope(parser, "tbody")
                && !helpers::has_element_in_table_scope(parser, "tfoot")
                && !helpers::has_element_in_table_scope(parser, "thead")
            {
                parser.errors.push(ParseError::Generic(
                    "unexpected token; no table body in scope",
                ));
                return Step::Done;
            }
            clear_stack_to_table_body_context(parser);
            parser.open_elements.pop();
            parser.insertion_mode = InsertionMode::InTable;
            Step::Reprocess
        }
        _ => {
            // Anything else: process using the rules for InTable.
            parser.insertion_mode = InsertionMode::InTable;
            Step::Reprocess
        }
    }
}

// ── InRow insertion mode (§13.2.6.4.14) ─────────────────────────

fn handle_in_row(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
    match token {
        Token::Tag(tag)
            if tag.kind == TagKind::Start && matches!(tag.name.as_str(), "td" | "th") =>
        {
            clear_stack_to_row_context(parser);
            helpers::insert_element(parser, tag);
            parser.insertion_mode = InsertionMode::InCell;
            helpers::add_formatting_marker(parser);
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::End && tag.name == "tr" => {
            if !helpers::has_element_in_table_scope(parser, "tr") {
                parser
                    .errors
                    .push(ParseError::Generic("end tr without tr in table scope"));
                return Step::Done;
            }
            clear_stack_to_row_context(parser);
            parser.open_elements.pop();
            parser.insertion_mode = InsertionMode::InTableBody;
            Step::Done
        }
        Token::Tag(tag)
            if (tag.kind == TagKind::Start
                && matches!(
                    tag.name.as_str(),
                    "caption" | "col" | "colgroup" | "tbody" | "tfoot" | "thead" | "tr"
                ))
                || (tag.kind == TagKind::End
                    && matches!(tag.name.as_str(), "tbody" | "tfoot" | "thead")) =>
        {
            // Act as if </tr> was seen, then reprocess.
            if !helpers::has_element_in_table_scope(parser, "tr") {
                parser.errors.push(ParseError::Generic(
                    "unexpected token; no tr in table scope",
                ));
                return Step::Done;
            }
            clear_stack_to_row_context(parser);
            parser.open_elements.pop();
            parser.insertion_mode = InsertionMode::InTableBody;
            Step::Reprocess
        }
        _ => {
            // Anything else: process using the rules for InTable.
            parser.insertion_mode = InsertionMode::InTable;
            Step::Reprocess
        }
    }
}

// ── InCell insertion mode (§13.2.6.4.15) ────────────────────────

fn handle_in_cell(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
    match token {
        Token::Tag(tag) if tag.kind == TagKind::End && matches!(tag.name.as_str(), "td" | "th") => {
            let name = tag.name.clone();
            if !helpers::has_element_in_table_scope(parser, &name) {
                parser
                    .errors
                    .push(ParseError::Generic("end cell without cell in table scope"));
                return Step::Done;
            }
            helpers::generate_implied_end_tags(parser, None);
            // Pop until the target cell is popped.
            while let Some(top) = parser.open_elements.pop() {
                let is_target = top
                    .borrow()
                    .kind
                    .as_element()
                    .map(|e| e.local_name == name)
                    .unwrap_or(false);
                if is_target {
                    break;
                }
            }
            helpers::clear_active_formatting_to_last_marker(parser);
            parser.insertion_mode = InsertionMode::InRow;
            Step::Done
        }
        Token::Tag(tag)
            if (tag.kind == TagKind::Start
                && matches!(
                    tag.name.as_str(),
                    "caption"
                        | "col"
                        | "colgroup"
                        | "tbody"
                        | "td"
                        | "tfoot"
                        | "th"
                        | "thead"
                        | "tr"
                ))
                || (tag.kind == TagKind::End
                    && matches!(tag.name.as_str(), "tbody" | "tfoot" | "thead" | "tr")) =>
        {
            // If a td or th is in table scope, close the cell and reprocess.
            if helpers::has_element_in_table_scope(parser, "td")
                || helpers::has_element_in_table_scope(parser, "th")
            {
                let target = if helpers::has_element_in_table_scope(parser, "td") {
                    "td"
                } else {
                    "th"
                };
                helpers::generate_implied_end_tags(parser, None);
                while let Some(top) = parser.open_elements.pop() {
                    let is_target = top
                        .borrow()
                        .kind
                        .as_element()
                        .map(|e| e.local_name == target)
                        .unwrap_or(false);
                    if is_target {
                        break;
                    }
                }
                helpers::clear_active_formatting_to_last_marker(parser);
                parser.insertion_mode = InsertionMode::InRow;
                Step::Reprocess
            } else {
                parser.errors.push(ParseError::Generic(
                    "unexpected token; no cell in table scope",
                ));
                Step::Done
            }
        }
        _ => {
            // Anything else: process using the rules for InBody.
            handle_in_body(parser, token)
        }
    }
}

// ── Remaining insertion modes (§13.2.6.4.6, §13.2.6.4.16–§13.2.6.4.23) ──

// ── InHeadNoscript insertion mode (§13.2.6.4.6) ────────────────

fn handle_in_head_noscript(
    parser: &mut HtmlTreeConstructor,
    token: &Token,
    tokenizer: &mut dyn Tokenizer,
) -> Step {
    match token {
        Token::Doctype(_) => {
            parser
                .errors
                .push(ParseError::Generic("unexpected DOCTYPE in noscript"));
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "html" => {
            handle_in_body(parser, token)
        }
        Token::Tag(tag) if tag.kind == TagKind::End && tag.name == "noscript" => {
            // Pop the noscript element and switch to InHead.
            parser.open_elements.pop();
            parser.insertion_mode = InsertionMode::InHead;
            Step::Done
        }
        Token::Character(c) if is_whitespace(*c) => handle_in_head(parser, token, tokenizer),
        Token::Comment(data) => handle_in_head(parser, &Token::Comment(data.clone()), tokenizer),
        Token::Tag(tag)
            if tag.kind == TagKind::Start
                && matches!(
                    tag.name.as_str(),
                    "basefont" | "bgsound" | "link" | "meta" | "noframes" | "style"
                ) =>
        {
            handle_in_head(parser, token, tokenizer)
        }
        Token::Tag(tag)
            if tag.kind == TagKind::Start && matches!(tag.name.as_str(), "head" | "noscript") =>
        {
            parser
                .errors
                .push(ParseError::Generic("unexpected head/noscript in noscript"));
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::End && tag.name == "br" => {
            // </br> is treated as a start tag (anything else).
            handle_anything_else_in_head_noscript(parser, token)
        }
        Token::EOF => {
            parser
                .errors
                .push(ParseError::Generic("unexpected EOF in noscript"));
            // Pop noscript, reprocess in InHead.
            parser.open_elements.pop();
            parser.insertion_mode = InsertionMode::InHead;
            Step::Reprocess
        }
        _ => handle_anything_else_in_head_noscript(parser, token),
    }
}

fn handle_anything_else_in_head_noscript(parser: &mut HtmlTreeConstructor, _token: &Token) -> Step {
    parser
        .errors
        .push(ParseError::Generic("unexpected token in noscript"));
    parser.open_elements.pop();
    parser.insertion_mode = InsertionMode::InHead;
    Step::Reprocess
}

// ── InSelect insertion mode (§13.2.6.4.16) ──────────────────────

fn handle_in_select(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
    match token {
        Token::Character('\0') => {
            parser
                .errors
                .push(ParseError::Generic("unexpected null in select"));
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
        Token::Doctype(_) => {
            parser
                .errors
                .push(ParseError::Generic("unexpected DOCTYPE in select"));
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "html" => {
            handle_in_body(parser, token)
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "option" => {
            // If current node is an option, pop it (§13.2.6.4.16).
            if let Some(top) = parser.open_elements.last() {
                let is_option = top
                    .borrow()
                    .kind
                    .as_element()
                    .map(|e| e.local_name == "option")
                    .unwrap_or(false);
                if is_option {
                    parser.open_elements.pop();
                }
            }
            helpers::insert_element(parser, tag);
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "optgroup" => {
            // If current node is an option, pop it.
            if let Some(top) = parser.open_elements.last() {
                let is_option = top
                    .borrow()
                    .kind
                    .as_element()
                    .map(|e| e.local_name == "option")
                    .unwrap_or(false);
                if is_option {
                    parser.open_elements.pop();
                }
            }
            // If current node is an optgroup, pop it.
            if let Some(top) = parser.open_elements.last() {
                let is_optgroup = top
                    .borrow()
                    .kind
                    .as_element()
                    .map(|e| e.local_name == "optgroup")
                    .unwrap_or(false);
                if is_optgroup {
                    parser.open_elements.pop();
                }
            }
            helpers::insert_element(parser, tag);
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "select" => {
            // Parse error; pop until a select element is popped (§13.2.6.4.16).
            parser
                .errors
                .push(ParseError::Generic("unexpected <select> in select"));
            while let Some(top) = parser.open_elements.pop() {
                let is_select = top
                    .borrow()
                    .kind
                    .as_element()
                    .map(|e| e.local_name == "select")
                    .unwrap_or(false);
                if is_select {
                    break;
                }
            }
            // Reset insertion mode per §13.2.6.4.2.
            reset_insertion_mode(parser);
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "input" => {
            // Parse error; if the input is not hidden, pop until select.
            parser
                .errors
                .push(ParseError::Generic("unexpected <input> in select"));
            let is_hidden = tag
                .attrs
                .iter()
                .any(|(k, v)| k.eq_ignore_ascii_case("type") && v.eq_ignore_ascii_case("hidden"));
            if is_hidden {
                helpers::insert_element(parser, tag);
                parser.open_elements.pop();
                Step::Done
            } else {
                while let Some(top) = parser.open_elements.pop() {
                    let is_select = top
                        .borrow()
                        .kind
                        .as_element()
                        .map(|e| e.local_name == "select")
                        .unwrap_or(false);
                    if is_select {
                        break;
                    }
                }
                reset_insertion_mode(parser);
                Step::Reprocess
            }
        }
        Token::Tag(tag)
            if tag.kind == TagKind::Start && matches!(tag.name.as_str(), "keygen" | "textarea") =>
        {
            parser
                .errors
                .push(ParseError::Generic("unexpected keygen/textarea in select"));
            while let Some(top) = parser.open_elements.pop() {
                let is_select = top
                    .borrow()
                    .kind
                    .as_element()
                    .map(|e| e.local_name == "select")
                    .unwrap_or(false);
                if is_select {
                    break;
                }
            }
            reset_insertion_mode(parser);
            Step::Reprocess
        }
        Token::Tag(tag)
            if tag.kind == TagKind::Start && matches!(tag.name.as_str(), "script" | "template") =>
        {
            handle_in_head(parser, token, &mut NullTokenizer)
        }
        Token::Tag(tag) if tag.kind == TagKind::End && tag.name == "select" => {
            if !helpers::has_element_in_select_scope(parser, "select") {
                parser
                    .errors
                    .push(ParseError::Generic("end select without select in scope"));
                return Step::Done;
            }
            while let Some(top) = parser.open_elements.pop() {
                let is_select = top
                    .borrow()
                    .kind
                    .as_element()
                    .map(|e| e.local_name == "select")
                    .unwrap_or(false);
                if is_select {
                    break;
                }
            }
            reset_insertion_mode(parser);
            Step::Done
        }
        Token::Tag(tag)
            if tag.kind == TagKind::End && matches!(tag.name.as_str(), "optgroup" | "option") =>
        {
            handle_in_select_end_tag(parser, &tag.name);
            Step::Done
        }
        Token::EOF => {
            // Pop until select is popped, then reprocess in the reset mode.
            if !helpers::has_element_in_select_scope(parser, "select") {
                parser
                    .errors
                    .push(ParseError::Generic("unexpected EOF; no select in scope"));
                return Step::Done;
            }
            while let Some(top) = parser.open_elements.pop() {
                let is_select = top
                    .borrow()
                    .kind
                    .as_element()
                    .map(|e| e.local_name == "select")
                    .unwrap_or(false);
                if is_select {
                    break;
                }
            }
            reset_insertion_mode(parser);
            Step::Reprocess
        }
        _ => {
            parser
                .errors
                .push(ParseError::Generic("unexpected token in select"));
            Step::Done
        }
    }
}

/// Handle </optgroup> and </option> in InSelect (§13.2.6.4.16).
fn handle_in_select_end_tag(parser: &mut HtmlTreeConstructor, name: &str) {
    if name == "optgroup" {
        // Let current node be the current node. If current node is an
        // option and its parent is an optgroup, pop the option. Then if
        // current node is an optgroup, pop it.
        if let Some(top) = parser.open_elements.last() {
            let is_option = top
                .borrow()
                .kind
                .as_element()
                .map(|e| e.local_name == "option")
                .unwrap_or(false);
            if is_option && parser.open_elements.len() >= 2 {
                let parent = &parser.open_elements[parser.open_elements.len() - 2];
                let parent_is_optgroup = parent
                    .borrow()
                    .kind
                    .as_element()
                    .map(|e| e.local_name == "optgroup")
                    .unwrap_or(false);
                if parent_is_optgroup {
                    parser.open_elements.pop();
                }
            }
        }
        if let Some(top) = parser.open_elements.last() {
            let is_optgroup = top
                .borrow()
                .kind
                .as_element()
                .map(|e| e.local_name == "optgroup")
                .unwrap_or(false);
            if is_optgroup {
                parser.open_elements.pop();
            }
        }
    } else {
        // </option>: if current node is an option, pop it.
        if let Some(top) = parser.open_elements.last() {
            let is_option = top
                .borrow()
                .kind
                .as_element()
                .map(|e| e.local_name == "option")
                .unwrap_or(false);
            if is_option {
                parser.open_elements.pop();
            }
        }
    }
}

// ── InSelectInTable insertion mode (§13.2.6.4.18) ───────────────

fn handle_in_select_in_table(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
    match token {
        Token::Tag(tag)
            if tag.kind == TagKind::Start
                && matches!(
                    tag.name.as_str(),
                    "caption" | "table" | "tbody" | "tfoot" | "thead" | "tr" | "td" | "th"
                ) =>
        {
            parser.errors.push(ParseError::Generic(
                "unexpected table tag in select-in-table",
            ));
            // Pop until a select element is popped, then reprocess.
            while let Some(top) = parser.open_elements.pop() {
                let is_select = top
                    .borrow()
                    .kind
                    .as_element()
                    .map(|e| e.local_name == "select")
                    .unwrap_or(false);
                if is_select {
                    break;
                }
            }
            reset_insertion_mode(parser);
            Step::Reprocess
        }
        Token::Tag(tag)
            if tag.kind == TagKind::End
                && matches!(
                    tag.name.as_str(),
                    "caption" | "table" | "tbody" | "tfoot" | "thead" | "tr" | "td" | "th"
                ) =>
        {
            parser.errors.push(ParseError::Generic(
                "unexpected table end tag in select-in-table",
            ));
            if helpers::has_element_in_table_scope(parser, &tag.name) {
                while let Some(top) = parser.open_elements.pop() {
                    let is_select = top
                        .borrow()
                        .kind
                        .as_element()
                        .map(|e| e.local_name == "select")
                        .unwrap_or(false);
                    if is_select {
                        break;
                    }
                }
                reset_insertion_mode(parser);
                Step::Reprocess
            } else {
                Step::Done
            }
        }
        _ => handle_in_select(parser, token),
    }
}

// ── InTemplate insertion mode (§13.2.6.4.19) ────────────────────

fn handle_in_template(
    parser: &mut HtmlTreeConstructor,
    token: &Token,
    tokenizer: &mut dyn Tokenizer,
) -> Step {
    match token {
        Token::Character(c) if is_whitespace(*c) => {
            helpers::insert_character(parser, *c);
            Step::Done
        }
        Token::Character(_) => {
            helpers::reconstruct_active_formatting_elements(parser);
            handle_in_body(parser, token)
        }
        Token::Comment(data) => {
            helpers::insert_comment(parser, data);
            Step::Done
        }
        Token::Doctype(_) => {
            parser
                .errors
                .push(ParseError::Generic("unexpected DOCTYPE in template"));
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "html" => {
            handle_in_body(parser, token)
        }
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
            handle_in_head(parser, token, tokenizer)
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "caption" => {
            pop_until_template_content(parser);
            helpers::add_formatting_marker(parser);
            helpers::insert_element(parser, tag);
            parser.insertion_mode = InsertionMode::InCaption;
            Step::Done
        }
        Token::Tag(tag)
            if tag.kind == TagKind::Start && matches!(tag.name.as_str(), "colgroup" | "col") =>
        {
            pop_until_template_content(parser);
            if tag.name == "col" {
                create_and_push(parser, "colgroup");
                parser.insertion_mode = InsertionMode::InColumnGroup;
                Step::Reprocess
            } else {
                helpers::insert_element(parser, tag);
                parser.insertion_mode = InsertionMode::InColumnGroup;
                Step::Done
            }
        }
        Token::Tag(tag)
            if tag.kind == TagKind::Start
                && matches!(
                    tag.name.as_str(),
                    "tbody" | "tfoot" | "thead" | "tr" | "td" | "th"
                ) =>
        {
            pop_until_template_content(parser);
            if matches!(tag.name.as_str(), "tbody" | "tfoot" | "thead") {
                helpers::insert_element(parser, tag);
                parser.insertion_mode = InsertionMode::InTableBody;
                Step::Done
            } else if tag.name == "tr" {
                helpers::insert_element(parser, tag);
                parser.insertion_mode = InsertionMode::InRow;
                Step::Done
            } else {
                create_and_push(parser, "tbody");
                parser.insertion_mode = InsertionMode::InTableBody;
                Step::Reprocess
            }
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "table" => {
            pop_until_template_content(parser);
            helpers::insert_element(parser, tag);
            parser.insertion_mode = InsertionMode::InTable;
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::End && tag.name == "template" => {
            handle_in_head(parser, token, tokenizer)
        }
        Token::EOF => {
            if !template_in_stack(parser) {
                parser.insertion_mode = parser
                    .original_insertion_mode
                    .take()
                    .unwrap_or(InsertionMode::InBody);
                Step::Reprocess
            } else {
                parser
                    .errors
                    .push(ParseError::Generic("unexpected EOF in template"));
                Step::Done
            }
        }
        _ => {
            // Any other start/end tag: reconstruct, process as InBody.
            helpers::reconstruct_active_formatting_elements(parser);
            parser.insertion_mode = parser
                .original_insertion_mode
                .take()
                .unwrap_or(InsertionMode::InBody);
            Step::Reprocess
        }
    }
}

/// Pop elements until the template content marker is reached (i.e., the
/// current node is a template element). Used by InTemplate.
fn pop_until_template_content(parser: &mut HtmlTreeConstructor) {
    while let Some(top) = parser.open_elements.last() {
        let is_template = top
            .borrow()
            .kind
            .as_element()
            .map(|e| e.local_name == "template")
            .unwrap_or(false);
        if is_template {
            break;
        }
        parser.open_elements.pop();
    }
}

/// Check if there is a template element on the stack of open elements.
fn template_in_stack(parser: &HtmlTreeConstructor) -> bool {
    parser.open_elements.iter().any(|n| {
        n.borrow()
            .kind
            .as_element()
            .map(|e| e.local_name == "template")
            .unwrap_or(false)
    })
}

// ── InFrameset insertion mode (§13.2.6.4.21) ────────────────────

fn handle_in_frameset(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
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
                .push(ParseError::Generic("unexpected DOCTYPE in frameset"));
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "html" => {
            handle_in_body(parser, token)
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "frameset" => {
            helpers::insert_element(parser, tag);
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::End && tag.name == "frameset" => {
            if parser.open_elements.len() == 1 {
                parser
                    .errors
                    .push(ParseError::Generic("unexpected </frameset> at root"));
                return Step::Done;
            }
            parser.open_elements.pop();
            // If current node is not a frameset, switch to AfterFrameset.
            let is_frameset = parser
                .open_elements
                .last()
                .and_then(|n| {
                    n.borrow()
                        .kind
                        .as_element()
                        .map(|e| e.local_name == "frameset")
                })
                .unwrap_or(false);
            if !is_frameset {
                parser.insertion_mode = InsertionMode::AfterFrameset;
            }
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "frame" => {
            helpers::insert_element(parser, tag);
            parser.open_elements.pop();
            parser.frameset_ok = false;
            Step::Done
        }
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
            handle_in_head(parser, token, &mut NullTokenizer)
        }
        Token::Tag(tag) if tag.kind == TagKind::End && tag.name == "template" => {
            handle_in_head(parser, token, &mut NullTokenizer)
        }
        Token::EOF => {
            if parser.open_elements.len() != 1 {
                parser
                    .errors
                    .push(ParseError::Generic("unexpected EOF in frameset"));
            }
            Step::Done
        }
        _ => {
            parser
                .errors
                .push(ParseError::Generic("unexpected token in frameset"));
            Step::Done
        }
    }
}

// ── AfterFrameset insertion mode (§13.2.6.4.22) ─────────────────

fn handle_after_frameset(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
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
                .push(ParseError::Generic("unexpected DOCTYPE after frameset"));
            Step::Done
        }
        Token::Tag(tag) if tag.kind == TagKind::Start && tag.name == "html" => {
            handle_in_body(parser, token)
        }
        Token::Tag(tag) if tag.kind == TagKind::End && tag.name == "html" => {
            parser.insertion_mode = InsertionMode::AfterAfterFrameset;
            Step::Done
        }
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
            handle_in_head(parser, token, &mut NullTokenizer)
        }
        Token::Tag(tag) if tag.kind == TagKind::End && tag.name == "template" => {
            handle_in_head(parser, token, &mut NullTokenizer)
        }
        Token::EOF => Step::Done,
        _ => {
            parser
                .errors
                .push(ParseError::Generic("unexpected token after frameset"));
            Step::Done
        }
    }
}

// ── AfterAfterFrameset insertion mode (§13.2.6.4.23) ────────────

fn handle_after_after_frameset(parser: &mut HtmlTreeConstructor, token: &Token) -> Step {
    match token {
        Token::Comment(data) => {
            // Insert at the Document.
            helpers::insert_comment_at(&parser.document, data, &parser.document);
            Step::Done
        }
        Token::Doctype(_) => {
            parser.errors.push(ParseError::Generic(
                "unexpected DOCTYPE after after frameset",
            ));
            Step::Done
        }
        Token::Character(c) if is_whitespace(*c) => handle_in_body(parser, token),
        Token::EOF => Step::Done,
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
            handle_in_head(parser, token, &mut NullTokenizer)
        }
        Token::Tag(tag) if tag.kind == TagKind::End && tag.name == "template" => {
            handle_in_head(parser, token, &mut NullTokenizer)
        }
        _ => {
            parser
                .errors
                .push(ParseError::Generic("unexpected token after after frameset"));
            Step::Done
        }
    }
}

// ── Insertion mode reset (§13.2.6.4.2) ──────────────────────────

/// Reset the insertion mode appropriately (§13.2.6.4.2).
///
/// Walks the stack of open elements from bottom to top. The first matching
/// condition sets the new insertion mode. If no condition matches, the
/// insertion mode is set to InBody.
pub fn reset_insertion_mode(parser: &mut HtmlTreeConstructor) {
    let last = parser.open_elements.len() - 1;
    for (i, node) in parser.open_elements.iter().enumerate() {
        let is_last = i == last;
        let local = node
            .borrow()
            .kind
            .as_element()
            .map(|e| e.local_name.clone());
        let local = local.as_deref().unwrap_or("");
        match local {
            "select" => {
                parser.insertion_mode = InsertionMode::InSelect;
                return;
            }
            "td" | "th" => {
                if !is_last {
                    parser.insertion_mode = InsertionMode::InCell;
                    return;
                }
            }
            "tr" => {
                parser.insertion_mode = InsertionMode::InRow;
                return;
            }
            "tbody" | "thead" | "tfoot" => {
                parser.insertion_mode = InsertionMode::InTableBody;
                return;
            }
            "caption" => {
                parser.insertion_mode = InsertionMode::InCaption;
                return;
            }
            "colgroup" => {
                parser.insertion_mode = InsertionMode::InColumnGroup;
                return;
            }
            "table" => {
                parser.insertion_mode = InsertionMode::InTable;
                return;
            }
            "template" => {
                // Use the template insertion mode stack if available.
                if let Some(&mode) = parser.template_insertion_modes.last() {
                    parser.insertion_mode = mode;
                } else {
                    parser.insertion_mode = InsertionMode::InTemplate;
                }
                return;
            }
            "head" => {
                if !is_last {
                    parser.insertion_mode = InsertionMode::InHead;
                    return;
                }
            }
            "body" => {
                parser.insertion_mode = InsertionMode::InBody;
                return;
            }
            "frameset" => {
                parser.insertion_mode = InsertionMode::InFrameset;
                return;
            }
            "html" => {
                if parser.head_element.is_none() {
                    parser.insertion_mode = InsertionMode::BeforeHead;
                } else {
                    parser.insertion_mode = InsertionMode::AfterHead;
                }
                return;
            }
            _ => {}
        }
    }
    parser.insertion_mode = InsertionMode::InBody;
}

/// A null tokenizer used as a placeholder when handlers need a tokenizer
/// reference but the current dispatch doesn't have one.
struct NullTokenizer;

impl crate::tokenizer::Tokenizer for NullTokenizer {
    fn next_token(&mut self) -> Option<Token> {
        None
    }
    fn set_state(&mut self, _state: State) {}
    fn state(&self) -> State {
        State::Data
    }
    fn reset(&mut self) {}
    fn set_appropriate_end_tag_name(&mut self, _name: Option<&str>) {}
}
