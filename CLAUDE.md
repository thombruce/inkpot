# CLAUDE.md

Guidance for working in this repo. Read the README for the `.ink` format and
`docs/ipc.md` for the Tauri command surface.

## What this is

inkpot is a plain-text prose format (`.ink`) with **three front ends over one
core**: a Tauri desktop editor, the `ink` CLI, and `ink lsp`, a language server
that brings inkpot to any LSP editor (Neovim first). All three are maintained
products, not the app plus extras — see "Three front ends" below. A document
parses into a `Node` tree with a **shared hierarchy of visible (`#`),
scene (`~`), and excluded (`%`) headings** (a three-state `Visibility`), inline
metadata, comments, and CriticMarkup. Three views render from the tree:
manuscript, outline, edit. Excluded (`%`) subtrees drop from the manuscript but
stay in outline/edit.

## Layout & boundaries

- `crates/ink-core/` — the real work. Parser + tree + renderers, zero UI deps.
  Everything downstream depends on this; keep it UI-agnostic and pure.
- `crates/ink-cli/` — the `ink` binary: the CLI subcommands (`render`, `fmt`,
  `export`) in `main.rs`, and the language server (`ink lsp`) in `lsp.rs`. The
  LSP deps (`lsp-server`, `lsp-types`, `serde_json`) live here only, never in
  `ink-core`.
- `app/src-tauri/` — Tauri v2 shell. Its own workspace (root `Cargo.toml`
  `exclude`s `app`), so `cargo test` stays fast and needs no webkit.
- `app/src/` — bundler-built frontend (Vite). No JS framework. Layout is a
  flex row: the outline rail plus a shared main area that shows **one** of
  editor / preview / codex / timeline / characters / notes / bibliography / map
  at a time. The view picker toggles a `body.show-*` class per view
  (`show-preview`, `show-codex`, `show-timeline`, `show-characters`,
  `show-notes`, `show-bibliography`, `show-map`); the outline rail collapses
  (`body.hide-outline`). All pure CSS class toggles in `main.js`/`style.css` —
  no grid tracks to juggle. Ctrl/Cmd+1…N picks the Nth view in picker order
  (keyed on `e.code`, never with Alt — AltGr is Ctrl+Alt on Windows); a footer
  status bar holds the file name and word count.
- **Adding a planning view follows one pattern** (timeline/characters/notes/map
  are the examples): a pure projection in `ink-core` (`render_timeline_html`,
  `render_characters_html`, `render_notes_html`, `map_markers`) → a stateless
  `#[tauri::command]` → a panel + view-picker option, refreshed each parse in
  `main.js`'s `refresh()`.
  Views that emit `data-jump` offsets share one click-to-scroll handler. Read
  the view keys from `ink-core::meta` (below), never scattered string literals.

## Commands

```sh
cargo test                                    # core tests (from repo root)
cargo fmt --all                               # format (CI: --check, both workspaces)
cargo fmt --manifest-path app/src-tauri/Cargo.toml   # …and the Tauri shell's workspace
cargo test -p ink-core --features pdf          # + the Shunn PDF emitter (genpdf)
cargo run -p ink-cli -- render --view=edit examples/sample.ink
cargo run -p ink-cli -- export --out=/tmp/m.pdf examples/sample.ink  # Shunn PDF
cargo run -p ink-cli -- fmt --write examples/sample.ink        # normalize to canonical house style
node app/src/reorder.test.mjs                 # reorder splice self-check
node app/src/fold.test.mjs                     # fold depth/section self-check
node app/src/metacomplete.test.mjs             # metadata-completion zone self-check
node app/src/character.test.mjs                # new-character scaffold splice self-check
node app/src/timescrub.test.mjs                # time-scrub character-position self-check
node app/src/mapproviders.test.mjs             # map-world folding + providers self-check
node app/src/filetree.test.mjs                 # project .ink tree build/prune/sort self-check
node app/src/caretsection.test.mjs             # caret -> outline-section pick self-check
node app/src/attribution.test.mjs              # CriticMarkup attribution-suffix mirror self-check
cargo test -p ink-cli                          # ink lsp: UTF-16 positions + an in-memory end-to-end session
cargo install --path crates/ink-cli            # put `ink` (and `ink lsp`) on PATH for your editor
cd app && npm run tauri dev                   # run the app (needs a display)
cd app && npm run build                       # frontend only -> app/dist
```

There is **no `cargo-tauri`-free way to `cargo run` the app in debug**:
`devUrl` is set, so a debug build expects the Vite dev server that
`beforeDevCommand` starts — use `npm run tauri dev`.

## Architecture decisions (don't relitigate)

- **Three front ends, one core.** The desktop app, the CLI, and the language
  server are peers over `ink-core`; none is the "real" product. A feature lands
  as a pure function in `ink-core`, then each front end exposes it where it fits:
  a Tauri command + panel, a CLI subcommand, an LSP capability. When you add or
  change a feature, **ask which front ends it belongs in** and keep them in step
  — e.g. a new projection the app shows may want an `ink` subcommand (epic #136)
  and an LSP surface; a behaviour change in `fmt`/`normalize` reaches all three
  (app on blur, `ink fmt`, LSP formatting). Docs follow: README (app usage, CLI,
  and the editor setup snippets), `docs/ipc.md` (Tauri only). The terminal
  roadmap is epic #136 (distribution #137, project-aware CLI #138, view parity
  #139, `ink check` #140, LSP #141).
- **The language server is Rust on the real parser — no mirror.** `ink lsp`
  (`crates/ink-cli/src/lsp.rs`, `lsp-server` + `lsp-types`) holds only the open
  buffers' text (full-document sync) and re-parses per request: stateless in
  spirit, like the Tauri commands. It is where editor intelligence for *other*
  editors lives, so it never needs a JS-style mirror; prefer an LSP capability
  over a tree-sitter grammar (which would be a fifth mirror, #38). Positions are
  LSP line + **UTF-16** column, converted from `ink-core`'s char offsets by
  `Lines` — exact even for astral chars, unlike the app. `ink_core::tokens` is
  the **parser's own token stream**, which `ink lsp` serves as semantic tokens
  (custom types, legend `TOKEN_TYPES`; split to one line each): `parse.rs`
  threads an optional sink (`Tk`) through the same `parse`/`scan` that build the
  tree (off, so free, for a plain `parse`), and
  takes spans from the same helpers the tree does (`heading`, `meta_spans`), so
  tokens can't disagree with the parse. Extend that sink when the grammar grows;
  never write a separate tokenizer. The project
  root comes from the client (`root_markers = { "Inkpot" }` in Neovim). Test new
  capabilities in the in-memory end-to-end test (`Connection::memory()`); for a
  real-editor check, headless Neovim works (`nvim --headless --clean -u
  <init.lua>` with the README snippet, then `vim.lsp.buf_request_sync`).
- **Text is canonical.** The editor buffer is the source of truth; Rust is
  stateless (parse in, render out). No document state crosses IPC. A scene move
  is a text splice, not a tree mutation — hence there is deliberately **no
  reorder command**; see `app/src/reorder.js`.
- **Autosave is path-gated.** A debounced write to `currentPath` fires once
  edits settle (`main.js`); untitled/example buffers have no path, so they are
  never written — the example doc never hits disk. A Tauri `onCloseRequested`
  guard flushes the autosave then prompts before discarding an unsaved untitled
  buffer. **Every write to the open file goes through `writeCurrent`**, which
  first checks the disk against `diskText` (what we last read/wrote) so an outside
  edit (Neovim, a sync client) is never overwritten (#135): a clean buffer
  reloads it (also on window focus); a dirty one keeps both, writing the disk
  version to a sibling `name.conflict-<stamp>.ink` (shown in the tree, kept out
  of the project bundle). Writes and checks are serialized. The 3-way merge (#119)
  replaces keep-both later. Note `onCloseRequested`'s default action is a JS
  `window.destroy()`, not a native close — so the capability must grant
  `core:window:allow-destroy` (in `capabilities/default.json`), or the app
  silently won't quit. Crash
  recovery of untitled drafts (app-data snapshots) is #13. **Leaving a document**
  (window blur, close, cross-file jump, explicit Save) also normalizes the buffer
  to canonical house style via the `normalize` command (#105) — *not* the
  keystroke autosave, so text never reflows mid-sentence; the result is applied as
  a minimal single-span edit (`mindiff.js`) so CodeMirror keeps the caret.
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
  stays the one active buffer; no project state is *held* in Rust, `ink-core`
  stays file-agnostic. Where a feature needs the whole project it passes the file
  bundle as a command argument, re-sent each call — the codex (`codex_project`,
  #28), the book export (`export_shunn_book`, #74), and the text views
  (`preview`/`manuscript`/`bibliography`/`export_shunn`, #89) all do this; it is a
  pure function of its inputs, not stored state. **Cross-file resolution is live**:
  the codex resolves `[[links]]`/backlinks across every project file (#28), and
  the text views resolve `[[links]]`/`[@cites]` project-wide too (#89) — the maps
  (`link_titles`/`cite_shorts`) build from all files via `project_ctx`, while the
  active file's tree is what's walked. `timeline`/`map`/`characters` stay per-file
  and would reuse the same bundle mechanism to go project-wide. Deferred: project
  settings in the marker.
- **Heading depth = marker count** (Model A). `#`/`~` set visibility, the count
  sets depth. Illegal downward jumps clamp to parent + 1 (`parse.rs`) — but only
  against a *real heading parent*, never the implicit root, so a document that
  opens deep (e.g. all `##`) keeps its headings as same-level siblings instead
  of demoting the first and nesting the rest.
- **Paragraph line breaks are Markdown-style** (#112). A bare `\n` inside a
  paragraph is a soft break (prints as a space); a line ending in `\` is a hard
  break (`Inline::Escaped("\n")`). A `\` ending the *paragraph* escapes nothing
  and is a silent no-op (`Escaped("")`: never prints, still round-trips) — so the
  editor can mute every line-ending `\` without lookahead, since a `StreamLanguage`
  can't see whether a line is a paragraph's last. That rule applies only at the
  outermost scan: a CriticMarkup body ending in `\` (`{+C:\}`) keeps it, since
  the span closes on the first `}`.
- **Spans are char (Unicode scalar) offsets**, to match JS string indexing.
  They agree with CodeMirror positions for BMP text; astral chars (emoji)
  drift — a known, accepted edge. Line offsets assume `\n` (normalize CRLF).
- **The CodeMirror language mirrors the Rust parser** (`app/src/inklang.js`,
  a `StreamLanguage`). Change one, change the other. `app/src/fold.js` (fold
  service, heading depth off the marker run) and `app/src/metacomplete.js`
  (metadata-completion zone detection: heading + `key:` rules) mirror it too — a
  heading- or meta-rule change touches all of them. `app/src/attribution.js`
  mirrors `split_attribution` (the CriticMarkup attribution-suffix grammar).
- **Edit suggestions are inline attributed CriticMarkup** (epic #111). A
  suggestion is a CriticMarkup span with a trailing `|@author[,@author…][|date]`
  suffix, parsed into `Inline::Attributed`; there is **no sidecar** — the span
  moves with the text, so anchoring is free. Attribution is metadata only: print
  projects every unresolved span as accepted, attributed or not, and never gains
  a different print meaning. Built so far: the syntax (#116). **Planned**, not
  yet in code: guarding against unreviewed text via export modes
  (accept/reject/mine-only, #117); per-span accept/reject, which removes the
  markup and adds the suggester to a `contributors:` front-matter list (#118;
  history belongs to #44); reconciling reviewers by a word-level 3-way merge *at
  ingest* (#119/#120), so overlapping suggestions are never stored.
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
- **Manuscript/publishing export goes through one seam** (`ink-core::shunn`, #23).
  `build_shunn` projects the tree into a flat, engine-independent `ShunnManuscript`
  model (title-page fields + a `ShunnBlock` stream); emitters consume *that*, never
  the tree. The PDF emitter uses **genpdf** and is behind the `pdf` **feature**
  (default-off, so `cargo test` and the app's fast path stay pure/webkit-free; the
  CLI and Tauri shell enable it). Courier Prime (OFL) is bundled via `include_bytes!`
  — no runtime font dependency. genpdf's known ceilings (flow-only placement, no
  subsetting) are marked `ponytail:`; the upgrade path if they bite is **krilla**,
  not typst. A future EPUB/KDP emitter (#80) reuses the same `ShunnManuscript`.
  Chapter convention: a top-level `#` = a chapter. A whole project exports as one
  book via `build_shunn_book` (title page from the `Inkpot` marker's front matter,
  else the first file; each file's `doc_blocks` concatenated). Note: genpdf drags an old
  transitive tree (stdweb → RUSTSEC-2020-0056, plus old printpdf/lopdf/time), so
  `cargo audit` lights up — expected, isolated behind the `pdf` feature, and the
  krilla path clears most of it.

## Conventions

- Parsers are hand-written line/inline scanners on purpose — no parser crates.
- Rust is formatted by `cargo fmt` (`rustfmt.toml`: `use_small_heuristics =
  "Max"`, which keeps the compact one-line style within 100 columns); CI runs
  `--check` on both workspaces, so format before pushing (#87).
- Non-trivial pure logic gets one runnable self-check (see
  `reorder.test.mjs`, `fold.test.mjs`, `ink-core/tests/parse.rs`). No test
  frameworks.
- Commit style: imperative subject, a short body explaining why, and a
  `Co-Authored-By` trailer.
- Commit or push only when asked.
- Merge PRs with a merge commit (`gh pr merge --merge`), then delete the branch
  locally and on the remote — no need to ask.

## Releasing

Tag `vX.Y.Z` and push it → `.github/workflows/release.yml` builds on a 3-OS
matrix (macos universal, ubuntu, windows — Tauri bundles natively) via
`tauri-apps/tauri-action` and publishes installers (`.AppImage`/`.deb`/`.rpm`,
universal `.dmg`, `.msi`/NSIS) to **GitHub Releases** as a **draft prerelease**
— review, then publish (`gh release edit vX.Y.Z --draft=false`). A
`create-release` job makes the draft **once** and outputs its id; the build
matrix uploads to it via `releaseId`, so no OS job creates its own release — the
old race that left **duplicate drafts** with assets split across them is gone
(#76). A `cli` matrix job builds the `ink` CLI (`--locked`) for five targets
(macOS arm64/x64 — both on the arm64 runner, so only arm64 is smoke-tested —
Linux x64 and arm64 natively on `ubuntu-22.04-arm`, no `cross`, and Windows x64) and uploads `ink-v<version>-<target>.tar.gz` (`.zip`
on Windows) to the same draft **by release id**, never by tag. **12 assets** should land on one draft: the 7 app installers
plus the 5 CLI archives. Builds are **unsigned** (signing #10, updater #11
tracked separately). Homebrew lives in `thombruce/homebrew-tap`: the app is a
cask (`Casks/inkpot.rb`), the CLI a formula (`Formula/inkpot-cli.rb`, installs
`ink`). Publishing a release fires `.github/workflows/homebrew.yml`, which
recomputes the dmg and CLI `sha256`s, bumps the cask's `version` + `sha256` and
**regenerates the formula whole** (like tonic's): sequential steps in one job
(no push race), cask first, so a missing CLI archive only fails the formula (needs
the `HOMEBREW_TAP_TOKEN` secret — a PAT with write access to the tap). No manual
bump. It runs on publish, not tag push, because draft asset URLs aren't public.
First release: v0.1.0.

The release **version is single-sourced in `app/src-tauri/Cargo.toml`**
(`tauri.conf.json` omits `version` and inherits it). Bump it there per
release and tag `vX.Y.Z`. SemVer, mapped to the format: stay in `0.x` until
the `.ink` format stabilises; `1.0`+ MAJOR = a format-breaking change.
`ink-core`/`ink-cli` crate versions are separate from the app's release version;
the **CLI ships under the app's tag** and reports it — the workflow sets
`INK_VERSION` from the tag for `ink --version` (a local build says `dev`).
**Commit `app/src-tauri/Cargo.lock` with the bump** — it pins `inkpot-app`'s
version, so a build regenerates it and leaves the tree dirty if it wasn't staged
(`git add app/src-tauri/{Cargo.toml,Cargo.lock} CHANGELOG.md`). Harmless to the
shipped binaries, but keep the lock in step.

Log user-facing changes under `## [Unreleased]` in `CHANGELOG.md` as you go;
when tagging, rename that heading to the new version + date and paste it into
the release notes (the workflow's `releaseBody` is static boilerplate).

## Known rough edges

Open work is tracked in GitHub issues. One parser gotcha to keep in mind: the
`.ink` parser silently drops a mis-classified metadata line — the
blank-line-after-heading convention (documented in the README) avoids it, and
the `edit` view (`fmt`/`normalize`) must preserve that blank line, at the top of
the file too (`fmt_keeps_colon_prose_out_of_meta`).

The meta zone also opens at the **top of the file**: a leading `key: value`
block is document front matter, parsed onto the root node (`title`, `author`,
etc.). Same three mirrors as any parser change — `parse.rs` (`in_meta` starts
true), `inklang.js` (`inMeta: true`), and the `edit` renderer (round-trips root
meta). A file that opens with a colon-bearing prose line is the same
misclassification trap; front matter must start on line 1.
