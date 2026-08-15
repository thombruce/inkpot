# Tauri IPC surface

Text is canonical on the frontend (the editor buffer). Rust is stateless: it
only parses and renders. No document state crosses the boundary, so there is
nothing to keep in sync.

## Commands

### `outline(src: string) -> OutlineNode`

Parse `src` and return the heading tree (root included, `level: 0`).

```ts
type Span = { start: number; end: number }; // char offsets into src

type OutlineNode = {
  id: number;            // stable preorder index within one parse
  level: number;
  visible: boolean;      // '#' => true, '~' scene => false
  title: string;
  meta_keys: string[];   // metadata keys only (values stay in the source)
  heading_span: Span;    // the heading line — scroll target
  node_span: Span;       // whole subtree — cut/paste range for drag-move
  children: OutlineNode[];
};
```

### `preview(src: string) -> string`

Returns the manuscript rendered as reading-view **HTML** (`<h1>`–`<h6>`, `<p>`,
`<strong>`, `<em>`) with CriticMarkup resolved and scenes/metadata/comments
dropped. Text is escaped in Rust, so the frontend can assign it via `innerHTML`.
(The plain-text `manuscript`/`outline`/`edit` views still exist in `ink-core`
for the CLI; the app only needs this one.)

## What does NOT cross IPC

- **Syntax highlighting** — a CodeMirror language mode on the frontend
  (`inklang.js`), no per-keystroke round trip.
- **Drag-reorder** — the frontend already holds `node_span`; a move is an
  editor transaction (delete range, insert at target), then re-parse. There is
  deliberately no `reorder` command.
- **File open/save** — `tauri-plugin-fs` + `tauri-plugin-dialog`, called from JS
  via `withGlobalTauri`.

## Offsets

Spans are char (Unicode scalar) offsets, matching JS UTF-16 indexing for BMP
text. Astral chars (emoji) drift; revisit if it bites. Line offsets assume
`\n` endings — normalize CRLF on load.
