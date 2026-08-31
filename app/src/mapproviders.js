// Built-in map worlds for the map view. A location's `map:` value (folded)
// selects a world; empty means Earth. Each provider is a web-mercator (EPSG:3857)
// XYZ tile source, so switching worlds is just swapping the tile URL — no CRS
// juggling (which is why OpenPlanetaryMap, built for web maps, is the right
// source for Mars/Moon). Import-free for mapproviders.test.mjs.
//
// The Mars/Moon tiles are OpenPlanetaryMap's direct S3 tilesets (the cartocdn
// "named map" API endpoint returns blank tiles). Verified live returning PNGs;
// each has a low native max zoom, so `maxNativeZoom` lets Leaflet upscale past it
// instead of blanking. Earth (OSM) is known-good. Unknown `map:` worlds have no
// provider yet: they still appear in the selector and place markers, but with no
// backdrop until the custom-image map lands (#51, needs project assets / #8).

export const PROVIDERS = {
  earth: {
    label: "Earth",
    url: "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
    maxZoom: 19,
    attribution: "© OpenStreetMap contributors",
  },
  mars: {
    label: "Mars",
    url: "https://s3-eu-west-1.amazonaws.com/whereonmars.cartodb.net/celestia_mars-shaded-16k_global/{z}/{x}/{y}.png",
    maxZoom: 10,
    maxNativeZoom: 5,
    tms: true, // OPM S3 tilesets are TMS (y-origin at bottom), unlike OSM's XYZ
    attribution: "NASA / Celestia · OpenPlanetaryMap",
  },
  moon: {
    label: "Moon",
    url: "https://s3.amazonaws.com/opmbuilder/301_moon/tiles/w/hillshaded-albedo/{z}/{x}/{y}.png",
    maxZoom: 10,
    maxNativeZoom: 6,
    tms: true,
    attribution: "NASA / USGS · OpenPlanetaryMap",
  },
};

// Fold a `map:` metadata value to a world key: trimmed, lowercased; empty means
// the default world, Earth. Unknown names pass through as their own world.
export function worldOf(mapValue) {
  const w = (mapValue || "").trim().toLowerCase();
  return w === "" ? "earth" : w;
}

// A human label for a world key — the provider's label, or the key title-cased
// for an unknown (custom) world.
export function worldLabel(world) {
  return PROVIDERS[world]?.label ?? world.charAt(0).toUpperCase() + world.slice(1);
}
