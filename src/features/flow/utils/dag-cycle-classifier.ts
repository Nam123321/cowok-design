// @story 04.1
// @implements AC-03, AC-04, AC-06
export enum DfsColor {
  WHITE = 0,
  GRAY = 1,
  BLACK = 2,
}

export interface CycleDetectionResult {
  hasCycle: boolean;
  cyclePath: string[];
}

interface DfsNeighborContext {
  u: string;
  neighbors: string[];
  colors: Map<string, DfsColor>;
  parent: Map<string, string | null>;
  currentPath: string[];
}

function exploreDfsNeighbors(
  ctx: DfsNeighborContext,
  dfsStep: (node: string) => boolean
): { hasCycle: boolean; cyclePath: string[] } {
  const { u, neighbors, colors, parent, currentPath } = ctx;
  for (const v of neighbors) {
    if (!colors.has(v)) colors.set(v, DfsColor.WHITE);
    if (colors.get(v) === DfsColor.GRAY) {
      const idx = currentPath.indexOf(v);
      const cyclePath = idx !== -1 ? [...currentPath.slice(idx), v] : [v, u, v];
      return { hasCycle: true, cyclePath };
    }
    if (colors.get(v) === DfsColor.WHITE) {
      parent.set(v, u);
      if (dfsStep(v)) return { hasCycle: true, cyclePath: [] };
    }
  }
  return { hasCycle: false, cyclePath: [] };
}

export function detectCycleDfs(adjacencyList: Map<string, string[]>): CycleDetectionResult {
  const colors = new Map<string, DfsColor>();
  const parent = new Map<string, string | null>();
  for (const node of adjacencyList.keys()) {
    colors.set(node, DfsColor.WHITE);
  }

  let finalCyclePath: string[] = [];

  function dfs(u: string, currentPath: string[]): boolean {
    colors.set(u, DfsColor.GRAY);
    currentPath.push(u);
    const neighbors = adjacencyList.get(u) || [];
    const check = exploreDfsNeighbors({ u, neighbors, colors, parent, currentPath }, (v) => dfs(v, currentPath));
    if (check.hasCycle) {
      if (check.cyclePath.length > 0) finalCyclePath = check.cyclePath;
      return true;
    }
    colors.set(u, DfsColor.BLACK);
    currentPath.pop();
    return false;
  }

  for (const node of adjacencyList.keys()) {
    if (colors.get(node) === DfsColor.WHITE && dfs(node, [])) {
      return { hasCycle: true, cyclePath: finalCyclePath };
    }
  }
  return { hasCycle: false, cyclePath: [] };
}

export function buildAdjacencyList(
  nodes: { id: string }[],
  transitions: { sourceNodeId: string; targetNodeId: string }[]
): Map<string, string[]>;
export function buildAdjacencyList(
  transitions: { sourceNodeId: string; targetNodeId: string }[],
  proposedEdge?: { sourceNodeId: string; targetNodeId: string }
): Map<string, string[]>;
export function buildAdjacencyList(
  first: Array<{ id: string }> | Array<{ sourceNodeId: string; targetNodeId: string }>,
  second?: Array<{ sourceNodeId: string; targetNodeId: string }> | { sourceNodeId: string; targetNodeId: string }
): Map<string, string[]> {
  const adj = new Map<string, string[]>();
  const isSecondSingleEdge = Boolean(second && !Array.isArray(second) && "sourceNodeId" in second);
  const isFirstTransitions = Boolean(first.length > 0 && "sourceNodeId" in first[0]);

  if (isSecondSingleEdge || isFirstTransitions) {
    const allEdges = [...(first as Array<{ sourceNodeId: string; targetNodeId: string }>)];
    if (isSecondSingleEdge) allEdges.push(second as { sourceNodeId: string; targetNodeId: string });
    for (const edge of allEdges) {
      if (!adj.has(edge.sourceNodeId)) adj.set(edge.sourceNodeId, []);
      if (!adj.has(edge.targetNodeId)) adj.set(edge.targetNodeId, []);
      adj.get(edge.sourceNodeId)!.push(edge.targetNodeId);
    }
    return adj;
  }

  for (const n of first as Array<{ id: string }>) {
    adj.set(n.id, []);
  }
  for (const t of Array.isArray(second) ? second : []) {
    const list = adj.get(t.sourceNodeId) || [];
    list.push(t.targetNodeId);
    adj.set(t.sourceNodeId, list);
  }
  return adj;
}

export function classifyHtmlDocument(
  manifest?: { isPresentation?: boolean; isUserflow?: boolean },
  domSnippet?: string,
  cachedType?: "slide-cms" | "userflow-dag",
  userOverride?: "slide-cms" | "userflow-dag"
): "slide-cms" | "userflow-dag" {
  if (userOverride) return userOverride;
  if (manifest?.isPresentation) return "slide-cms";
  if (manifest?.isUserflow) return "userflow-dag";
  if (domSnippet) {
    if (domSnippet.includes("presentation-stage") || domSnippet.includes("slide")) return "slide-cms";
    if (domSnippet.includes("data-ff-flow") || domSnippet.includes("dag-canvas")) return "userflow-dag";
  }
  return cachedType || "slide-cms";
}

export function findOrphanNodes(nodes: { id: string }[], edges: { source: string; target: string }[]): string[] {
  if (nodes.length <= 1) return [];
  const connected = new Set<string>();
  for (const edge of edges) {
    connected.add(edge.source);
    connected.add(edge.target);
  }
  return nodes.filter((n) => !connected.has(n.id)).map((n) => n.id);
}
