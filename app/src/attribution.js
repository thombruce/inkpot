// Attribution suffix on a CriticMarkup span — `{+text|@Ada|2026-10-04}` (#116).
// Mirrors `split_attribution` in ink-core's parse.rs: the suffix counts only when
// well formed at the very end (`|@author[,@author…]` + optional `|date`), so any
// other `|` is plain content. Change one, change the other.

const ISO_DATE = /^\d{4}-\d{2}-\d{2}(T[0-9:.+\-Z]*)?$/;

// `@Ada Lovelace, @Sam` → every comma-separated item is `@` + a non-empty name.
const isAuthors = (field) =>
  field.split(",").every((a) => /^@\s*\S/.test(a.trim()));

// Index in `body` of the `|` that starts the attribution suffix, or -1 if none.
export function attributionStart(body) {
  const parts = body.split("|");
  if (parts.length < 2) return -1;
  const last = parts[parts.length - 1];
  // `…|@authors|date`: needs content before the authors (3+ parts).
  if (parts.length >= 3 && ISO_DATE.test(last) && isAuthors(parts[parts.length - 2])) {
    return body.length - last.length - parts[parts.length - 2].length - 2;
  }
  if (isAuthors(last)) return body.length - last.length - 1;
  return -1;
}
