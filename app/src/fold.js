// Pure fold logic for the edit view, mirroring the parser's Model A: a heading's
// depth is its leading marker run length, and it owns every line down to the next
// heading of depth <= its own. Kept import-free so fold.test.mjs runs under node
// without pulling in CodeMirror; main.js wraps these in a foldService.

// Heading depth = length of the leading run of one repeated #/~/% marker followed
// by whitespace (matches inklang.js `^([#~%])\1*\s` and parse.rs). Null if the
// line is not a heading.
export function headingDepth(text) {
  const m = /^([#~%])\1*(?=\s)/.exec(text);
  return m ? m[0].length : null;
}

// Last line (1-based, inclusive) of the section owned by the heading at line
// `head`: scan forward for the next heading of depth <= head's and take the line
// before it, else the last line. `depthOf(n)` returns a 1-based line's depth (or
// null); `lineCount` is the 1-based line total. Null if `head` isn't a heading.
export function sectionEndLine(depthOf, lineCount, head) {
  const depth = depthOf(head);
  if (depth == null) return null;
  for (let n = head + 1; n <= lineCount; n++) {
    const d = depthOf(n);
    if (d != null && d <= depth) return n - 1;
  }
  return lineCount;
}
