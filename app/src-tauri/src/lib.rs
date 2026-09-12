//! Tauri shell: stateless commands over `ink-core` (parse in, render out). Text is canonical on
//! the frontend; Rust only parses and renders. See docs/ipc.md for the surface.

use ink_core::shunn::render_shunn_pdf;
use ink_core::{
    build_shunn_book, build_shunn_project, map_markers, parse, render_bibliography_project,
    render_characters_html, render_codex_project_html, render_html_project,
    render_manuscript_project, render_timeline_html, resolve_titles, scene_timeline, word_count,
    Node, Span, Visibility,
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

/// Render the active file as a reading-view manuscript in HTML, resolving
/// `[[links]]`/`[@cites]` across the project bundle (#89).
#[tauri::command]
fn preview(files: Vec<ProjectFile>, active: String) -> String {
    let parsed = parse_bundle(files);
    let idx = active_index(&parsed, &active);
    let roots: Vec<&Node> = parsed.iter().map(|(_, n)| n).collect();
    render_html_project(&roots, idx)
}

/// Render the active file as the plain-text manuscript, for export — project-wide
/// reference resolution (#89).
#[tauri::command]
fn manuscript(files: Vec<ProjectFile>, active: String) -> String {
    let parsed = parse_bundle(files);
    let idx = active_index(&parsed, &active);
    let roots: Vec<&Node> = parsed.iter().map(|(_, n)| n).collect();
    render_manuscript_project(&roots, idx)
}

/// One file of the project bundle: its path (the jump target for cross-file
/// references) and its current source (the live editor buffer for the active
/// file, disk text for the rest — the frontend decides). Shared by every command
/// that resolves references across the project (codex, preview, manuscript,
/// bibliography, PDF export).
#[derive(serde::Deserialize)]
struct ProjectFile {
    path: String,
    src: String,
}

/// Parse a bundle into `(path, Node)` pairs, preserving order.
fn parse_bundle(files: Vec<ProjectFile>) -> Vec<(String, Node)> {
    files.into_iter().map(|f| (f.path, parse(&f.src))).collect()
}

/// The index of the active file in a parsed bundle (matched by path), or 0.
fn active_index(parsed: &[(String, Node)], active: &str) -> usize {
    parsed.iter().position(|(p, _)| p == active).unwrap_or(0)
}

/// Render the project codex — every file's excluded (`%`) subtrees — as HTML,
/// with references and backlinks resolved across files. Stateless: the whole
/// project bundle is the argument, re-sent each refresh; no document state is
/// held. A loose single file is just a project of one.
#[tauri::command]
fn codex_project(files: Vec<ProjectFile>) -> String {
    let parsed = parse_bundle(files);
    let docs: Vec<(String, &Node)> = parsed.iter().map(|(p, n)| (p.clone(), n)).collect();
    render_codex_project_html(&docs)
}

/// Render the timeline — headings with a `time:` value, time-ordered — as HTML.
#[tauri::command]
fn timeline(src: String) -> String {
    render_timeline_html(&parse(&src))
}

/// Render the character panel — the `% Characters` section's entities — as HTML.
#[tauri::command]
fn characters(src: String) -> String {
    render_characters_html(&parse(&src))
}

/// Render the bibliography — a Harvard reference list of the sources this file
/// cites via `[@key]` — as HTML for the bibliography panel.
#[tauri::command]
fn bibliography(files: Vec<ProjectFile>, active: String) -> String {
    let parsed = parse_bundle(files);
    let idx = active_index(&parsed, &active);
    let docs: Vec<(String, &Node)> = parsed.iter().map(|(p, n)| (p.clone(), n)).collect();
    render_bibliography_project(&docs, idx)
}

/// A location marker for the map view: title, position, and jump offset.
#[derive(Serialize)]
struct Marker {
    title: String,
    lat: f64,
    lon: f64,
    map: String,
    offset: usize,
}

/// Location entities with a parseable `coords:` value, as map markers.
#[tauri::command]
fn map(src: String) -> Vec<Marker> {
    map_markers(&parse(&src))
        .into_iter()
        .map(|m| Marker { title: m.title, lat: m.lat, lon: m.lon, map: m.map, offset: m.offset })
        .collect()
}

/// A scene for the time-scrub: time, title, location, characters, jump offset.
#[derive(Serialize)]
struct Scene {
    time: String,
    title: String,
    location: String,
    characters: Vec<String>,
    exits: Vec<String>,
    offset: usize,
}

/// Time-ordered scenes (headings with a `time:` value) with their `location:`,
/// `characters:`, and `exits:`, for the time-scrub.
#[tauri::command]
fn scenes(src: String) -> Vec<Scene> {
    scene_timeline(&parse(&src))
        .into_iter()
        .map(|s| Scene {
            time: s.time,
            title: s.title,
            location: s.location,
            characters: s.characters,
            exits: s.exits,
            offset: s.offset,
        })
        .collect()
}

/// Render the active file to a Shunn manuscript PDF and write it to `path`,
/// resolving `[[links]]`/`[@cites]` across the project bundle (#89). Bytes are
/// written from Rust (genpdf), so no PDF data crosses IPC.
#[tauri::command]
fn export_shunn(files: Vec<ProjectFile>, active: String, path: String) -> Result<(), String> {
    let parsed = parse_bundle(files);
    let idx = active_index(&parsed, &active);
    let roots: Vec<&Node> = parsed.iter().map(|(_, n)| n).collect();
    let bytes = render_shunn_pdf(&build_shunn_project(&roots, idx))?;
    std::fs::write(&path, bytes).map_err(|e| format!("{path}: {e}"))
}

/// Render a whole project to one Shunn book PDF: `sources` are the project's
/// files in order, `marker` is the `Inkpot` marker's text (its front matter is
/// the work's title-page metadata, falling back to the first file). Bytes are
/// written from Rust — no PDF crosses IPC.
#[tauri::command]
fn export_shunn_book(sources: Vec<String>, marker: String, path: String) -> Result<(), String> {
    let docs: Vec<Node> = sources.iter().map(|s| parse(s)).collect();
    let refs: Vec<&Node> = docs.iter().collect();
    let bytes = render_shunn_pdf(&build_shunn_book(&parse(&marker), &refs))?;
    std::fs::write(&path, bytes).map_err(|e| format!("{path}: {e}"))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            outline, preview, manuscript, codex_project, timeline, characters, bibliography, map,
            scenes, export_shunn, export_shunn_book
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
