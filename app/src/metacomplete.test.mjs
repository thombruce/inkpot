// Run: node app/src/metacomplete.test.mjs
import assert from "node:assert/strict";
import { metaZone, DOC_KEYS, SCENE_KEYS } from "./metacomplete.js";

// 1-based line-text accessor over an array.
const at = (arr) => (n) => arr[n - 1];

// Typing on line 1 (nothing above) -> document front matter.
assert.equal(metaZone(at([""]), 1), "front");
// A block of meta lines above, still front matter.
assert.equal(metaZone(at(["title: X", "author: Y", ""]), 2), "front");

// Directly under a heading -> that heading's meta block.
assert.equal(metaZone(at(["# Chapter", ""]), 2), "scene");
assert.equal(metaZone(at(["~ Scene", "pov: A", ""]), 3), "scene");

// A blank line closes the zone.
assert.equal(metaZone(at(["# Chapter", "", ""]), 3), null);
// A body (non-meta) line closes the zone.
assert.equal(metaZone(at(["# Chapter", "body prose here", ""]), 3), null);
// Front matter closed by a blank, then a heading -> the heading's zone wins.
assert.equal(metaZone(at(["title: x", "", "# Chapter", ""]), 4), "scene");
// A multiline value's indented continuation lines don't close the zone: a key
// completion below still sees front matter.
assert.equal(metaZone(at(["contact:", "  221B Baker St", "  London", ""]), 4), "front");
assert.equal(metaZone(at(["# C", "loc:", "  Riverside", ""]), 4), "scene");

// The Shunn front-matter fields are seeded.
for (const k of ["title", "author", "byline", "contact"]) {
  assert.ok(DOC_KEYS.includes(k), `DOC_KEYS missing ${k}`);
}
assert.ok(SCENE_KEYS.includes("pov"));

console.log("metacomplete: all assertions passed");
