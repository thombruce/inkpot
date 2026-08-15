//! inkpot core: parse the `.ink` prose format into a tree and render views.
//!
//! Text is canonical. A document parses into a [`Node`] tree (rooted at an
//! implicit level-0 node). Views are read-only walks of that tree.

mod parse;
mod render;

pub use parse::parse;
pub use render::{render, View};

/// A structural unit: heading + metadata + body prose + nested children.
///
/// The root node has `level == 0`, `visible == true`, empty `title`, and holds
/// any preamble text (before the first heading) plus the top-level headings.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Depth in the shared hierarchy. Marker *count* in the source.
    pub level: u8,
    /// `#` headings print (`true`); `~` scene headings do not (`false`).
    pub visible: bool,
    pub title: String,
    /// `key: value` lines directly under the heading. Never printed.
    pub meta: Vec<(String, String)>,
    pub body: Vec<Block>,
    pub children: Vec<Node>,
}

/// A block of content inside a node's body.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// A paragraph: consecutive non-blank lines, scanned into inlines.
    Para(Vec<Inline>),
    /// A `/`-prefixed line comment. Never printed.
    LineComment(String),
}

/// An inline span within a paragraph.
#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String),
    /// `**bold**` — visible markup, prints.
    Bold(String),
    /// `*italic*` — visible markup, prints.
    Italic(String),
    /// `{+insertion}` — CriticMarkup, accepted into print.
    Insert(String),
    /// `{-deletion}` — CriticMarkup, dropped from print.
    Delete(String),
    /// `{~old~new}` — CriticMarkup substitution; `new` prints.
    Sub { old: String, new: String },
    /// `{/comment}` — inline comment, never prints.
    Comment(String),
}
