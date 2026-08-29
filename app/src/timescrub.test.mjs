// Run: node app/src/timescrub.test.mjs
import assert from "node:assert/strict";
import { characterPositions, occupiedLocations } from "./timescrub.js";

// Fold-matched coords lookup, like main.js builds from the map markers.
const coords = { london: { lat: 51.5, lon: -0.1 }, rome: { lat: 41.9, lon: 12.5 } };
const coordsOf = (name) => coords[name.toLowerCase().trim()] ?? null;

const scenes = [
  { time: "10", title: "A", location: "London", characters: ["Alice", "Bob"] },
  { time: "20", title: "B", location: "Rome", characters: ["Alice"] },
  { time: "30", title: "C", location: "Nowhere", characters: ["Bob"] }, // no coords
  { time: "40", title: "D", location: "", characters: ["Carol"] }, // no location
];

// At the first scene, both are in London.
{
  const p = characterPositions(scenes, coordsOf, 0);
  assert.equal(p.Alice.location, "London");
  assert.equal(p.Bob.location, "London");
}

// Scrub forward: Alice moves to Rome, Bob stays (his later scene has no coords).
{
  const p = characterPositions(scenes, coordsOf, 1);
  assert.equal(p.Alice.location, "Rome");
  assert.equal(p.Bob.location, "London", "Bob keeps his last locatable position");
}

// A scene whose location has no coords doesn't move anyone; Carol (no location)
// never places.
{
  const p = characterPositions(scenes, coordsOf, 3);
  assert.equal(p.Bob.location, "London", "unlocatable scene ignored");
  assert.equal(p.Carol, undefined, "no-location scene places no one");
}

// Cursor past the end is clamped.
{
  const p = characterPositions(scenes, coordsOf, 99);
  assert.equal(p.Alice.location, "Rome");
}

// Grouping: co-located characters collapse to one marker with both names.
{
  const p = characterPositions(scenes, coordsOf, 0);
  const locs = occupiedLocations(p);
  assert.equal(locs.length, 1, "both in one place -> one marker");
  assert.deepEqual(locs[0].names.sort(), ["Alice", "Bob"]);
}

console.log("timescrub: all assertions passed");
