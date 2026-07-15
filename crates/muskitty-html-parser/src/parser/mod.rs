//! Tree construction stage (§13.2.6).
//!
//! Consumes the token stream produced by the tokenizer and builds the DOM
//! tree per the WHATWG HTML insertion mode state machine.
//!
//! # Architecture
//!
//! - [`HtmlTreeConstructor`] holds the parser state: open elements stack,
//!   active formatting elements list, current insertion mode, and flags.
//! - [`dispatch`] routes each token to the handler for the current insertion
//!   mode.
//! - [`helpers`] contains the "insert a node" / "create an element" helper
//!   algorithms from §13.2.6.2.
//! - [`insertion_mode`] defines the 23 insertion modes from §13.2.6.1.

mod dispatch;
mod helpers;
mod insertion_mode;

pub use insertion_mode::InsertionMode;

use std::cell::RefCell;
use std::rc::Rc;

use muskitty_dom::Node;

use crate::error::ParseError;
use crate::tokenizer::{Token, Tokenizer};

/// An entry in the list of active formatting elements (§13.2.6.2).
///
/// The list holds either a reference to an element on the open elements
/// stack, or a marker that delimits a section (pushed when entering table
/// contexts, template content, etc.).
#[derive(Clone)]
pub enum ActiveFormattingEntry {
    /// A marker entry, used to delimit sections of the list.
    Marker,
    /// An element entry, holding a reference to the formatting element.
    Element(Rc<RefCell<Node>>),
}

/// The HTML tree construction stage.
///
/// Holds the state of the insertion mode state machine (§13.2.6) and the
/// DOM tree being built. The `document` field is the output root; the
/// `open_elements` stack tracks the current open element chain.
pub struct HtmlTreeConstructor {
    /// The output Document node. Inserted elements are ultimately attached
    /// here (directly or via the `<html>` / `<head>` / `<body>` chain).
    pub document: Rc<RefCell<Node>>,
    /// The stack of open elements (§13.2.6.2). The top is the current node.
    pub open_elements: Vec<Rc<RefCell<Node>>>,
    /// The list of active formatting elements (§13.2.6.2). Used by the
    /// adoption agency algorithm; populated in Phase 3.3.
    pub active_formatting_elements: Vec<ActiveFormattingEntry>,
    /// The current insertion mode (§13.2.6.1).
    pub insertion_mode: InsertionMode,
    /// The original insertion mode, saved when entering Text mode or
    /// template content (§13.2.6.5, §13.2.6.16).
    pub original_insertion_mode: Option<InsertionMode>,
    /// The `<head>` element pointer, set in BeforeHead mode (§13.2.6.4).
    pub head_element: Option<Rc<RefCell<Node>>>,
    /// The `<form>` element pointer, updated in InBody mode (§13.2.6.4).
    pub form_element: Option<Rc<RefCell<Node>>>,
    /// Whether foster parenting is active (§13.2.6.3). Used by table
    /// insertion modes; deferred to Phase 3.4.
    pub foster_parenting: bool,
    /// The "frameset-ok" flag (§13.2.6.1). Initially true; set to false by
    /// certain tokens that prevent subsequent `<frameset>`.
    pub frameset_ok: bool,
    /// The scripting flag (§13.2.6.1). Defaults to false for non-scripting
    /// parsers; affects `<noscript>` handling and template content.
    pub scripting_flag: bool,
    /// Parse errors accumulated during tree construction (§13.2.6).
    pub errors: Vec<ParseError>,
}

impl HtmlTreeConstructor {
    /// Create a new tree constructor that will build into `document`.
    ///
    /// Per §13.2.6.1, the initial insertion mode is `Initial`, the
    /// frameset-ok flag is true, and the scripting flag defaults to false.
    pub fn new(document: Rc<RefCell<Node>>) -> Self {
        Self {
            document,
            open_elements: Vec::new(),
            active_formatting_elements: Vec::new(),
            insertion_mode: InsertionMode::Initial,
            original_insertion_mode: None,
            head_element: None,
            form_element: None,
            foster_parenting: false,
            frameset_ok: true,
            scripting_flag: false,
            errors: Vec::new(),
        }
    }

    /// Return the current node (§13.2.6.2).
    ///
    /// The current node is the top of the open elements stack. If the
    /// stack is empty (before any element is pushed), the current node is
    /// the Document itself.
    pub fn current_node(&self) -> Rc<RefCell<Node>> {
        self.open_elements
            .last()
            .cloned()
            .unwrap_or_else(|| self.document.clone())
    }

    /// Feed a single token to the tree construction state machine.
    ///
    /// Dispatches the token to the handler for the current insertion mode.
    /// If the handler returns `Step::Reprocess`, the same token is fed again
    /// to the (now switched) insertion mode. This loop terminates because
    /// every reprocess step must change `insertion_mode` or return `Done`.
    ///
    /// The `tokenizer` is passed so handlers can switch the tokenizer's
    /// content model (e.g. to RCDATA for `<title>`, per §13.2.6.4.4).
    pub fn run(&mut self, token: &Token, tokenizer: &mut dyn Tokenizer) {
        loop {
            match dispatch::dispatch(self, token, tokenizer) {
                dispatch::Step::Done => return,
                dispatch::Step::Reprocess => continue,
            }
        }
    }
}
