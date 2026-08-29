//! Three read-only views over the [`Node`] tree.

use crate::meta::{is_self_naming, ID};
use crate::{Block, Inline, Node, Visibility};
use std::collections::HashMap;
use std::fmt::Write;
use std::rc::Rc;

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
        View::Manuscript => manuscript(root, &root_ctx(root), &mut out),
        View::Outline => outline(root, &root_ctx(root), &mut out),
        View::Edit => edit(root, &mut out),
        View::Codex => codex(root, &root_ctx(root), &[], &mut out),
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

/// Resolution context for `{{ … }}` interpolation at a node. `number` is the
/// node's 1-based position among its non-excluded siblings, `total` their count
/// (manuscript-authoritative: `%` subtrees never consume a number). `vars` is
/// the metadata cascade — front matter plus every ancestor's `key: value`, this
/// node's own last, nearest wins.
struct Ctx {
    number: i64,
    total: i64,
    vars: HashMap<String, String>,
    // Shared, document-wide: fold(title)|fold(id) -> an entity's resolved title,
    // for printing `[[links]]`. Rc so per-node Ctx clones stay cheap.
    links: Rc<HashMap<String, String>>,
}

/// The root's context: front matter as variables, no position. Carries the
/// `[[link]]` title map, so prose rendering resolves link display text.
fn root_ctx(root: &Node) -> Ctx {
    Ctx { links: Rc::new(link_titles(root)), ..base_ctx(root) }
}

/// A root context without the link map. Used where link resolution isn't needed
/// and would recurse — `link_titles` builds on `resolve_titles`, which must not
/// re-enter `root_ctx`/`link_titles`.
fn base_ctx(root: &Node) -> Ctx {
    let mut vars = HashMap::new();
    for (k, v) in &root.meta {
        vars.insert(k.clone(), v.clone());
    }
    Ctx { number: 0, total: 0, vars, links: Rc::new(HashMap::new()) }
}

/// One `Ctx` per child of `node`, in order: numbering over the non-excluded
/// siblings, and the parent cascade extended with each child's own metadata.
fn child_ctxs(node: &Node, ctx: &Ctx) -> Vec<Ctx> {
    let total = node
        .children
        .iter()
        .filter(|c| c.visibility != Visibility::Excluded)
        .count() as i64;
    let mut number = 0i64;
    node.children
        .iter()
        .map(|child| {
            let n = if child.visibility != Visibility::Excluded {
                number += 1;
                number
            } else {
                0 // excluded nodes are outside manuscript numbering
            };
            let mut vars = ctx.vars.clone();
            for (k, v) in &child.meta {
                vars.insert(k.clone(), v.clone());
            }
            Ctx { number: n, total, vars, links: ctx.links.clone() }
        })
        .collect()
}

/// Build the `[[link]]` resolution map: fold(title) and fold(id) of every codex
/// entity (a `%` heading with a title) mapped to its resolved title. Ids win
/// over titles on collision, matching `CodexIndex::resolve_idx`; and on a
/// duplicate id the *first* declaration wins, matching `by_id` — so the printed
/// title never diverges from the link target. Duplicate titles collapse to one
/// key too; for static titles that prints identically (an interpolated title
/// resolves per-position, so a repeated `{{…}}` title would print its first
/// occurrence — negligible, no one links such a title).
fn link_titles(root: &Node) -> HashMap<String, String> {
    let mut entities = Vec::new();
    collect_entities(root, &mut entities);
    let resolved = resolve_titles(root);
    let title_of = |e: &Node| {
        resolved.get(&e.heading_span.start).cloned().unwrap_or_else(|| e.title.clone())
    };
    let mut m = HashMap::new();
    for e in &entities {
        m.entry(fold_name(&e.title)).or_insert_with(|| title_of(e));
    }
    // Ids win over titles, first id wins over later ones. Build separately then
    // extend so ids override a colliding title key without a later id clobbering
    // an earlier one.
    let mut ids = HashMap::new();
    for e in &entities {
        if let Some((_, v)) = e.meta.iter().find(|(k, _)| k == ID) {
            ids.entry(fold_name(v)).or_insert_with(|| title_of(e));
        }
    }
    m.extend(ids);
    m
}

/// The display text for a `[[target]]`: the resolved title of whatever it
/// resolves to, or the target verbatim if nothing matches.
fn link_text(target: &str, links: &HashMap<String, String>) -> String {
    links.get(&fold_name(target)).cloned().unwrap_or_else(|| target.to_string())
}

/// Resolve every heading's `{{…}}` interpolation, keyed by the heading's start
/// offset (`heading_span.start`, unique per heading). For consumers that walk the
/// [`Node`] tree themselves — the app's outline rail — and need the same resolved
/// titles the rendered views show. The root heading (offset 0, no title) is not
/// included; a heading without interpolation maps to its plain title.
pub fn resolve_titles(root: &Node) -> HashMap<usize, String> {
    let mut map = HashMap::new();
    for (child, cctx) in root.children.iter().zip(child_ctxs(root, &base_ctx(root))) {
        resolve_titles_walk(child, &cctx, &mut map);
    }
    map
}

fn resolve_titles_walk(node: &Node, ctx: &Ctx, map: &mut HashMap<usize, String>) {
    map.insert(node.heading_span.start, substitute(&node.title, ctx));
    for (child, cctx) in node.children.iter().zip(child_ctxs(node, ctx)) {
        resolve_titles_walk(child, &cctx, map);
    }
}

/// Resolve `{{ … }}` interpolations in `text`. An unresolved expression (unknown
/// variable, malformed arithmetic) is left verbatim — visible in the output as a
/// signal it is unfinished, matching CriticMarkup's ethos. So a stray `{{foo}}`
/// in ordinary prose just prints as-is.
///
/// A `\{{` renders a literal `{{`. This works for headings; in *prose* the
/// parser strips the backslash before render (`{` is escapable), so a prose
/// `\{{` cannot escape a *resolvable* var — but an unknown one passes through raw
/// anyway, covering the common case.
// ponytail: no Inline::Interp token — prose is scanned once, we substitute the
// Text runs at render. Upgrade to a parse-time token (like `[[wikilinks]]`) only
// if prose needs to escape a resolvable var.
fn substitute(text: &str, ctx: &Ctx) -> String {
    if !text.contains("{{") {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find("{{") {
        if rest[..pos].ends_with('\\') {
            out.push_str(&rest[..pos - 1]); // drop the escaping backslash
            out.push_str("{{");
            rest = &rest[pos + 2..];
            continue;
        }
        out.push_str(&rest[..pos]);
        let after = &rest[pos + 2..];
        match after.find("}}") {
            Some(end) => {
                let expr = &after[..end];
                match eval(expr, ctx) {
                    Some(v) => out.push_str(&v),
                    None => {
                        out.push_str("{{");
                        out.push_str(expr);
                        out.push_str("}}");
                    }
                }
                rest = &after[end + 2..];
            }
            None => {
                out.push_str("{{");
                out.push_str(after);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Evaluate an interpolation expression. `Some` on success; `None` when a
/// variable is unknown or the expression is malformed (the caller keeps the raw
/// `{{…}}`). A bare metadata key yields its string value; anything else is
/// integer arithmetic (`+ - * / ( )`, unary `-`) over `number`, `total`, and
/// integer-valued metadata.
fn eval(expr: &str, ctx: &Ctx) -> Option<String> {
    let key = expr.trim();
    // Built-ins win over metadata of the same name, so don't shortcut them here;
    // the arithmetic path below resolves `number`/`total`.
    if key != "number" && key != "total" {
        if let Some(v) = ctx.vars.get(key) {
            return Some(v.clone());
        }
    }
    let mut p = ExprParser { b: expr.as_bytes(), i: 0, ctx };
    let v = p.expr()?;
    p.ws();
    (p.i == p.b.len()).then(|| v.to_string())
}

/// Recursive-descent evaluator for the integer arithmetic subset. Deliberately
/// tiny: no functions, strings, or conditionals — keep it that way.
struct ExprParser<'a> {
    b: &'a [u8],
    i: usize,
    ctx: &'a Ctx,
}

impl ExprParser<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn expr(&mut self) -> Option<i64> {
        let mut v = self.term()?;
        loop {
            self.ws();
            match self.b.get(self.i) {
                // checked_* so an overflowing expression resolves to None and is
                // left verbatim, never panicking (debug) or wrapping (release).
                Some(b'+') => { self.i += 1; v = v.checked_add(self.term()?)?; }
                Some(b'-') => { self.i += 1; v = v.checked_sub(self.term()?)?; }
                _ => return Some(v),
            }
        }
    }
    fn term(&mut self) -> Option<i64> {
        let mut v = self.factor()?;
        loop {
            self.ws();
            match self.b.get(self.i) {
                Some(b'*') => { self.i += 1; v = v.checked_mul(self.factor()?)?; }
                // checked_div covers both zero and the i64::MIN / -1 overflow.
                Some(b'/') => { self.i += 1; v = v.checked_div(self.factor()?)?; }
                _ => return Some(v),
            }
        }
    }
    fn factor(&mut self) -> Option<i64> {
        self.ws();
        let &c = self.b.get(self.i)?;
        match c {
            b'-' => { self.i += 1; self.factor()?.checked_neg() }
            b'(' => {
                self.i += 1;
                let v = self.expr()?;
                self.ws();
                (self.b.get(self.i) == Some(&b')')).then(|| {
                    self.i += 1;
                    v
                })
            }
            b'0'..=b'9' => {
                let start = self.i;
                while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
                    self.i += 1;
                }
                std::str::from_utf8(&self.b[start..self.i]).ok()?.parse().ok()
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let start = self.i;
                while self.i < self.b.len()
                    && (self.b[self.i].is_ascii_alphanumeric() || self.b[self.i] == b'_')
                {
                    self.i += 1;
                }
                let name = std::str::from_utf8(&self.b[start..self.i]).ok()?;
                match name {
                    "number" => Some(self.ctx.number),
                    "total" => Some(self.ctx.total),
                    _ => self.ctx.vars.get(name)?.trim().parse().ok(),
                }
            }
            _ => None,
        }
    }
}

fn manuscript(node: &Node, ctx: &Ctx, out: &mut String) {
    // Excluded subtrees never reach the manuscript.
    if node.visibility == Visibility::Excluded {
        return;
    }
    // Visible headings print as Markdown ATX headings (depth = marker count,
    // clamped to h6); scenes contribute body only. Emphasis stays `**`/`*`, so
    // the whole manuscript is valid Markdown — convert onward with pandoc.
    if node.level > 0 && node.visibility == Visibility::Visible && !node.title.is_empty() {
        let hashes = "#".repeat(node.level.min(6) as usize);
        writeln!(out, "{hashes} {}\n", substitute(&node.title, ctx)).ok();
    }
    for block in &node.body {
        if let Block::Para(spans) = block {
            let line = print_inlines(spans, ctx);
            if !line.trim().is_empty() {
                writeln!(out, "{}\n", line.trim()).ok();
            }
        }
    }
    for (child, cctx) in node.children.iter().zip(child_ctxs(node, ctx)) {
        manuscript(child, &cctx, out);
    }
}

/// Manuscript word count of a subtree: prose words that would print. Excluded
/// (`%`) subtrees, comments, deletions, and metadata contribute nothing;
/// substitutions count their replacement; visible heading titles are not counted
/// (prose only, matching how writers tally). Mirrors `manuscript`'s resolution.
pub fn word_count(node: &Node) -> usize {
    word_count_ctx(node, &root_ctx(node))
}

fn word_count_ctx(node: &Node, ctx: &Ctx) -> usize {
    if node.visibility == Visibility::Excluded {
        return 0;
    }
    let mut n = 0;
    for block in &node.body {
        if let Block::Para(spans) = block {
            n += print_inlines(spans, ctx).split_whitespace().count();
        }
    }
    for (child, cctx) in node.children.iter().zip(child_ctxs(node, ctx)) {
        n += word_count_ctx(child, &cctx);
    }
    n
}

/// Render `root` as a manuscript in HTML: visible headings become `<h1>`–`<h6>`,
/// paragraphs `<p>`, bold/italic `<strong>`/`<em>`, with CriticMarkup resolved
/// and scenes/metadata/comments dropped. Text is escaped; single newlines
/// within a paragraph become `<br>` (so verse lines survive).
pub fn render_html(root: &Node) -> String {
    let mut out = String::new();
    manuscript_html(root, &root_ctx(root), &mut out);
    out
}

fn manuscript_html(node: &Node, ctx: &Ctx, out: &mut String) {
    if node.visibility == Visibility::Excluded {
        return;
    }
    if node.level > 0 && node.visibility == Visibility::Visible && !node.title.is_empty() {
        let lvl = node.level.min(6);
        writeln!(out, "<h{lvl}>{}</h{lvl}>", escape(&substitute(&node.title, ctx))).ok();
    }
    for block in &node.body {
        if let Block::Para(spans) = block {
            let html = html_inlines(spans, ctx);
            if !html.trim().is_empty() {
                writeln!(out, "<p>{html}</p>").ok();
            }
        }
    }
    for (child, cctx) in node.children.iter().zip(child_ctxs(node, ctx)) {
        manuscript_html(child, &cctx, out);
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
    by_name: HashMap<String, Vec<usize>>, // folded title -> entity indices, doc order
    by_id: HashMap<String, usize>,        // folded `id:` meta -> entities idx (first wins)
    by_offset: HashMap<usize, usize>,     // heading offset -> entities idx
    scopes: HashMap<usize, Vec<String>>,  // heading offset -> folded visible-ancestor scope
    backlinks: Vec<Vec<Backref>>,
}

fn fold_name(s: &str) -> String {
    s.trim().to_lowercase()
}

impl<'a> CodexIndex<'a> {
    fn build(root: &'a Node) -> Self {
        let mut entities = Vec::new();
        collect_entities(root, &mut entities);
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_id = HashMap::new();
        let mut by_offset = HashMap::new();
        for (i, e) in entities.iter().enumerate() {
            by_name.entry(fold_name(&e.title)).or_default().push(i);
            // `id:` is a rename-proof handle; first declaration of an id wins.
            if let Some((_, v)) = e.meta.iter().find(|(k, _)| k == ID) {
                by_id.entry(fold_name(v)).or_insert(i);
            }
            by_offset.insert(e.heading_span.start, i);
        }
        let mut scopes = HashMap::new();
        // base_ctx: scope building only substitutes titles, never resolves links.
        collect_scopes(root, &base_ctx(root), &[], &mut scopes);
        // Referrer labels use the same resolved titles as the views, so a
        // `# Chapter {{number}}` backlink reads "Chapter 3", not the raw formula.
        let titles = resolve_titles(root);
        let backlinks = (0..entities.len()).map(|_| Vec::new()).collect();
        let mut idx = CodexIndex { entities, by_name, by_id, by_offset, scopes, backlinks };
        idx.backlinks = idx.compute_backlinks(root, &titles);
        idx
    }

    /// The folded visible-heading scope a reference *inside* `node` sees: the
    /// node's own sits-in scope, plus the node itself when it is a visible
    /// (`#`/`~`) heading — so a `[[Note]]` in a chapter's body reaches a `%` note
    /// nested under that chapter, not just the chapter's siblings.
    fn ref_scope(&self, node: &Node, titles: &HashMap<usize, String>) -> Vec<String> {
        let mut s = self.scopes.get(&node.heading_span.start).cloned().unwrap_or_default();
        if node.visibility != Visibility::Excluded {
            let t = fold_name(titles.get(&node.heading_span.start).map_or(node.title.as_str(), |x| x));
            if !t.is_empty() {
                s.push(t);
            }
        }
        s
    }

    /// Resolve a name to an entity index. An `id:` handle wins first and is
    /// document-global (a stable handle is unique by design, so it ignores
    /// scope). Otherwise resolve by title, nearest scope first: among same-named
    /// entities whose sits-in scope is a prefix of `ref_scope`, the deepest wins;
    /// a root-scoped (empty) entity is a prefix of everything. If none enclose the
    /// referrer, fall back to the first same-named entity (document order).
    fn resolve_idx(&self, name: &str, ref_scope: &[String]) -> Option<usize> {
        let key = fold_name(name);
        if let Some(&i) = self.by_id.get(&key) {
            return Some(i);
        }
        let cands = self.by_name.get(&key)?;
        let scope_of = |&i: &usize| {
            self.scopes.get(&self.entities[i].heading_span.start).map(Vec::as_slice).unwrap_or(&[])
        };
        cands
            .iter()
            .copied()
            .filter(|i| ref_scope.starts_with(scope_of(i)))
            .max_by_key(|i| scope_of(i).len())
            .or_else(|| cands.first().copied())
    }

    /// Resolve one metadata value part to the target entity's heading offset,
    /// scoped to the referring entity.
    fn resolve(&self, part: &str, ref_scope: &[String]) -> Option<usize> {
        self.resolve_idx(part, ref_scope).map(|i| self.entities[i].heading_span.start)
    }

    /// Walk every node; a name that resolves to an entity — from a metadata value
    /// (comma-split) or a prose `[[wikilink]]` — records a backlink from the
    /// holding node, resolved by nearest scope. Whole-tree, deduped per referrer,
    /// self-skip.
    fn compute_backlinks(&self, root: &Node, titles: &HashMap<usize, String>) -> Vec<Vec<Backref>> {
        let mut backlinks: Vec<Vec<Backref>> = (0..self.entities.len()).map(|_| Vec::new()).collect();
        self.walk_backlinks(root, titles, &mut backlinks);
        backlinks
    }

    fn walk_backlinks(
        &self,
        node: &Node,
        titles: &HashMap<usize, String>,
        backlinks: &mut [Vec<Backref>],
    ) {
        for child in &node.children {
            let from = child.heading_span.start;
            let rscope = self.ref_scope(child, titles);
            let mut names: Vec<&str> = Vec::new();
            for (k, v) in &child.meta {
                if is_self_naming(k) {
                    continue; // `id` names this node, not an outgoing reference
                }
                names.extend(v.split(','));
            }
            for block in &child.body {
                if let Block::Para(spans) = block {
                    collect_links(spans, &mut names);
                }
            }
            for name in names {
                let Some(idx) = self.resolve_idx(name, &rscope) else { continue };
                if self.entities[idx].heading_span.start == from {
                    continue; // a node naming itself is not a backlink
                }
                let bl = &mut backlinks[idx];
                if !bl.iter().any(|b| b.offset == from) {
                    let title = titles.get(&from).cloned().unwrap_or_else(|| child.title.clone());
                    bl.push(Backref { title, offset: from });
                }
            }
            self.walk_backlinks(child, titles, backlinks);
        }
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

/// Map each heading offset to the folded resolved titles of its visible (`#`/`~`)
/// ancestors — its "sits-in" scope. Excluded (`%`) ancestors don't extend it, so
/// notes nested under a `%` share their nearest visible chapter's scope.
fn collect_scopes(node: &Node, ctx: &Ctx, scope: &[String], out: &mut HashMap<usize, Vec<String>>) {
    for (child, cctx) in node.children.iter().zip(child_ctxs(node, ctx)) {
        out.insert(child.heading_span.start, scope.to_vec());
        let inner = if child.visibility == Visibility::Excluded {
            scope.to_vec()
        } else {
            let mut s = scope.to_vec();
            let t = fold_name(&substitute(&child.title, &cctx));
            if !t.is_empty() {
                s.push(t);
            }
            s
        };
        collect_scopes(child, &cctx, &inner, out);
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
    codex_html(root, &root_ctx(root), &idx, &[], &mut out);
    out
}

/// `scope` is the resolved titles of the visible (`#`/`~`) ancestors walked
/// through to reach here — a `%% Synopsis` under `## Chapter 1` renders under
/// scope `["Chapter 1"]`, so two same-named notes in different chapters read
/// distinctly, and references (`[[…]]`, metadata) resolve to the nearest such
/// note by scope.
fn codex_html(node: &Node, ctx: &Ctx, idx: &CodexIndex, scope: &[String], out: &mut String) {
    for (child, cctx) in node.children.iter().zip(child_ctxs(node, ctx)) {
        if child.visibility == Visibility::Excluded {
            out.push_str("<section class=\"codex-section\">");
            if !scope.is_empty() {
                write!(out, "<div class=\"codex-scope\">{}</div>", escape(&scope.join(" / "))).ok();
            }
            codex_html_entry(child, 0, &cctx, idx, out);
            out.push_str("</section>");
        } else {
            codex_html(child, &cctx, idx, &pushed_scope(scope, &child.title, &cctx), out);
        }
    }
}

/// Extend a scope with a visible ancestor's resolved title, dropping empties.
fn pushed_scope(scope: &[String], title: &str, ctx: &Ctx) -> Vec<String> {
    let mut inner = scope.to_vec();
    let t = substitute(title, ctx);
    if !t.is_empty() {
        inner.push(t);
    }
    inner
}

fn codex_html_entry(node: &Node, depth: usize, ctx: &Ctx, idx: &CodexIndex, out: &mut String) {
    // Section title at h2, entities h3, deeper nesting steps down, capped at h6.
    let lvl = (depth + 2).min(6);
    let title = if node.title.is_empty() {
        "(untitled)".to_string()
    } else {
        substitute(&node.title, ctx)
    };
    writeln!(out, "<h{lvl}>{}</h{lvl}>", escape(&title)).ok();
    if !node.meta.is_empty() {
        // Meta values resolve from this entity's own scope (excluded, so its
        // sits-in scope), matching how a `[[link]]` here would resolve.
        let rscope = idx.scopes.get(&node.heading_span.start).cloned().unwrap_or_default();
        out.push_str("<dl>");
        for (k, v) in &node.meta {
            // `id` names this entity; render it plain, never as a self-link.
            let dd = if is_self_naming(k) { escape(v) } else { meta_value_html(v, idx, &rscope) };
            write!(out, "<dt>{}</dt><dd>{}</dd>", escape(k), dd).ok();
        }
        out.push_str("</dl>");
    }
    for block in &node.body {
        if let Block::Para(spans) = block {
            let html = html_inlines(spans, ctx);
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
    for (child, cctx) in node.children.iter().zip(child_ctxs(node, ctx)) {
        out.push_str("<article class=\"entity\">");
        codex_html_entry(child, depth + 1, &cctx, idx, out);
        out.push_str("</article>");
    }
}

/// A metadata value's comma parts, each linked if it names an entity. Empty
/// parts (a trailing comma) are dropped.
fn meta_value_html(v: &str, idx: &CodexIndex, ref_scope: &[String]) -> String {
    v.split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|part| match idx.resolve(part, ref_scope) {
            Some(off) => format!("<a class=\"ref\" data-jump=\"{off}\">{}</a>", escape(part)),
            None => escape(part),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn html_inlines(spans: &[Inline], ctx: &Ctx) -> String {
    spans.iter().filter_map(|s| inline_html(s, ctx)).collect()
}

fn inline_html(span: &Inline, ctx: &Ctx) -> Option<String> {
    Some(match span {
        Inline::Text(s) => escape(&substitute(s, ctx)).replace('\n', "<br>"),
        Inline::Bold(cs) => format!("<strong>{}</strong>", html_inlines(cs, ctx)),
        Inline::Italic(cs) => format!("<em>{}</em>", html_inlines(cs, ctx)),
        Inline::Insert(cs) => html_inlines(cs, ctx),
        Inline::Sub { new, .. } => html_inlines(new, ctx),
        Inline::Link(s) => format!("<a class=\"wikilink\">{}</a>", escape(&link_text(s, &ctx.links))),
        Inline::Delete(_) | Inline::Comment(_) => return None,
    })
}

/// Escape the HTML-significant characters in prose text.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn outline(node: &Node, ctx: &Ctx, out: &mut String) {
    if node.level > 0 {
        let sigil = sigil(node.visibility);
        let indent = "  ".repeat((node.level - 1) as usize);
        let marker: String = std::iter::repeat(sigil).take(node.level as usize).collect();
        let title = if node.title.is_empty() {
            "(untitled)".to_string()
        } else {
            substitute(&node.title, ctx)
        };
        write!(out, "{indent}{marker} {title}").ok();
        if !node.meta.is_empty() {
            let keys = node.meta.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>().join(", ");
            write!(out, "  [{keys}]").ok();
        }
        out.push('\n');
    }
    for (child, cctx) in node.children.iter().zip(child_ctxs(node, ctx)) {
        outline(child, &cctx, out);
    }
}

/// Codex view: render each excluded (`%`) subtree as a grouped entity index.
/// Non-excluded nodes are walked through (not shown) so a `% Notes` block nested
/// under a visible chapter is still collected. Once inside an excluded root the
/// whole subtree is codex, so it renders unconditionally.
fn codex(node: &Node, ctx: &Ctx, scope: &[String], out: &mut String) {
    for (child, cctx) in node.children.iter().zip(child_ctxs(node, ctx)) {
        if child.visibility == Visibility::Excluded {
            if !scope.is_empty() {
                writeln!(out, "[{}]", scope.join(" / ")).ok();
            }
            codex_entry(child, 0, &cctx, out);
            out.push('\n');
        } else {
            codex(child, &cctx, &pushed_scope(scope, &child.title, &cctx), out);
        }
    }
}

fn codex_entry(node: &Node, depth: usize, ctx: &Ctx, out: &mut String) {
    let indent = "  ".repeat(depth);
    let title = if node.title.is_empty() {
        "(untitled)".to_string()
    } else {
        substitute(&node.title, ctx)
    };
    writeln!(out, "{indent}{title}").ok();
    for (k, v) in &node.meta {
        writeln!(out, "{indent}  {k}: {v}").ok();
    }
    for (child, cctx) in node.children.iter().zip(child_ctxs(node, ctx)) {
        codex_entry(child, depth + 1, &cctx, out);
    }
}

fn edit(node: &Node, out: &mut String) {
    if node.level > 0 {
        let sigil = sigil(node.visibility);
        let marker: String = std::iter::repeat(sigil).take(node.level as usize).collect();
        writeln!(out, "{marker} {}", node.title).ok();
    }
    // Meta round-trips for every node, including the root's document front matter.
    // A multiline value (embedded newlines) re-serializes as `key:` + indented
    // continuation lines — the form the parser reads back into one value.
    for (k, v) in &node.meta {
        if v.contains('\n') {
            writeln!(out, "{k}:").ok();
            for part in v.split('\n') {
                writeln!(out, "  {part}").ok();
            }
        } else {
            writeln!(out, "{k}: {v}").ok();
        }
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
fn print_inlines(spans: &[Inline], ctx: &Ctx) -> String {
    spans.iter().filter_map(|s| inline_print(s, ctx)).collect()
}

fn inline_print(span: &Inline, ctx: &Ctx) -> Option<String> {
    Some(match span {
        Inline::Text(s) => substitute(s, ctx),
        Inline::Bold(cs) => format!("**{}**", print_inlines(cs, ctx)),
        Inline::Italic(cs) => format!("*{}*", print_inlines(cs, ctx)),
        Inline::Insert(cs) => print_inlines(cs, ctx),
        Inline::Sub { new, .. } => print_inlines(new, ctx),
        Inline::Link(s) => link_text(s, &ctx.links),
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
