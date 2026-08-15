//! inkpot core: parse the `.ink` prose format into a tree and render views.
//!
//! Text is canonical. A document parses into a [`Node`] tree (rooted at an
//! implicit level-0 node). Views are read-only walks of that tree.

mod parse;
mod render;

pub use parse::parse;
pub use render::{render, render_html, View};

/// A half-open range of **char** (Unicode scalar) offsets into the source.
///
/// Char offsets, not bytes: they line up with JS string indexing on the
/// frontend (except astral chars — see the parser notes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// How a heading and its subtree reach the manuscript. The marker char sets it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    /// `#` — heading and body print.
    Visible,
    /// `~` — heading hidden, body still prints (a scene).
    Scene,
    /// `%` — heading, body, and all descendants excluded from the manuscript
    /// (kept in the document; shown in outline and edit views).
    Excluded,
}

/// A structural unit: heading + metadata + body prose + nested children.
///
/// The root node has `level == 0`, `Visibility::Visible`, empty `title`, and
/// holds any preamble text (before the first heading) plus the top-level headings.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Depth in the shared hierarchy. Marker *count* in the source.
    pub level: u8,
    /// Whether the heading/subtree prints, per its marker (`#`/`~`/`%`).
    pub visibility: Visibility,
    pub title: String,
    /// `key: value` lines directly under the heading. Never printed.
    pub meta: Vec<(String, String)>,
    pub body: Vec<Block>,
    pub children: Vec<Node>,
    /// The heading line itself (`~~~ Title`). Empty `0..0` for the root.
    /// Frontend uses `heading_span.start` to scroll the editor to a heading.
    pub heading_span: Span,
    /// The whole subtree: heading through the end of its last descendant
    /// (up to the next heading of equal-or-shallower level, else end of doc).
    /// Frontend uses this to cut/paste a scene when drag-reordering.
    pub node_span: Span,
}

/// A block of content inside a node's body.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// A paragraph: consecutive non-blank lines, scanned into inlines.
    Para(Vec<Inline>),
    /// A `/`-prefixed line comment. Never printed.
    LineComment(String),
}

/// An inline span within a paragraph. Markup operands are themselves inline
/// sequences, so markup nests: `{+She was **furious**.}` parses the bold.
#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String),
    /// `**bold**` — visible markup, prints.
    Bold(Vec<Inline>),
    /// `*italic*` — visible markup, prints.
    Italic(Vec<Inline>),
    /// `{+insertion}` — CriticMarkup, accepted into print.
    Insert(Vec<Inline>),
    /// `{-deletion}` — CriticMarkup, dropped from print.
    Delete(Vec<Inline>),
    /// `{~old~new}` — CriticMarkup substitution; `new` prints.
    Sub { old: Vec<Inline>, new: Vec<Inline> },
    /// `{/comment}` — inline comment, never prints. Kept raw (never rendered).
    Comment(String),
}
