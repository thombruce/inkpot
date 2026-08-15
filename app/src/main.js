import { EditorView, minimalSetup } from "codemirror";
import { EditorState } from "@codemirror/state";
import { ink } from "./inklang.js";
import { spliceMove } from "./reorder.js";

const { invoke } = window.__TAURI__.core;
const dialog = window.__TAURI__.dialog;
const fs = window.__TAURI__.fs;

const outlineEl = document.getElementById("outline");
const previewEl = document.getElementById("preview");
const viewbar = document.querySelector(".viewbar");
const filenameEl = document.getElementById("filename");

let view = "manuscript";
let currentPath = null; // path of the open file, or null if unsaved
let dirty = false;
let loading = false; // true while replacing the doc programmatically
let draggedNode = null; // outline node being dragged, or null

const INK_FILTERS = [{ name: "inkpot", extensions: ["ink", "md", "txt"] }];

const SAMPLE = `# Chapter 1

## The Arrival

~~~ The Kitchen
time: dawn
pov: Alice

She stood at the counter. {+Steam rose from the kettle.} The window was
{~grey~pale with morning}. {/is this too early in the timeline?}

~~~ The Hallway
time: dawn

They passed in the **narrow** hall without a *word*.

# Chapter 2

## Departure

The train left at noon.
`;

function debounce(fn, ms) {
  let t;
  return (...a) => {
    clearTimeout(t);
    t = setTimeout(() => fn(...a), ms);
  };
}

const refresh = debounce(async () => {
  const src = editor.state.doc.toString();
  const [tree, rendered] = await Promise.all([
    invoke("outline", { src }),
    invoke("render", { src, view }),
  ]);
  drawOutline(tree);
  previewEl.textContent = rendered;
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
  },
  { dark: true },
);

const editor = new EditorView({
  parent: document.getElementById("editor"),
  state: EditorState.create({
    doc: SAMPLE,
    extensions: [
      minimalSetup,
      EditorView.lineWrapping,
      ink(),
      theme,
      EditorView.updateListener.of((u) => {
        if (!u.docChanged) return;
        if (!loading) markDirty(true); // skip programmatic loads
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
    el.className = "item" + (node.visible ? "" : " scene");
    el.style.paddingLeft = 12 + (node.level - 1) * 14 + "px";
    const sigil = (node.visible ? "#" : "~").repeat(node.level);
    el.textContent = `${sigil} ${node.title || "(untitled)"}`;
    if (node.meta_keys.length) {
      const keys = document.createElement("span");
      keys.className = "keys";
      keys.textContent = `[${node.meta_keys.join(", ")}]`;
      el.appendChild(keys);
    }
    el.addEventListener("click", () => jumpTo(node.heading_span.start));
    makeDraggable(el, node);
    outlineEl.appendChild(el);
    for (const c of node.children) walk(c);
  }
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

viewbar.addEventListener("click", (e) => {
  const btn = e.target.closest("button");
  if (!btn) return;
  view = btn.dataset.view;
  for (const b of viewbar.children) b.classList.toggle("active", b === btn);
  refresh();
});

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

async function openFile() {
  if (dirty) {
    const ok = await dialog.confirm("Discard unsaved changes?", { title: "inkpot" });
    if (!ok) return;
  }
  const path = await dialog.open({ multiple: false, filters: INK_FILTERS });
  if (!path) return; // cancelled
  const text = await fs.readTextFile(path);
  setDoc(text);
  currentPath = path;
  markDirty(false);
  refresh();
}

async function saveFile() {
  if (!currentPath) return saveFileAs();
  await fs.writeTextFile(currentPath, editor.state.doc.toString());
  markDirty(false);
}

async function saveFileAs() {
  const path = await dialog.save({
    defaultPath: currentPath ?? "untitled.ink",
    filters: INK_FILTERS,
  });
  if (!path) return; // cancelled
  await fs.writeTextFile(path, editor.state.doc.toString());
  currentPath = path;
  markDirty(false);
}

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
  }
});

refresh();
