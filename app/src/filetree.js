// Pure project-tree builder for the file rail. The directory reader is injected
// (main.js passes Tauri's `fs.readDir`) so this is testable without a filesystem.
// Recurses subdirectories, keeps `.ink` files and the dirs that contain them
// (empty dirs pruned), orders dirs-before-files by name, and skips symlinked
// dirs (loop-safe). A node with `children` is a directory; otherwise a file.

export async function buildTree(dir, readDir) {
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
      const children = await buildTree(path, readDir);
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
