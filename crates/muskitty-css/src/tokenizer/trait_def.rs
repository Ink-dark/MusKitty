//! Tokenizer trait definition.
//!
//! CSS Syntax Module Level 3 §4.3 "Tokenizer Algorithms".

use super::types::{State, Token};

/// The CSS tokenizer.
///
/// Implements the tokenization algorithm of CSS Syntax §4.3. The tokenizer
/// consumes Unicode code points from an input stream (preprocessed per
/// §5.3 to normalize newlines) and emits [`Token`]s.
///
/// # Algorithm structure
///
/// Unlike the HTML tokenizer's explicit state machine, the CSS tokenizer
/// is recursive-descent: a single entry point `consume_a_token` (§4.3.1)
/// dispatches to sub-algorithms (`consume_an_ident_like_token` §4.3.4,
/// `consume_a_numeric_token` §4.3.3, `consume_a_string_token` §4.3.5,
/// `consume_a_url_token` §4.3.6) which in turn call primitives
/// (`consume_an_escaped_code_point` §4.3.7, `consume_an_ident_sequence`
/// §4.3.12, `consume_a_number` §4.3.13).
///
/// # Reentrancy
///
/// The CSS tokenizer is *not* reentrant in the same way the HTML
/// tokenizer is: CSS has no analogue of HTML's content-model switching
/// (RCDATA / RAWTEXT / ScriptData). The state is therefore minimal —
/// just "consuming" vs. "EOF emitted". Future CSSOM incremental-parsing
/// use cases may require reentrancy; the trait exposes
/// [`state`](Tokenizer::state) and [`set_state`](Tokenizer::set_state)
/// to support that future need.
pub trait Tokenizer {
    /// Consume and return the next token from the input stream.
    ///
    /// Implements the §4.3.1 "Consume a token" algorithm at the top level.
    /// Returns `Some(token)` for each token, ending with
    /// `Some(Token::Eof)` for `<EOF-token>` (§5.3). After `<EOF-token>`
    /// has been emitted, returns `None`.
    fn next_token(&mut self) -> Option<Token>;

    /// Return the current tokenizer state.
    fn state(&self) -> State;

    /// Set the current tokenizer state.
    ///
    /// Currently only useful for resetting to [`State::Data`] after EOF,
    /// which re-enables token production (though the input stream is
    /// already exhausted, so this is a no-op in practice).
    fn set_state(&mut self, state: State);

    /// Reset the tokenizer to its initial state over the same input.
    ///
    /// Clears any partial token state and resets position to 0. Used by
    /// future CSSOM incremental parsing and by test harnesses.
    fn reset(&mut self);
}
