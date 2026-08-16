// Pure metadata-key completion logic, mirroring the parser's meta zones (see
// parse.rs / inklang.js). Import-free so metacomplete.test.mjs runs under node;
// main.js wraps `metaZone` in a CodeMirror completion source.

// Seed keys. Document front matter carries the work's identity — the fields the
// Shunn manuscript export (#23) needs; a heading's meta block describes a scene.
export const DOC_KEYS = ["title", "author", "byline", "contact"];
export const SCENE_KEYS = ["pov", "time", "characters", "location"];

const HEADING = /^([#~%])\1*\s/; // uniform marker run + space (Model A)
const META = /^[\w-]+:/; // `key:` — single-token key, matching meta_line in parse.rs

// Which meta zone contains the in-progress line `lineNum` (1-based)? Scan the
// lines above: a heading means we're in its post-heading meta block ("scene"); a
// blank or non-meta line means the zone is closed (null); reaching the top of the
// file through only meta lines means document front matter ("front").
export function metaZone(lineTextAt, lineNum) {
  for (let n = lineNum - 1; n >= 1; n--) {
    const t = lineTextAt(n);
    if (HEADING.test(t)) return "scene";
    if (t.trim() === "" || !META.test(t)) return null;
  }
  return "front";
}
