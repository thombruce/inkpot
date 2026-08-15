// Pure text relocation for drag-reorder. Cut the subtree [from,to) out of `doc`
// and reinsert it at `insertAt` (an offset in the *original* doc). Returns the
// new text and the caret offset `at` where the moved block now starts.
//
// node_span carries the blank line(s) a node trails before the next heading, so
// a naive splice leaves uneven seams (headings glued together, or doubled
// gaps). We strip the block's edge blanks and rejoin neighbours with exactly
// one blank line. That's lossless here: N blank lines mean the same as one.
//
// Returns null for a no-op: dropping inside the dragged range itself.
export function spliceMove(doc, from, to, insertAt) {
  if (insertAt >= from && insertAt <= to) return null;

  // The moved subtree, stripped of the blank lines it carried at its edges.
  const block = doc.slice(from, to).replace(/^\n+/, "").replace(/\s+$/, "");

  // Remove the source range; find the insertion point in the remainder.
  const rest = doc.slice(0, from) + doc.slice(to);
  const at = insertAt > to ? insertAt - (to - from) : insertAt;

  // Trim the seam so the moved block and its new neighbours are separated by
  // exactly one blank line.
  const before = rest.slice(0, at).replace(/\s+$/, "");
  const after = rest.slice(at).replace(/^\n+/, "");

  const parts = [before, block, after].filter((s) => s.length > 0);
  let text = parts.join("\n\n").replace(/\s+$/, "");
  if (text.length) text += "\n";

  return { text, at: before.length ? before.length + 2 : 0 };
}
