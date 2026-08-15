// A CodeMirror 6 StreamLanguage for the .ink format. Line-oriented, mirroring
// the Rust parser: headings (# visible / ~ scene), a metadata zone right after
// a heading, `/` line comments, CriticMarkup, and **bold** / *italic*.

import { StreamLanguage, HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { Tag } from "@lezer/highlight";

// Custom tags so the theme can style each construct precisely.
const t = {
  heading: Tag.define(),
  scene: Tag.define(),
  meta: Tag.define(),
  comment: Tag.define(),
  insert: Tag.define(),
  del: Tag.define(),
  sub: Tag.define(),
  strong: Tag.define(),
  emphasis: Tag.define(),
};

const inkMode = StreamLanguage.define({
  startState: () => ({ inMeta: false }),
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
      if (state.inMeta) {
        if (stream.match(/^[A-Za-z0-9_-]+:.*/)) return "meta";
        state.inMeta = false; // not a meta line: fall through to body
      }
      if (stream.match(/^\/.*/)) return "comment"; // `/` line comment
    }

    // Inline constructs. Order matters: bold before italic.
    if (stream.match(/^\{\+[^}]*\}/)) return "insert";
    if (stream.match(/^\{-[^}]*\}/)) return "del";
    if (stream.match(/^\{~[^}]*\}/)) return "sub";
    if (stream.match(/^\{\/[^}]*\}/)) return "comment";
    if (stream.match(/^\*\*[^*]+\*\*/)) return "strong";
    if (stream.match(/^\*[^*\n]+\*/)) return "emphasis";

    // Plain run: advance to the next construct opener or end of line.
    if (!stream.match(/^[^{*]+/)) stream.next();
    return null;
  },
  tokenTable: {
    heading: t.heading,
    scene: t.scene,
    meta: t.meta,
    comment: t.comment,
    insert: t.insert,
    del: t.del,
    sub: t.sub,
    strong: t.strong,
    emphasis: t.emphasis,
  },
});

const highlight = HighlightStyle.define([
  { tag: t.heading, color: "#7aa2f7", fontWeight: "bold" },
  { tag: t.scene, color: "#c9a26b", fontStyle: "italic" },
  { tag: t.meta, color: "#9a9aa6" },
  { tag: t.comment, color: "#6a9955", fontStyle: "italic" },
  { tag: t.insert, color: "#73c990" },
  { tag: t.del, color: "#e06c75", textDecoration: "line-through" },
  { tag: t.sub, color: "#d19a66" },
  { tag: t.strong, fontWeight: "bold", color: "#e6e6ea" },
  { tag: t.emphasis, fontStyle: "italic", color: "#e6e6ea" },
]);

export const ink = () => [inkMode, syntaxHighlighting(highlight)];
