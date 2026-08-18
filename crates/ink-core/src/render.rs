//! Three read-only views over the [`Node`] tree.

use crate::{Block, Inline, Node, Visibility};
use std::collections::HashMap;
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
    /// Codex: the excluded (`%`) subtrees as a grouped entity index. Each
    /// top-level `%` section (Characters, Locations, Timeline, …) is a heading;
    /// its nested entries list their title + metadata. Stage 1 of the codex
    /// (issue #9): derive an index from what authors already write, no new syntax.
    Codex,
}

/// Render `root` as the given view.
pub fn render(root: &Node, view: View) -> String {
    let mut out = String::new();
    match view {
        View::Manuscript => manuscript(root, &mut out),
        View::Outline => outline(root, &mut out),
        View::Edit => edit(root, &mut out),
        View::Codex => codex(root, &mut out),
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

/// A referrer: a node whose metadata names an entity. `offset` is its heading's
/// char offset — the frontend jumps there via `data-jump`.
struct Backref {
    title: String,
    offset: usize,
}

/// The codex cross-reference index. Entities are every `%` heading with a title;
/// a metadata value resolves to one when a comma-separated, trimmed, case-folded
/// part equals its title — no key whitelist, a value is a link iff it names an
/// entity. `backlinks[i]` are the nodes that reference entity `i`.
struct CodexIndex<'a> {
    entities: Vec<&'a Node>,
    by_name: HashMap<String, usize>,  // folded title -> entities idx (first wins)
    by_offset: HashMap<usize, usize>, // heading offset -> entities idx
    backlinks: Vec<Vec<Backref>>,
}

fn fold_name(s: &str) -> String {
    s.trim().to_lowercase()
}

impl<'a> CodexIndex<'a> {
    fn build(root: &'a Node) -> Self {
        let mut entities = Vec::new();
        collect_entities(root, &mut entities);
        let mut by_name = HashMap::new();
        let mut by_offset = HashMap::new();
        for (i, e) in entities.iter().enumerate() {
            // First entity of a name wins; a later duplicate is unreachable by
            // reference. ponytail: name collisions resolve to the first; add
            // disambiguation (e.g. [[wikilinks]] with a path) if it bites.
            by_name.entry(fold_name(&e.title)).or_insert(i);
            by_offset.insert(e.heading_span.start, i);
        }
        let mut backlinks: Vec<Vec<Backref>> = (0..entities.len()).map(|_| Vec::new()).collect();
        collect_backlinks(root, &by_name, &entities, &mut backlinks);
        CodexIndex { entities, by_name, by_offset, backlinks }
    }

    /// Resolve one metadata value part to the target entity's heading offset.
    fn resolve(&self, part: &str) -> Option<usize> {
        self.by_name
            .get(&fold_name(part))
            .map(|&i| self.entities[i].heading_span.start)
    }
}

/// Every excluded (`%`) heading with a title, in document order.
fn collect_entities<'a>(node: &'a Node, out: &mut Vec<&'a Node>) {
    for child in &node.children {
        if child.visibility == Visibility::Excluded && !child.title.is_empty() {
            out.push(child);
        }
        collect_entities(child, out);
    }
}

/// Walk every node; a name that resolves to an entity — from a metadata value
/// (comma-split) or a prose `[[wikilink]]` — records a backlink from the holding
/// node. Whole-tree, so a scene's `characters:` or `[[Alice]]` links its
/// entities, not just entity-to-entity edges. Deduped per referrer; self-skip.
fn collect_backlinks(
    node: &Node,
    by_name: &HashMap<String, usize>,
    entities: &[&Node],
    backlinks: &mut [Vec<Backref>],
) {
    for child in &node.children {
        let from = child.heading_span.start;
        let mut names: Vec<&str> = Vec::new();
        for (_k, v) in &child.meta {
            names.extend(v.split(','));
        }
        for block in &child.body {
            if let Block::Para(spans) = block {
                collect_links(spans, &mut names);
            }
        }
        for name in names {
            let Some(&idx) = by_name.get(&fold_name(name)) else { continue };
            if entities[idx].heading_span.start == from {
                continue; // a node naming itself is not a backlink
            }
            let bl = &mut backlinks[idx];
            if !bl.iter().any(|b| b.offset == from) {
                bl.push(Backref { title: child.title.clone(), offset: from });
            }
        }
        collect_backlinks(child, by_name, entities, backlinks);
    }
}

/// Collect the targets of every `[[wikilink]]` in an inline sequence, recursing
/// into nested markup (bold/italic/criticmarkup).
fn collect_links<'a>(spans: &'a [Inline], out: &mut Vec<&'a str>) {
    for s in spans {
        match s {
            Inline::Link(t) => out.push(t),
            Inline::Bold(cs) | Inline::Italic(cs) | Inline::Insert(cs) | Inline::Delete(cs) => {
                collect_links(cs, out)
            }
            Inline::Sub { old, new } => {
                collect_links(old, out);
                collect_links(new, out);
            }
            Inline::Text(_) | Inline::Comment(_) => {}
        }
    }
}

/// Render the codex — the excluded (`%`) subtrees — as HTML for the app's codex
/// panel. Each top-level `%` section is a `<section>`; its entries nest as
/// `<article class="entity">` with an `<h_>` name, a `<dl>` of metadata, body
/// prose, and a "Referenced by" list. Metadata values and backlinks that resolve
/// to an entity are `<a class="ref" data-jump="offset">` links. Mirrors the
/// plain-text [`View::Codex`] walk. Text is escaped for `innerHTML` assignment.
pub fn render_codex_html(root: &Node) -> String {
    let idx = CodexIndex::build(root);
    let mut out = String::new();
    codex_html(root, &idx, &mut out);
    out
}

fn codex_html(node: &Node, idx: &CodexIndex, out: &mut String) {
    for child in &node.children {
        if child.visibility == Visibility::Excluded {
            out.push_str("<section class=\"codex-section\">");
            codex_html_entry(child, 0, idx, out);
            out.push_str("</section>");
        } else {
            codex_html(child, idx, out);
        }
    }
}

fn codex_html_entry(node: &Node, depth: usize, idx: &CodexIndex, out: &mut String) {
    // Section title at h2, entities h3, deeper nesting steps down, capped at h6.
    let lvl = (depth + 2).min(6);
    let title = if node.title.is_empty() { "(untitled)" } else { &node.title };
    writeln!(out, "<h{lvl}>{}</h{lvl}>", escape(title)).ok();
    if !node.meta.is_empty() {
        out.push_str("<dl>");
        for (k, v) in &node.meta {
            write!(out, "<dt>{}</dt><dd>{}</dd>", escape(k), meta_value_html(v, idx)).ok();
        }
        out.push_str("</dl>");
    }
    for block in &node.body {
        if let Block::Para(spans) = block {
            let html = html_inlines(spans);
            if !html.trim().is_empty() {
                writeln!(out, "<p>{html}</p>").ok();
            }
        }
    }
    // Backlinks: nodes whose metadata names this entity.
    if let Some(&i) = idx.by_offset.get(&node.heading_span.start) {
        let bl = &idx.backlinks[i];
        if !bl.is_empty() {
            out.push_str("<div class=\"backlinks\">Referenced by ");
            for (n, b) in bl.iter().enumerate() {
                if n > 0 {
                    out.push_str(", ");
                }
                let t = if b.title.is_empty() { "(untitled)" } else { &b.title };
                write!(out, "<a class=\"ref\" data-jump=\"{}\">{}</a>", b.offset, escape(t)).ok();
            }
            out.push_str("</div>");
        }
    }
    // Nested entries wrap so styling can indent them under their parent.
    for child in &node.children {
        out.push_str("<article class=\"entity\">");
        codex_html_entry(child, depth + 1, idx, out);
        out.push_str("</article>");
    }
}

/// A metadata value's comma parts, each linked if it names an entity. Empty
/// parts (a trailing comma) are dropped.
fn meta_value_html(v: &str, idx: &CodexIndex) -> String {
    v.split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|part| match idx.resolve(part) {
            Some(off) => format!("<a class=\"ref\" data-jump=\"{off}\">{}</a>", escape(part)),
            None => escape(part),
        })
        .collect::<Vec<_>>()
        .join(", ")
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
        Inline::Link(s) => format!("<a class=\"wikilink\">{}</a>", escape(s)),
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

/// Codex view: render each excluded (`%`) subtree as a grouped entity index.
/// Non-excluded nodes are walked through (not shown) so a `% Notes` block nested
/// under a visible chapter is still collected. Once inside an excluded root the
/// whole subtree is codex, so it renders unconditionally.
fn codex(node: &Node, out: &mut String) {
    for child in &node.children {
        if child.visibility == Visibility::Excluded {
            codex_entry(child, 0, out);
            out.push('\n');
        } else {
            codex(child, out);
        }
    }
}

fn codex_entry(node: &Node, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    let title = if node.title.is_empty() { "(untitled)" } else { &node.title };
    writeln!(out, "{indent}{title}").ok();
    for (k, v) in &node.meta {
        writeln!(out, "{indent}  {k}: {v}").ok();
    }
    for child in &node.children {
        codex_entry(child, depth + 1, out);
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
        Inline::Link(s) => s.clone(),
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
        Inline::Link(s) => format!("[[{s}]]"),
    }
}
