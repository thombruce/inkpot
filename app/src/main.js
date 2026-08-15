import { EditorView, minimalSetup } from "codemirror";
import { EditorState } from "@codemirror/state";
import { ink } from "./inklang.js";

const { invoke } = window.__TAURI__.core;

const outlineEl = document.getElementById("outline");
const previewEl = document.getElementById("preview");
const viewbar = document.querySelector(".viewbar");

let view = "manuscript";

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
        if (u.docChanged) refresh();
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

viewbar.addEventListener("click", (e) => {
  const btn = e.target.closest("button");
  if (!btn) return;
  view = btn.dataset.view;
  for (const b of viewbar.children) b.classList.toggle("active", b === btn);
  refresh();
});

refresh();
