// Run: node app/src/metacomplete.test.mjs
import assert from "node:assert/strict";
import { metaZone, valueSegment, referenceSegment, refClose, DOC_KEYS, SCENE_KEYS } from "./metacomplete.js";

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
// But an indented line under a NON-empty value is body (parser closes meta), so
// the zone is closed — matches parse.rs, not just "any indented line continues".
assert.equal(metaZone(at(["key: value", "    indented text", ""]), 3), null);

// The Shunn front-matter fields are seeded.
for (const k of ["title", "author", "byline", "contact"]) {
  assert.ok(DOC_KEYS.includes(k), `DOC_KEYS missing ${k}`);
}
assert.ok(SCENE_KEYS.includes("pov"));
assert.ok(SCENE_KEYS.includes("coords"), "coords should be a suggested key (map view)");

// valueSegment: the entity-name part being typed in a value.
// No colon yet -> null (key completion territory).
assert.equal(valueSegment("charac", 6), null);
// First value: segment starts after the colon + one space.
assert.deepEqual(valueSegment("characters: A", 13), { typed: "A", fromCol: 12 });
// After a comma: segment is the part after the last comma, leading space skipped.
assert.deepEqual(valueSegment("characters: Alice, B", 20), { typed: "B", fromCol: 19 });
// Empty value (just typed the colon): empty segment at the value start.
assert.deepEqual(valueSegment("pov:", 4), { typed: "", fromCol: 4 });
// A multi-word entity name is captured whole (spaces within the segment).
assert.deepEqual(valueSegment("location: The Wat", 17), { typed: "The Wat", fromCol: 10 });

// referenceSegment: the target/key being typed inside an inline reference.
// Both house and borrowed openers; kind by opener.
assert.deepEqual(referenceSegment("See {&Ali", 9), { kind: "link", typed: "Ali", fromCol: 6 });
assert.deepEqual(referenceSegment("See [[Ali", 9), { kind: "link", typed: "Ali", fromCol: 6 });
assert.deepEqual(referenceSegment("As {@pears", 10), { kind: "cite", typed: "pears", fromCol: 5 });
assert.deepEqual(referenceSegment("As [@pears", 10), { kind: "cite", typed: "pears", fromCol: 5 });
// Empty prefix right after the opener.
assert.deepEqual(referenceSegment("x {&", 4), { kind: "link", typed: "", fromCol: 4 });
// Multi-word link target captured whole.
assert.deepEqual(referenceSegment("{&Alice Har", 11), { kind: "link", typed: "Alice Har", fromCol: 2 });
// Closed reference before the caret -> not in a reference.
assert.equal(referenceSegment("{&Alice} and ", 13), null);
// No opener / plain prose -> null.
assert.equal(referenceSegment("just prose", 10), null);
// Nearest opener wins when two are open on the line.
assert.deepEqual(referenceSegment("{&a} then {@b", 13), { kind: "cite", typed: "b", fromCol: 12 });

// refClose: which closer to append when accepting a completion.
assert.equal(refClose("{&", ""), "}"); // house link, nothing after -> close
assert.equal(refClose("{@", " and"), "}"); // house cite -> close
assert.equal(refClose("[[", ""), "]]"); // borrowed wikilink -> ]]
assert.equal(refClose("[@", ""), "]"); // borrowed cite -> ]
assert.equal(refClose("{&", "} rest"), ""); // already closed -> don't double
assert.equal(refClose("[[", "]] rest"), ""); // already closed
assert.equal(refClose("??", ""), ""); // unknown opener -> nothing

console.log("metacomplete: all assertions passed");
