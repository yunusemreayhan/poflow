import type { Task } from "./store/api";

export interface TreeNode {
  task: Task;
  children: TreeNode[];
}

export function buildTree(tasks: Task[]): TreeNode[] {
  const map = new Map<number, TreeNode>();
  const roots: TreeNode[] = [];
  for (const t of tasks) map.set(t.id, { task: t, children: [] });
  for (const t of tasks) {
    const node = map.get(t.id)!;
    if (t.parent_id && map.has(t.parent_id)) {
      map.get(t.parent_id)!.children.push(node);
    } else {
      roots.push(node);
    }
  }
  return roots;
}

export function countDescendants(node: TreeNode): number {
  let c = node.children.length;
  for (const ch of node.children) c += countDescendants(ch);
  return c;
}
