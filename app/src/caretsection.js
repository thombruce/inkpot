// Pure caret->outline-section pick (#19). Given rows carrying their subtree span
// `{ start, end }`, return the index of the row for the section the caret sits
// in, or -1 if none. Child spans nest inside parents, so the innermost match is
// the row with the greatest start. `pos < end` keeps a caret on a section
// boundary out of the previous sibling; a caret at the very end of the document
// (`pos === docLen`) still lands in the last containing section.
export function deepestSectionAt(items, pos, docLen) {
  let best = -1;
  for (let i = 0; i < items.length; i++) {
    const it = items[i];
    const inside = it.start <= pos && (pos < it.end || pos === docLen);
    if (inside && (best < 0 || it.start > items[best].start)) best = i;
  }
  return best;
}
