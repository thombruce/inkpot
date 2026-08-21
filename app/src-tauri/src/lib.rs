//! Tauri shell: stateless commands over `ink-core` (parse in, render out). Text is canonical on
//! the frontend; Rust only parses and renders. See docs/ipc.md for the surface.

use ink_core::{
    parse, render, render_codex_html, render_html, resolve_titles, word_count, Node, Span, View,
    Visibility,
};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
struct SpanDto {
    start: usize,
    end: usize,
}

impl From<Span> for SpanDto {
    fn from(s: Span) -> Self {
        SpanDto { start: s.start, end: s.end }
    }
}

/// The outline tree handed to the frontend. Mirrors `ink_core::Node` minus the
/// prose body, plus a stable preorder `id` for DOM keys and drag tracking.
#[derive(Serialize)]
struct OutlineNode {
    id: usize,
    level: u8,
    /// "visible" | "scene" | "excluded".
    visibility: &'static str,
    title: String,
    meta_keys: Vec<String>,
    /// Manuscript word count of this subtree (root carries the document total).
    words: usize,
    heading_span: SpanDto,
    node_span: SpanDto,
    children: Vec<OutlineNode>,
}

fn to_outline(node: &Node, titles: &HashMap<usize, String>, next_id: &mut usize) -> OutlineNode {
    let id = *next_id;
    *next_id += 1;
    OutlineNode {
        id,
        level: node.level,
        visibility: match node.visibility {
            Visibility::Visible => "visible",
            Visibility::Scene => "scene",
            Visibility::Excluded => "excluded",
        },
        // Resolved title (matches the rendered views); the root has no heading.
        title: if node.level == 0 {
            node.title.clone()
        } else {
            titles.get(&node.heading_span.start).cloned().unwrap_or_else(|| node.title.clone())
        },
        meta_keys: node.meta.iter().map(|(k, _)| k.clone()).collect(),
        words: word_count(node),
        heading_span: node.heading_span.into(),
        node_span: node.node_span.into(),
        // Preorder: assign this node's id before descending (matches the docs).
        children: node.children.iter().map(|c| to_outline(c, titles, next_id)).collect(),
    }
}

/// Parse `src` and return the outline tree (root included, level 0).
#[tauri::command]
fn outline(src: String) -> OutlineNode {
    let root = parse(&src);
    let titles = resolve_titles(&root);
    let mut next_id = 0;
    to_outline(&root, &titles, &mut next_id)
}

/// Render `src` as a reading-view manuscript in HTML.
#[tauri::command]
fn preview(src: String) -> String {
    render_html(&parse(&src))
}

/// Render `src` as the plain-text manuscript, for export.
#[tauri::command]
fn manuscript(src: String) -> String {
    render(&parse(&src), View::Manuscript)
}

/// Render the codex — the excluded (`%`) subtrees — as HTML for the codex panel.
#[tauri::command]
fn codex(src: String) -> String {
    render_codex_html(&parse(&src))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![outline, preview, manuscript, codex])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
