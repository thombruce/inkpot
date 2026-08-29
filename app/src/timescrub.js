// Pure time-scrub logic: given the time-ordered scenes and a way to look up a
// location's coordinates, work out where each character is as of a cursor. The
// cursor is an *index* into the sorted scenes (ordinal), so this is agnostic to
// the time format — it just walks the order ink-core already produced. Leaflet
// rendering lives in main.js; this stays import-free for timescrub.test.mjs.

// Where each character is as of scene index `cursor` (inclusive): their most
// recent scene (at or before the cursor) that names both them and a locatable
// place. A scene's `exits:` removes those characters from that scene onward
// (death, departure) — applied after the scene's placements, so an exit wins at
// its own scene; naming a character again later re-adds them. `coordsOf(name)`
// returns { lat, lon } or null (a location with no coords, or an unknown name).
// Returns { character -> { location, lat, lon } }.
export function characterPositions(scenes, coordsOf, cursor) {
  const positions = {};
  const end = Math.min(cursor, scenes.length - 1);
  for (let i = 0; i <= end; i++) {
    const scene = scenes[i];
    const coords = scene.location ? coordsOf(scene.location) : null;
    if (coords) {
      for (const name of scene.characters) {
        positions[name] = { location: scene.location, lat: coords.lat, lon: coords.lon };
      }
    }
    for (const name of scene.exits ?? []) delete positions[name];
  }
  return positions;
}

// Group positions by location for rendering: one entry per occupied place, with
// the characters present there — so overlapping characters become a single
// labelled marker rather than a stack. Returns [{ location, lat, lon, names }].
export function occupiedLocations(positions) {
  const byLoc = {};
  for (const [name, p] of Object.entries(positions)) {
    const key = `${p.lat},${p.lon}`;
    (byLoc[key] ??= { location: p.location, lat: p.lat, lon: p.lon, names: [] }).names.push(name);
  }
  return Object.values(byLoc);
}
