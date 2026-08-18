// A CodeMirror 6 StreamLanguage for the .ink format. Line-oriented, mirroring
// the Rust parser: headings (# visible / ~ scene / % excluded), a metadata zone
// right after a heading, `/` line comments, CriticMarkup, and **bold** / *italic*.

import { StreamLanguage, HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { Tag } from "@lezer/highlight";

// Custom tags so the theme can style each construct precisely.
const t = {
  heading: Tag.define(),
  scene: Tag.define(),
  meta: Tag.define(),
  excluded: Tag.define(),
  comment: Tag.define(),
  insert: Tag.define(),
  del: Tag.define(),
  sub: Tag.define(),
  strong: Tag.define(),
  emphasis: Tag.define(),
  link: Tag.define(),
};

// Left-flanking opener: char at `at` exists and is not whitespace.
const flankOpen = (s, at) => at < s.length && !/\s/.test(s[at]);

// Right-flanking closer: `delim` at/after `from` with a non-whitespace,
// non-backslash char before it and non-empty content (idx > from).
function findClose(s, from, delim) {
  let idx = s.indexOf(delim, from);
  while (idx !== -1) {
    const before = s[idx - 1];
    if (idx > from && before && !/\s/.test(before) && before !== "\\") return idx;
    idx = s.indexOf(delim, idx + delim.length);
  }
  return -1;
}

const inkMode = StreamLanguage.define({
  // Mirrors parse.rs: meta starts active so a leading `key: value` block (document
  // front matter) highlights; a blank line or non-meta line closes it.
  startState: () => ({ inMeta: true }),
  blankLine: (state) => {
    state.inMeta = false; // a blank line closes the metadata zone
  },
  token(stream, state) {
    if (stream.sol()) {
      if (stream.match(/^#+\s.*/)) {
        state.inMeta = true;
        return "heading";
      }
      if (stream.match(/^~+\s.*/)) {
        state.inMeta = true;
        return "scene";
      }
      if (stream.match(/^%+\s.*/)) {
        state.inMeta = true;
        return "excluded";
      }
      if (state.inMeta) {
        if (stream.match(/^[A-Za-z0-9_-]+:.*/)) return "meta";
        state.inMeta = false; // not a meta line: fall through to body
      }
      if (stream.match(/^\/.*/)) return "comment"; // `/` line comment
    }

    // Backslash escape: consume `\` + the next char as literal.
    if (stream.peek() === "\\") {
      stream.next();
      if (!stream.eol()) stream.next();
      return null;
    }

    // CriticMarkup.
    if (stream.match(/^\{\+[^}]*\}/)) return "insert";
    if (stream.match(/^\{-[^}]*\}/)) return "del";
    if (stream.match(/^\{~[^}]*\}/)) return "sub";
    if (stream.match(/^\{\/[^}]*\}/)) return "comment";

    // Wikilink [[Target]] — cross-reference to a codex entity.
    if (stream.match(/^\[\[[^\]]+\]\]/)) return "link";

    // Emphasis with flanking. Order matters: bold before italic.
    const s = stream.string, p = stream.pos;
    if (s.startsWith("**", p) && flankOpen(s, p + 2)) {
      const end = findClose(s, p + 2, "**");
      if (end > p + 2) {
        stream.pos = end + 2;
        return "strong";
      }
    }
    if (s[p] === "*" && flankOpen(s, p + 1)) {
      const end = findClose(s, p + 1, "*");
      if (end > p + 1) {
        stream.pos = end + 1;
        return "emphasis";
      }
    }

    // Plain run up to the next opener (stop at { * \ [ so those get their turn).
    if (!stream.match(/^[^{*\\[]+/)) stream.next();
    return null;
  },
  tokenTable: {
    heading: t.heading,
    scene: t.scene,
    excluded: t.excluded,
    meta: t.meta,
    comment: t.comment,
    insert: t.insert,
    del: t.del,
    sub: t.sub,
    strong: t.strong,
    emphasis: t.emphasis,
    link: t.link,
  },
});

const highlight = HighlightStyle.define([
  { tag: t.heading, color: "#7aa2f7", fontWeight: "bold" },
  { tag: t.scene, color: "#c9a26b", fontStyle: "italic" },
  // Muted italic reads as "set aside / not published" — works for both a cut
  // section and a notes section (strikethrough would imply deletion only).
  { tag: t.excluded, color: "#8b93a3", fontStyle: "italic" },
  { tag: t.meta, color: "#9a9aa6" },
  { tag: t.comment, color: "#6a9955", fontStyle: "italic" },
  { tag: t.insert, color: "#73c990" },
  { tag: t.del, color: "#e06c75", textDecoration: "line-through" },
  { tag: t.sub, color: "#d19a66" },
  { tag: t.strong, fontWeight: "bold", color: "#e6e6ea" },
  { tag: t.emphasis, fontStyle: "italic", color: "#e6e6ea" },
  { tag: t.link, color: "#7aa2f7", textDecoration: "underline" },
]);

export const ink = () => [inkMode, syntaxHighlighting(highlight)];
