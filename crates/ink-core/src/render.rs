//! Three read-only views over the [`Node`] tree.

use crate::{Block, Inline, Node};
use std::fmt::Write;

/// Which projection of the document to render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Print view: visible headings + resolved criticmarkup, no scenes/meta/comments.
    Manuscript,
    /// Every heading (visible + invisible), indented, with metadata keys.
    Outline,
    /// Everything, re-serialized: sigils, meta, raw markup, comments.
    Edit,
}

/// Render `root` as the given view.
pub fn render(root: &Node, view: View) -> String {
    let mut out = String::new();
    match view {
        View::Manuscript => manuscript(root, &mut out),
        View::Outline => outline(root, &mut out),
        View::Edit => edit(root, &mut out),
    }
    out
}

fn manuscript(node: &Node, out: &mut String) {
    // Visible headings print their title; scenes contribute body only.
    if node.level > 0 && node.visible && !node.title.is_empty() {
        writeln!(out, "{}\n", node.title).ok();
    }
    for block in &node.body {
        if let Block::Para(spans) = block {
            let line = spans.iter().filter_map(inline_print).collect::<String>();
            if !line.trim().is_empty() {
                writeln!(out, "{}\n", line.trim()).ok();
            }
        }
    }
    for child in &node.children {
        manuscript(child, out);
    }
}

fn outline(node: &Node, out: &mut String) {
    if node.level > 0 {
        let sigil = if node.visible { '#' } else { '~' };
        let indent = "  ".repeat((node.level - 1) as usize);
        let marker: String = std::iter::repeat(sigil).take(node.level as usize).collect();
        let title = if node.title.is_empty() { "(untitled)" } else { &node.title };
        write!(out, "{indent}{marker} {title}").ok();
        if !node.meta.is_empty() {
            let keys = node.meta.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>().join(", ");
            write!(out, "  [{keys}]").ok();
        }
        out.push('\n');
    }
    for child in &node.children {
        outline(child, out);
    }
}

fn edit(node: &Node, out: &mut String) {
    if node.level > 0 {
        let sigil = if node.visible { '#' } else { '~' };
        let marker: String = std::iter::repeat(sigil).take(node.level as usize).collect();
        writeln!(out, "{marker} {}", node.title).ok();
        for (k, v) in &node.meta {
            writeln!(out, "{k}: {v}").ok();
        }
        if !node.meta.is_empty() {
            out.push('\n');
        }
    }
    for block in &node.body {
        match block {
            Block::Para(spans) => {
                let line = spans.iter().map(inline_source).collect::<String>();
                writeln!(out, "{line}\n").ok();
            }
            Block::LineComment(c) => {
                writeln!(out, "/ {c}").ok();
            }
        }
    }
    for child in &node.children {
        edit(child, out);
    }
}

/// Inline -> print output: visible markup kept, criticmarkup resolved.
fn inline_print(span: &Inline) -> Option<String> {
    Some(match span {
        Inline::Text(s) => s.clone(),
        Inline::Bold(s) => format!("**{s}**"),
        Inline::Italic(s) => format!("*{s}*"),
        Inline::Insert(s) => s.clone(),
        Inline::Sub { new, .. } => new.clone(),
        Inline::Delete(_) | Inline::Comment(_) => return None,
    })
}

/// Inline -> source form (round-trip within a paragraph).
fn inline_source(span: &Inline) -> String {
    match span {
        Inline::Text(s) => s.clone(),
        Inline::Bold(s) => format!("**{s}**"),
        Inline::Italic(s) => format!("*{s}*"),
        Inline::Insert(s) => format!("{{+{s}}}"),
        Inline::Delete(s) => format!("{{-{s}}}"),
        Inline::Sub { old, new } => format!("{{~{old}~{new}}}"),
        Inline::Comment(s) => format!("{{/{s}}}"),
    }
}
