// Self-check for minimalEdit (the on-save normalize caret-preserving diff, #105).
// Run: node app/src/mindiff.test.mjs
import { minimalEdit } from "./mindiff.js";
import assert from "node:assert";

// Applying the edit to `cur` reproduces `next`.
const apply = (cur, { from, to, insert }) => cur.slice(0, from) + insert + cur.slice(to);

function check(cur, next) {
  const e = minimalEdit(cur, next);
  assert.strictEqual(apply(cur, e), next, `edit must reproduce next: ${JSON.stringify(e)}`);
  return e;
}

// Equal strings -> empty no-op (steady state: already house, no rewrite).
{
  const e = check("same text", "same text");
  assert.deepStrictEqual(e, { from: 9, to: 9, insert: "" }, "no-op edit");
}

// A single localized token swap ([[alice]] -> {&alice}) touches only that span.
{
  const cur = "See [[alice]] soon.";
  const next = "See {&alice} soon.";
  const e = check(cur, next);
  assert.strictEqual(e.from, 4, "prefix 'See ' kept");
  assert.strictEqual(cur.slice(e.from, e.to), "[[alice]]", "replaced span is just the token");
  assert.strictEqual(e.insert, "{&alice}", "inserted house form");
}

// Interpolation swap and a trailing edit keep the common suffix out of the span.
check("Chapter {{number}} end", "Chapter {=number} end");

// Insertion-only and deletion-only degenerate cases.
check("abc", "abXc"); // insert
check("abXc", "abc"); // delete

// Multiple scattered changes collapse to one span spanning first..last diff
// (acceptable: a one-time conversion of a legacy file, caret maps to span end).
{
  const cur = "[[a]] mid [[b]]";
  const next = "{&a} mid {&b}";
  check(cur, next);
}

// Astral char inside the unchanged prefix/suffix is preserved (BMP-agnostic).
check("😀 [[x]] 😀", "😀 {&x} 😀");

console.log("mindiff: all assertions passed");
