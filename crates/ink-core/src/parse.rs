//! Line-oriented block scanner + inline scanner. Hand-written on purpose: the
//! grammar is small and diverges from Markdown enough that a crate is a fight.

use crate::{Attribution, Block, CiteItem, Inline, Node, Span, Token, TokenKind, Visibility};

/// Parse a `.ink` document into a root [`Node`] (level 0).
///
/// Spans are **char** offsets. Line offsets assume `\n` endings; a stray `\r`
/// (CRLF) would shift them by one per line — normalize on load if it matters.
pub fn parse(src: &str) -> Node {
    parse_impl(src, None)
}

/// Highlighting tokens for `src` (#141), sorted by position. Same parse as
/// [`parse`] — the scan that builds the tree also reports each part's span — so
/// tokens can never disagree with what the document means.
pub fn tokens(src: &str) -> Vec<Token> {
    let mut out = Vec::new();
    parse_impl(src, Some(&mut out));
    // Pushed in source order: a node's meta as it's read, its body (buffered)
    // before the next heading's line.
    debug_assert!(out.windows(2).all(|w| w[0].span.end <= w[1].span.start), "tokens out of order");
    out
}

/// The parse, with an optional token sink (off for [`parse`], so it costs nothing).
fn parse_impl(src: &str, mut toks: Option<&mut Vec<Token>>) -> Node {
    let doc_len = src.chars().count();
    let root = Node {
        level: 0,
        visibility: Visibility::Visible,
        title: String::new(),
        meta: Vec::new(),
        body: Vec::new(),
        body_starts: Vec::new(),
        children: Vec::new(),
        heading_span: Span { start: 0, end: 0 },
        node_span: Span { start: 0, end: doc_len },
    };
    // Stack of raw-pointer-free indices into the tree is awkward; instead we
    // keep a stack of owned nodes and fold them together as levels close.
    let mut stack: Vec<Node> = vec![root];
    // Buffer of body lines (with each line's char offset) for the node currently
    // on top of the stack.
    let mut body_lines: Vec<(usize, &str)> = Vec::new();
    // Are we in a meta zone? Starts true so a leading `key: value` block becomes
    // document front matter on the root; otherwise it opens right after a heading.
    let mut in_meta = true;
    // While Some(i), the last meta pair (index i on the current node) has an empty
    // value opened for a multiline block; indented lines extend it.
    let mut multiline_idx: Option<usize> = None;
    let mut offset = 0usize; // char offset at the start of the current line

    for raw in src.lines() {
        let line_start = offset;
        let line_end = offset + raw.chars().count();
        offset = line_end + 1; // +1 for the '\n' str::lines() stripped

        if let Some((level, visibility, marks, title)) = heading(raw) {
            flush_body(stack.last_mut().unwrap(), &mut body_lines, toks.as_deref_mut());
            if let Some(t) = toks.as_deref_mut() {
                let kind = TokenKind::Heading(visibility);
                t.push(line_tok(raw, line_start, 0..marks, kind));
                let base = line_start + raw[..title.start].chars().count();
                scan_tokens(&raw[title.clone()], kind, t, base);
            }
            let title = raw[title].to_string();
            // Clamp illegal downward jumps to parent+1 (Model A: count is depth),
            // but only against a real heading parent — never the implicit root, or
            // a document that opens deep (e.g. all `##`) would see its first
            // heading demoted to level 1 and the rest nested under it.
            let parent_level = stack.last().map(|n| n.level).unwrap_or(0);
            let level = if stack.len() > 1 { level.min(parent_level + 1) } else { level };
            // Close any siblings/deeper nodes until top is a valid parent.
            while stack.len() > 1 && stack.last().unwrap().level >= level {
                let done = stack.pop().unwrap();
                stack.last_mut().unwrap().children.push(done);
            }
            stack.push(Node {
                level,
                visibility,
                title,
                meta: Vec::new(),
                body: Vec::new(),
                body_starts: Vec::new(),
                children: Vec::new(),
                heading_span: Span { start: line_start, end: line_end },
                // .end is filled in by close_spans once we know where the
                // next equal-or-shallower heading begins.
                node_span: Span { start: line_start, end: doc_len },
            });
            in_meta = true;
            multiline_idx = None;
            continue;
        }

        // Meta zone: `key: value` lines at the top of the file (document front
        // matter) or directly after a heading, until a blank line or the first
        // non-matching line.
        if in_meta {
            // A key with an empty value opens a multiline block: indented lines
            // extend it (trimmed, newline-joined). Any non-indented line closes it.
            if let Some(i) = multiline_idx {
                if is_indented(raw) {
                    let part = trimmed(raw, 0..raw.len());
                    if let Some(t) = toks.as_deref_mut() {
                        t.push(line_tok(raw, line_start, part.clone(), TokenKind::MetaValue));
                    }
                    let value = &mut stack.last_mut().unwrap().meta[i].1;
                    if !value.is_empty() {
                        value.push('\n');
                    }
                    value.push_str(&raw[part]);
                    continue;
                }
                multiline_idx = None; // dedent ends the block; re-classify this line
            }
            if raw.trim().is_empty() {
                in_meta = false;
                continue; // swallow the blank separator
            }
            if let Some((key, colon, value)) = meta_spans(raw) {
                if let Some(t) = toks.as_deref_mut() {
                    t.push(line_tok(raw, line_start, key.clone(), TokenKind::MetaKey));
                    t.push(line_tok(raw, line_start, colon..colon + 1, TokenKind::Marker));
                    // (no value token for a `key:` that opens a multiline value)
                    if !value.is_empty() {
                        t.push(line_tok(raw, line_start, value.clone(), TokenKind::MetaValue));
                    }
                }
                let (k, v) = (raw[key].to_string(), raw[value].to_string());
                let node = stack.last_mut().unwrap();
                let empty = v.is_empty();
                node.meta.push((k, v));
                if empty {
                    multiline_idx = Some(node.meta.len() - 1);
                }
                continue;
            }
            in_meta = false; // fall through: this line is body
        }

        body_lines.push((line_start, raw));
    }

    flush_body(stack.last_mut().unwrap(), &mut body_lines, toks);
    // Fold the remaining stack back into the root.
    while stack.len() > 1 {
        let done = stack.pop().unwrap();
        stack.last_mut().unwrap().children.push(done);
    }
    let mut root = stack.pop().unwrap();
    close_spans(&mut root, doc_len);
    root
}

/// Fill in each node's `node_span.end`: a node runs until its next sibling
/// begins, or (for a last child) until its parent's end.
fn close_spans(node: &mut Node, end: usize) {
    node.node_span.end = end;
    for i in 0..node.children.len() {
        let child_end = node.children.get(i + 1).map(|next| next.node_span.start).unwrap_or(end);
        close_spans(&mut node.children[i], child_end);
    }
}

/// `# Title` / `~~ Scene` / `%% Cut` -> (level, visibility, marker run length,
/// title's byte range). None if not a heading. The ranges are what both the tree
/// and the tokens use, so the two can't disagree on where a title starts.
fn heading(line: &str) -> Option<(u8, Visibility, usize, std::ops::Range<usize>)> {
    let mut chars = line.chars();
    let first = chars.next()?;
    let visibility = match first {
        '#' => Visibility::Visible,
        '~' => Visibility::Scene,
        '%' => Visibility::Excluded,
        _ => return None,
    };
    let mut count = 1u8;
    let rest = line[first.len_utf8()..].chars();
    let mut idx = first.len_utf8();
    for c in rest {
        if c == first {
            count += 1;
            idx += c.len_utf8();
        } else {
            break;
        }
    }
    // Must be followed by a space, else it's body text (e.g. "#hashtag").
    line[idx..].strip_prefix(' ')?;
    Some((count, visibility, idx, trimmed(line, idx + 1..line.len())))
}

/// `range` of `line` with surrounding whitespace dropped (an empty range at its
/// end if it's all whitespace).
fn trimmed(line: &str, range: std::ops::Range<usize>) -> std::ops::Range<usize> {
    let s = &line[range.clone()];
    let start = range.start + (s.len() - s.trim_start().len());
    start..(range.start + s.trim_end().len()).max(start)
}

/// A non-blank line that begins with whitespace — a continuation of a multiline
/// metadata value (a `key:` with an empty value, followed by indented lines).
fn is_indented(line: &str) -> bool {
    !line.trim().is_empty() && line.starts_with([' ', '\t'])
}

/// `key: value` where key is a single token. None otherwise. The single-token
/// key rule keeps prose like "She said: hello" out of the meta zone.
pub(crate) fn meta_line(line: &str) -> Option<(String, String)> {
    let (key, _, value) = meta_spans(line)?;
    Some((line[key].to_string(), line[value].to_string()))
}

/// [`meta_line`] as byte ranges: (key, the colon's index, value), trimmed — so
/// the tree and the tokens read a meta line identically (`time : 1` keys `time`).
fn meta_spans(line: &str) -> Option<(std::ops::Range<usize>, usize, std::ops::Range<usize>)> {
    let colon = line.find(':')?;
    let key = trimmed(line, 0..colon);
    let k = &line[key.clone()];
    if k.is_empty() || !k.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return None;
    }
    Some((key, colon, trimmed(line, colon + 1..line.len())))
}

/// Drain buffered body lines into the node: `/` lines become comments,
/// blank-separated runs become paragraphs.
fn flush_body(node: &mut Node, lines: &mut Vec<(usize, &str)>, mut toks: Option<&mut Vec<Token>>) {
    let mut para: Vec<&str> = Vec::new();
    let mut para_start = 0;
    for &(start, line) in lines.iter() {
        if let Some(comment) = line.strip_prefix('/') {
            push_para(node, &mut para, para_start, toks.as_deref_mut());
            if let Some(t) = toks.as_deref_mut() {
                t.push(line_tok(line, start, 0..line.len(), TokenKind::Comment));
            }
            node.body.push(Block::LineComment(comment.trim().to_string()));
            node.body_starts.push(start);
        } else if line.trim().is_empty() {
            push_para(node, &mut para, para_start, toks.as_deref_mut());
        } else {
            if para.is_empty() {
                para_start = start;
            }
            para.push(line);
        }
    }
    push_para(node, &mut para, para_start, toks);
    lines.clear();
}

/// Scan a buffered paragraph into the node. Its lines are consecutive source
/// lines, so the joined text is the source slice from `start` (LF endings).
fn push_para(node: &mut Node, para: &mut Vec<&str>, start: usize, toks: Option<&mut Vec<Token>>) {
    if para.is_empty() {
        return;
    }
    let text = para.join("\n");
    let inlines = match toks {
        Some(t) => scan_tokens(&text, TokenKind::Text, t, start),
        None => scan_inline(&text),
    };
    node.body.push(Block::Para(inlines));
    node.body_starts.push(start);
    para.clear();
}

/// A token over a byte range of a source line starting at char `line_start`.
fn line_tok(line: &str, line_start: usize, b: std::ops::Range<usize>, kind: TokenKind) -> Token {
    let start = line_start + line[..b.start].chars().count();
    let end = start + line[b].chars().count();
    Token { span: Span { start, end }, kind, bold: false, italic: false }
}

/// Scan `text` (which starts at char `char_base` in the source) with the token
/// sink on: the same scan as [`scan_inline`], plus its parts' spans appended to
/// `out` as char offsets. `kind` is what plain runs are (a heading's title text).
fn scan_tokens(text: &str, kind: TokenKind, out: &mut Vec<Token>, char_base: usize) -> Vec<Inline> {
    let mut tk = Tk { out: Some(Vec::new()), base: 0, kind, bold: false, italic: false };
    let inlines = scan(text, true, &mut tk);
    let mut toks = tk.out.unwrap_or_default();
    // A CriticMarkup span's markers are emitted after its body, so sort first.
    toks.sort_by_key(|t| t.span.start);
    // Byte -> char offsets, walking forward once (tokens don't overlap).
    let (mut byte, mut ch) = (0, char_base);
    let mut at = |b: usize| {
        ch += text[byte..b].chars().count();
        byte = b;
        ch
    };
    for t in &mut toks {
        t.span = Span { start: at(t.span.start), end: at(t.span.end) };
    }
    out.extend(toks);
    inlines
}

/// The inline scanner's token sink. `out` is None for a plain parse, making every
/// `emit` a no-op. Its tokens' spans are *byte* offsets into the top-level text
/// until [`scan_tokens`] converts them; `base` is where the text being scanned (a
/// markup body, in a recursive scan) starts within it.
struct Tk {
    out: Option<Vec<Token>>,
    base: usize,
    /// What plain text is here: `Text`, or the enclosing insert/delete/heading.
    kind: TokenKind,
    bold: bool,
    italic: bool,
}

impl Tk {
    fn off() -> Self {
        Tk { out: None, base: 0, kind: TokenKind::Text, bold: false, italic: false }
    }

    fn emit(&mut self, start: usize, end: usize, kind: TokenKind) {
        if let Some(out) = &mut self.out {
            if start < end {
                let span = Span { start: self.base + start, end: self.base + end };
                out.push(Token { span, kind, bold: self.bold, italic: self.italic });
            }
        }
    }

    /// A plain run: a token only if it's styled (not bare body text).
    fn text(&mut self, start: usize, end: usize) {
        if self.kind != TokenKind::Text || self.bold || self.italic {
            self.emit(start, end, self.kind);
        }
    }

    /// Scan a markup body that starts at byte `offset` of the current text, with
    /// plain text there as `kind` (if given) and emphasis added.
    fn within<R>(
        &mut self,
        offset: usize,
        kind: Option<TokenKind>,
        bold: bool,
        italic: bool,
        f: impl FnOnce(&mut Tk) -> R,
    ) -> R {
        let saved = (self.base, self.kind, self.bold, self.italic);
        self.base += offset;
        self.kind = kind.unwrap_or(self.kind);
        self.bold |= bold;
        self.italic |= italic;
        let r = f(self);
        (self.base, self.kind, self.bold, self.italic) = saved;
        r
    }
}

/// Parse the inside of a citation (`[@…]` or `{@…}`) into items. Split on `;`;
/// each item is `[@]key[, locator]` — strip an optional leading `@` (the first
/// item's `@` was the trigger; the rest carry their own), then split the first
/// comma into key + locator. Empty-key items are dropped.
/// ponytail: a locator containing `;` (e.g. `pp. 1; 3`) mis-splits — `;` is the
/// item separator; rare, unsupported.
pub(crate) fn parse_cite_items(inside: &str) -> Vec<CiteItem> {
    inside
        .split(';')
        .filter_map(|item| {
            let item = item.trim();
            let item = item.strip_prefix('@').unwrap_or(item);
            let (key, locator) = match item.split_once(',') {
                Some((k, l)) => (k.trim(), l.trim()),
                None => (item.trim(), ""),
            };
            (!key.is_empty())
                .then(|| CiteItem { key: key.to_string(), locator: locator.to_string() })
        })
        .collect()
}

/// Scan a paragraph's text into inline spans.
pub(crate) fn scan_inline(text: &str) -> Vec<Inline> {
    scan(text, true, &mut Tk::off())
}

/// `top` is false for a recursive scan of a markup body (emphasis/CriticMarkup),
/// whose end is the span's closer, not the paragraph's.
fn scan(text: &str, top: bool, tk: &mut Tk) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut plain_start = 0;
    // Where the current plain run starts for tokens: like `plain_start`, except an
    // interpolation ends the token run but stays inside the `Text` inline.
    let mut tok_start = 0;

    // Push any pending plain text before a markup token.
    macro_rules! flush_plain {
        ($end:expr) => {
            if $end > plain_start {
                out.push(Inline::Text(text[plain_start..$end].to_string()));
            }
            tk.text(tok_start, $end);
        };
    }

    'scan: while i < bytes.len() {
        // Backslash escape: the next char is a literal marker. Kept as `Escaped`
        // (not plain `Text`) so the edit/source round-trip can re-emit the `\` —
        // otherwise the char could re-form a marker on reparse (e.g. `\{{` → live
        // interpolation). Prints as the bare char in every other view.
        // A `\` ending the paragraph (or title) escapes nothing: `Escaped("")`,
        // so it never prints — like every line-ending `\` (#112), and matching the
        // editor, which can't see whether a line is a paragraph's last — but still
        // round-trips, so normalize never deletes a char the author typed. Only at
        // the top: a CriticMarkup body ending in `\` (`{+C:\}`) keeps it literal.
        if top && bytes[i] == b'\\' && i + 1 == bytes.len() {
            flush_plain!(i);
            tk.emit(i, i + 1, TokenKind::Escape);
            out.push(Inline::Escaped(String::new()));
            i += 1;
            plain_start = i;
            tok_start = i;
            continue;
        }
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            let next = i + 1;
            let len = next_char_len(bytes, next);
            let ch = &text[next..next + len];
            if is_escapable(ch) {
                flush_plain!(i);
                // A hard break's `\n` is invisible: the token is the `\` alone, so
                // it stays on its line (LSP tokens are single-line).
                let end = if ch == "\n" { i + 1 } else { next + len };
                tk.emit(i, end, TokenKind::Escape);
                out.push(Inline::Escaped(ch.to_string()));
                i = next + len;
                plain_start = i;
                tok_start = i;
                continue;
            }
        }
        // Interpolation {{…}} / {=…} is resolved later by `substitute`, not here.
        // Skip over it as literal text so its inner braces/dashes aren't mis-scanned
        // as CriticMarkup (e.g. the `{-` in `{{-1 * n}}` is not a deletion). Kept in
        // the plain run so the whole expression reaches `substitute` intact.
        for (open, close) in [("{{", "}}"), ("{=", "}")] {
            if text[i..].starts_with(open) {
                if let Some(end) = find(text, i + open.len(), close) {
                    tk.text(tok_start, i);
                    tk.emit(i, end + close.len(), TokenKind::Interp);
                    i = end + close.len();
                    tok_start = i;
                    continue 'scan;
                }
            }
        }
        // CriticMarkup: {+ {- {~ {/
        if bytes[i] == b'{' && i + 1 < bytes.len() {
            if let Some((inline, end)) = critic(text, i, tk) {
                flush_plain!(i);
                out.push(inline);
                i = end;
                plain_start = i;
                tok_start = i;
                continue;
            }
        }
        // Citation — borrowed `[@…]` or house `{@…}`, closed by `]`/`}`. Same
        // grouped syntax either way (#99). See `parse_cite_items`.
        for (open, close) in [("[@", "]"), ("{@", "}")] {
            if text[i..].starts_with(open) {
                if let Some(end) = find(text, i + 2, close) {
                    let items = parse_cite_items(&text[i + 2..end]);
                    if !items.is_empty() {
                        flush_plain!(i);
                        tk.emit(i, i + 2, TokenKind::Marker);
                        tk.emit(i + 2, end, TokenKind::Cite);
                        tk.emit(end, end + 1, TokenKind::Marker);
                        out.push(Inline::Cite(items));
                        i = end + 1;
                        plain_start = i;
                        tok_start = i;
                        continue 'scan;
                    }
                }
            }
        }
        // Wikilink — borrowed `[[Target]]` or house `{&Target}`; both reference a
        // codex entity by name (#99). Closed by `]]` / `}`.
        for (open, close, skip) in [("[[", "]]", 2), ("{&", "}", 1)] {
            if text[i..].starts_with(open) {
                if let Some(end) = find(text, i + 2, close) {
                    let target = text[i + 2..end].trim();
                    if !target.is_empty() {
                        flush_plain!(i);
                        tk.emit(i, i + 2, TokenKind::Marker);
                        tk.emit(i + 2, end, TokenKind::Link);
                        tk.emit(end, end + skip, TokenKind::Marker);
                        out.push(Inline::Link(target.to_string()));
                        i = end + skip;
                        plain_start = i;
                        tok_start = i;
                        continue 'scan;
                    }
                }
            }
        }
        // Bold **...** (check before single *). Flanking: opener followed by
        // non-space, closer preceded by non-space.
        if text[i..].starts_with("**") && opens(text, i + 2) {
            if let Some(end) = find_closing(text, i + 2, "**") {
                flush_plain!(i);
                tk.emit(i, i + 2, TokenKind::Marker);
                let body =
                    tk.within(i + 2, None, true, false, |tk| scan(&text[i + 2..end], false, tk));
                tk.emit(end, end + 2, TokenKind::Marker);
                out.push(Inline::Bold(body));
                i = end + 2;
                plain_start = i;
                tok_start = i;
                continue;
            }
        }
        // Italic *...*
        if bytes[i] == b'*' && opens(text, i + 1) {
            if let Some(end) = find_closing(text, i + 1, "*") {
                flush_plain!(i);
                tk.emit(i, i + 1, TokenKind::Marker);
                let body =
                    tk.within(i + 1, None, false, true, |tk| scan(&text[i + 1..end], false, tk));
                tk.emit(end, end + 1, TokenKind::Marker);
                out.push(Inline::Italic(body));
                i = end + 1;
                plain_start = i;
                tok_start = i;
                continue;
            }
        }
        i += next_char_len(bytes, i);
    }
    flush_plain!(text.len());
    out
}

/// Chars that a backslash escapes into a literal (markers + backslash itself).
fn is_escapable(ch: &str) -> bool {
    // `\n`: a backslash ending a line is a hard line break (CommonMark); a bare
    // newline inside a paragraph is a soft break that prints as a space (#112).
    matches!(ch, "*" | "{" | "}" | "#" | "~" | "/" | "\\" | "[" | "]" | "\n")
}

/// Left-flanking opener: the char at `at` exists and is not whitespace.
fn opens(text: &str, at: usize) -> bool {
    text[at..].chars().next().is_some_and(|c| !c.is_whitespace())
}

/// Right-flanking closer: find `delim` at/after `from` whose immediately
/// preceding char is non-whitespace, not a backslash, and leaves non-empty
/// content (`pos > from`).
fn find_closing(text: &str, from: usize, delim: &str) -> Option<usize> {
    let mut search = from;
    while let Some(pos) = find(text, search, delim) {
        if pos > from {
            match text[..pos].chars().next_back() {
                Some(c) if !c.is_whitespace() && c != '\\' => return Some(pos),
                _ => {}
            }
        }
        search = pos + delim.len();
    }
    None
}

/// Parse one CriticMarkup span starting at `{`. Returns (inline, end-after-`}`).
/// Reports tokens only once the span is known to be well formed.
fn critic(text: &str, start: usize, tk: &mut Tk) -> Option<(Inline, usize)> {
    use TokenKind::{Delete, Insert, Marker};
    let kind = *text.as_bytes().get(start + 1)?;
    let content_start = start + 2;
    let close = find(text, content_start, "}")?;
    // Peel the attribution suffix first: it covers the whole span, so for a
    // substitution it must not end up inside `new`.
    let (content, attr) = split_attribution(&text[content_start..close]);
    let body = |tk: &mut Tk, at: usize, part: &str, k| {
        tk.within(at, Some(k), false, false, |tk| scan(part, false, tk))
    };
    let inline = match kind {
        b'+' => Inline::Insert(body(tk, content_start, content, Insert)),
        b'-' => Inline::Delete(body(tk, content_start, content, Delete)),
        b'/' => {
            tk.emit(start, close + 1, TokenKind::Comment); // a comment is muted whole
            Inline::Comment(content.to_string())
        }
        b'~' => {
            let (old, new) = content.split_once('~')?;
            let tilde = content_start + old.len();
            let old = body(tk, content_start, old, Delete);
            tk.emit(tilde, tilde + 1, Marker);
            Inline::Sub { old, new: body(tk, tilde + 1, new, Insert) }
        }
        _ => return None,
    };
    if kind != b'/' {
        tk.emit(start, content_start, Marker);
        tk.emit(content_start + content.len(), close, TokenKind::Attribution); // empty if none
        tk.emit(close, close + 1, Marker);
    }
    let inline = match attr {
        Some(a) => Inline::Attributed(Box::new(inline), a),
        None => inline,
    };
    Some((inline, close + 1))
}

/// Split a CriticMarkup body into (content, attribution) (#116). The suffix is
/// recognized only when well formed at the very end — `|@author[,@author…]` plus
/// an optional `|date` — so any other `|` stays literal content and needs no
/// escape (`{+a|b}` inserts `a|b`). Mirrored by `attribution.js` for the editor.
fn split_attribution(body: &str) -> (&str, Option<Attribution>) {
    let mut parts = body.rsplitn(3, '|');
    let last = parts.next().unwrap_or_default();
    let (Some(prev), rest) = (parts.next(), parts.next()) else {
        return (body, None);
    };
    // `…|@authors|date`: the date is last, the authors before it.
    if is_iso_date(last) {
        if let (Some(authors), Some(content)) = (authors(prev), rest) {
            let date = Some(last.to_string());
            return (content, Some(Attribution { authors, date }));
        }
    }
    // `…|@authors`: no date. The content is everything before the last `|`.
    match authors(last) {
        Some(authors) => {
            (&body[..body.len() - last.len() - 1], Some(Attribution { authors, date: None }))
        }
        None => (body, None),
    }
}

/// `@Ada Lovelace,@Sam` → names, or None unless every comma-separated item is
/// `@` + a non-empty name. Whitespace around items and names is trimmed. A name
/// can't contain a span delimiter — `|`, `,`, `}` by construction, and `~` here —
/// so `{~a|@x~y}` stays the substitution `a|@x` → `y`, as before attribution.
fn authors(field: &str) -> Option<Vec<String>> {
    field
        .split(',')
        .map(|a| {
            let name = a.trim().strip_prefix('@')?.trim();
            (!name.is_empty() && !name.contains('~')).then(|| name.to_string())
        })
        .collect()
}

/// ISO 8601 date `YYYY-MM-DD`, optionally followed by a `T…` time part.
fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    b.len() >= 10
        && digits(0..4)
        && b[4] == b'-'
        && digits(5..7)
        && b[7] == b'-'
        && digits(8..10)
        && (b.len() == 10
            || (b[10] == b'T'
                && b[11..].iter().all(|c| c.is_ascii_digit() || b":.+-Z".contains(c))))
}

/// Byte index of the next occurrence of `needle` at or after `from`.
fn find(text: &str, from: usize, needle: &str) -> Option<usize> {
    text[from..].find(needle).map(|rel| from + rel)
}

/// UTF-8 length of the char starting at byte `i`.
fn next_char_len(bytes: &[u8], i: usize) -> usize {
    match bytes[i] {
        b if b < 0x80 => 1,
        b if b >> 5 == 0b110 => 2,
        b if b >> 4 == 0b1110 => 3,
        _ => 4,
    }
}
