//! Selectors Level 4 §3 data model.
//!
//! Defines the selector data structures produced by the parser. Every
//! type in this module corresponds to a term in the §3 grammar
//! ("Selector Syntax and Structure") or to a sub-term referenced by §3
//! from later sections (§4 logical combinations, §5 elemental
//! selectors, §6 attribute selectors, §13 tree-structural
//! pseudo-classes, §14 pseudo-elements, §15 combinators).
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §3 L716-1357.
//!
//! # Orderings
//!
//! Per §3 L809-826, a [`ComplexSelector`] is stored rightmost-first:
//! `units[0]` is the subject (the rightmost compound selector),
//! `units[1]` is the compound to its left, etc. This matches the
//! right-to-left matching direction mandated by §18 L4902-4919.

use muskitty_css::tokenizer::Token;

/// §3 L858-873: A selector represents a pattern of element(s) in a
/// tree.
///
/// A [`SelectorList`] is a comma-separated list of complex selectors
/// (§3 L856-857). All selectors in the list share the same
/// "subject-of-match" semantics: a single element matches the list if
/// it matches at least one of the complex selectors.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SelectorList(pub Vec<ComplexSelector>);

impl SelectorList {
    /// §17 L4547-4548: max specificity over all complex selectors in
    /// the list. Returns `(0,0,0)` for an empty list.
    pub fn specificity_max(&self) -> crate::specificity::Specificity {
        crate::specificity::Specificity::max_of_list(self)
    }
}

/// §3 L809-826: A complex selector is a sequence of compound selectors
/// separated by combinators.
///
/// Storage is rightmost-first: `units[0]` is the subject (rightmost
/// compound selector in source order), `units[1]` is the compound to
/// its left, ..., `units[len-1]` is the leftmost compound in source
/// order. The combinator on `units[idx]` links it to the next
/// leftward unit `units[idx+1]` and is stored on the rightward unit
/// (i.e. on `units[idx]`, the unit closer to the subject). The
/// leftmost unit in source order (`units[len-1]`) always has
/// `combinator == None` because there is no further-left unit to link
/// to; the subject (`units[0]`) carries the combinator that links it
/// to `units[1]` when the complex selector has more than one unit.
#[derive(Debug, Clone, PartialEq)]
pub struct ComplexSelector {
    pub units: Vec<ComplexSelectorUnit>,
}

impl ComplexSelector {
    /// §17 L4536-4548: compute the specificity of this complex
    /// selector. Delegates to [`crate::specificity::specificity_of_complex`].
    pub fn specificity(&self) -> crate::specificity::Specificity {
        crate::specificity::specificity_of_complex(self)
    }
}

/// A compound selector paired with the combinator that links it to
/// the next leftward unit (`units[idx+1]` in the parent
/// [`ComplexSelector`]). The leftmost unit in source order
/// (`units[len-1]`) has `combinator == None`; the subject (`units[0]`)
/// carries the combinator linking it to `units[1]` when present.
#[derive(Debug, Clone, PartialEq)]
pub struct ComplexSelectorUnit {
    pub compound: CompoundSelector,
    pub combinator: Option<Combinator>,
}

/// §3 L746-760: A compound selector is a sequence of simple
/// selectors with no combinator between them.
///
/// Field order mirrors the §3 grammar: type selector (or universal
/// selector) must come first, followed by zero or more subclass
/// selectors (id / class / attribute / pseudo-class), followed by
/// zero or more pseudo-compound selectors (pseudo-element + trailing
/// pseudo-classes).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CompoundSelector {
    /// §3 L750-752: type selector or universal selector, must come
    /// first in a compound selector. `None` when the compound has no
    /// type selector (e.g. `.foo`).
    pub type_selector: Option<TypeSelector>,
    /// Subclass selectors in source order: id / class / attribute /
    /// pseudo-class. Per §3 L753-760 they may appear in any order
    /// after the type selector.
    pub subclasses: Vec<SubclassSelector>,
    /// §3 L762-787: pseudo-compound selectors (pseudo-element + any
    /// trailing pseudo-classes that apply to it). Empty for selectors
    /// without a pseudo-element.
    pub pseudo_compounds: Vec<PseudoCompoundSelector>,
}

/// §3 L798-805 + §15 L4360-4532: Combinator between two compound
/// selectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combinator {
    /// §15 L4369: descendant combinator (whitespace).
    Descendant,
    /// §15 L4376: child combinator (`>`).
    Child,
    /// §15 L4383: next-sibling combinator (`+`).
    NextSibling,
    /// §15 L4390: subsequent-sibling combinator (`~`).
    SubsequentSibling,
}

/// §5 L1808-1824: Type (tag name) selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeSelector {
    /// §5 L1867-1872: optional namespace prefix (`ns|tag`, `*|tag`,
    /// `|tag`). `None` means no prefix was written.
    pub ns_prefix: Option<NsPrefix>,
    /// Tag name (lowercase for HTML; case-sensitive for XML) or
    /// universal selector.
    pub name: TypeSelectorName,
}

/// §5 L1825-1866: The name part of a type selector — either a
/// concrete tag name or the universal selector `*`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeSelectorName {
    /// Concrete tag name (e.g. "div", "svg", "rect").
    Name(String),
    /// §5 L1825-1866: Universal selector (`*`).
    Universal,
}

/// §5 L1867-1872: namespace prefix (`ns|tag` or `*|tag`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NsPrefix {
    pub prefix: NsPrefixKind,
}

/// §5 L1867-1872: Kind of namespace prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NsPrefixKind {
    /// `ns|tag` — named namespace. The string carries the prefix
    /// name as written (e.g. "svg", "math").
    Named(String),
    /// `*|tag` — any namespace.
    Any,
    /// `|tag` — no namespace (empty prefix).
    None,
}

/// §3 L4674-4685: subclass-selector = id | class | attribute |
/// pseudo-class.
#[derive(Debug, Clone, PartialEq)]
pub enum SubclassSelector {
    /// §6.6 L2463-2533: `#id`.
    Id(IdSelector),
    /// §6.5 L2376-2462: `.class`.
    Class(ClassSelector),
    /// §6 L1996-2533: `[attr=value]`.
    Attribute(AttributeSelector),
    /// §13/§7-§12: `:pseudo-class` or `:pseudo-class(args)`.
    PseudoClass(PseudoClass),
}

/// §6.6 L2463-2533: ID selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdSelector {
    pub id: String,
}

/// §6.5 L2376-2462: Class selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassSelector {
    pub class: String,
}

/// §6 L1996-2533: Attribute selector (full representation, parsed
/// once).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeSelector {
    /// Attribute name (with optional namespace prefix).
    pub name: WqName,
    /// `None` for presence selector `[attr]`; otherwise the matcher
    /// (`=`, `~=`, `|=`, `^=`, `$=`, `*=`).
    pub matcher: Option<AttrMatcher>,
    /// `None` for presence selector; otherwise the value being
    /// compared.
    pub value: Option<AttrValue>,
    /// §6.3 L2193-2264: case-sensitivity modifier (`i` / `s`). `None`
    /// means no modifier was written; the default then depends on
    /// the attribute kind per §6.3.
    pub modifier: Option<AttrModifier>,
}

/// §6.1 L2023-2135: `[attr=value]` / `[attr~=value]` /
/// `[attr|=value]` / `[attr^=value]` / `[attr$=value]` /
/// `[attr*=value]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrMatcher {
    /// `[attr=value]` (§6.1 L2037-2054): exact match.
    Exact,
    /// `[attr~=value]` (§6.2 L2137-2162): whitespace-separated list
    /// contains value.
    Includes,
    /// `[attr|=value]` (§6.1 L2055-2080): exact match or prefix
    /// followed by hyphen.
    DashMatch,
    /// `[attr^=value]` (§6.2 L2137-2162): value is a prefix.
    Prefix,
    /// `[attr$=value]` (§6.2 L2137-2162): value is a suffix.
    Suffix,
    /// `[attr*=value]` (§6.2 L2137-2162): value is a substring.
    Substring,
}

/// §6.3 L2193-2264: case-sensitivity modifier on an attribute
/// selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrModifier {
    /// `i` — case-insensitive matching.
    CaseInsensitive,
    /// `s` — case-sensitive matching.
    CaseSensitive,
}

/// §5 L4679-4685: wq-name = ns-prefix? ident-token. Used for attribute
/// names and type names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WqName {
    /// Optional namespace prefix.
    pub ns_prefix: Option<NsPrefix>,
    /// Local name (the ident-token after the optional prefix).
    pub local_name: String,
}

/// §6 L1996-2533: attribute value (string-token or ident-token).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrValue {
    /// Quoted string value: `[attr="value"]`.
    String(String),
    /// Unquoted ident value: `[attr=value]`.
    Ident(String),
}

/// §13/§4 pseudo-class.
///
/// A pseudo-class is `:name` or `:name(args)`. The optional argument
/// is captured as a typed [`PseudoClassArgument`] when the parser
/// knows the pseudo-class's argument grammar (An+B, selector list,
/// etc.); otherwise the raw tokens are preserved.
#[derive(Debug, Clone, PartialEq)]
pub struct PseudoClass {
    /// Pseudo-class name as written, lowercased per §3 L1245-1306
    /// (ident-token case folding).
    pub name: String,
    /// Optional argument. `None` for value-less pseudo-classes like
    /// `:root` / `:empty`.
    pub argument: Option<PseudoClassArgument>,
}

/// Argument carried by a parameterised pseudo-class.
#[derive(Debug, Clone, PartialEq)]
pub enum PseudoClassArgument {
    /// For `:nth-child(An+B [of S]?)`, `:nth-last-child(An+B [of S]?)`,
    /// `:nth-of-type(An+B)`, `:nth-last-of-type(An+B)`. The optional
    /// `SelectorList` carries the `of S` argument when present
    /// (§13.3 L3968, §13.4 L4077). Always `None` for `:nth-of-type`
    /// and `:nth-last-of-type` (those do not accept `of S` syntax).
    AnPlusB(AnPlusB, Option<SelectorList>),
    /// For `:is()`, `:not()`, `:where()`, `:has()` — a selector list.
    SelectorList(SelectorList),
    /// For `:lang(*)`, `:dir(*)`, `:current(*)`, etc. — preserved
    /// component values for caller-side interpretation.
    Raw(Vec<Token>),
}

/// §13.5 An+B notation (used by `:nth-child()`, `:nth-last-child()`,
/// `:nth-of-type()`, `:nth-last-of-type()`).
///
/// Represents a linear form `a*k + b` where `k` is a non-negative
/// integer (1-based sibling index). The matcher checks whether the
/// element's index satisfies the equation for some `k >= 0`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AnPlusB {
    /// The `A` coefficient. May be negative (e.g. `-n+3` → a=-1).
    pub a: i64,
    /// The `B` constant. May be negative (e.g. `2n-1` → b=-1).
    pub b: i64,
}

/// §3 L762-787: pseudo-compound selector (pseudo-element + trailing
/// pseudo-classes that apply to it).
#[derive(Debug, Clone, PartialEq)]
pub struct PseudoCompoundSelector {
    pub pseudo_element: PseudoElement,
    /// Pseudo-classes appearing after the pseudo-element in source
    /// order, e.g. `::before:hover` → `trailing_pseudo_classes =
    /// [PseudoClass { name: "hover", ... }]`.
    pub trailing_pseudo_classes: Vec<PseudoClass>,
}

/// §14 Pseudo-element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PseudoElement {
    /// Pseudo-element name as written, lowercased per §3 L1245-1306.
    pub name: String,
    /// §14 legacy single-colon form: `:before`, `:after`,
    /// `:first-line`, `:first-letter`. True when the source used the
    /// single-colon form recognised for backwards compatibility; false
    /// for the modern `::name` form.
    pub legacy: bool,
}
