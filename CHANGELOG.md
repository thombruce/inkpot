# Changelog

Notable changes per release. Format follows
[Keep a Changelog](https://keepachangelog.com/). Versions use SemVer mapped to
the `.ink` format (see `CLAUDE.md`): `0.x` until the format stabilises, then
`1.0`+ MAJOR marks a format-breaking change.

## [Unreleased]

### Added

- Find/replace in the editor (#15).
- Export the manuscript as Markdown, via a save dialog (#20).
- Live word count — document total in the toolbar and a per-section count in
  the outline (#16).
- Recent-files list, and reopening the last-edited file on launch (#17).
- Document front matter: a leading `key: value` block (title, author, …) parsed
  as document-level metadata, kept in-file (#24).

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

[Unreleased]: https://github.com/thombruce/inkpot/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/thombruce/inkpot/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/thombruce/inkpot/releases/tag/v0.1.0
