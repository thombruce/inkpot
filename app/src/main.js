import { EditorView, minimalSetup } from "codemirror";
import { EditorState } from "@codemirror/state";
import { keymap } from "@codemirror/view";
import { search, searchKeymap, highlightSelectionMatches } from "@codemirror/search";
import { autocompletion, completionKeymap } from "@codemirror/autocomplete";
import { foldService, foldGutter, codeFolding } from "@codemirror/language";
import { ink } from "./inklang.js";
import { headingDepth, sectionEndLine } from "./fold.js";
import { DOC_KEYS, SCENE_KEYS, metaZone } from "./metacomplete.js";
import { spliceMove } from "./reorder.js";

const { invoke } = window.__TAURI__.core;
const dialog = window.__TAURI__.dialog;
const fs = window.__TAURI__.fs;

const outlineEl = document.getElementById("outline");
const previewEl = document.getElementById("preview");
const filenameEl = document.getElementById("filename");
const wordcountEl = document.getElementById("wordcount");
const recentEl = document.getElementById("recent");

let currentPath = null; // path of the open file, or null if unsaved
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

const refresh = debounce(async () => {
  const src = editor.state.doc.toString();
  const [tree, html] = await Promise.all([
    invoke("outline", { src }),
    invoke("preview", { src }),
  ]);
  drawOutline(tree);
  previewEl.innerHTML = html;
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
      autocompletion({ override: [completeMetaKey] }),
      keymap.of([...searchKeymap, ...completionKeymap]),
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
  return true;
}

async function openFile() {
  if (!(await confirmDiscard())) return;
  const path = await dialog.open({ multiple: false, filters: INK_FILTERS });
  if (!path) return; // cancelled
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
}

const outlineBtn = document.getElementById("toggleOutline");
outlineBtn.addEventListener("click", () => {
  const hidden = document.body.classList.toggle("hide-outline");
  outlineBtn.classList.toggle("active", !hidden);
});

const previewBtn = document.getElementById("togglePreview");
previewBtn.addEventListener("click", () => {
  const showing = document.body.classList.toggle("show-preview");
  previewBtn.textContent = showing ? "Edit" : "Preview";
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
