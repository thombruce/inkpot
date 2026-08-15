//! Tauri shell: two stateless commands over `ink-core`. Text is canonical on
//! the frontend; Rust only parses and renders. See docs/ipc.md for the surface.

use ink_core::{parse, render as core_render, Node, Span, View};
use serde::Serialize;

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
    visible: bool,
    title: String,
    meta_keys: Vec<String>,
    heading_span: SpanDto,
    node_span: SpanDto,
    children: Vec<OutlineNode>,
}

fn to_outline(node: &Node, next_id: &mut usize) -> OutlineNode {
    let id = *next_id;
    *next_id += 1;
    OutlineNode {
        id,
        level: node.level,
        visible: node.visible,
        title: node.title.clone(),
        meta_keys: node.meta.iter().map(|(k, _)| k.clone()).collect(),
        heading_span: node.heading_span.into(),
        node_span: node.node_span.into(),
        // Preorder: assign this node's id before descending (matches the docs).
        children: node.children.iter().map(|c| to_outline(c, next_id)).collect(),
    }
}

/// Parse `src` and return the outline tree (root included, level 0).
#[tauri::command]
fn outline(src: String) -> OutlineNode {
    let mut next_id = 0;
    to_outline(&parse(&src), &mut next_id)
}

/// Render `src` in the given view: "manuscript" | "outline" | "edit".
#[tauri::command]
fn render(src: String, view: String) -> Result<String, String> {
    let view = match view.as_str() {
        "manuscript" => View::Manuscript,
        "outline" => View::Outline,
        "edit" => View::Edit,
        other => return Err(format!("unknown view '{other}'")),
    };
    Ok(core_render(&parse(&src), view))
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![outline, render])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
