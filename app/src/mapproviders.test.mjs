// Run: node app/src/mapproviders.test.mjs
import assert from "node:assert/strict";
import { PROVIDERS, worldOf, worldLabel } from "./mapproviders.js";

// Empty / whitespace map: value -> the default world, Earth.
assert.equal(worldOf(""), "earth");
assert.equal(worldOf("  "), "earth");
assert.equal(worldOf(undefined), "earth");
// Case/space folded.
assert.equal(worldOf(" Mars "), "mars");
assert.equal(worldOf("MOON"), "moon");
// Unknown world passes through as its own key.
assert.equal(worldOf("Xandar"), "xandar");

// Built-in providers each have the fields the map view needs.
for (const key of ["earth", "mars", "moon"]) {
  const p = PROVIDERS[key];
  assert.ok(p && p.url.includes("{z}") && p.attribution, `provider ${key} incomplete`);
}

// Labels: provider label for known, title-cased key for unknown.
assert.equal(worldLabel("mars"), "Mars");
assert.equal(worldLabel("xandar"), "Xandar");

console.log("mapproviders: all assertions passed");
