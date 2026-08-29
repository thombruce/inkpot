// Pure text-splice for scaffolding a new character note. The character builder
// (#45) is read-only over the codex render; the one write it makes is inserting
// a `%% Name` template into the `% Characters` section. Text stays canonical —
// this returns the new source plus the range covering the placeholder name, so
// the editor can select it for immediate typing. Import-free for character.test.mjs.

const PLACEHOLDER = "New Character";

// Build the new document text and the selection over the inserted name. If a
// `% Characters` section exists, the entry goes just under its heading; otherwise
// a new section is appended. Returns { text, selFrom, selTo } (char offsets).
export function scaffoldCharacter(src, name = PLACEHOLDER) {
  const entry = `%% ${name}\nrole: \n`;
  const heading = src.match(/^% Characters[ \t]*$/m);
  let insertAt;
  let insert;
  if (heading) {
    // Right after the heading line; the leading blank line keeps the
    // heading/entry separation the parser expects.
    const nl = src.indexOf("\n", heading.index);
    insertAt = nl === -1 ? src.length : nl + 1;
    insert = `\n${entry}`;
  } else {
    // No section yet: append one, padding to a blank-line boundary first.
    const pad = src.length === 0 ? "" : src.endsWith("\n\n") ? "" : src.endsWith("\n") ? "\n" : "\n\n";
    insertAt = src.length;
    insert = `${pad}% Characters\n\n${entry}`;
  }
  const selFrom = insertAt + insert.indexOf(name);
  return {
    text: src.slice(0, insertAt) + insert + src.slice(insertAt),
    selFrom,
    selTo: selFrom + name.length,
  };
}
