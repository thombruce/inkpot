# Changelog

Notable changes per release. Format follows
[Keep a Changelog](https://keepachangelog.com/). Versions use SemVer mapped to
the `.ink` format (see `CLAUDE.md`): `0.x` until the format stabilises, then
`1.0`+ MAJOR marks a format-breaking change.

## [Unreleased]

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
