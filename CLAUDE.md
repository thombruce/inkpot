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
- `app/src/` — bundler-built frontend (Vite). No JS framework.

## Commands

```sh
cargo test                                    # core tests (from repo root)
cargo run -p ink-cli -- render --view=edit examples/sample.ink
node app/src/reorder.test.mjs                 # reorder splice self-check
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
- **Heading depth = marker count** (Model A). `#`/`~` set visibility, the count
  sets depth. Illegal jumps clamp to parent + 1 (`parse.rs`).
- **Spans are char (Unicode scalar) offsets**, to match JS string indexing.
  They agree with CodeMirror positions for BMP text; astral chars (emoji)
  drift — a known, accepted edge. Line offsets assume `\n` (normalize CRLF).
- **The CodeMirror language mirrors the Rust parser** (`app/src/inklang.js`,
  a `StreamLanguage`). Change one, change the other.
- **Frontend uses `withGlobalTauri`** — `window.__TAURI__.{core,dialog,fs}`, no
  `@tauri-apps/api`/plugin npm packages. Keep it that way unless a global is
  missing.

## Conventions

- Parsers are hand-written line/inline scanners on purpose — no parser crates.
- Non-trivial pure logic gets one runnable self-check (see
  `reorder.test.mjs`, `ink-core/tests/parse.rs`). No test frameworks.
- Commit style: imperative subject, a short body explaining why, and a
  `Co-Authored-By` trailer.
- Commit or push only when asked.

## Releasing (planned, not built — see #6)

Distribution will be tag-triggered GitHub Actions building on a 3-OS matrix
(ubuntu/macos/windows — Tauri bundles natively, no cross-compile) via
`tauri-apps/tauri-action`, publishing installers to **GitHub Releases**
(`.AppImage`/`.deb`/`.rpm`, `.dmg`, `.msi`/NSIS). First release ships
**unsigned** (macOS notarization ~$99/yr and Windows code-signing deferred
until friction is real). Auto-update via the Tauri v2 updater plugin is a
later add. Blockers: `bundle.active` is `false` and there's no `release.yml`
yet (the icon is done — real set generated from `assets/inkpot.png`).

The release **version is single-sourced in `app/src-tauri/Cargo.toml`**
(`tauri.conf.json` omits `version` and inherits it). Bump it there per
release and tag `vX.Y.Z`. SemVer, mapped to the format: stay in `0.x` until
the `.ink` format stabilises; `1.0`+ MAJOR = a format-breaking change.
`ink-core`/`ink-cli` versions are separate from the app's release version.

## Known rough edges

Tracked as GitHub issues #1–#4: metadata blank-line rule (docs, #1), no escape
for literal markup (#2), inline markup doesn't nest (#3), reorder blank-line
seams (#4). The `.ink` parser silently drops a mis-classified metadata line —
the blank-line-after-heading convention avoids it.
