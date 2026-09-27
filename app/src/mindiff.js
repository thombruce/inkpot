// The minimal single-span edit turning `cur` into `next`: trim the common prefix
// and common suffix, and replace only the differing middle. Dispatched as a
// CodeMirror change so the editor maps the caret/selection across it (used by
// the on-save house-style normalization, #105). Equal strings yield an empty
// no-op edit (from === to, insert === "").
//
// Offsets are UTF-16 code units — CodeMirror positions and JS string indices
// agree on those (astral chars span two units in both, consistent). A shared
// surrogate pair can't split the boundary: the scan stops at the first *differing*
// unit, and an equal pair matches both halves.
export function minimalEdit(cur, next) {
  let s = 0;
  const min = Math.min(cur.length, next.length);
  while (s < min && cur[s] === next[s]) s++;
  let end = cur.length;
  let endNext = next.length;
  while (end > s && endNext > s && cur[end - 1] === next[endNext - 1]) {
    end--;
    endNext--;
  }
  return { from: s, to: end, insert: next.slice(s, endNext) };
}
