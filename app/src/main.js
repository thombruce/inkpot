import { EditorView, minimalSetup } from "codemirror";
import { EditorState } from "@codemirror/state";
import { keymap } from "@codemirror/view";
import { search, searchKeymap, highlightSelectionMatches } from "@codemirror/search";
import { autocompletion, completionKeymap, acceptCompletion } from "@codemirror/autocomplete";
import { foldService, foldGutter, codeFolding } from "@codemirror/language";
import { ink } from "./inklang.js";
import { headingDepth, sectionEndLine } from "./fold.js";
import { DOC_KEYS, SCENE_KEYS, metaZone, valueSegment, HEADING } from "./metacomplete.js";
import { spliceMove } from "./reorder.js";
import { scaffoldCharacter } from "./character.js";
import L from "leaflet";
import "leaflet/dist/leaflet.css";
import { characterPositions, occupiedLocations } from "./timescrub.js";
import { PROVIDERS, worldOf, worldLabel } from "./mapproviders.js";
import { buildTree, firstFile } from "./filetree.js";

const { invoke } = window.__TAURI__.core;
const dialog = window.__TAURI__.dialog;
const fs = window.__TAURI__.fs;

const outlineEl = document.getElementById("outline");
const previewEl = document.getElementById("preview");
const codexEl = document.getElementById("codex");
const timelineEl = document.getElementById("timeline");
const charactersEl = document.getElementById("characters");
const filenameEl = document.getElementById("filename");
const wordcountEl = document.getElementById("wordcount");
const recentEl = document.getElementById("recent");
const filesEl = document.getElementById("files");

let currentPath = null; // path of the open file, or null if unsaved
let projectRoot = null; // the project root folder, or null (single-file mode)
let projectTree = []; // nested .ink tree under the root (dirs + files, name order)
const collapsedDirs = new Set(); // dir paths the user has collapsed (survives rescans)
let dirty = false;
let loading = false; // true while replacing the doc programmatically
let draggedNode = null; // outline node being dragged, or null

const INK_FILTERS = [{ name: "inkpot", extensions: ["ink", "md", "txt"] }];

// Recent files: paths persisted in webview localStorage (survives restarts via
// the app's data dir), most-recent first. The app opens the top one on launch.
const RECENT_KEY = "inkpot.recent";
const RECENT_MAX = 10;

function getRecent() {
  try {
    return JSON.parse(localStorage.getItem(RECENT_KEY)) ?? [];
  } catch {
    return [];
  }
}

function pushRecent(path) {
  const list = [path, ...getRecent().filter((p) => p !== path)].slice(0, RECENT_MAX);
  localStorage.setItem(RECENT_KEY, JSON.stringify(list));
  drawRecent();
}

function dropRecent(path) {
  localStorage.setItem(RECENT_KEY, JSON.stringify(getRecent().filter((p) => p !== path)));
  drawRecent();
}

function drawRecent() {
  const list = getRecent();
  recentEl.replaceChildren();
  recentEl.appendChild(new Option(list.length ? "Recent…" : "No recent files", ""));
  for (const p of list) recentEl.appendChild(new Option(p.split("/").pop(), p));
  recentEl.disabled = list.length === 0;
}

function debounce(fn, ms) {
  let t;
  return (...a) => {
    clearTimeout(t);
    t = setTimeout(() => fn(...a), ms);
  };
}

// Codex entity names (titles of `%` excluded headings), refreshed each parse.
// Feeds value completion; a plain snapshot, so the completion source stays sync.
let entityTitles = [];

// Map state: the latest markers from the `map` command, and the Leaflet map +
// marker layer (created lazily the first time the map view is shown — Leaflet
// needs a sized, visible container). `drawMarkers` re-renders from the snapshot.
let mapMarkers = [];
let leafletMap = null;
let markerLayer = null;
let characterLayer = null;
let tileLayer = null;
// Signature of the markers last fitted into view. Refit only when the set
// changes, so returning to the map keeps the user's pan/zoom.
let fittedKey = null;
// Which world (map) is shown; markers and tiles filter to it.
let currentWorld = "earth";
// Time-scrub: the time-ordered scenes, and the cursor as an index into them
// (null = the latest scene, so the map opens with everyone at their last place).
let sceneList = [];
let scrubIndex = null;

// A location's coords + world by name, fold-matched to a map marker (case/space
// folded, like codex resolution). null if the name has no coordinate-bearing
// marker. The world lets the scrub place a character only on their own map.
function coordsOf(name) {
  const key = name.toLowerCase().trim();
  const m = mapMarkers.find((mk) => mk.title.toLowerCase().trim() === key);
  return m ? { lat: m.lat, lon: m.lon, world: worldOf(m.map) } : null;
}

// Swap the tile backdrop to the current world's provider (a built-in world has
// one; an unknown/custom world has none yet — blank backdrop, markers still show).
function setTiles() {
  if (tileLayer) {
    tileLayer.remove();
    tileLayer = null;
  }
  const p = PROVIDERS[currentWorld];
  if (p) {
    tileLayer = L.tileLayer(p.url, {
      maxZoom: p.maxZoom,
      maxNativeZoom: p.maxNativeZoom, // upscale past the tileset's native max
      tms: p.tms, // TMS tilesets (Mars/Moon) have the y-axis flipped vs XYZ
      attribution: p.attribution,
    }).addTo(leafletMap);
  }
}

function initMap() {
  leafletMap = L.map("map").setView([20, 0], 2);
  markerLayer = L.layerGroup().addTo(leafletMap);
  characterLayer = L.layerGroup().addTo(leafletMap); // above the location layer
  setTiles();
  drawMarkers();
  drawScrub();
}

// Switch to a world: swap tiles, re-fit, redraw markers and character positions.
function switchWorld(world) {
  currentWorld = world;
  fittedKey = null; // a different world's markers get a fresh fit
  setTiles();
  drawMarkers();
  drawScrub();
}

// Render the marker snapshot onto the map (no-op until the map exists). Each
// marker jumps to its heading on click, like the codex/timeline links. Colour
// tracks the app's --accent so it matches the palette.
function drawMarkers() {
  if (!markerLayer) return;
  markerLayer.clearLayers();
  const accent = getComputedStyle(document.documentElement).getPropertyValue("--accent").trim() || "#7aa2f7";
  const points = [];
  for (const m of mapMarkers) {
    if (worldOf(m.map) !== currentWorld) continue; // only this world's locations
    const marker = L.circleMarker([m.lat, m.lon], {
      radius: 6,
      color: accent,
      fillColor: accent,
      fillOpacity: 0.8,
    });
    marker.bindTooltip(m.title || "(untitled)");
    marker.on("click", () => {
      setView("editor");
      jumpTo(m.offset);
    });
    marker.addTo(markerLayer);
    points.push([m.lat, m.lon]);
  }
  // Fit only when the markers changed and the map is visible (fitBounds on a
  // hidden/0-size container mis-measures); an unchanged set keeps the view.
  const key = points.map((p) => p.join()).join(";");
  if (points.length && key !== fittedKey && document.body.classList.contains("show-map")) {
    leafletMap.fitBounds(points, { padding: [40, 40], maxZoom: 12 });
    fittedKey = key;
  }
}

const scrubBar = document.getElementById("scrubBar");
const scrubInput = document.getElementById("scrub");
const scrubLabel = document.getElementById("scrubLabel");

// The cursor's scene index (clamped): the slider's value, defaulting to the last
// scene when the user hasn't scrubbed.
function scrubCursor() {
  const last = sceneList.length - 1;
  if (last < 0) return -1;
  return Math.min(scrubIndex ?? last, last);
}

// Reflect the current scene set in the slider (range, value, visibility) and
// redraw the character markers. Called after each parse and on map show.
function updateScrub() {
  const last = sceneList.length - 1;
  // Only worth showing if some scene can actually place a character on the map.
  const placeable = sceneList.some((s) => s.characters.length && coordsOf(s.location));
  scrubBar.hidden = !placeable;
  if (!placeable) {
    if (characterLayer) characterLayer.clearLayers();
    return;
  }
  scrubInput.max = String(last);
  scrubInput.value = String(scrubCursor());
  drawScrub();
}

// Draw character markers for the current cursor: one amber marker per occupied
// location, labelled with who is there, plus the cursor's scene in the readout.
function drawScrub() {
  if (!characterLayer) return;
  characterLayer.clearLayers();
  const cursor = scrubCursor();
  if (cursor < 0) return;
  const scene = sceneList[cursor];
  scrubLabel.textContent = scene ? `${scene.time} — ${scene.title || "(untitled)"}` : "";
  // Only characters on the currently-shown world.
  const positions = characterPositions(sceneList, coordsOf, cursor);
  const here = {};
  for (const [name, p] of Object.entries(positions)) {
    if (p.world === currentWorld) here[name] = p;
  }
  for (const loc of occupiedLocations(here)) {
    L.circleMarker([loc.lat, loc.lon], {
      radius: 7,
      color: "#e0af68",
      fillColor: "#e0af68",
      fillOpacity: 0.9,
    })
      .bindTooltip(loc.names.join(", "), { permanent: true, direction: "top", className: "scrub-tip" })
      .addTo(characterLayer);
  }
}

scrubInput.addEventListener("input", () => {
  scrubIndex = Number(scrubInput.value);
  drawScrub();
});

const worldBar = document.getElementById("worldBar");
const worldSelect = document.getElementById("world");

// Populate the world selector from the worlds present in the markers (Earth
// always offered). Hidden unless there's more than one world. The current
// selection stays sticky — kept in the list even if its markers momentarily
// vanish — so a transient parse mid-edit never silently kicks the user to Earth
// (and, since currentWorld never changes here, the tiles never go stale).
function updateWorlds() {
  const worlds = new Set(mapMarkers.map((m) => worldOf(m.map)));
  worlds.add("earth");
  worlds.add(currentWorld);
  const ordered = ["earth", ...[...worlds].filter((w) => w !== "earth").sort()];
  worldBar.hidden = ordered.length < 2;
  worldSelect.replaceChildren();
  for (const w of ordered) {
    const opt = document.createElement("option");
    opt.value = w;
    opt.textContent = worldLabel(w);
    opt.selected = w === currentWorld;
    worldSelect.appendChild(opt);
  }
}

worldSelect.addEventListener("change", () => switchWorld(worldSelect.value));

function collectEntities(node, out) {
  if (node.visibility === "excluded" && node.title) out.push(node.title);
  for (const child of node.children) collectEntities(child, out);
  return out;
}

const refresh = debounce(async () => {
  const src = editor.state.doc.toString();
  const [tree, html, codexHtml, timelineHtml, charactersHtml, markers, sceneData] =
    await Promise.all([
      invoke("outline", { src }),
      invoke("preview", { src }),
      invoke("codex", { src }),
      invoke("timeline", { src }),
      invoke("characters", { src }),
      invoke("map", { src }),
      invoke("scenes", { src }),
    ]);
  entityTitles = [...new Set(collectEntities(tree, []))];
  drawOutline(tree);
  previewEl.innerHTML = html;
  codexEl.innerHTML = codexHtml;
  timelineEl.innerHTML = timelineHtml;
  charactersEl.innerHTML = charactersHtml;
  mapMarkers = markers;
  sceneList = sceneData;
  updateWorlds();
  drawMarkers();
  updateScrub();
  // Root carries the whole-document manuscript word count.
  wordcountEl.textContent = `${tree.words.toLocaleString()} words`;
}, 150);

// A dark theme matching the app palette.
const theme = EditorView.theme(
  {
    "&": { height: "100%", backgroundColor: "#1e1e24", color: "#e6e6ea" },
    ".cm-content": {
      caretColor: "#7aa2f7",
      fontFamily: "ui-monospace, monospace",
      fontSize: "14px",
      lineHeight: "1.6",
      padding: "16px 0",
    },
    ".cm-cursor": { borderLeftColor: "#7aa2f7" },
    "&.cm-focused": { outline: "none" },
    ".cm-line": { padding: "0 16px" },
    // Fold gutter: blend into the editor, dim arrows that light up on hover.
    ".cm-gutters": { backgroundColor: "#1e1e24", color: "#6a6a76", border: "none" },
    ".cm-foldGutter .cm-gutterElement": { cursor: "pointer", color: "#6a6a76" },
    ".cm-foldGutter .cm-gutterElement:hover": { color: "#7aa2f7" },
    ".cm-foldPlaceholder": {
      backgroundColor: "#2a2a33",
      border: "1px solid #34343e",
      color: "#9a9aa6",
      margin: "0 4px",
      padding: "0 6px",
      borderRadius: "4px",
    },
    // Find/replace panel + match highlights, on the dark palette.
    ".cm-panels": { backgroundColor: "#26262e", color: "#e6e6ea" },
    ".cm-panels.cm-panels-top": { borderBottom: "1px solid #34343e" },
    ".cm-search label, .cm-search button, .cm-search input": { fontSize: "12px" },
    ".cm-search input": {
      backgroundColor: "#1e1e24",
      color: "#e6e6ea",
      border: "1px solid #34343e",
      borderRadius: "3px",
    },
    ".cm-search button": {
      backgroundColor: "transparent",
      color: "#e6e6ea",
      border: "1px solid #34343e",
      borderRadius: "3px",
      cursor: "pointer",
    },
    ".cm-search button:hover": { borderColor: "#7aa2f7" },
    ".cm-selectionMatch": { backgroundColor: "#7aa2f733" },
    ".cm-searchMatch": { backgroundColor: "#7aa2f733", outline: "1px solid #7aa2f7" },
    ".cm-searchMatch.cm-searchMatch-selected": { backgroundColor: "#c9a26b66" },
    // Autocomplete tooltip (metadata keys).
    ".cm-tooltip": {
      backgroundColor: "#26262e",
      border: "1px solid #34343e",
      color: "#e6e6ea",
      borderRadius: "4px",
    },
    ".cm-tooltip-autocomplete ul li": { fontFamily: "ui-monospace, monospace" },
    ".cm-tooltip-autocomplete ul li[aria-selected]": {
      backgroundColor: "#7aa2f7",
      color: "#1e1e24",
    },
  },
  { dark: true },
);

// Fold a heading's whole section (the body below it, down to the next heading of
// depth <= its own). Reparses nothing — reads marker depth straight off the text.
const inkFold = foldService.of((state, lineStart) => {
  const head = state.doc.lineAt(lineStart).number;
  const endLine = sectionEndLine(
    (n) => headingDepth(state.doc.line(n).text),
    state.doc.lines,
    head,
  );
  if (endLine == null) return null;
  const from = state.doc.line(head).to; // fold from end of the heading line
  const to = state.doc.line(endLine).to;
  return to > from ? { from, to } : null;
});

// Complete metadata keys inside a meta zone: document front matter at the top
// (title/author/…) or a heading's meta block (pov/time/…). Only while typing the
// key — caret before any colon, no leading indent.
function completeMetaKey(context) {
  const line = context.state.doc.lineAt(context.pos);
  const before = line.text.slice(0, context.pos - line.from);
  if (!/^[\w-]*$/.test(before)) return null; // past the key (colon/space) or not a key token
  if (!context.explicit && before.length === 0) return null; // don't pop on an empty line
  const zone = metaZone((n) => context.state.doc.line(n).text, line.number);
  if (!zone) return null;
  const keys = zone === "front" ? DOC_KEYS : SCENE_KEYS;
  return {
    from: line.from,
    options: keys.map((k) => ({ label: k, type: "property", apply: `${k}: ` })),
  };
}

// Complete codex entity names inside a scene meta value: `characters: Ali|` or
// after a comma. Only in a heading's meta block (front matter names the work, not
// entities), and only once past the colon. Reads the `entityTitles` snapshot.
function completeMetaValue(context) {
  const line = context.state.doc.lineAt(context.pos);
  // metaZone reports the prior open zone; it can't tell the current line is itself
  // a heading (a colon in a title — "Chapter 1: The Meeting" — would look like a
  // value with no blank line before it). Bail on heading lines explicitly.
  if (HEADING.test(line.text)) return null;
  if (metaZone((n) => context.state.doc.line(n).text, line.number) !== "scene") return null;
  const seg = valueSegment(line.text, context.pos - line.from);
  if (!seg) return null; // still in the key
  if (!context.explicit && seg.typed.length === 0) return null;
  const q = seg.typed.toLowerCase();
  const options = entityTitles
    .filter((t) => t.toLowerCase().includes(q))
    .map((t) => ({ label: t, type: "variable" }));
  if (options.length === 0) return null;
  return { from: line.from + seg.fromCol, options };
}

const editor = new EditorView({
  parent: document.getElementById("editor"),
  state: EditorState.create({
    doc: "",
    extensions: [
      minimalSetup,
      EditorView.lineWrapping,
      ink(),
      codeFolding(),
      foldGutter(),
      inkFold,
      search({ top: true }),
      highlightSelectionMatches(),
      autocompletion({ override: [completeMetaKey, completeMetaValue] }),
      // Tab accepts the highlighted completion; a no-op (falls through) when the
      // tooltip is closed. Enter also accepts, via completionKeymap.
      keymap.of([{ key: "Tab", run: acceptCompletion }, ...searchKeymap, ...completionKeymap]),
      theme,
      EditorView.updateListener.of((u) => {
        if (!u.docChanged) return;
        if (!loading) {
          markDirty(true); // skip programmatic loads
          autosave();
        }
        refresh();
      }),
    ],
  }),
});

function drawOutline(root) {
  outlineEl.replaceChildren();
  for (const child of root.children) walk(child);

  function walk(node) {
    const el = document.createElement("div");
    el.className = "item " + node.visibility; // visible | scene | excluded
    el.style.paddingLeft = 12 + (node.level - 1) * 14 + "px";
    const sigil = MARKER[node.visibility].repeat(node.level);
    const label = document.createElement("span");
    label.className = "label";
    label.textContent = `${sigil} ${node.title || "(untitled)"}`;
    label.title = node.title || "(untitled)"; // full text when ellipsized
    if (node.meta_keys.length) {
      const keys = document.createElement("span");
      keys.className = "keys";
      keys.textContent = `[${node.meta_keys.join(", ")}]`;
      label.appendChild(keys);
    }
    el.appendChild(label);
    if (node.words > 0) {
      const wc = document.createElement("span");
      wc.className = "wc";
      wc.textContent = node.words.toLocaleString();
      wc.title = `${node.words.toLocaleString()} words`;
      el.appendChild(wc);
    }
    el.addEventListener("click", () => jumpTo(node.heading_span.start));
    makeDraggable(el, node);

    // Segmented state set: click any of # / ~ / % to go straight to that state.
    const current = MARKER[node.visibility];
    const controls = document.createElement("span");
    controls.className = "controls";
    for (const [glyph, title] of STATES) {
      const active = glyph === current;
      controls.appendChild(
        makeToggle(glyph, active, title, () => {
          if (!active) setMarker(node, glyph);
        }),
      );
    }
    el.appendChild(controls);

    outlineEl.appendChild(el);
    for (const c of node.children) walk(c);
  }
}

// The three heading states, as (sigil, tooltip), and the visibility -> sigil map.
const STATES = [
  ["#", "Visible heading"],
  ["~", "Scene (hidden heading, body prints)"],
  ["%", "Excluded from manuscript"],
];
const MARKER = { visible: "#", scene: "~", excluded: "%" };

// Build an outline control button that sets a heading state.
function makeToggle(glyph, active, title, onClick) {
  const b = document.createElement("button");
  b.className = "toggle" + (active ? " active" : "");
  b.textContent = glyph;
  b.draggable = false;
  b.title = title;
  b.addEventListener("click", (e) => {
    e.stopPropagation();
    onClick();
  });
  return b;
}

// Rewrite a heading's marker run (its `level` sigil chars) in place, switching
// its state. The states are exclusive — `%` overwrites `#`/`~` and vice versa —
// so there is nothing to "remember"; you pick the state you want directly.
function setMarker(node, marker) {
  const { start } = node.heading_span;
  editor.dispatch({
    changes: { from: start, to: start + node.level, insert: marker.repeat(node.level) },
  });
}

// Move the caret to a char offset and scroll it into view. Char offsets match
// CodeMirror positions for BMP text; astral chars (emoji) would drift.
function jumpTo(offset) {
  const pos = Math.min(offset, editor.state.doc.length);
  editor.dispatch({ selection: { anchor: pos }, scrollIntoView: true });
  editor.focus();
}

// Drag-reorder: dropping node A onto node B relocates A's whole subtree text
// to before/after B (upper/lower half of the target). It's a pure text move —
// A keeps its own heading markers, so a reparse re-derives the new hierarchy.
function makeDraggable(el, node) {
  el.draggable = true;
  el.addEventListener("dragstart", (e) => {
    draggedNode = node;
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", String(node.id));
  });
  el.addEventListener("dragover", (e) => {
    if (!draggedNode || draggedNode.id === node.id) return;
    e.preventDefault();
    const rect = el.getBoundingClientRect();
    const after = e.clientY > rect.top + rect.height / 2;
    el.classList.toggle("drop-before", !after);
    el.classList.toggle("drop-after", after);
  });
  el.addEventListener("dragleave", () => {
    el.classList.remove("drop-before", "drop-after");
  });
  el.addEventListener("drop", (e) => {
    e.preventDefault();
    const after = el.classList.contains("drop-after");
    el.classList.remove("drop-before", "drop-after");
    if (draggedNode && draggedNode.id !== node.id) {
      moveNode(draggedNode, node, after ? "after" : "before");
    }
    draggedNode = null;
  });
}

function moveNode(drag, target, pos) {
  const { start: from, end: to } = drag.node_span;
  const insertAt = pos === "before" ? target.node_span.start : target.node_span.end;
  const moved = spliceMove(editor.state.doc.toString(), from, to, insertAt);
  if (!moved) return; // drop inside the dragged subtree — no-op

  editor.dispatch({
    changes: { from: 0, to: editor.state.doc.length, insert: moved.text },
    selection: { anchor: moved.at },
    scrollIntoView: true,
  });
}

// --- File open/save -------------------------------------------------------

function markDirty(d) {
  dirty = d;
  updateFilename();
}

function updateFilename() {
  const base = currentPath ? currentPath.split("/").pop() : "untitled";
  filenameEl.textContent = dirty ? `${base} *` : base;
}

// Replace the whole document without tripping the dirty flag.
function setDoc(text) {
  loading = true;
  editor.dispatch({
    changes: { from: 0, to: editor.state.doc.length, insert: text },
  });
  loading = false;
}

// Returns false if there are unsaved changes and the user declined to discard.
async function confirmDiscard() {
  if (!dirty) return true;
  return dialog.confirm("Discard unsaved changes?", { title: "inkpot" });
}

async function newFile() {
  if (!(await confirmDiscard())) return;
  setDoc("");
  currentPath = null;
  markDirty(false);
  refresh();
}

// Load a path into the buffer (no discard guard — callers clear first). Returns
// false if the file is gone/unreadable, dropping it from the recent list.
async function loadPath(path) {
  let text;
  try {
    text = await fs.readTextFile(path);
  } catch {
    dropRecent(path); // moved or deleted — forget it
    return false;
  }
  setDoc(text);
  currentPath = path;
  markDirty(false);
  pushRecent(path);
  refresh();
  await syncProject(); // adopt/keep the project root and redraw the file tree
  return true;
}

async function openFile() {
  if (!(await confirmDiscard())) return;
  const path = await dialog.open({ multiple: false, filters: INK_FILTERS });
  if (!path) return; // cancelled
  await loadPath(path);
}

// Open a folder as the project root and load its first file. A project is just a
// folder + the .ink tree under it — app-layer only, no state across IPC.
async function openFolder() {
  if (!(await confirmDiscard())) return;
  const dir = await dialog.open({ directory: true });
  if (!dir) return; // cancelled
  setRoot(dir);
  await scanProject();
  const first = firstFile(projectTree);
  if (first) await loadPath(first); // loadPath -> syncProject keeps this root
  else drawFiles(); // empty project: still show the (empty) rail
}

// Re-read the root's `.ink` tree (via the pure builder) and redraw. No-op without
// a project root.
async function scanProject() {
  if (!projectRoot) return;
  projectTree = await buildTree(projectRoot, fs.readDir);
  drawFiles();
}

// Keep the project in step with the active file: adopt the file's own directory
// as the root when there's no project, or when the file sits outside the current
// root; then rescan. Called after every load, so New/Save-As/open-elsewhere all
// re-sync the tree instead of leaving a stale snapshot.
async function syncProject() {
  if (!currentPath) return;
  const dir = currentPath.slice(0, currentPath.lastIndexOf("/"));
  if (!projectRoot || !currentPath.startsWith(projectRoot + "/")) setRoot(dir);
  await scanProject();
}

// Set the project root, forgetting the previous project's collapsed-dir state
// (its absolute paths are meaningless in a new project). No-op if unchanged.
function setRoot(dir) {
  if (dir === projectRoot) return;
  projectRoot = dir;
  collapsedDirs.clear();
}

// Render the project `.ink` tree at the top of the outline rail. Hidden in
// single-file mode; the active file is marked, directories are collapsible and
// keep their collapsed state across rescans.
function drawFiles() {
  filesEl.hidden = projectRoot === null;
  filesEl.replaceChildren(...projectTree.map(renderNode));
}

function renderNode(node) {
  if (node.children) {
    const details = document.createElement("details");
    details.open = !collapsedDirs.has(node.path);
    details.addEventListener("toggle", () => {
      if (details.open) collapsedDirs.delete(node.path);
      else collapsedDirs.add(node.path);
    });
    const summary = document.createElement("summary");
    summary.className = "file-dir";
    summary.textContent = node.name;
    details.append(summary, ...node.children.map(renderNode));
    return details;
  }
  const item = document.createElement("div");
  item.className = "file-item" + (node.path === currentPath ? " active" : "");
  item.textContent = node.name;
  item.title = node.path;
  item.addEventListener("click", () => switchFile(node.path));
  return item;
}

// Switch the active project file (with the usual discard guard). A no-op if it's
// already active; loadPath -> syncProject redraws and re-marks the tree.
async function switchFile(path) {
  if (path === currentPath) return;
  if (!(await confirmDiscard())) return;
  await loadPath(path);
}

async function saveFile() {
  if (!currentPath) return saveFileAs();
  await fs.writeTextFile(currentPath, editor.state.doc.toString());
  markDirty(false);
}

// Autosave: write the buffer to its file once edits settle. Only fires when the
// doc has a path — untitled/example buffers have nowhere to go and are left for
// the user to Save As first (crash recovery of untitled drafts is the stretch
// goal in issue #12).
async function autosaveNow() {
  if (!currentPath || !dirty) return;
  await fs.writeTextFile(currentPath, editor.state.doc.toString());
  markDirty(false);
}
const autosave = debounce(autosaveNow, 1000);

// Flush a pending autosave on the way out, so the ~1s debounce window can't
// swallow the last edits. blur covers app-switching; beforeunload covers close.
window.addEventListener("blur", autosaveNow);
window.addEventListener("beforeunload", autosaveNow);

// Guard the window close: flush any pathed autosave first, then — if edits
// remain unsaved (i.e. an untitled buffer with nowhere to autosave) — ask
// before discarding. Tauri intercepts the OS close, so a browser beforeunload
// prompt won't fire here; this is the real gate.
window.__TAURI__.window?.getCurrentWindow().onCloseRequested(async (event) => {
  await autosaveNow();
  if (!(await confirmDiscard())) event.preventDefault();
});

async function saveFileAs() {
  const path = await dialog.save({
    defaultPath: currentPath ?? "untitled.ink",
    filters: INK_FILTERS,
  });
  if (!path) return; // cancelled
  await fs.writeTextFile(path, editor.state.doc.toString());
  currentPath = path;
  markDirty(false);
  pushRecent(path);
  await syncProject(); // the new file (or new folder) shows up in the tree
}

// A file may have been added/removed in the folder outside the app; re-scan when
// the window regains focus so the tree stays current.
window.addEventListener("focus", () => {
  scanProject();
});

const outlineBtn = document.getElementById("toggleOutline");
outlineBtn.addEventListener("click", () => {
  const hidden = document.body.classList.toggle("hide-outline");
  outlineBtn.classList.toggle("active", !hidden);
});

// Editor / preview / codex / timeline share one space; the toolbar toggles which
// shows. They are mutually exclusive (each hides the editor), so switching one
// off returns to the editor and turning one on clears the others.
const previewBtn = document.getElementById("togglePreview");
const codexBtn = document.getElementById("toggleCodex");
const timelineBtn = document.getElementById("toggleTimeline");
const charactersBtn = document.getElementById("toggleCharacters");
const mapBtn = document.getElementById("toggleMap");

function setView(view) {
  document.body.classList.toggle("show-preview", view === "preview");
  document.body.classList.toggle("show-codex", view === "codex");
  document.body.classList.toggle("show-timeline", view === "timeline");
  document.body.classList.toggle("show-characters", view === "characters");
  document.body.classList.toggle("show-map", view === "map");
  previewBtn.textContent = view === "preview" ? "Edit" : "Preview";
  previewBtn.classList.toggle("active", view === "preview");
  codexBtn.classList.toggle("active", view === "codex");
  timelineBtn.classList.toggle("active", view === "timeline");
  charactersBtn.classList.toggle("active", view === "characters");
  mapBtn.classList.toggle("active", view === "map");
  // Leaflet needs a visible, sized container: create it on first show, and
  // recompute its size on later shows (it was display:none in between).
  if (view === "map") {
    if (!leafletMap) initMap();
    else {
      leafletMap.invalidateSize();
      drawMarkers();
      drawScrub();
    }
  }
}

previewBtn.addEventListener("click", () => {
  setView(document.body.classList.contains("show-preview") ? "editor" : "preview");
});
codexBtn.addEventListener("click", () => {
  setView(document.body.classList.contains("show-codex") ? "editor" : "codex");
});
timelineBtn.addEventListener("click", () => {
  setView(document.body.classList.contains("show-timeline") ? "editor" : "timeline");
});
charactersBtn.addEventListener("click", () => {
  setView(document.body.classList.contains("show-characters") ? "editor" : "characters");
});
mapBtn.addEventListener("click", () => {
  setView(document.body.classList.contains("show-map") ? "editor" : "map");
});

// Codex, timeline, and character links carry the target heading's char offset.
// The editor is hidden while they show, so switch back first, then scroll to it.
for (const panel of [codexEl, timelineEl, charactersEl]) {
  panel.addEventListener("click", (e) => {
    const link = e.target.closest("[data-jump]");
    if (!link) return;
    setView("editor");
    jumpTo(Number(link.dataset.jump));
  });
}

// Scaffold a new character: splice a `%% Name` template into the buffer (text
// stays canonical), switch to the editor, and select the placeholder name so it
// can be typed over. A reparse redraws the panels.
document.getElementById("newCharacter").addEventListener("click", () => {
  const { text, selFrom, selTo } = scaffoldCharacter(editor.state.doc.toString());
  setView("editor");
  editor.dispatch({
    changes: { from: 0, to: editor.state.doc.length, insert: text },
    selection: { anchor: selFrom, head: selTo },
    scrollIntoView: true,
  });
  editor.focus();
});

// Export the rendered manuscript (visible headings + resolved markup, scenes
// and excluded subtrees dropped) as plain text — parse/render stays in Rust.
async function exportManuscript() {
  const text = await invoke("manuscript", { src: editor.state.doc.toString() });
  const base = currentPath
    ? currentPath.split("/").pop().replace(/\.[^.]+$/, "")
    : "manuscript";
  const path = await dialog.save({
    defaultPath: `${base}.md`,
    filters: [{ name: "Markdown", extensions: ["md", "txt"] }],
  });
  if (!path) return; // cancelled
  await fs.writeTextFile(path, text);
}

// Recent-files dropdown: pick one to open it (with the usual discard guard).
recentEl.addEventListener("change", async () => {
  const path = recentEl.value;
  recentEl.selectedIndex = 0; // snap back to the "Recent…" label
  if (path && (await confirmDiscard())) await loadPath(path);
});

document.getElementById("export").addEventListener("click", exportManuscript);
document.getElementById("new").addEventListener("click", newFile);
document.getElementById("open").addEventListener("click", openFile);
document.getElementById("openFolder").addEventListener("click", openFolder);
document.getElementById("save").addEventListener("click", saveFile);
document.getElementById("saveAs").addEventListener("click", saveFileAs);

window.addEventListener("keydown", (e) => {
  if (!(e.ctrlKey || e.metaKey)) return;
  if (e.key === "s") {
    e.preventDefault();
    e.shiftKey ? saveFileAs() : saveFile();
  } else if (e.key === "o") {
    e.preventDefault();
    openFile();
  } else if (e.key === "n") {
    e.preventDefault();
    newFile();
  }
});

// Startup: reopen the most recent file (blank buffer if none, or if it's gone).
drawRecent();
(async () => {
  const last = getRecent()[0];
  if (!(last && (await loadPath(last)))) refresh(); // loadPath refreshes on success
})();
