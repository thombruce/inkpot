//! Three read-only views over the [`Node`] tree.

use crate::{Block, Inline, Node, Visibility};
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
    if view == View::Manuscript {
        // Each block trails a blank line; collapse the final run to one newline.
        out.truncate(out.trim_end_matches('\n').len());
        if !out.is_empty() {
            out.push('\n');
        }
    }
    out
}

/// The source heading char for a visibility.
fn sigil(v: Visibility) -> char {
    match v {
        Visibility::Visible => '#',
        Visibility::Scene => '~',
        Visibility::Excluded => '%',
    }
}

fn manuscript(node: &Node, out: &mut String) {
    // Excluded subtrees never reach the manuscript.
    if node.visibility == Visibility::Excluded {
        return;
    }
    // Visible headings print as Markdown ATX headings (depth = marker count,
    // clamped to h6); scenes contribute body only. Emphasis stays `**`/`*`, so
    // the whole manuscript is valid Markdown — convert onward with pandoc.
    if node.level > 0 && node.visibility == Visibility::Visible && !node.title.is_empty() {
        let hashes = "#".repeat(node.level.min(6) as usize);
        writeln!(out, "{hashes} {}\n", node.title).ok();
    }
    for block in &node.body {
        if let Block::Para(spans) = block {
            let line = print_inlines(spans);
            if !line.trim().is_empty() {
                writeln!(out, "{}\n", line.trim()).ok();
            }
        }
    }
    for child in &node.children {
        manuscript(child, out);
    }
}

/// Manuscript word count of a subtree: prose words that would print. Excluded
/// (`%`) subtrees, comments, deletions, and metadata contribute nothing;
/// substitutions count their replacement; visible heading titles are not counted
/// (prose only, matching how writers tally). Mirrors `manuscript`'s resolution.
pub fn word_count(node: &Node) -> usize {
    if node.visibility == Visibility::Excluded {
        return 0;
    }
    let mut n = 0;
    for block in &node.body {
        if let Block::Para(spans) = block {
            n += print_inlines(spans).split_whitespace().count();
        }
    }
    for child in &node.children {
        n += word_count(child);
    }
    n
}

/// Render `root` as a manuscript in HTML: visible headings become `<h1>`–`<h6>`,
/// paragraphs `<p>`, bold/italic `<strong>`/`<em>`, with CriticMarkup resolved
/// and scenes/metadata/comments dropped. Text is escaped; single newlines
/// within a paragraph become `<br>` (so verse lines survive).
pub fn render_html(root: &Node) -> String {
    let mut out = String::new();
    manuscript_html(root, &mut out);
    out
}

fn manuscript_html(node: &Node, out: &mut String) {
    if node.visibility == Visibility::Excluded {
        return;
    }
    if node.level > 0 && node.visibility == Visibility::Visible && !node.title.is_empty() {
        let lvl = node.level.min(6);
        writeln!(out, "<h{lvl}>{}</h{lvl}>", escape(&node.title)).ok();
    }
    for block in &node.body {
        if let Block::Para(spans) = block {
            let html = html_inlines(spans);
            if !html.trim().is_empty() {
                writeln!(out, "<p>{html}</p>").ok();
            }
        }
    }
    for child in &node.children {
        manuscript_html(child, out);
    }
}

fn html_inlines(spans: &[Inline]) -> String {
    spans.iter().filter_map(inline_html).collect()
}

fn inline_html(span: &Inline) -> Option<String> {
    Some(match span {
        Inline::Text(s) => escape(s).replace('\n', "<br>"),
        Inline::Bold(cs) => format!("<strong>{}</strong>", html_inlines(cs)),
        Inline::Italic(cs) => format!("<em>{}</em>", html_inlines(cs)),
        Inline::Insert(cs) => html_inlines(cs),
        Inline::Sub { new, .. } => html_inlines(new),
        Inline::Delete(_) | Inline::Comment(_) => return None,
    })
}

/// Escape the HTML-significant characters in prose text.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn outline(node: &Node, out: &mut String) {
    if node.level > 0 {
        let sigil = sigil(node.visibility);
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
        let sigil = sigil(node.visibility);
        let marker: String = std::iter::repeat(sigil).take(node.level as usize).collect();
        writeln!(out, "{marker} {}", node.title).ok();
    }
    // Meta round-trips for every node, including the root's document front matter.
    for (k, v) in &node.meta {
        writeln!(out, "{k}: {v}").ok();
    }
    if !node.meta.is_empty() {
        out.push('\n');
    }
    for block in &node.body {
        match block {
            Block::Para(spans) => {
                let line = source_inlines(spans);
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

/// Inline sequence -> print output: visible markup kept, criticmarkup resolved.
fn print_inlines(spans: &[Inline]) -> String {
    spans.iter().filter_map(inline_print).collect()
}

fn inline_print(span: &Inline) -> Option<String> {
    Some(match span {
        Inline::Text(s) => s.clone(),
        Inline::Bold(cs) => format!("**{}**", print_inlines(cs)),
        Inline::Italic(cs) => format!("*{}*", print_inlines(cs)),
        Inline::Insert(cs) => print_inlines(cs),
        Inline::Sub { new, .. } => print_inlines(new),
        Inline::Delete(_) | Inline::Comment(_) => return None,
    })
}

/// Inline sequence -> source form (round-trip within a paragraph).
fn source_inlines(spans: &[Inline]) -> String {
    spans.iter().map(inline_source).collect()
}

fn inline_source(span: &Inline) -> String {
    match span {
        Inline::Text(s) => s.clone(),
        Inline::Bold(cs) => format!("**{}**", source_inlines(cs)),
        Inline::Italic(cs) => format!("*{}*", source_inlines(cs)),
        Inline::Insert(cs) => format!("{{+{}}}", source_inlines(cs)),
        Inline::Delete(cs) => format!("{{-{}}}", source_inlines(cs)),
        Inline::Sub { old, new } => {
            format!("{{~{}~{}}}", source_inlines(old), source_inlines(new))
        }
        Inline::Comment(s) => format!("{{/{s}}}"),
    }
}
