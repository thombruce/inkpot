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
  visibility: "visible" | "scene" | "excluded"; // '#' / '~' / '%'
  title: string;
  meta_keys: string[];   // metadata keys only (values stay in the source)
  words: number;         // manuscript word count of this subtree (root = doc total)
  heading_span: Span;    // the heading line — scroll target
  node_span: Span;       // whole subtree — cut/paste range for drag-move
  children: OutlineNode[];
};
```

### `preview(src: string) -> string`

Returns the manuscript rendered as reading-view **HTML** (`<h1>`–`<h6>`, `<p>`,
`<strong>`, `<em>`) with CriticMarkup resolved and scenes/metadata/comments
dropped. Text is escaped in Rust, so the frontend can assign it via `innerHTML`.
(The plain-text `outline`/`edit` views still exist in `ink-core` for the CLI;
the app renders HTML for preview and plain text for export.)

### `manuscript(src: string) -> string`

Returns the manuscript as **Markdown** (visible headings as `#`-by-depth ATX,
`**`/`*` emphasis, CriticMarkup resolved, scenes/metadata/comments and excluded
subtrees dropped) — the `ink-core` `View::Manuscript` render, for the frontend's
Export. Convert to docx/PDF/etc. downstream with pandoc.

### `codex(src: string) -> string`

Returns the **codex** as HTML for the app's codex panel: the excluded (`%`)
subtrees rendered as a grouped entity index. Each top-level `%` section is a
`<section class="codex-section">`; entries nest as `<article class="entity">`
with an `<h2>`–`<h6>` name (by depth), a `<dl>` of their metadata, body prose as
`<p>`, and a "Referenced by" backlink list. A name that resolves to an entity —
a metadata value (comma-split, trimmed, case-folded) or a prose `[[wikilink]]` —
becomes a backlink; resolved metadata values and each backlink render as
`<a class="ref" data-jump="<char-offset>">`. The frontend reads
`data-jump` to scroll the editor to that heading (see `main.js`). Text is
escaped, so the frontend assigns it via `innerHTML`. The codex (issue #9) is
derived from what authors already write, no new syntax. (The plain-text
`View::Codex` render — no links — backs `ink render --view=codex` for the CLI.)

### `timeline(src: string) -> string`

Returns the **timeline** as HTML for the app's timeline panel: an `<ol class="timeline">`
of every heading that carries a `time:` metadata value, ordered by that value.
If every value is a number, they sort numerically (so signed year counts — a
cosmic timeline from `-13700000000` onward — order correctly); otherwise they
sort lexically, which keeps ISO `YYYY-MM-DD` dates chronological. Each `<li>` is a `<time>`
plus an `<a class="ref" data-jump="<char-offset>">` linking to the heading (the
frontend reads `data-jump` to scroll the editor there, same as the codex).
Headings without a `time:` value don't appear. Text is escaped for `innerHTML`.

### `characters(src: string) -> string`

Returns the **character panel** as HTML: the `%` section whose title case-folds to
`characters`, rendered as entity cards with the same per-entity markup as `codex`
(`<section class="codex-section">`, `<article class="entity">`, `<dl>` metadata,
body `<p>`, "Referenced by" backlinks, `data-jump` links) — but **without** the
`codex-scope` breadcrumb (the panel is a focused cast list, so the visible-ancestor
context is dropped). Other `%` sections are omitted; empty (no output) if there is
no `% Characters` section. Text is escaped for `innerHTML`. The panel's one write —
scaffolding a new `%% Name` entry — is a frontend text splice (`app/src/character.js`),
not an IPC call; text stays canonical.

### `map(src: string) -> Marker[]`

Returns the map markers as **structured JSON** (not HTML — Leaflet places markers
from coordinates):

```ts
type Marker = {
  title: string;   // the entity's resolved title
  lat: number;     // decimal degrees
  lon: number;
  offset: number;  // heading char offset — the frontend jumps the editor here
};
```

Every codex entity (a `%` heading with a title) that carries a parseable
`coords: <lat>, <lon>` value becomes a marker; entities without valid coordinates
(missing, unparseable, or out of geographic range) are omitted. Not filtered to a
`% Locations` section — any entity with coordinates is placeable. The map view is
read-only; the OpenStreetMap basemap needs network (markers still position
offline, without tile imagery).

### `scenes(src: string) -> Scene[]`

Returns the time-ordered scenes for the **time-scrub** (the map's time slider):

```ts
type Scene = {
  time: string;         // the raw `time:` value
  title: string;        // resolved heading title
  location: string;     // the `location:` value (may be "")
  characters: string[]; // the `characters:` value, comma-split
  exits: string[];      // the `exits:` value — characters who leave here
  offset: number;       // heading char offset
};
```

Every heading with a `time:` value, ordered like the timeline (numeric if all
values are numbers, else lexical). The frontend joins `location` to a `map`
marker by folded name to get coordinates, then walks the list up to the cursor
index to place each character at their most-recent located scene; a scene's
`exits` marks a character's last appearance — shown in that scene, dropped from
the next one on. The join and per-cursor computation are client-side
(`app/src/timescrub.js`) — no IPC per slider tick.

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
