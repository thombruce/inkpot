// Pure text relocation for drag-reorder. Cut [from,to) out of `doc` and
// reinsert it at `insertAt` (an offset in the *original* doc). Returns the new
// text and the caret offset `at` where the moved chunk now starts.
//
// Returns null for a no-op: dropping inside the dragged range itself.
export function spliceMove(doc, from, to, insertAt) {
  if (insertAt >= from && insertAt <= to) return null;
  const chunk = doc.slice(from, to);
  const without = doc.slice(0, from) + doc.slice(to);
  // Removing [from,to) shifts anything past `to` left by the chunk length.
  const at = insertAt > to ? insertAt - (to - from) : insertAt;
  return { text: without.slice(0, at) + chunk + without.slice(at), at };
}
