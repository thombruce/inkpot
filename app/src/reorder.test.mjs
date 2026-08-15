// Run: node app/src/reorder.test.mjs
import assert from "node:assert/strict";
import { spliceMove } from "./reorder.js";

const noTripleNewline = (t) => assert.ok(!/\n{3,}/.test(t), `doubled blank line in:\n${t}`);

// Move the last chapter (no trailing blank) to the front. Seams stay clean.
{
  const doc = "# A\nbody a\n\n# B\nbody b\n\n# C\nbody c\n";
  const from = doc.indexOf("# C");
  const r = spliceMove(doc, from, doc.length, 0);
  assert.equal(r.text, "# C\nbody c\n\n# A\nbody a\n\n# B\nbody b\n");
  assert.equal(r.at, 0); // caret at the moved block
  noTripleNewline(r.text);
}

// Move a heading-only node between two headings — must not glue headings.
{
  const doc = "# A\n\n# B\nbody b\n";
  const to = doc.indexOf("# B");
  const r = spliceMove(doc, 0, to, doc.length); // move A to the end
  assert.equal(r.text, "# B\nbody b\n\n# A\n");
  assert.ok(r.text.slice(r.at).startsWith("# A"));
  noTripleNewline(r.text);
}

// Move a middle chapter after the last one.
{
  const doc = "# A\nbody a\n\n# B\nbody b\n\n# C\nbody c\n";
  const from = doc.indexOf("# B");
  const to = doc.indexOf("# C");
  const r = spliceMove(doc, from, to, doc.length);
  assert.equal(r.text, "# A\nbody a\n\n# C\nbody c\n\n# B\nbody b\n");
  noTripleNewline(r.text);
}

// No-op: dropping inside the dragged range.
{
  const doc = "# A\nbody a\n\n# B\nbody b\n";
  assert.equal(spliceMove(doc, 0, 5, 3), null);
  assert.equal(spliceMove(doc, 0, 5, 0), null); // boundary start
  assert.equal(spliceMove(doc, 0, 5, 5), null); // boundary end
}

console.log("reorder: all assertions passed");
