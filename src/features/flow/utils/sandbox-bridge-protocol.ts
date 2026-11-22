// @story 06.1
// @implements AC-05
import React from "react";
import {
  generateBootSecret,
  generateEpochNonce,
  wrapUserHtmlSrcDoc,
  type SandboxSubStateLabels,
} from "./html-sandbox-wrapper";
import type { ElementInspectMetadata } from "../../../lib/validations/flow-workbench";
export {
  MAX_OUTBOUND_QUEUE_FRAMES,
  processValidSlotMessage,
  type BridgeCallbackParams,
  type InboundHandlerDeps,
} from "./sandbox-bridge-handlers";
import { MAX_OUTBOUND_QUEUE_FRAMES } from "./sandbox-bridge-handlers";

export type { ElementInspectMetadata };

export interface SpatialTargetSelection {
  targetType: "html_element" | "spatial_region";
  triggerSource: "select-click" | "select-marquee" | "interact-contextmenu";
  selector: string;
  label: string;
  elementFfId: string;
  coveredFfIds: string[];
  domHash?: string;
  rect: ElementRectBox;
  metadata?: ElementInspectMetadata;
  modifiers?: {
    altKey?: boolean;
    shiftKey?: boolean;
    metaKey?: boolean;
    ctrlKey?: boolean;
  };
}

export const POOL_MAX_SLOTS = 5;
const EMPTY_ELEMENT_RECTS: Record<string, ElementRectBox> = Object.freeze({});
const RANDOM_ID_RADIX = 36;
const RANDOM_SLICE_START = 2;
const RANDOM_SLICE_END = 7;

export type ProtocolLifecycleStatus = "IDLE" | "BOOTSTRAPPING" | "PORT_BOUND" | "PROTOCOL_FAILED";

export interface ElementRectBox {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface IframeKeepAliveSlot {
  slotId: string;
  cacheKey: string;
  projectId: string;
  nodeId: string;
  artifactSha256: string;
  rawHtml: string;
  subStateLabels?: SandboxSubStateLabels;
  epoch: number;
  bootSecret: string;
  channelSecret: string;
  scriptNonce: string;
  wrappedSrcDoc: string;
  port1: MessagePort | null;
  initPortDispatched: boolean;
  isPortVerified: boolean;
  retryCount: number;
  protocolStatus: ProtocolLifecycleStatus;
  elementRects: Record<string, ElementRectBox>;
  lastAcceptedSeq: number;
  lastUsedAt: number;
  outboundQueue: Array<Record<string, unknown>>;
  iframeRef: React.RefObject<HTMLIFrameElement | null>;
}

export function isSandboxFrameAllowedToBind(iframeEl: HTMLIFrameElement): boolean {
  const sandboxAttr = iframeEl.getAttribute("sandbox") || "";
  if (!sandboxAttr.includes("allow-scripts") || sandboxAttr.includes("allow-same-origin")) return false;
  if (iframeEl.hasAttribute("src") || !iframeEl.hasAttribute("srcdoc")) return false;
  try {
    const href = iframeEl.contentWindow?.location?.href;
    if (href && href !== "about:srcdoc" && href !== "about:blank") return false;
  } catch {
    return true;
  }
  return true;
}

export function createKeepAliveSlot(opts: {
  cacheKey: string;
  projectId: string;
  nodeId: string;
  artifactSha256: string;
  rawHtml: string;
  subStateLabels?: SandboxSubStateLabels;
}): IframeKeepAliveSlot {
  const epoch = generateEpochNonce();
  const bootSecret = generateBootSecret();
  const channelSecret = generateBootSecret();
  const scriptNonce = generateBootSecret();
  const randomSuffix = Math.random().toString(RANDOM_ID_RADIX).slice(RANDOM_SLICE_START, RANDOM_SLICE_END);
  return {
    slotId: `slot-${Date.now()}-${randomSuffix}`,
    cacheKey: opts.cacheKey,
    projectId: opts.projectId,
    nodeId: opts.nodeId,
    artifactSha256: opts.artifactSha256,
    rawHtml: opts.rawHtml,
    subStateLabels: opts.subStateLabels,
    epoch,
    bootSecret,
    channelSecret,
    scriptNonce,
    wrappedSrcDoc: wrapUserHtmlSrcDoc(opts.rawHtml, {
      epoch,
      bootSecret,
      scriptNonce,
      artifactSha256: opts.artifactSha256,
      subStateLabels: opts.subStateLabels,
    }),
    port1: null,
    initPortDispatched: false,
    isPortVerified: false,
    retryCount: 0,
    protocolStatus: "BOOTSTRAPPING",
    elementRects: {},
    lastAcceptedSeq: 0,
    lastUsedAt: Date.now(),
    outboundQueue: [],
    iframeRef: React.createRef<HTMLIFrameElement | null>(),
  };
}

export function evictOldestSlotIfFull(
  slots: IframeKeepAliveSlot[],
  pinnedNodeIds: readonly string[] = []
): IframeKeepAliveSlot[] {
  if (slots.length < POOL_MAX_SLOTS) return [...slots];
  const pinnedSet = new Set(pinnedNodeIds.filter(Boolean));
  const candidates = slots.filter((s) => !pinnedSet.has(s.nodeId));
  const poolToSort = candidates.length > 0 ? candidates : slots;
  const evicted = [...poolToSort].sort((a, b) => a.lastUsedAt - b.lastUsedAt)[0];
  if (!evicted) return [...slots];
  evicted.port1?.close();
  if (evicted.iframeRef.current) evicted.iframeRef.current.srcdoc = "";
  return slots.filter((s) => s.slotId !== evicted.slotId);
}

/**
 * Coalesces pre-PORT_ACK outbound PATCH_BY_FF_ID frames by `(slideUid, ffId)` inside the 32-frame queue
 * so rapid typing before bootstrap completes never drops edits or overflows the queue.
 */
export function enqueueOutboundFrame(
  queue: Array<Record<string, unknown>>,
  payload: Record<string, unknown>
): void {
  if (payload.type === "PATCH_BY_FF_ID" && typeof payload.ffId === "string") {
    const idx = queue.findIndex(
      (f) =>
        f.type === "PATCH_BY_FF_ID" &&
        f.ffId === payload.ffId &&
        (f.slideUid ?? "") === (payload.slideUid ?? "")
    );
    if (idx !== -1) {
      queue[idx] = { ...queue[idx], ...payload };
      return;
    }
  }
  if (queue.length >= MAX_OUTBOUND_QUEUE_FRAMES) {
    queue.shift();
  }
  queue.push(payload);
}

export function buildActiveSlotSnapshot(
  activeSlot: IframeKeepAliveSlot | undefined,
  resolvedSha256: string,
  fallbackRef: React.RefObject<HTMLIFrameElement | null>,
  protocolError: string | null
) {
  return {
    epoch: activeSlot?.epoch ?? 1,
    bootSecret: activeSlot?.bootSecret ?? "",
    channelSecret: activeSlot?.channelSecret ?? "",
    artifactSha256: activeSlot?.artifactSha256 ?? resolvedSha256,
    isPortVerified: activeSlot?.isPortVerified ?? false,
    protocolStatus: activeSlot?.protocolStatus ?? "IDLE",
    retryCount: activeSlot?.retryCount ?? 0,
    protocolError,
    elementRects: activeSlot?.elementRects ?? EMPTY_ELEMENT_RECTS,
    iframeRef: activeSlot?.iframeRef ?? fallbackRef,
    wrappedSrcDoc: activeSlot?.wrappedSrcDoc ?? "",
    activeSlotId: activeSlot?.slotId ?? "",
  };
}
