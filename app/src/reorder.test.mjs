// Run: node app/src/reorder.test.mjs
import assert from "node:assert/strict";
import { spliceMove } from "./reorder.js";

// A|B|C, three 2-char blocks. Move C (4..6) before A (0..0).
{
  const r = spliceMove("AABBCC", 4, 6, 0);
  assert.equal(r.text, "CCAABB");
  assert.equal(r.at, 0);
}

// Move A (0..2) to after B, i.e. insert at C's start (4) — shifts left.
{
  const r = spliceMove("AABBCC", 0, 2, 4);
  assert.equal(r.text, "BBAACC");
  assert.equal(r.at, 2); // 4 - (2-0)
}

// Move A to the very end (insert at doc end, past the cut).
{
  const r = spliceMove("AABBCC", 0, 2, 6);
  assert.equal(r.text, "BBCCAA");
  assert.equal(r.at, 4); // 6 - (2-0)
}

// No-op: dropping inside the dragged range.
assert.equal(spliceMove("AABBCC", 2, 4, 3), null);
assert.equal(spliceMove("AABBCC", 2, 4, 2), null); // boundary start
assert.equal(spliceMove("AABBCC", 2, 4, 4), null); // boundary end

console.log("reorder: all assertions passed");
