// Pure metadata-key completion logic, mirroring the parser's meta zones (see
// parse.rs / inklang.js). Import-free so metacomplete.test.mjs runs under node;
// main.js wraps `metaZone` in a CodeMirror completion source.

// Seed keys. Document front matter carries the work's identity — the fields the
// Shunn manuscript export (#23) needs; a heading's meta block describes a scene.
export const DOC_KEYS = ["title", "author", "byline", "contact"];
export const SCENE_KEYS = ["pov", "time", "characters", "location"];

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
