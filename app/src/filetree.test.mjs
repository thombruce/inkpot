// Run: node app/src/filetree.test.mjs
import assert from "node:assert/strict";
import { buildTree, firstFile, allFiles, findRoot } from "./filetree.js";

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

// allFiles flattens every .ink path in manuscript order: depth-first, matching
// the tree's dirs-before-files-by-name ordering (#74 concatenated export).
assert.deepEqual(allFiles(tree), [
  "/p/notes/world.ink",
  "/p/01-one.ink",
  "/p/02-two.ink",
]);
assert.deepEqual(allFiles([]), []);

// An unreadable dir yields an empty tree, not a throw.
assert.deepEqual(await buildTree("/nope", readDir), []);

// findRoot walks up to the nearest directory with a marker.
{
  const marked = new Set(["/home/novel"]);
  const hasMarker = async (dir) => marked.has(dir);
  // A file nested two levels down resolves to the marked ancestor.
  assert.equal(await findRoot("/home/novel/chapters/ch1.ink", hasMarker), "/home/novel");
  // A file directly in the marked dir resolves to it.
  assert.equal(await findRoot("/home/novel/intro.ink", hasMarker), "/home/novel");
  // No marker up the tree -> null (caller falls back to the file's own dir).
  assert.equal(await findRoot("/home/loose/a.ink", hasMarker), null);
}

// Depth guard: a marker at the very top can't make buildTree recurse an enormous
// subtree — a chain far deeper than the cap comes back bounded.
{
  const deep = {};
  let path = "/r";
  for (let i = 0; i < 20; i++) {
    deep[path] = [
      { name: "sub", isDirectory: true },
      { name: "leaf.ink", isDirectory: false },
    ];
    path += "/sub";
  }
  const deepRead = async (d) => deep[d] ?? [];
  const tree = await buildTree("/r", deepRead);
  const depthOf = (nodes) =>
    nodes.reduce((m, n) => Math.max(m, n.children ? 1 + depthOf(n.children) : 1), 0);
  assert.ok(depthOf(tree) <= 10, `depth should be capped, got ${depthOf(tree)}`);
}

console.log("filetree: all assertions passed");
