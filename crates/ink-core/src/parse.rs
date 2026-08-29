//! Line-oriented block scanner + inline scanner. Hand-written on purpose: the
//! grammar is small and diverges from Markdown enough that a crate is a fight.

use crate::{Block, Inline, Node, Span, Visibility};

/// Parse a `.ink` document into a root [`Node`] (level 0).
///
/// Spans are **char** offsets. Line offsets assume `\n` endings; a stray `\r`
/// (CRLF) would shift them by one per line — normalize on load if it matters.
pub fn parse(src: &str) -> Node {
    let doc_len = src.chars().count();
    let root = Node {
        level: 0,
        visibility: Visibility::Visible,
        title: String::new(),
        meta: Vec::new(),
        body: Vec::new(),
        children: Vec::new(),
        heading_span: Span { start: 0, end: 0 },
        node_span: Span { start: 0, end: doc_len },
    };
    // Stack of raw-pointer-free indices into the tree is awkward; instead we
    // keep a stack of owned nodes and fold them together as levels close.
    let mut stack: Vec<Node> = vec![root];
    // Buffer of body lines for the node currently on top of the stack.
    let mut body_lines: Vec<&str> = Vec::new();
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

        if let Some((level, visibility, title)) = heading(raw) {
            flush_body(stack.last_mut().unwrap(), &mut body_lines);
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
                    let value = &mut stack.last_mut().unwrap().meta[i].1;
                    if !value.is_empty() {
                        value.push('\n');
                    }
                    value.push_str(raw.trim());
                    continue;
                }
                multiline_idx = None; // dedent ends the block; re-classify this line
            }
            if raw.trim().is_empty() {
                in_meta = false;
                continue; // swallow the blank separator
            }
            if let Some((k, v)) = meta_line(raw) {
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

        body_lines.push(raw);
    }

    flush_body(stack.last_mut().unwrap(), &mut body_lines);
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
        let child_end = node
            .children
            .get(i + 1)
            .map(|next| next.node_span.start)
            .unwrap_or(end);
        close_spans(&mut node.children[i], child_end);
    }
}

/// `# Title` / `~~ Scene` / `%% Cut` -> (level, visibility, title). None if not
/// a heading.
fn heading(line: &str) -> Option<(u8, Visibility, String)> {
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
    let after = &line[idx..];
    let title = after.strip_prefix(' ')?.trim().to_string();
    Some((count, visibility, title))
}

/// A non-blank line that begins with whitespace — a continuation of a multiline
/// metadata value (a `key:` with an empty value, followed by indented lines).
fn is_indented(line: &str) -> bool {
    !line.trim().is_empty() && line.starts_with([' ', '\t'])
}

/// `key: value` where key is a single token. None otherwise. The single-token
/// key rule keeps prose like "She said: hello" out of the meta zone.
fn meta_line(line: &str) -> Option<(String, String)> {
    let (k, v) = line.split_once(':')?;
    let k = k.trim();
    if k.is_empty() || !k.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return None;
    }
    Some((k.to_string(), v.trim().to_string()))
}

/// Drain buffered body lines into the node: `/` lines become comments,
/// blank-separated runs become paragraphs.
fn flush_body(node: &mut Node, lines: &mut Vec<&str>) {
    let mut para: Vec<&str> = Vec::new();
    for &line in lines.iter() {
        if let Some(comment) = line.strip_prefix('/') {
            if !para.is_empty() {
                node.body.push(Block::Para(scan_inline(&para.join("\n"))));
                para.clear();
            }
            node.body.push(Block::LineComment(comment.trim().to_string()));
        } else if line.trim().is_empty() {
            if !para.is_empty() {
                node.body.push(Block::Para(scan_inline(&para.join("\n"))));
                para.clear();
            }
        } else {
            para.push(line);
        }
    }
    if !para.is_empty() {
        node.body.push(Block::Para(scan_inline(&para.join("\n"))));
    }
    lines.clear();
}

/// Scan a paragraph's text into inline spans.
fn scan_inline(text: &str) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut plain_start = 0;

    // Push any pending plain text before a markup token.
    macro_rules! flush_plain {
        ($end:expr) => {
            if $end > plain_start {
                out.push(Inline::Text(text[plain_start..$end].to_string()));
            }
        };
    }

    while i < bytes.len() {
        // Backslash escape: the next char is literal (drops the backslash).
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            let next = i + 1;
            let len = next_char_len(bytes, next);
            let ch = &text[next..next + len];
            if is_escapable(ch) {
                flush_plain!(i);
                out.push(Inline::Text(ch.to_string()));
                i = next + len;
                plain_start = i;
                continue;
            }
        }
        // CriticMarkup: {+ {- {~ {/
        if bytes[i] == b'{' && i + 1 < bytes.len() {
            if let Some((inline, end)) = critic(text, i) {
                flush_plain!(i);
                out.push(inline);
                i = end;
                plain_start = i;
                continue;
            }
        }
        // Wikilink [[Target]] — cross-reference to a codex entity by name.
        if text[i..].starts_with("[[") {
            if let Some(end) = find(text, i + 2, "]]") {
                let target = text[i + 2..end].trim();
                if !target.is_empty() {
                    flush_plain!(i);
                    out.push(Inline::Link(target.to_string()));
                    i = end + 2;
                    plain_start = i;
                    continue;
                }
            }
        }
        // Bold **...** (check before single *). Flanking: opener followed by
        // non-space, closer preceded by non-space.
        if text[i..].starts_with("**") && opens(text, i + 2) {
            if let Some(end) = find_closing(text, i + 2, "**") {
                flush_plain!(i);
                out.push(Inline::Bold(scan_inline(&text[i + 2..end])));
                i = end + 2;
                plain_start = i;
                continue;
            }
        }
        // Italic *...*
        if bytes[i] == b'*' && opens(text, i + 1) {
            if let Some(end) = find_closing(text, i + 1, "*") {
                flush_plain!(i);
                out.push(Inline::Italic(scan_inline(&text[i + 1..end])));
                i = end + 1;
                plain_start = i;
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
    matches!(ch, "*" | "{" | "}" | "#" | "~" | "/" | "\\" | "[" | "]")
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
fn critic(text: &str, start: usize) -> Option<(Inline, usize)> {
    let kind = text.as_bytes().get(start + 1)?;
    let content_start = start + 2;
    let close = find(text, content_start, "}")?;
    let content = &text[content_start..close];
    let inline = match kind {
        b'+' => Inline::Insert(scan_inline(content)),
        b'-' => Inline::Delete(scan_inline(content)),
        b'/' => Inline::Comment(content.to_string()),
        b'~' => {
            let (old, new) = content.split_once('~')?;
            Inline::Sub {
                old: scan_inline(old),
                new: scan_inline(new),
            }
        }
        _ => return None,
    };
    Some((inline, close + 1))
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
