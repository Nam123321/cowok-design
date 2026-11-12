/**
 * Sugiyama / Dagre-style Topological Layer Auto-Layout (Story 04.1 AC-08)
 * Arranges nodes cleanly in horizontal topological layers without overlapping edges.
 */

const DEFAULT_START_X = 80;
const DEFAULT_START_Y = 100;
const DEFAULT_COL_SPACING = 290;
const DEFAULT_ROW_SPACING = 190;

export interface AutoLayoutOptions {
  startX?: number;
  startY?: number;
  colSpacing?: number;
  rowSpacing?: number;
}

function computeTopologicalLayers(
  nodeIds: string[],
  transitions: { sourceNodeId: string; targetNodeId: string }[]
): Map<string, number> {
  const inDegree = new Map<string, number>();
  const adj = new Map<string, string[]>();
  const layerMap = new Map<string, number>();

  for (const id of nodeIds) {
    inDegree.set(id, 0);
    adj.set(id, []);
    layerMap.set(id, 0);
  }

  for (const t of transitions) {
    if (adj.has(t.sourceNodeId) && inDegree.has(t.targetNodeId)) {
      adj.get(t.sourceNodeId)!.push(t.targetNodeId);
      inDegree.set(t.targetNodeId, (inDegree.get(t.targetNodeId) || 0) + 1);
    }
  }

  const queue: string[] = [];
  for (const [id, deg] of inDegree.entries()) {
    if (deg === 0) queue.push(id);
  }
  if (queue.length === 0 && nodeIds.length > 0) {
    queue.push(nodeIds[0]);
  }

  const visited = new Set<string>();
  while (queue.length > 0) {
    const u = queue.shift()!;
    visited.add(u);
    const currentLayer = layerMap.get(u) || 0;
    for (const v of adj.get(u) || []) {
      layerMap.set(v, Math.max(layerMap.get(v) || 0, currentLayer + 1));
      const nextDeg = (inDegree.get(v) || 1) - 1;
      inDegree.set(v, nextDeg);
      if (nextDeg <= 0 && !visited.has(v) && !queue.includes(v)) {
        queue.push(v);
      }
    }
  }

  return layerMap;
}

export function computeTopologicalAutoLayout<T extends { id: string; posX: number; posY: number }>(
  nodes: T[],
  transitions: { sourceNodeId: string; targetNodeId: string }[],
  options: AutoLayoutOptions = {}
): T[] {
  if (nodes.length === 0) return [];
  const startX = options.startX ?? DEFAULT_START_X;
  const startY = options.startY ?? DEFAULT_START_Y;
  const colSpacing = options.colSpacing ?? DEFAULT_COL_SPACING;
  const rowSpacing = options.rowSpacing ?? DEFAULT_ROW_SPACING;

  const layerMap = computeTopologicalLayers(
    nodes.map((n) => n.id),
    transitions
  );

  const layers = new Map<number, T[]>();
  for (const n of nodes) {
    const layer = layerMap.get(n.id) || 0;
    if (!layers.has(layer)) layers.set(layer, []);
    layers.get(layer)!.push(n);
  }

  const posLookup = new Map<string, { posX: number; posY: number }>();
  const sortedLayers = Array.from(layers.keys()).sort((a, b) => a - b);
  for (const layerIdx of sortedLayers) {
    const group = layers.get(layerIdx)!;
    group.forEach((node, rowIdx) => {
      posLookup.set(node.id, {
        posX: startX + layerIdx * colSpacing,
        posY: startY + rowIdx * rowSpacing,
      });
    });
  }

  return nodes.map((n) => {
    const coords = posLookup.get(n.id);
    return coords ? { ...n, posX: coords.posX, posY: coords.posY } : n;
  });
}
