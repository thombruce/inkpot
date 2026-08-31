# Changelog

Notable changes per release. Format follows
[Keep a Changelog](https://keepachangelog.com/). Versions use SemVer mapped to
the `.ink` format (see `CLAUDE.md`): `0.x` until the format stabilises, then
`1.0`+ MAJOR marks a format-breaking change.

## [Unreleased]

## [0.10.0] - 2026-08-31

### Added

- **Projects / multi-file**: open a folder of `.ink` files as a project. Its files
  show as a collapsible tree (subdirectories and all) at the top of the outline
  rail; click one to switch the active file. Open Folder marks the folder with an
  `Inkpot` file, so opening any file nested inside it later brings up the whole
  project. Opening, saving, or adding files keeps the tree in step. Reuses the
  existing editor/save/autosave — single-file open still works unchanged.

## [0.9.0] - 2026-08-31

### Added

- **Multiple maps**: a location's `%` note can name its world with `map:` (e.g.
  `map: Mars`), and the map view gets a world selector that switches the tile
  backdrop and shows only that world's locations (the time-scrub follows suit).
  **Mars** and the **Moon** ship as built-in worlds alongside Earth (via
  OpenPlanetaryMap tiles); a location with no `map:` stays on Earth as before.

## [0.8.0] - 2026-08-29

### Added

- **Time-scrub on the map**: a time slider steps through your scenes in
  story-time order, placing each character at their most recent `location:` as
  the cursor advances — so you can watch the cast move across the map. Give
  scenes a `time:`, `location:`, and `characters:`, and locations a `coords:`.
  The slider walks scenes in order, so it works with any `time:` format. A scene
  can also list `exits:` — characters whose last scene this is (death,
  departure) — shown in that scene, then off the map from the next one on.

## [0.7.1] - 2026-08-29

### Fixed

- The **timeline sorts all-numeric `time:` values as numbers**, so a timeline of
  years — including negatives and very large magnitudes (e.g. `-13700000000` for
  the Big Bang) — orders correctly. Previously all values sorted as text, which
  mis-ordered unpadded numbers and inverted negatives. ISO `YYYY-MM-DD` dates are
  unaffected (they still sort chronologically as text).

## [0.7.0] - 2026-08-29

### Added

- `id:` metadata gives a section a rename-proof handle: codex references
  (`[[wikilinks]]` and metadata values) resolve to an entity by its `id`
  regardless of its title, so renaming a `%` entity doesn't break links to it.
  `id` is document-global and reserved — it names the section itself, never an
  outgoing reference.
- `[[wikilinks]]` now print the **resolved title** of the entity they point to in
  every view (manuscript, HTML preview, codex), falling back to the raw target
  when nothing matches. So `[[alice]]` reads "Alice Hargrove" in the manuscript,
  not the raw handle.
- Metadata values can **span multiple lines**: leave the value after the colon
  empty and put the text on the following indented lines (e.g. a postal
  `contact:` block). They join into one value until a blank or non-indented line.
- A **comma-separated metadata value is a list** — `characters: Alice, Bob`
  resolves each part on its own, so both link to their `%` entities. (Already the
  behaviour; now documented and covered by a test.)
- **Autocomplete codex entity names** in a scene's metadata values: typing after
  a key like `characters:` or `location:` (or after a comma) suggests the titles
  of your `%` entities, so references stay consistent without retyping names.
- **Timeline view** (toolbar → Timeline): every heading with a `time:` value,
  listed in story-time order — ISO dates (`YYYY-MM-DD`) sort chronologically.
  Click an entry to jump to that scene in the editor.
- **Characters panel** (toolbar → Characters): your `% Characters` section as a
  focused card list — each character's fields, notes, and backlinks — with a
  **+ New character** button that scaffolds a `%% Name` template into the section.
  Click a character to jump to its note.
- **Map view** (toolbar → Map): locations placed on an OpenStreetMap map. Give a
  location's `%` note a `coords: <lat>, <lon>` value and it appears as a marker;
  click a marker to jump to the note. Read-only for now; the basemap needs a
  network connection.

## [0.6.0] - 2026-08-22

### Added

- Codex entries nested under a visible (`#`/`~`) heading now show a scope
  breadcrumb of their ancestor titles, so repeated notes (e.g. a `%% Synopsis`
  per chapter) read distinctly instead of as identical entries.
- Codex references (`[[wikilinks]]` and metadata values) resolve to the
  **nearest** same-named note by scope: a `[[Synopsis]]` inside a chapter links
  that chapter's Synopsis, not the first one declared. Root-level entities still
  resolve from anywhere.

## [0.5.0] - 2026-08-21

### Added

- `{{ … }}` interpolation in headings and prose, resolved at render time:
  `{{number}}`/`{{total}}` for manuscript-authoritative heading numbering,
  metadata keys (cascading to front matter), and integer arithmetic over them
  (e.g. `# Chapter {{number - total}}`). Unknown expressions stay verbatim.

## [0.4.0] - 2026-08-18

### Added

- **Codex view** — a knowledge-base index of the excluded (`%`) subtrees:
  each `%` section (Characters, Locations, Timeline, …) with its entries and
  their metadata. A toolbar toggle opens it beside the editor; `ink render
  --view=codex` renders it as plain text. Derived from what you already write,
  no new syntax (stage 1 of the codex epic, #9).
- **Codex cross-references** — a metadata value that names an entity (e.g.
  `location: London`, `characters: Alice`) becomes a link in the codex, and each
  entity lists the scenes and entries that reference it ("Referenced by …").
  Clicking a link jumps the editor to that heading. Resolution is
  comma-aware and case-insensitive (stage 2 of #9).
- **`[[wikilinks]]`** — reference a codex entity from prose. `[[Alice]]` prints
  as the bare name in the manuscript and adds the scene to Alice's codex
  backlinks ("scenes mentioning Alice"). Highlighted in the editor and styled in
  the reading preview; escape a literal `[` with `\[` (stage 3 of #9).

## [0.3.1] - 2026-08-16

### Added

- Tab accepts the highlighted metadata-key completion (Enter already did).

### Fixed

- A document opening at a deeper heading (e.g. all `##`) no longer demotes its
  first heading to level 1 and nests the rest beneath it — the depth clamp only
  applies against a real heading parent, so same-depth headings stay siblings
  (and render at the same size).

## [0.3.0] - 2026-08-16

### Added

- Find/replace in the editor (#15).
- Export the manuscript as Markdown, via a save dialog (#20).
- Live word count — document total in the toolbar and a per-section count in
  the outline (#16).
- Recent-files list, and reopening the last-edited file on launch (#17).
- Document front matter: a leading `key: value` block (title, author, …) parsed
  as document-level metadata, kept in-file (#24).
- Metadata key autocomplete in the meta zones — document front matter
  (title/author/…) and heading meta blocks (pov/time/…) (#22).

### Changed

- Manuscript render is now valid Markdown: visible headings emit `#`-by-depth
  ATX headings (previously stripped to bare titles) (#20).
- Launch to a blank buffer or the last file instead of the built-in example
  document (#17).

## [0.2.0] - 2026-08-16

### Added

- Autosave to the current file once edits settle, with a window-close guard that
  prompts before discarding an unsaved untitled buffer.
- Editor/preview toggle and a collapsible outline, replacing the fixed
  three-column split.
- Section folding in the edit view.

## [0.1.0] - 2026-08-15

Initial release: the `.ink` parser and tree, three views
(manuscript/outline/edit), the Tauri desktop editor with live highlighting,
drag-reorder, inline metadata, comments, CriticMarkup, and excluded (`%`)
sections. Prebuilt installers for Linux/macOS/Windows; macOS Homebrew cask.

[Unreleased]: https://github.com/thombruce/inkpot/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/thombruce/inkpot/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/thombruce/inkpot/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/thombruce/inkpot/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/thombruce/inkpot/releases/tag/v0.1.0
