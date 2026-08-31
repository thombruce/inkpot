# CLAUDE.md

Guidance for working in this repo. Read the README for the `.ink` format and
`docs/ipc.md` for the Tauri command surface.

## What this is

inkpot is a plain-text prose format (`.ink`) plus a Tauri desktop editor. A
document parses into a `Node` tree with a **shared hierarchy of visible (`#`),
scene (`~`), and excluded (`%`) headings** (a three-state `Visibility`), inline
metadata, comments, and CriticMarkup. Three views render from the tree:
manuscript, outline, edit. Excluded (`%`) subtrees drop from the manuscript but
stay in outline/edit.

## Layout & boundaries

- `crates/ink-core/` — the real work. Parser + tree + renderers, zero UI deps.
  Everything downstream depends on this; keep it UI-agnostic and pure.
- `crates/ink-cli/` — thin CLI wrapper.
- `app/src-tauri/` — Tauri v2 shell. Its own workspace (root `Cargo.toml`
  `exclude`s `app`), so `cargo test` stays fast and needs no webkit.
- `app/src/` — bundler-built frontend (Vite). No JS framework. Layout is a
  flex row: the outline rail plus a shared main area that shows **one** of
  editor / preview / codex / timeline / characters / map at a time. The toolbar
  toggles a `body.show-*` class per view (`show-preview`, `show-codex`,
  `show-timeline`, `show-characters`, `show-map`); the outline rail collapses
  (`body.hide-outline`). All pure CSS class toggles in `main.js`/`style.css` —
  no grid tracks to juggle.
- **Adding a planning view follows one pattern** (timeline/characters/map are
  the examples): a pure projection in `ink-core` (`render_timeline_html`,
  `render_characters_html`, `map_markers`) → a stateless `#[tauri::command]` →
  a panel + toolbar toggle, refreshed each parse in `main.js`'s `refresh()`.
  Views that emit `data-jump` offsets share one click-to-scroll handler. Read
  the view keys from `ink-core::meta` (below), never scattered string literals.

## Commands

```sh
cargo test                                    # core tests (from repo root)
cargo run -p ink-cli -- render --view=edit examples/sample.ink
node app/src/reorder.test.mjs                 # reorder splice self-check
node app/src/fold.test.mjs                     # fold depth/section self-check
node app/src/metacomplete.test.mjs             # metadata-completion zone self-check
node app/src/character.test.mjs                # new-character scaffold splice self-check
node app/src/timescrub.test.mjs                # time-scrub character-position self-check
node app/src/mapproviders.test.mjs             # map-world folding + providers self-check
node app/src/filetree.test.mjs                 # project .ink tree build/prune/sort self-check
cd app && npm run tauri dev                   # run the app (needs a display)
cd app && npm run build                       # frontend only -> app/dist
```

There is **no `cargo-tauri`-free way to `cargo run` the app in debug**:
`devUrl` is set, so a debug build expects the Vite dev server that
`beforeDevCommand` starts — use `npm run tauri dev`.

## Architecture decisions (don't relitigate)

- **Text is canonical.** The editor buffer is the source of truth; Rust is
  stateless (parse in, render out). No document state crosses IPC. A scene move
  is a text splice, not a tree mutation — hence there is deliberately **no
  reorder command**; see `app/src/reorder.js`.
- **Autosave is path-gated.** A debounced write to `currentPath` fires once
  edits settle (`main.js`); untitled/example buffers have no path, so they are
  never written — the example doc never hits disk. A Tauri `onCloseRequested`
  guard flushes the autosave then prompts before discarding an unsaved untitled
  buffer. Note `onCloseRequested`'s default action is a JS `window.destroy()`,
  not a native close — so the capability must grant `core:window:allow-destroy`
  (in `capabilities/default.json`), or the app silently won't quit. Crash
  recovery of untitled drafts (app-data snapshots) is #13.
- **A project is a folder + a derived `.ink` tree, app-layer only** (#8). The root
  is found by walking up from the active file to the nearest `Inkpot` marker file
  (`syncProject`/`findRoot`, needs `fs:allow-exists`), else the file's own
  directory. So opening a file nested in a project shows the whole project; a
  loose file is its own one-folder project. Open Folder writes the `Inkpot` marker
  (extension-less, so it never shows in the tree; its front-matter content is
  reserved for future project settings — decided on #8, no ordering manifest,
  files order by name). `buildTree` (`filetree.js`, pure, `fs.readDir` injected,
  needs `fs:allow-read-dir`) walks the root into a nested tree; the rail renders it
  as collapsible `<details>`. Rescan fires on load, save, and window focus.
  Picking a file calls the same `loadPath` single-file uses, so `currentPath`
  stays the one active buffer; no project state crosses IPC, `ink-core` stays
  file-agnostic. Deferred: project settings in the marker, cross-file codex (#28).
- **Heading depth = marker count** (Model A). `#`/`~` set visibility, the count
  sets depth. Illegal downward jumps clamp to parent + 1 (`parse.rs`) — but only
  against a *real heading parent*, never the implicit root, so a document that
  opens deep (e.g. all `##`) keeps its headings as same-level siblings instead
  of demoting the first and nesting the rest.
- **Spans are char (Unicode scalar) offsets**, to match JS string indexing.
  They agree with CodeMirror positions for BMP text; astral chars (emoji)
  drift — a known, accepted edge. Line offsets assume `\n` (normalize CRLF).
- **The CodeMirror language mirrors the Rust parser** (`app/src/inklang.js`,
  a `StreamLanguage`). Change one, change the other. `app/src/fold.js` (fold
  service, heading depth off the marker run) and `app/src/metacomplete.js`
  (metadata-completion zone detection: heading + `key:` rules) mirror it too — a
  heading- or meta-rule change touches all of them.
- **Frontend uses `withGlobalTauri`** — `window.__TAURI__.{core,dialog,fs}`, no
  `@tauri-apps/api`/plugin npm packages. Keep it that way unless a global is
  missing. Bundled *rendering* deps are a separate matter: CodeMirror, and
  **Leaflet** (the map view) — the one non-editor rendering dep, pulling OSM
  tiles over the network. The map is the only feature that isn't fully offline.
- **Reserved metadata keys live in one place, `ink-core::meta`** — `id` (the
  self-naming handle) plus the view keys that have earned core behavior (`time`
  for the timeline, `coords` for the map). Behavior references these, not
  `k == "id"` literals; a view key gets a constant only once core reads it. The
  editor's key-completion seed (`SCENE_KEYS` in `metacomplete.js`) is the
  frontend mirror — add a new suggested key there too.

## Conventions

- Parsers are hand-written line/inline scanners on purpose — no parser crates.
- Non-trivial pure logic gets one runnable self-check (see
  `reorder.test.mjs`, `fold.test.mjs`, `ink-core/tests/parse.rs`). No test
  frameworks.
- Commit style: imperative subject, a short body explaining why, and a
  `Co-Authored-By` trailer.
- Commit or push only when asked.

## Releasing

Tag `vX.Y.Z` and push it → `.github/workflows/release.yml` builds on a 3-OS
matrix (macos universal, ubuntu, windows — Tauri bundles natively) via
`tauri-apps/tauri-action` and publishes installers (`.AppImage`/`.deb`/`.rpm`,
universal `.dmg`, `.msi`/NSIS) to **GitHub Releases** as a **draft prerelease**
— review, then publish (`gh release edit vX.Y.Z --draft=false`). Builds are
**unsigned** (signing #10, updater #11 tracked separately). macOS is also a
Homebrew cask in `thombruce/homebrew-tap` (`Casks/inkpot.rb`); publishing a
release fires `.github/workflows/homebrew.yml`, which recomputes the dmg
`sha256` and bumps the cask's `version` + `sha256` automatically (needs the
`HOMEBREW_TAP_TOKEN` secret — a PAT with write access to the tap). No manual
bump. First release: v0.1.0.

The release **version is single-sourced in `app/src-tauri/Cargo.toml`**
(`tauri.conf.json` omits `version` and inherits it). Bump it there per
release and tag `vX.Y.Z`. SemVer, mapped to the format: stay in `0.x` until
the `.ink` format stabilises; `1.0`+ MAJOR = a format-breaking change.
`ink-core`/`ink-cli` versions are separate from the app's release version.

Log user-facing changes under `## [Unreleased]` in `CHANGELOG.md` as you go;
when tagging, rename that heading to the new version + date and paste it into
the release notes (the workflow's `releaseBody` is static boilerplate).

## Known rough edges

Open work is tracked in GitHub issues. One parser gotcha to keep in mind: the
`.ink` parser silently drops a mis-classified metadata line — the
blank-line-after-heading convention (documented in the README) avoids it.

The meta zone also opens at the **top of the file**: a leading `key: value`
block is document front matter, parsed onto the root node (`title`, `author`,
etc.). Same three mirrors as any parser change — `parse.rs` (`in_meta` starts
true), `inklang.js` (`inMeta: true`), and the `edit` renderer (round-trips root
meta). A file that opens with a colon-bearing prose line is the same
misclassification trap; front matter must start on line 1.
