//! `ink lsp` — a Language Server Protocol server over `ink-core` (#141), so any
//! LSP editor (Neovim, Helix, VS Code, Zed) gets inkpot features from the real
//! parser, with no editor-side mirror. Speaks LSP over stdio.
//!
//! Stateless in spirit, like the app: the only state is the open buffers' text
//! (full-document sync), and every request re-parses. Document symbols (the
//! outline), folding ranges, formatting (`ink fmt`'s house style), and semantic
//! tokens (highlighting, from the parser's own `ink_core::tokens`).

use ink_core::{
    parse, render, resolve_titles, tokens, word_count, Node, Span, TokenKind, View, Visibility,
};
use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Notification as _,
};
use lsp_types::request::{
    DocumentSymbolRequest, FoldingRangeRequest, Formatting, Request as _, SemanticTokensFullRequest,
};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentFormattingParams, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse,
    FoldingRange, FoldingRangeParams, OneOf, Position, Range, SemanticToken, SemanticTokenModifier,
    SemanticTokenType, SemanticTokens, SemanticTokensFullOptions, SemanticTokensLegend,
    SemanticTokensOptions, SemanticTokensParams, SemanticTokensResult,
    SemanticTokensServerCapabilities, ServerCapabilities, SymbolKind, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextEdit,
};
use std::collections::HashMap;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Run the server on stdin/stdout until the client shuts it down.
pub fn run() -> Result<()> {
    let (conn, io) = Connection::stdio();
    serve(&conn)?;
    drop(conn); // closes the writer, so the IO threads can finish
    io.join()?;
    Ok(())
}

fn serve(conn: &Connection) -> Result<()> {
    let caps = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        document_symbol_provider: Some(OneOf::Left(true)),
        folding_range_provider: Some(true.into()),
        document_formatting_provider: Some(OneOf::Left(true)),
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            SemanticTokensOptions {
                legend: SemanticTokensLegend {
                    token_types: TOKEN_TYPES.iter().map(|&t| SemanticTokenType::new(t)).collect(),
                    token_modifiers: TOKEN_MODIFIERS
                        .iter()
                        .map(|&m| SemanticTokenModifier::new(m))
                        .collect(),
                },
                full: Some(SemanticTokensFullOptions::Bool(true)),
                ..Default::default()
            },
        )),
        ..Default::default()
    };
    // The two-step initialize, so the response can carry serverInfo: editors show
    // which `ink` release they're talking to (Neovim's `:checkhealth vim.lsp`).
    let (id, _params) = conn.initialize_start()?;
    conn.initialize_finish(
        id,
        serde_json::json!({
            "capabilities": caps,
            "serverInfo": { "name": "ink", "version": crate::VERSION },
        }),
    )?;

    let mut docs: HashMap<String, String> = HashMap::new(); // uri -> buffer text
    for msg in &conn.receiver {
        match msg {
            Message::Request(req) => {
                if conn.handle_shutdown(&req)? {
                    return Ok(());
                }
                let resp = respond(&docs, req);
                conn.sender.send(resp.into())?;
            }
            // `exit` before `shutdown` (handle_shutdown consumes the proper one):
            // the spec says exit with code 1.
            Message::Notification(n) if n.method == "exit" => {
                return Err("exit without shutdown".into())
            }
            Message::Notification(n) => on_notification(&mut docs, n),
            Message::Response(_) => {} // we send no requests
        }
    }
    Ok(())
}

// Buffers are stored with `\n` endings: `ink-core` offsets assume them, so
// positions, the canonical-form check and format edits all line up. A CRLF
// buffer keeps the same line/column positions, and the client keeps its own
// line endings.
fn on_notification(docs: &mut HashMap<String, String>, n: Notification) {
    let method = n.method.clone();
    let res = match method.as_str() {
        DidOpenTextDocument::METHOD => n.extract::<DidOpenTextDocumentParams>(&method).map(|p| {
            docs.insert(
                p.text_document.uri.to_string(),
                p.text_document.text.replace("\r\n", "\n"),
            );
        }),
        DidChangeTextDocument::METHOD => {
            n.extract::<DidChangeTextDocumentParams>(&method).map(|mut p| {
                // Full sync: the last change carries the whole document.
                if let Some(c) = p.content_changes.pop() {
                    docs.insert(p.text_document.uri.to_string(), c.text.replace("\r\n", "\n"));
                }
            })
        }
        DidCloseTextDocument::METHOD => n.extract::<DidCloseTextDocumentParams>(&method).map(|p| {
            docs.remove(p.text_document.uri.as_str());
        }),
        _ => Ok(()),
    };
    // A dropped change leaves stale text behind; at least say so (stderr is the
    // client's LSP log).
    if let Err(e) = res {
        eprintln!("ink lsp: ignored malformed {method}: {e:?}");
    }
}

fn respond(docs: &HashMap<String, String>, req: Request) -> Response {
    let id = req.id.clone();
    // An unknown document is an error, not an empty one: an empty outline or a
    // no-op format would hide that the server never saw the buffer.
    let text = |uri: &lsp_types::Uri| {
        docs.get(uri.as_str())
            .map(String::as_str)
            .ok_or_else(|| format!("unknown document {uri:?}"))
    };
    let result = match req.method.as_str() {
        DocumentSymbolRequest::METHOD => params::<DocumentSymbolParams>(req).and_then(|p| {
            text(&p.text_document.uri).map(|t| json(DocumentSymbolResponse::Nested(symbols(t))))
        }),
        FoldingRangeRequest::METHOD => params::<FoldingRangeParams>(req)
            .and_then(|p| text(&p.text_document.uri).map(|t| json(folds(t)))),
        Formatting::METHOD => params::<DocumentFormattingParams>(req)
            .and_then(|p| text(&p.text_document.uri).map(|t| json(format(t)))),
        SemanticTokensFullRequest::METHOD => params::<SemanticTokensParams>(req).and_then(|p| {
            text(&p.text_document.uri).map(|t| {
                let data = semantic_tokens(t);
                json(SemanticTokensResult::Tokens(SemanticTokens { result_id: None, data }))
            })
        }),
        _ => {
            return Response::new_err(
                id,
                lsp_server::ErrorCode::MethodNotFound as i32,
                format!("unhandled method {}", req.method),
            )
        }
    };
    match result {
        Ok(v) => Response::new_ok(id, v),
        Err(e) => Response::new_err(id, lsp_server::ErrorCode::InvalidParams as i32, e),
    }
}

fn params<P: serde::de::DeserializeOwned>(req: Request) -> std::result::Result<P, String> {
    let method = req.method.clone();
    req.extract(&method).map(|(_, p)| p).map_err(|e| format!("{e:?}"))
}

fn json(v: impl serde::Serialize) -> serde_json::Value {
    serde_json::to_value(v).expect("lsp-types serialize")
}

/// Maps `ink-core`'s char offsets to LSP positions (line + UTF-16 column, the
/// LSP default encoding) — correct for astral chars, unlike the app's
/// char-offset-as-JS-index shortcut.
struct Lines<'a> {
    text: &'a str,
    /// Per line: (char offset, byte offset) of its first char.
    starts: Vec<(usize, usize)>,
}

impl<'a> Lines<'a> {
    fn new(text: &'a str) -> Self {
        let mut starts = vec![(0, 0)];
        for (ci, (bi, c)) in text.char_indices().enumerate() {
            if c == '\n' {
                starts.push((ci + 1, bi + 1));
            }
        }
        Lines { text, starts }
    }

    fn line_of(&self, off: usize) -> usize {
        self.starts.partition_point(|&(c, _)| c <= off) - 1
    }

    fn pos(&self, off: usize) -> Position {
        let line = self.line_of(off);
        let (c0, b0) = self.starts[line];
        let col = self.text[b0..].chars().take(off - c0).map(char::len_utf16).sum::<usize>();
        Position::new(line as u32, col as u32)
    }

    fn range(&self, s: Span) -> Range {
        Range::new(self.pos(s.start), self.pos(s.end))
    }

    fn is_blank(&self, line: usize) -> bool {
        let b0 = self.starts[line].1;
        let b1 = self.starts.get(line + 1).map_or(self.text.len(), |&(_, b)| b);
        self.text[b0..b1].trim().is_empty()
    }
}

/// The heading tree as nested symbols: range = the whole subtree, selection =
/// the heading line. Titles are resolved (interpolation) like the app's outline.
fn symbols(text: &str) -> Vec<DocumentSymbol> {
    let root = parse(text);
    let titles = resolve_titles(&root);
    let lines = Lines::new(text);
    root.children.iter().map(|n| symbol(n, &titles, &lines)).collect()
}

#[allow(deprecated)] // DocumentSymbol::deprecated is a required-but-deprecated field
fn symbol(node: &Node, titles: &HashMap<usize, String>, lines: &Lines) -> DocumentSymbol {
    let title = titles.get(&node.heading_span.start).unwrap_or(&node.title);
    let (kind, what) = match node.visibility {
        Visibility::Visible => (SymbolKind::MODULE, "heading"),
        Visibility::Scene => (SymbolKind::EVENT, "scene"),
        Visibility::Excluded => (SymbolKind::OBJECT, "excluded"),
    };
    DocumentSymbol {
        // LSP rejects an empty name; a bare `#` heading has none.
        name: if title.trim().is_empty() { "(untitled)".into() } else { title.clone() },
        detail: Some(match word_count(node) {
            1 => format!("{what} · 1 word"),
            n => format!("{what} · {n} words"),
        }),
        kind,
        tags: None,
        deprecated: None,
        range: lines.range(node.node_span),
        selection_range: lines.range(node.heading_span),
        children: Some(node.children.iter().map(|c| symbol(c, titles, lines)).collect()),
    }
}

/// One fold per heading: its line through the subtree's last non-blank line.
fn folds(text: &str) -> Vec<FoldingRange> {
    let root = parse(text);
    let lines = Lines::new(text);
    let mut out = Vec::new();
    fold_walk(&root, &lines, &mut out);
    out
}

fn fold_walk(node: &Node, lines: &Lines, out: &mut Vec<FoldingRange>) {
    for c in &node.children {
        let start = lines.line_of(c.heading_span.start);
        let mut end = lines.line_of(c.node_span.end.saturating_sub(1)); // end is exclusive
        while end > start && lines.is_blank(end) {
            end -= 1;
        }
        if end > start {
            out.push(FoldingRange {
                start_line: start as u32,
                end_line: end as u32,
                ..Default::default()
            });
        }
        fold_walk(c, lines, out);
    }
}

/// The semantic token legend; a token's type is its index here. Custom names, as
/// prose has no standard LSP types for a heading or an insertion — the README
/// links them to Neovim's `@markup`/`@diff` groups — except `comment`, a standard
/// type every theme colors with no setup.
const TOKEN_TYPES: &[&str] = &[
    "heading",
    "scene",
    "excluded",
    "metaKey",
    "metaValue",
    "comment",
    "marker",
    "text",
    "insert",
    "delete",
    "attribution",
    "link",
    "cite",
    "interp",
    "escape",
];
const TOKEN_MODIFIERS: &[&str] = &["bold", "italic"];

/// A token kind's index in [`TOKEN_TYPES`]. Exhaustive, so a new kind can't be
/// left out of the legend.
fn token_type(kind: TokenKind) -> u32 {
    use TokenKind::*;
    match kind {
        Heading(Visibility::Visible) => 0,
        Heading(Visibility::Scene) => 1,
        Heading(Visibility::Excluded) => 2,
        MetaKey => 3,
        MetaValue => 4,
        Comment => 5,
        Marker => 6,
        Text => 7,
        Insert => 8,
        Delete => 9,
        Attribution => 10,
        Link => 11,
        Cite => 12,
        Interp => 13,
        Escape => 14,
    }
}

/// `ink_core::tokens` as LSP semantic tokens: delta-encoded, UTF-16 columns, one
/// line each — a token crossing a soft break (an insertion over two lines) is
/// split at the line end, as clients without multiline-token support expect.
fn semantic_tokens(text: &str) -> Vec<SemanticToken> {
    let lines = Lines::new(text);
    let total = text.chars().count();
    let (mut out, mut prev) = (Vec::new(), Position::new(0, 0));
    for t in tokens(text) {
        let token_modifiers_bitset = t.bold as u32 | (t.italic as u32) << 1;
        let mut s = t.span.start;
        while s < t.span.end {
            let l = lines.line_of(s);
            let line_end = lines.starts.get(l + 1).map_or(total, |&(c, _)| c - 1); // its `\n`
            let e = t.span.end.min(line_end);
            if e > s {
                let (a, b) = (lines.pos(s), lines.pos(e));
                let delta_line = a.line - prev.line;
                out.push(SemanticToken {
                    delta_line,
                    delta_start: if delta_line == 0 {
                        a.character - prev.character
                    } else {
                        a.character
                    },
                    length: b.character - a.character,
                    token_type: token_type(t.kind),
                    token_modifiers_bitset,
                });
                prev = a;
            }
            s = line_end + 1;
        }
    }
    out
}

/// `ink fmt` to the house style, as one edit over just the changed span (common
/// prefix/suffix trimmed, like the app's `mindiff.js`), so the editor keeps the
/// cursor, marks and folds outside it. None if already canonical.
fn format(text: &str) -> Vec<TextEdit> {
    let out = render(&parse(text), View::Edit);
    if out == text {
        return vec![];
    }
    let (a, b): (Vec<char>, Vec<char>) = (text.chars().collect(), out.chars().collect());
    let pre = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let room = a.len().min(b.len()) - pre;
    let suf = a.iter().rev().zip(b.iter().rev()).take(room).take_while(|(x, y)| x == y).count();
    let lines = Lines::new(text);
    let span = Range::new(lines.pos(pre), lines.pos(a.len() - suf));
    vec![TextEdit::new(span, b[pre..b.len() - suf].iter().collect())]
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_server::RequestId;
    use serde_json::{json, Value};

    /// Decoded (line, column, length, type name, modifiers) of `semantic_tokens`.
    fn decoded(text: &str) -> Vec<(u32, u32, u32, &'static str, u32)> {
        let (mut line, mut col) = (0, 0);
        semantic_tokens(text)
            .iter()
            .map(|t| {
                line += t.delta_line;
                col = if t.delta_line == 0 { col + t.delta_start } else { t.delta_start };
                (line, col, t.length, TOKEN_TYPES[t.token_type as usize], t.token_modifiers_bitset)
            })
            .collect()
    }

    #[test]
    fn semantic_tokens_are_single_line_utf16() {
        // The insertion crosses a soft break: one token per line. 😀 is two UTF-16
        // units, so `{+` starts at column 4, not 3.
        let got = decoded("# Hi\n\nx😀 {+a **b**\nc} y\n");
        assert_eq!(
            got,
            [
                (0, 0, 1, "heading", 0),
                (0, 2, 2, "heading", 0),
                (2, 4, 2, "marker", 0),
                (2, 6, 2, "insert", 0),
                (2, 8, 2, "marker", 0),
                (2, 10, 1, "insert", 1), // bold, still an insertion
                (2, 11, 2, "marker", 0),
                (3, 0, 1, "insert", 0), // the insertion's second line
                (3, 1, 1, "marker", 0),
            ]
        );
        // Every kind maps inside the legend.
        let src = include_str!("../../../examples/sample.ink");
        assert!(decoded(src).iter().all(|t| TOKEN_TYPES.contains(&t.3)));
    }

    #[test]
    fn utf16_positions() {
        // "é" is 1 UTF-16 unit, "😀" is 2; columns count units, lines split on \n.
        let l = Lines::new("aé😀b\nx");
        assert_eq!(l.pos(0), Position::new(0, 0));
        assert_eq!(l.pos(2), Position::new(0, 2)); // before 😀
        assert_eq!(l.pos(3), Position::new(0, 4)); // after 😀: +2
        assert_eq!(l.pos(4), Position::new(0, 5)); // end of line 0, before \n
        assert_eq!(l.pos(5), Position::new(1, 0));
        assert_eq!(l.pos(6), Position::new(1, 1)); // end of doc
    }

    /// Drive the real server loop over an in-memory connection: initialize, open
    /// a doc, ask for symbols/folds/formatting, shut down.
    #[test]
    fn end_to_end() {
        let (server, client) = Connection::memory();
        let t = std::thread::spawn(move || serve(&server).unwrap());
        let send = |m: Message| client.sender.send(m).unwrap();
        let mut next = 0;
        let mut call = |method: &str, params: Value| -> Value {
            next += 1;
            send(Request::new(RequestId::from(next), method.into(), params).into());
            match client.receiver.recv().unwrap() {
                Message::Response(r) => r.response_result.expect("ok response"),
                m => panic!("expected response, got {m:?}"),
            }
        };

        let init = call("initialize", json!({ "capabilities": {} }));
        assert_eq!(init["capabilities"]["documentFormattingProvider"], true);
        assert_eq!(init["serverInfo"]["name"], "ink");
        assert_eq!(init["serverInfo"]["version"], crate::VERSION);
        let legend = &init["capabilities"]["semanticTokensProvider"]["legend"];
        assert_eq!(legend["tokenTypes"].as_array().unwrap().len(), TOKEN_TYPES.len());
        send(Notification::new("initialized".into(), json!({})).into());

        let uri = "file:///p/ch1.ink";
        let src = "# One\n\nText 😀 here.\n\n~~ Scene\n\nMore.\n\n# Two\n";
        send(
            Notification::new(
                "textDocument/didOpen".into(),
                json!({ "textDocument": { "uri": uri, "languageId": "ink", "version": 1, "text": src } }),
            )
            .into(),
        );
        let doc = json!({ "textDocument": { "uri": uri } });

        let syms = call("textDocument/documentSymbol", doc.clone());
        assert_eq!(syms[0]["name"], "One");
        assert_eq!(syms[0]["children"][0]["name"], "Scene");
        assert_eq!(syms[0]["children"][0]["kind"], 24); // EVENT
        assert_eq!(syms[0]["selectionRange"]["start"], json!({ "line": 0, "character": 0 }));
        assert_eq!(syms[1]["name"], "Two");
        assert_eq!(syms[1]["range"]["start"]["line"], 8);

        // `# One` folds to "More." (line 6), not the blank line before `# Two`.
        let folds = call("textDocument/foldingRange", doc.clone());
        assert_eq!(folds[0]["startLine"], 0);
        assert_eq!(folds[0]["endLine"], 6);
        assert_eq!(folds[1]["startLine"], 4); // the scene
        assert_eq!(folds.as_array().unwrap().len(), 2); // `# Two` is one line: no fold

        // A borrowed `[[link]]` normalizes to the house `{&link}`; full sync replaces text.
        send(
            Notification::new(
                "textDocument/didChange".into(),
                json!({ "textDocument": { "uri": uri, "version": 2 }, "contentChanges": [{ "text": "# A\n\nSee [[bob]].\n" }] }),
            )
            .into(),
        );
        let fmt = call(
            "textDocument/formatting",
            json!({ "textDocument": { "uri": uri }, "options": { "tabSize": 2, "insertSpaces": true } }),
        );
        // Only the changed span: `[[bob]].` becomes `{&bob}.` plus the house
        // trailing blank line; `# A\n\nSee ` and the final `\n` are kept.
        assert_eq!(fmt[0]["newText"], "{&bob}.\n");
        assert_eq!(fmt[0]["range"]["start"], json!({ "line": 2, "character": 4 }));
        assert_eq!(fmt[0]["range"]["end"], json!({ "line": 2, "character": 12 }));

        // CRLF buffers are stored as LF, so a canonical CRLF doc formats clean.
        let canon = "# A\r\n\r\nSee {&bob}.\r\n\r\n";
        send(
            Notification::new(
                "textDocument/didChange".into(),
                json!({ "textDocument": { "uri": uri, "version": 3 }, "contentChanges": [{ "text": canon }] }),
            )
            .into(),
        );
        let fmt = call(
            "textDocument/formatting",
            json!({ "textDocument": { "uri": uri }, "options": { "tabSize": 2, "insertSpaces": true } }),
        );
        assert_eq!(fmt, json!([]));

        // A document the server never saw is an error, not an empty outline.
        send(
            Request::new(
                RequestId::from(999),
                "textDocument/documentSymbol".into(),
                json!({ "textDocument": { "uri": "file:///nope.ink" } }),
            )
            .into(),
        );
        match client.receiver.recv().unwrap() {
            Message::Response(r) => assert!(r.response_result.is_err()),
            m => panic!("expected response, got {m:?}"),
        }

        call("shutdown", Value::Null);
        send(Notification::new("exit".into(), Value::Null).into());
        t.join().unwrap();
    }

    /// `exit` without `shutdown` is an error, so `ink lsp` exits with code 1.
    #[test]
    fn exit_without_shutdown_fails() {
        let (server, client) = Connection::memory();
        let t = std::thread::spawn(move || serve(&server).is_err());
        client
            .sender
            .send(
                Request::new(
                    RequestId::from(1),
                    "initialize".into(),
                    json!({ "capabilities": {} }),
                )
                .into(),
            )
            .unwrap();
        client.receiver.recv().unwrap();
        client.sender.send(Notification::new("initialized".into(), json!({})).into()).unwrap();
        client.sender.send(Notification::new("exit".into(), Value::Null).into()).unwrap();
        assert!(t.join().unwrap());
    }
}
