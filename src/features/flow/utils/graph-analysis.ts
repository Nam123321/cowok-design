export interface GraphNode {
  id: string;
  [key: string]: unknown;
}

export interface GraphEdge {
  id?: string;
  source: string;
  target: string;
  [key: string]: unknown;
}

export interface GraphMetrics {
  totalNodes: number;
  totalEdges: number;
  orphanNodeIds: string[];
  orphanCount: number;
  hasCycle: boolean;
}

/**
 * Finds all orphaned nodes in the flow graph (in-degree == 0 and out-degree == 0)
 * (AC-03 requirement)
 */
export function findOrphanNodes(nodes: GraphNode[], edges: GraphEdge[]): string[] {
  const connectedNodeIds = new Set<string>();

  for (const edge of edges) {
    if (edge.source) connectedNodeIds.add(edge.source);
    if (edge.target) connectedNodeIds.add(edge.target);
  }

  const orphans: string[] = [];
  for (const node of nodes) {
    if (!connectedNodeIds.has(node.id)) {
      orphans.push(node.id);
    }
  }

  return orphans;
}

/**
 * Detects whether the flow graph contains cycles using Depth-First Search with recursion stack
 */
export function detectCycles(nodes: GraphNode[], edges: GraphEdge[]): { hasCycle: boolean; cycleNodeIds: string[] } {
  const adjacencyList = new Map<string, string[]>();

  for (const node of nodes) {
    adjacencyList.set(node.id, []);
  }

  for (const edge of edges) {
    if (adjacencyList.has(edge.source)) {
      adjacencyList.get(edge.source)!.push(edge.target);
    }
  }

  const visited = new Set<string>();
  const recursionStack = new Set<string>();
  const cycleNodes: string[] = [];

  function dfs(currentId: string): boolean {
    visited.add(currentId);
    recursionStack.add(currentId);

    const neighbors = adjacencyList.get(currentId) || [];
    for (const neighbor of neighbors) {
      if (!visited.has(neighbor)) {
        if (dfs(neighbor)) return true;
      } else if (recursionStack.has(neighbor)) {
        cycleNodes.push(currentId, neighbor);
        return true;
      }
    }

    recursionStack.delete(currentId);
    return false;
  }

  let hasCycle = false;
  for (const node of nodes) {
    if (!visited.has(node.id)) {
      if (dfs(node.id)) {
        hasCycle = true;
        break;
      }
    }
  }

  return { hasCycle, cycleNodeIds: Array.from(new Set(cycleNodes)) };
}

/**
 * Computes top-level graph metrics for workbench header and status bars
 */
export function calculateGraphMetrics(nodes: GraphNode[], edges: GraphEdge[]): GraphMetrics {
  const orphanNodeIds = findOrphanNodes(nodes, edges);
  const { hasCycle } = detectCycles(nodes, edges);

  return {
    totalNodes: nodes.length,
    totalEdges: edges.length,
    orphanNodeIds,
    orphanCount: orphanNodeIds.length,
    hasCycle,
  };
}
