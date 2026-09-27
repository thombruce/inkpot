// Pure metadata-key completion logic, mirroring the parser's meta zones (see
// parse.rs / inklang.js). Import-free so metacomplete.test.mjs runs under node;
// main.js wraps `metaZone` in a CodeMirror completion source.

// Seed keys. Document front matter carries the work's identity — the fields the
// Shunn manuscript export (#23) needs; a heading's meta block describes a scene.
export const DOC_KEYS = ["title", "author", "byline", "contact"];
export const SCENE_KEYS = ["pov", "time", "characters", "location", "coords", "map", "exits"];
// Bibliographic keys for `%` source entities cited with [@key] (#84). metaZone
// can't yet tell a `%` heading from a `~` scene, so these are offered under any
// non-front heading alongside SCENE_KEYS — harmless extra suggestions on a scene.
export const SOURCE_KEYS = ["author", "year", "title", "edition", "place", "publisher"];

export const HEADING = /^([#~%])\1*\s/; // uniform marker run + space (Model A)
const META = /^([\w-]+):(.*)$/; // `key: value` — single-token key, matching meta_line

// Which meta zone contains the in-progress line `lineNum` (1-based)? Replays the
// parser's forward state machine (parse.rs / inklang.js) over the lines *above*
// the cursor so the mirror is exact — in particular, an indented line only
// continues a multiline value if the key that opened it had an empty value.
// Returns "front" (document front matter), "scene" (a heading's meta block), or
// null (the zone is closed — the cursor is in body).
export function metaZone(lineTextAt, lineNum) {
  let inMeta = true; // front matter is open at the top of the file
  let multiline = false; // an empty-value `key:` opened a multiline block
  let zone = "front";
  for (let n = 1; n < lineNum; n++) {
    const t = lineTextAt(n);
    if (HEADING.test(t)) {
      inMeta = true;
      multiline = false;
      zone = "scene";
      continue;
    }
    if (!inMeta) continue;
    if (multiline && /^[ \t]+\S/.test(t)) continue; // continuation of the value
    multiline = false;
    const m = META.exec(t);
    if (m) {
      multiline = m[2].trim() === ""; // empty value opens a multiline block
      continue;
    }
    inMeta = false; // a blank or non-meta line closes the zone
  }
  return inMeta ? zone : null;
}

// The entity-name segment being typed in a metadata *value* — the text after the
// last comma (values are comma-separated lists; see meta_value_html in render.rs).
// Given the line text and the caret column, returns `{ typed, fromCol }` where
// `fromCol` is the column the segment starts at (for the completion's `from`), or
// null if the caret is not past a `key:` colon. Front-matter vs scene gating is
// the caller's job (via metaZone).
// Reference-completion zone: is the caret inside an *open* inline reference on
// this line? Returns `{ kind, typed, fromCol }` or null. `kind` is "link" for a
// wikilink (`[[` or `{&`) or "cite" for a citation (`[@` or `{@`). "Open" = no
// closing `]`/`}` between the opener and the caret, and no second opener in the
// typed run (so the *nearest* opener wins). Mirrors the inline openers in
// parse.rs / inklang.js; both house and borrowed spellings are recognized.
export function referenceSegment(lineText, caretCol) {
  const before = lineText.slice(0, caretCol);
  // Excluding `]}[{` from the typed run stops it at a close or a nearer opener.
  const m = /(\[\[|\{&|\[@|\{@)([^\]}[{]*)$/.exec(before);
  if (!m) return null;
  const kind = m[1] === "[[" || m[1] === "{&" ? "link" : "cite";
  return { kind, typed: m[2], fromCol: caretCol - m[2].length };
}

// The closing delimiter to append when completing a reference opened by `open`,
// given the text `after` the caret: the matching closer, or "" if the reference
// is already closed (don't double up) or the opener is unknown.
const REF_CLOSE = { "[[": "]]", "[@": "]", "{&": "}", "{@": "}" };
export function refClose(open, after) {
  const close = REF_CLOSE[open] ?? "";
  return close && !after.startsWith(close) ? close : "";
}

export function valueSegment(lineText, caretCol) {
  const before = lineText.slice(0, caretCol);
  const colon = before.indexOf(":");
  if (colon === -1) return null; // still typing the key
  const seg = before.slice(colon + 1);
  const lastComma = seg.lastIndexOf(",");
  const part = seg.slice(lastComma + 1);
  const leading = part.length - part.trimStart().length;
  return { typed: part.trim(), fromCol: colon + 1 + lastComma + 1 + leading };
}
