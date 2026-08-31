// Run: node app/src/filetree.test.mjs
import assert from "node:assert/strict";
import { buildTree, firstFile } from "./filetree.js";

// A fake filesystem: dir path -> entries. readDir throws on unknown dirs.
const FS = {
  "/p": [
    { name: "02-two.ink", isDirectory: false },
    { name: "01-one.ink", isDirectory: false },
    { name: "notes", isDirectory: true },
    { name: "empty", isDirectory: true },
    { name: "README.md", isDirectory: false }, // not .ink
    { name: "link", isDirectory: true, isSymlink: true }, // skipped
  ],
  "/p/notes": [{ name: "world.ink", isDirectory: false }],
  "/p/empty": [{ name: "cover.png", isDirectory: false }], // no .ink -> pruned
  "/p/link": [{ name: "loop.ink", isDirectory: false }],
};
const readDir = async (dir) => {
  if (!(dir in FS)) throw new Error("no such dir");
  return FS[dir];
};

const tree = await buildTree("/p", readDir);

// Dirs before files, each group name-sorted. `empty` pruned (no .ink); symlinked
// `link` skipped; non-.ink `README.md` excluded.
assert.deepEqual(
  tree.map((n) => n.name),
  ["notes", "01-one.ink", "02-two.ink"],
  `tree order/prune wrong: ${JSON.stringify(tree.map((n) => n.name))}`,
);

// The directory nests its .ink child with a full path.
const notes = tree[0];
assert.ok(notes.children, "notes should be a directory node");
assert.deepEqual(notes.children.map((n) => n.path), ["/p/notes/world.ink"]);

// firstFile is depth-first: descends into `notes` before the top-level files.
assert.equal(firstFile(tree), "/p/notes/world.ink");
assert.equal(firstFile([]), null);

// An unreadable dir yields an empty tree, not a throw.
assert.deepEqual(await buildTree("/nope", readDir), []);

console.log("filetree: all assertions passed");
