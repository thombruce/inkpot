//! `ink lsp` — a Language Server Protocol server over `ink-core` (#141), so any
//! LSP editor (Neovim, Helix, VS Code, Zed) gets inkpot features from the real
//! parser, with no editor-side mirror. Speaks LSP over stdio.
//!
//! Stateless in spirit, like the app: the only state is the open buffers' text
//! (full-document sync), and every request re-parses. Slice 1: document symbols
//! (the outline), folding ranges, and formatting (`ink fmt`'s house style).

use ink_core::{parse, render, resolve_titles, word_count, Node, Span, View, Visibility};
use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Notification as _,
};
use lsp_types::request::{DocumentSymbolRequest, FoldingRangeRequest, Formatting, Request as _};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentFormattingParams, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse,
    FoldingRange, FoldingRangeParams, OneOf, Position, Range, ServerCapabilities, SymbolKind,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit,
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
        ..Default::default()
    };
    conn.initialize(serde_json::to_value(caps)?)?;

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
            Message::Notification(n) => on_notification(&mut docs, n),
            Message::Response(_) => {} // we send no requests
        }
    }
    Ok(())
}

fn on_notification(docs: &mut HashMap<String, String>, n: Notification) {
    match n.method.as_str() {
        DidOpenTextDocument::METHOD => {
            if let Ok(p) = n.extract::<DidOpenTextDocumentParams>(DidOpenTextDocument::METHOD) {
                docs.insert(p.text_document.uri.to_string(), p.text_document.text);
            }
        }
        DidChangeTextDocument::METHOD => {
            // Full sync: the last change carries the whole document.
            if let Ok(mut p) =
                n.extract::<DidChangeTextDocumentParams>(DidChangeTextDocument::METHOD)
            {
                if let Some(c) = p.content_changes.pop() {
                    docs.insert(p.text_document.uri.to_string(), c.text);
                }
            }
        }
        DidCloseTextDocument::METHOD => {
            if let Ok(p) = n.extract::<DidCloseTextDocumentParams>(DidCloseTextDocument::METHOD) {
                docs.remove(p.text_document.uri.as_str());
            }
        }
        _ => {}
    }
}

fn respond(docs: &HashMap<String, String>, req: Request) -> Response {
    let id = req.id.clone();
    let text = |uri: &lsp_types::Uri| docs.get(uri.as_str()).map(String::as_str).unwrap_or("");
    let result = match req.method.as_str() {
        DocumentSymbolRequest::METHOD => {
            req.extract::<DocumentSymbolParams>(DocumentSymbolRequest::METHOD).map(|(_, p)| {
                serde_json::to_value(DocumentSymbolResponse::Nested(symbols(text(
                    &p.text_document.uri,
                ))))
            })
        }
        FoldingRangeRequest::METHOD => req
            .extract::<FoldingRangeParams>(FoldingRangeRequest::METHOD)
            .map(|(_, p)| serde_json::to_value(folds(text(&p.text_document.uri)))),
        Formatting::METHOD => req
            .extract::<DocumentFormattingParams>(Formatting::METHOD)
            .map(|(_, p)| serde_json::to_value(format(text(&p.text_document.uri)))),
        _ => {
            return Response::new_err(
                id,
                lsp_server::ErrorCode::MethodNotFound as i32,
                format!("unhandled method {}", req.method),
            )
        }
    };
    match result {
        Ok(Ok(v)) => Response::new_ok(id, v),
        _ => {
            Response::new_err(id, lsp_server::ErrorCode::InvalidParams as i32, "bad params".into())
        }
    }
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

/// `ink fmt`: one whole-document edit to the house style, or none if it's
/// already canonical.
fn format(text: &str) -> Vec<TextEdit> {
    let out = render(&parse(text), View::Edit);
    if out == text {
        return vec![];
    }
    let lines = Lines::new(text);
    let end = lines.pos(text.chars().count());
    vec![TextEdit::new(Range::new(Position::new(0, 0), end), out)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_server::RequestId;
    use serde_json::{json, Value};

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
        assert_eq!(fmt[0]["newText"], "# A\n\nSee {&bob}.\n\n");
        assert_eq!(fmt[0]["range"]["end"], json!({ "line": 3, "character": 0 }));

        call("shutdown", Value::Null);
        send(Notification::new("exit".into(), Value::Null).into());
        t.join().unwrap();
    }
}
