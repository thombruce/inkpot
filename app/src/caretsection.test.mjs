// Run: node app/src/caretsection.test.mjs
import assert from "node:assert/strict";
import { deepestSectionAt } from "./caretsection.js";

// A doc: # A [0,30) with a nested ## B [10,30); # C [30,50). docLen = 50.
const items = [
  { start: 0, end: 30 },  // 0: A (contains B)
  { start: 10, end: 30 }, // 1: B, nested in A
  { start: 30, end: 50 }, // 2: C
];
const docLen = 50;
const at = (pos) => deepestSectionAt(items, pos, docLen);

// Caret in A's heading, before B -> A (only A contains it).
assert.equal(at(5), 0);
// Caret inside B -> the deeper of the two containing rows.
assert.equal(at(20), 1);
// On the B/C boundary (pos === B.end === C.start): `pos < end` keeps it out of
// A and B, so it lands in C.
assert.equal(at(30), 2);
// Caret in C.
assert.equal(at(40), 2);
// Caret at the very end of the doc -> last containing section (C), not nothing.
assert.equal(at(50), 2);
// Caret at A.start (offset 0) sits inside A.
assert.equal(deepestSectionAt(items, 0, docLen), 0);
assert.equal(deepestSectionAt([], 5, 10), -1); // no rows

console.log("caretsection: all assertions passed");
