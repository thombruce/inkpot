// Pure text-splice for scaffolding a new character note. The character builder
// (#45) is read-only over the codex render; the one write it makes is inserting
// a `%% Name` template into the `% Characters` section. Text stays canonical —
// this returns the new source plus the range covering the placeholder name, so
// the editor can select it for immediate typing. Import-free for character.test.mjs.

const PLACEHOLDER = "New Character";

// A Characters section: any excluded heading (`%` run) whose title is
// "Characters" (case-folded), matching what render_characters_html shows. The
// marker run is captured so the new entry can nest one level deeper — under
// `%% Characters` the entry is `%%% Name`, not a stray top-level `% Characters`.
const SECTION = /^(%+)[ \t]+characters[ \t]*$/im;

// Build the new document text and the selection over the inserted name. The
// entry goes just under the first existing Characters section (at whatever
// depth), nested one marker deeper; if there is none, a top-level `% Characters`
// section is appended. Returns { text, selFrom, selTo } (char offsets).
export function scaffoldCharacter(src, name = PLACEHOLDER) {
  const section = SECTION.exec(src);
  let insertAt;
  let insert;
  if (section) {
    // One marker deeper than the section, so the entry nests under it.
    const marker = "%".repeat(section[1].length + 1);
    const nl = src.indexOf("\n", section.index);
    insertAt = nl === -1 ? src.length : nl + 1;
    // Leading blank line keeps the heading/entry separation the parser expects.
    insert = `\n${marker} ${name}\nrole: \n`;
  } else {
    // No section yet: append one, padding to a blank-line boundary first.
    const pad = src.length === 0 ? "" : src.endsWith("\n\n") ? "" : src.endsWith("\n") ? "\n" : "\n\n";
    insertAt = src.length;
    insert = `${pad}% Characters\n\n%% ${name}\nrole: \n`;
  }
  const selFrom = insertAt + insert.indexOf(name);
  return {
    text: src.slice(0, insertAt) + insert + src.slice(insertAt),
    selFrom,
    selTo: selFrom + name.length,
  };
}
