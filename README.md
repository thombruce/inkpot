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

A `key: value` block **at the very top of the file** (before any heading) is
**document front matter** — the work's own metadata (`title`, `author`,
`contact`, `byline`, …), kept in-file so an `.ink` document is self-contained.
Same rules: it must start on line 1, and a blank line ends it. Keys are free-form
(the set above is convention, not enforced).

`id:` on a `%` entity is the one **reserved** key: it gives that entity a
rename-proof handle. A `[[link]]` or metadata value matching an `id` resolves to
that entity whatever its title reads (so renaming `% Alice` to `% Alicia` doesn't
break `[[alice]]`). Ids are document-global and unique (first declaration wins);
unlike a title, an `id` never counts as an outgoing reference to another entity.

### Markup

- Visible: `**bold**`, `*italic*` — these print.
- CriticMarkup (non-destructive edits, none print except accepted insertions):
  - `{+insertion}` — accepted into the manuscript
  - `{-deletion}` — dropped from the manuscript
  - `{~old~new}` — substitution; `new` prints
  - `{/comment}` — inline comment
- `/` at the start of a line — a whole-line comment.
- `[[Target]]` — a wikilink to a codex entity (a `%` heading of that name, or
  its `id:`). Prints the entity's title in every view — `[[alice]]` shows "Alice
  Hargrove" — falling back to the raw target if nothing matches; in the codex it
  also adds the scene to that entity's backlinks. Escape a literal `[` with `\[`.

### Interpolation

`{{ … }}` in a heading or prose is resolved at render time — every view that
shows text (manuscript, HTML preview, outline rail, codex):

- `{{number}}` — this heading's 1-based position among its siblings.
- `{{total}}` — how many siblings there are.
- `{{key}}` — a metadata value: the nearest one on this node or an ancestor,
  falling back to document front matter. So `{{title}}` reaches the front matter
  anywhere.
- Integer arithmetic over the above: `+ - * / ( )` and unary `-`. E.g.
  `# Chapter {{number}} of {{total}}`, or a countup to zero with
  `# Chapter {{number - total}}`.

Numbering is **manuscript-authoritative**: excluded (`%`) siblings never consume
a number. An unresolved expression (unknown key, malformed arithmetic) is left
verbatim — a visible `{{…}}` in the page marks it unfinished, like CriticMarkup.
A `\{{` in a heading is a literal `{{`. Interpolation is not resolved inside a
`[[wikilink]]` target — that names an entity literally, so `[[{{key}}]]` stays
raw.

### Views

- **Manuscript** — print view as Markdown: visible headings (`#` by depth),
  `**`/`*` emphasis, resolved CriticMarkup; no scenes, metadata, or comments.
  Convert onward with pandoc. (The app's Export writes this.)
- **Outline** — every heading, visible and invisible, indented, with metadata keys.
- **Edit** — everything, re-serialized.
- **Codex** — the excluded (`%`) subtrees as a grouped entity index: each `%`
  section (Characters, Locations, Timeline, …) with its entries and their
  metadata. A knowledge base derived from what you already write, no new syntax.

## Repository layout

```
crates/
  ink-core/     Parser, tree, and the view renderers. Zero UI deps.
  ink-cli/      `ink render --view=… <file.ink>` — wraps ink-core.
app/
  src/          Frontend: CodeMirror editor, outline, preview, codex (Vite, no framework).
  src-tauri/    Tauri v2 desktop shell. Stateless commands over ink-core (parse, render views).
examples/       Sample .ink documents (sample.ink, codex.ink).
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
cargo run -p ink-cli -- render --view=codex      examples/codex.ink
```

### Desktop app

Requires Node and the system webview (`webkit2gtk-4.1` on Linux).

```sh
cd app
npm install
npm run tauri dev      # live dev with hot reload
npm run tauri build    # standalone build
```

The app: a live-highlighted `.ink` editor with foldable sections, a collapsible
outline panel (click to jump, drag to rearrange scenes and chapters, and toggle
any heading between visible `#` / scene `~` / excluded `%`), a formatted reading
preview toggled from the editor (rendered headings, bold, and italics — no
markers), and file open/save with autosave to the current file plus an
unsaved-changes guard on untitled buffers. It reopens your most recent file on
launch and keeps a recent-files list.

## Installing a release

Prebuilt installers are published to
[Releases](https://github.com/thombruce/inkpot/releases) — `.AppImage`/`.deb`/`.rpm`
(Linux), `.dmg` (macOS), `.msi`/`.exe` (Windows).

macOS via Homebrew:

```sh
brew install --cask thombruce/tap/inkpot
```

Early builds are **unsigned**, so the OS warns on first launch:

- **macOS** — right-click the app → **Open** (once), instead of double-clicking.
- **Windows** — on the SmartScreen prompt, **More info → Run anyway**.
- **Linux** — no warning; mark `.AppImage` executable, or install the `.deb`/`.rpm`.

## Development

```sh
cargo test                      # ink-core parser/render tests (app excluded)
node app/src/reorder.test.mjs   # drag-reorder splice self-check
node app/src/fold.test.mjs       # edit-view fold depth/section self-check
node app/src/metacomplete.test.mjs # metadata-completion zone self-check
```

The Tauri app is a separate workspace (heavy GUI deps), so `cargo test` at the
root stays fast and webkit-free.

## Status

**v0.4.0 released** — grab an installer from
[Releases](https://github.com/thombruce/inkpot/releases), or macOS via Homebrew
(above). The full loop works: write, highlight, outline, rearrange, fold,
preview, autosave, exclude sections, plus a **codex** knowledge base — a derived
index of your `%` sections (characters, locations, timeline) with
metadata/`[[wikilink]]` cross-references and per-entity backlinks
(single-document; cross-file is #28). Builds are unsigned for now. Ongoing work
is tracked as [issues](https://github.com/thombruce/inkpot/issues).
