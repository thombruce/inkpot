// Run: node app/src/fold.test.mjs
import assert from "node:assert/strict";
import { headingDepth, sectionEndLine } from "./fold.js";

// headingDepth: marker run length, any of #/~/%, only with trailing whitespace.
assert.equal(headingDepth("# Chapter"), 1);
assert.equal(headingDepth("### Deep"), 3);
assert.equal(headingDepth("~~ Scene"), 2);
assert.equal(headingDepth("%% Cut"), 2);
assert.equal(headingDepth("body text"), null);
assert.equal(headingDepth("#no-space"), null); // needs whitespace after the run
assert.equal(headingDepth(""), null);

// sectionEndLine over a 1-based line array. Depths, line by line:
//   1 "# A"      depth 1
//   2 "body"     -
//   3 "## A.1"   depth 2
//   4 "body"     -
//   5 "# B"      depth 1
//   6 "body"     -
const depths = [null, 1, null, 2, null, 1, null];
const depthOf = (n) => depths[n];
const N = 6;

// Chapter A (line 1) owns through line 4 — stops before the next depth-1 heading.
assert.equal(sectionEndLine(depthOf, N, 1), 4);
// Sub-heading A.1 (line 3) owns only its body, stops before B (depth 1 <= 2).
assert.equal(sectionEndLine(depthOf, N, 3), 4);
// Chapter B (line 5) is last — owns to end of doc.
assert.equal(sectionEndLine(depthOf, N, 5), 6);
// A body line is not a heading — nothing to fold.
assert.equal(sectionEndLine(depthOf, N, 2), null);

// A heading with no body (immediately followed by a shallower/equal heading):
// end == head, so main.js's `to > from` guard makes it non-foldable.
{
  const d = [null, 1, 1]; // "# A", "# B"
  assert.equal(sectionEndLine((n) => d[n], 2, 1), 1); // == head line, empty section
}

console.log("fold: all assertions passed");
