// Self-check for the CriticMarkup attribution-suffix mirror. Run: node app/src/attribution.test.mjs
import assert from "node:assert/strict";
import { attributionStart as at } from "./attribution.js";

// Well formed at the end → index of the suffix's `|`.
assert.equal(at("new words|@Ada Lovelace|2026-10-04"), 9);
assert.equal(at("gone|@Ada"), 4);
assert.equal(at("teh~the|@Ada, @Sam|2026-10-04T14:03Z"), 7);
assert.equal(at("a|b|@Ada|2026-10-04"), 3, "only the last fields are the suffix");
assert.equal(at("|@Ada"), 0, "empty content is fine");

// Not well formed → no suffix (the `|` is content).
for (const body of ["a|b", "a|@", "a|@Ada|soon", "a|@Ada,Sam", "plain", "@Ada", "@Ada|2026-10-04", "a|@x~y", "a~b|@x~y"]) {
  assert.equal(at(body), -1, body);
}

console.log("attribution: ok");
