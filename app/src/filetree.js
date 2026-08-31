// Pure project-tree helpers for the file rail. Filesystem access is injected
// (main.js passes Tauri's `fs.readDir` / `fs.exists`) so these are testable
// without a filesystem.

// The parent directory of a path. "" once past the filesystem root, which ends
// the walk-up loop in findRoot.
export const dirname = (p) => p.slice(0, p.lastIndexOf("/"));

// Walk up from a file to the nearest directory where `hasMarker(dir)` resolves
// truthy (main.js checks for the `Inkpot` file) — the project root. null if there
// is none up to the filesystem root.
export async function findRoot(filePath, hasMarker) {
  let dir = dirname(filePath);
  while (dir) {
    if (await hasMarker(dir)) return dir;
    dir = dirname(dir);
  }
  return null;
}

// Build the `.ink` tree under `dir`: recurse subdirectories, keep `.ink` files
// and the dirs that contain them (empty dirs pruned), order dirs-before-files by
// name, skip symlinked dirs (loop-safe). A node with `children` is a directory;
// otherwise a file. `depth` guards a pathological root (e.g. a stray marker high
// in the tree) from recursing an enormous subtree.
export async function buildTree(dir, readDir, depth = 0) {
  if (depth > 8) return []; // deeper than any real .ink project nests
  let entries;
  try {
    entries = await readDir(dir);
  } catch {
    return [];
  }
  const dirs = [];
  const files = [];
  for (const e of entries) {
    const path = `${dir}/${e.name}`;
    if (e.isDirectory && !e.isSymlink) {
      const children = await buildTree(path, readDir, depth + 1);
      if (children.length) dirs.push({ name: e.name, path, children });
    } else if (e.name.endsWith(".ink")) {
      files.push({ name: e.name, path });
    }
  }
  const byName = (a, b) => a.name.localeCompare(b.name);
  return [...dirs.sort(byName), ...files.sort(byName)];
}

// The first `.ink` file in a tree (depth-first), or null.
export function firstFile(nodes) {
  for (const n of nodes) {
    const f = n.children ? firstFile(n.children) : n.path;
    if (f) return f;
  }
  return null;
}
