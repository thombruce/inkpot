# inkpot

[![CI](https://github.com/thombruce/inkpot/actions/workflows/ci.yml/badge.svg)](https://github.com/thombruce/inkpot/actions/workflows/ci.yml)

A writing format and desktop app for prose (and poetry) that keeps a whole
work — novel, short story, article — in one continuous document, while letting
you rearrange its parts, annotate invisibly, and edit non-destructively.

The idea: a single plain-text format with **visible headings** (chapters,
sections that go to print) and **invisible headings** (scenes, notes that don't)
sharing one hierarchy, plus inline metadata, comments, and edit-tracking. From
that one source you generate a clean manuscript, a structural outline, or a
full editing view.

Plain text, git-friendly, no lock-in.

## The `.ink` format

```
# Chapter 1

## The Arrival

~~~ The Kitchen
time: dawn
pov: Alice
characters: Alice, Bob

She stood at the counter. {+Steam rose from the kettle.} The window was
{~grey~pale with morning}. {/is this too early in the timeline?}

/ remember to seed the argument here

They passed in the **narrow** hall without a *word*.
```

### Headings — one shared hierarchy

| Marker | Meaning | Prints? |
|--------|---------|---------|
| `#`, `##`, `###`, … | Visible heading (chapter, section) | yes |
| `~`, `~~`, `~~~`, … | Invisible heading (scene, beat) | no (body still prints) |
| `%`, `%%`, `%%%`, … | Excluded section (notes, cut drafts) | no (whole subtree omitted) |

The **count** is the depth, regardless of marker — a `~~~` scene nests inside a
`##` section, a `~~` scene is a peer of a `##` section. An illegal jump (e.g.
level 1 straight to level 4) is clamped to parent + 1.

A `%` section keeps its heading, body, and every nested child in the document
(they show in the outline and edit views) but omits the whole subtree from the
manuscript — for research notes, or cutting a scene while keeping it for later.

### Metadata

`key: value` lines **directly beneath a heading** are metadata (scene time, POV,
characters, …). Never printed. **A blank line ends the metadata block** — prose
that could look like `key: value` must be separated from its heading by a blank
line (as it normally would be).

### Markup

- Visible: `**bold**`, `*italic*` — these print.
- CriticMarkup (non-destructive edits, none print except accepted insertions):
  - `{+insertion}` — accepted into the manuscript
  - `{-deletion}` — dropped from the manuscript
  - `{~old~new}` — substitution; `new` prints
  - `{/comment}` — inline comment
- `/` at the start of a line — a whole-line comment.

### Views

- **Manuscript** — print view: visible headings + resolved CriticMarkup; no
  scenes, metadata, or comments.
- **Outline** — every heading, visible and invisible, indented, with metadata keys.
- **Edit** — everything, re-serialized.

## Repository layout

```
crates/
  ink-core/     Parser, tree, and the three view renderers. Zero UI deps.
  ink-cli/      `ink render --view=… <file.ink>` — wraps ink-core.
app/
  src/          Frontend: CodeMirror editor, outline, preview (Vite, no framework).
  src-tauri/    Tauri v2 desktop shell. Two stateless commands over ink-core.
examples/       Sample .ink document.
docs/ipc.md     The Tauri IPC contract.
```

Text is canonical: the editor buffer is the source of truth. Rust only parses
and renders — nothing document-shaped is held as mutable state, so there is
nothing to keep in sync. Rearranging a scene is a text splice, not a tree edit.

## Usage

### CLI

```sh
cargo run -p ink-cli -- render --view=manuscript examples/sample.ink
cargo run -p ink-cli -- render --view=outline    examples/sample.ink
cargo run -p ink-cli -- render --view=edit       examples/sample.ink
```

### Desktop app

Requires Node and the system webview (`webkit2gtk-4.1` on Linux).

```sh
cd app
npm install
npm run tauri dev      # live dev with hot reload
npm run tauri build    # standalone build
```

The app: a live-highlighted `.ink` editor, an outline panel (click to jump,
drag to rearrange scenes and chapters), a formatted reading preview (rendered
headings, bold, and italics — no markers), and file open/save with an
unsaved-changes guard.

## Development

```sh
cargo test                      # ink-core parser/render tests (app excluded)
node app/src/reorder.test.mjs   # drag-reorder splice self-check
```

The Tauri app is a separate workspace (heavy GUI deps), so `cargo test` at the
root stays fast and webkit-free.

## Status

Early but functional: the full loop works — write, highlight, outline,
rearrange, preview, open/save. Known rough edges are tracked as
[issues](https://github.com/thombruce/inkpot/issues) (escape rules, nested
markup, reorder seams).
