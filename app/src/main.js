const { invoke } = window.__TAURI__.core;

const srcEl = document.getElementById("src");
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

// Debounce re-parse/render so we don't hammer IPC on every keystroke.
function debounce(fn, ms) {
  let t;
  return (...a) => {
    clearTimeout(t);
    t = setTimeout(() => fn(...a), ms);
  };
}

async function refresh() {
  const src = srcEl.value;
  const [tree, rendered] = await Promise.all([
    invoke("outline", { src }),
    invoke("render", { src, view }),
  ]);
  drawOutline(tree);
  previewEl.textContent = rendered;
}

function drawOutline(root) {
  outlineEl.replaceChildren();
  // Skip the level-0 root; render its descendants.
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
    // Click jumps the editor caret to this heading.
    el.addEventListener("click", () => jumpTo(node.heading_span.start));
    outlineEl.appendChild(el);
    for (const c of node.children) walk(c);
  }
}

// Move the caret to a char offset and scroll it into view. Char offsets match
// JS UTF-16 indexing for BMP text; astral chars (emoji) would drift.
function jumpTo(offset) {
  srcEl.focus();
  srcEl.setSelectionRange(offset, offset);
  // Approximate scroll: proportion of the char offset through the document.
  const ratio = offset / Math.max(1, srcEl.value.length);
  srcEl.scrollTop = ratio * (srcEl.scrollHeight - srcEl.clientHeight);
}

viewbar.addEventListener("click", (e) => {
  const btn = e.target.closest("button");
  if (!btn) return;
  view = btn.dataset.view;
  for (const b of viewbar.children) b.classList.toggle("active", b === btn);
  refresh();
});

srcEl.addEventListener("input", debounce(refresh, 150));
srcEl.value = SAMPLE;
refresh();
