// @story 04.1, 04.5, 05.1
// @implements AC-02, AC-03 (IframeKeepAlivePool LRU-5, In-Place Hydration, CSPRNG Nonce & Monotonic Epoch Bridge Hook)
import React, { useCallback, useEffect, useRef, useState } from "react";
import {
  IframeToHostMessageSchema,
  computeArtifactSha256,
  computeArtifactSha256Sync,
  generateBootSecret,
  generateEpochNonce,
  wrapUserHtmlSrcDoc,
  type SandboxSubStateLabels,
} from "../utils/html-sandbox-wrapper";
import type { FixtureVariant } from "../../../lib/validations/screen-fixture";

export const POOL_MAX_SLOTS = 5;
const RANDOM_ID_RADIX = 36;
const RANDOM_SLICE_START = 2;
const RANDOM_SLICE_END = 7;

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
  epoch: number;
  bootSecret: string;
  scriptNonce: string;
  wrappedSrcDoc: string;
  port1: MessagePort | null;
  isPortVerified: boolean;
  elementRects: Record<string, ElementRectBox>;
  lastAcceptedSeq: number;
  lastUsedAt: number;
  iframeRef: React.RefObject<HTMLIFrameElement | null>;
}

export interface SandboxedBridgeParams {
  projectId?: string;
  activeTabId: string;
  selectedNodeId?: string | null;
  selectedNodeKey?: string | null;
  structureLens: "userflow" | "slide-cms";
  activeSlideId: string;
  activeSubState: FixtureVariant;
  effectiveCustomHtml?: string;
  rawHtml: string;
  subStateLabels?: SandboxSubStateLabels;
  onRegionClick: (regionId: string, label: string, ffId?: string, domHash?: string, rect?: ElementRectBox) => void;
  onQuoteText?: (text: string) => void;
}

function isSandboxFrameAllowedToBind(iframeEl: HTMLIFrameElement): boolean {
  const sandboxAttr = iframeEl.getAttribute("sandbox") || "";
  if (!sandboxAttr.includes("allow-scripts") || sandboxAttr.includes("allow-same-origin")) return false;
  try {
    const href = iframeEl.contentWindow?.location?.href;
    if (href && href !== "about:srcdoc" && href !== "about:blank") return false;
  } catch {
    return true;
  }
  return true;
}

function createKeepAliveSlot(opts: {
  cacheKey: string;
  projectId: string;
  nodeId: string;
  artifactSha256: string;
  rawHtml: string;
  subStateLabels?: SandboxSubStateLabels;
}): IframeKeepAliveSlot {
  const epoch = generateEpochNonce();
  const bootSecret = generateBootSecret();
  const scriptNonce = generateBootSecret();
  const randomSuffix = Math.random().toString(RANDOM_ID_RADIX).slice(RANDOM_SLICE_START, RANDOM_SLICE_END);
  return {
    slotId: `slot-${Date.now()}-${randomSuffix}`,
    cacheKey: opts.cacheKey,
    projectId: opts.projectId,
    nodeId: opts.nodeId,
    artifactSha256: opts.artifactSha256,
    epoch,
    bootSecret,
    scriptNonce,
    wrappedSrcDoc: wrapUserHtmlSrcDoc(opts.rawHtml, {
      epoch,
      bootSecret,
      scriptNonce,
      artifactSha256: opts.artifactSha256,
      subStateLabels: opts.subStateLabels,
    }),
    port1: null,
    isPortVerified: false,
    elementRects: {},
    lastAcceptedSeq: 0,
    lastUsedAt: Date.now(),
    iframeRef: React.createRef<HTMLIFrameElement | null>(),
  };
}

function evictOldestSlotIfFull(slots: IframeKeepAliveSlot[]): IframeKeepAliveSlot[] {
  if (slots.length < POOL_MAX_SLOTS) return [...slots];
  const evicted = [...slots].sort((a, b) => a.lastUsedAt - b.lastUsedAt)[0];
  if (!evicted) return [...slots];
  evicted.port1?.close();
  if (evicted.iframeRef.current) evicted.iframeRef.current.srcdoc = "";
  return slots.filter((s) => s.slotId !== evicted.slotId);
}

interface InboundHandlerDeps {
  slot: IframeKeepAliveSlot;
  channel: MessageChannel;
  nodeLabel: string;
  activeSubState: FixtureVariant;
  seqRef: React.MutableRefObject<number>;
  refreshSlots: () => void;
  onRegionClick: SandboxedBridgeParams["onRegionClick"];
  onQuoteText?: (text: string) => void;
}

function handleElementClickEvent(msg: Extract<ReturnType<typeof IframeToHostMessageSchema.parse>, { type: "ELEMENT_CLICKED" }>, deps: InboundHandlerDeps): void {
  const ffId = msg.elementFfId || msg.selector;
  if (msg.rect) {
    deps.slot.elementRects = { ...deps.slot.elementRects, [ffId]: msg.rect };
    deps.refreshSlots();
  }
  deps.onRegionClick(msg.selector, `@[Element ${deps.nodeLabel} • #${ffId}]`, ffId, msg.domHash, msg.rect);
}

function dispatchSlotMessageByType(msg: ReturnType<typeof IframeToHostMessageSchema.parse>, deps: InboundHandlerDeps): void {
  const { slot, channel, activeSubState, seqRef, refreshSlots, onQuoteText } = deps;
  if (msg.type === "PORT_ACK") {
    if (msg.artifactSha256 !== slot.artifactSha256) { channel.port1.close(); return; }
    slot.isPortVerified = true;
    refreshSlots();
    return;
  }
  if (msg.type === "READY" && activeSubState && activeSubState !== "DEFAULT") {
    channel.port1.postMessage({ type: "FLOW_PREVIEW_HYDRATE", epoch: slot.epoch, bootSecret: slot.bootSecret, channelSecret: slot.bootSecret, variant: activeSubState, artifactSha256: slot.artifactSha256, seq: ++seqRef.current });
    return;
  }
  if (msg.type === "ELEMENT_RECTS") { slot.elementRects = { ...slot.elementRects, ...msg.rects }; refreshSlots(); return; }
  if (msg.type === "ELEMENT_CLICKED") { handleElementClickEvent(msg, deps); return; }
  if (msg.type === "TEXT_QUOTE") onQuoteText?.(msg.text);
}

function processValidSlotMessage(ev: MessageEvent, deps: InboundHandlerDeps): void {
  const parsed = IframeToHostMessageSchema.safeParse(ev.data);
  if (!parsed.success || parsed.data.epoch !== deps.slot.epoch) { deps.channel.port1.close(); return; }
  const msg = parsed.data;
  const secretAck = msg.bootSecretAck || (msg as unknown as Record<string, unknown>).channelSecret;
  if (!secretAck || secretAck !== deps.slot.bootSecret) { deps.channel.port1.close(); return; }
  const eventSeq = (msg as unknown as Record<string, unknown>).seq;
  if (typeof eventSeq === "number") {
    if (eventSeq <= deps.slot.lastAcceptedSeq) return;
    deps.slot.lastAcceptedSeq = eventSeq;
  }
  dispatchSlotMessageByType(msg, deps);
}

function useKeepAlivePoolState(opts: {
  projectId: string;
  nodeId: string;
  rawHtml: string;
  activeSubState: FixtureVariant;
  subStateLabels?: SandboxSubStateLabels;
}) {
  const { projectId, nodeId, rawHtml, activeSubState, subStateLabels } = opts;
  const [resolvedSha256, setResolvedSha256] = useState<string>(() => computeArtifactSha256Sync(rawHtml));
  const [slots, setSlots] = useState<IframeKeepAliveSlot[]>([]);
  const [activeSlotKey, setActiveSlotKey] = useState<string>("");
  const seqRef = useRef(0);
  const slotsRef = useRef<IframeKeepAliveSlot[]>([]);
  slotsRef.current = slots;

  useEffect(() => {
    let cancelled = false;
    computeArtifactSha256(rawHtml).then((sha) => { if (!cancelled) setResolvedSha256(sha); }).catch(() => {});
    return () => { cancelled = true; };
  }, [rawHtml]);

  const targetCacheKey = `${projectId}::${nodeId}::${resolvedSha256}`;
  useEffect(() => {
    const existing = slotsRef.current.find((s) => s.cacheKey === targetCacheKey);
    if (existing) { existing.lastUsedAt = Date.now(); setActiveSlotKey(targetCacheKey); return; }
    const nextSlots = evictOldestSlotIfFull(slotsRef.current);
    nextSlots.push(createKeepAliveSlot({ cacheKey: targetCacheKey, projectId, nodeId, artifactSha256: resolvedSha256, rawHtml, subStateLabels }));
    setSlots(nextSlots);
    setActiveSlotKey(targetCacheKey);
  }, [nodeId, projectId, rawHtml, resolvedSha256, subStateLabels, targetCacheKey]);

  const activeSlot = slots.find((s) => s.cacheKey === activeSlotKey) || slots[0] || null;
  useEffect(() => {
    if (!activeSlot?.port1) return;
    activeSlot.port1.postMessage({ type: "FLOW_PREVIEW_HYDRATE", epoch: activeSlot.epoch, bootSecret: activeSlot.bootSecret, channelSecret: activeSlot.bootSecret, variant: activeSubState, artifactSha256: activeSlot.artifactSha256, seq: ++seqRef.current });
  }, [activeSlot?.artifactSha256, activeSlot?.bootSecret, activeSlot?.epoch, activeSlot?.port1, activeSubState]);

  useEffect(() => () => { slotsRef.current.forEach((s) => s.port1?.close()); }, []);

  return { resolvedSha256, slots, setSlots, slotsRef, seqRef, activeSlot };
}

function resolveTargetNodeId(params: Pick<SandboxedBridgeParams, "selectedNodeId" | "structureLens" | "activeSlideId">): string {
  if (params.selectedNodeId) return params.selectedNodeId;
  return params.structureLens === "slide-cms" ? params.activeSlideId : "SCR-02";
}

function buildActiveSlotSnapshot(activeSlot: IframeKeepAliveSlot | undefined, resolvedSha256: string, fallbackRef: React.RefObject<HTMLIFrameElement | null>) {
  return {
    epoch: activeSlot?.epoch ?? 1,
    bootSecret: activeSlot?.bootSecret ?? "",
    artifactSha256: activeSlot?.artifactSha256 ?? resolvedSha256,
    isPortVerified: activeSlot?.isPortVerified ?? false,
    elementRects: activeSlot?.elementRects ?? {},
    iframeRef: activeSlot?.iframeRef ?? fallbackRef,
    wrappedSrcDoc: activeSlot?.wrappedSrcDoc ?? "",
    activeSlotId: activeSlot?.slotId ?? "",
  };
}

export function useSandboxedIframeBridge(params: SandboxedBridgeParams) {
  const projectId = params.projectId || "default-proj";
  const nodeId = resolveTargetNodeId(params);
  const fallbackRef = useRef<HTMLIFrameElement>(null);
  const pool = useKeepAlivePoolState({ projectId, nodeId, rawHtml: params.rawHtml, activeSubState: params.activeSubState, subStateLabels: params.subStateLabels });
  const { activeSlot, resolvedSha256, slots, setSlots, slotsRef, seqRef } = pool;
  const nodeLabel = params.selectedNodeKey || params.selectedNodeId || "SCR-02";

  const handleSlotIframeLoad = useCallback((slotId: string, iframeEl: HTMLIFrameElement | null) => {
    if (!iframeEl || !isSandboxFrameAllowedToBind(iframeEl) || typeof MessageChannel === "undefined" || !iframeEl.contentWindow) return;
    const slot = slotsRef.current.find((s) => s.slotId === slotId);
    if (!slot) return;
    slot.port1?.close();
    const channel = new MessageChannel();
    channel.port1.onmessage = (ev) => processValidSlotMessage(ev, { slot, channel, nodeLabel, activeSubState: params.activeSubState, seqRef, refreshSlots: () => setSlots((p) => [...p]), onRegionClick: params.onRegionClick, onQuoteText: params.onQuoteText });
    iframeEl.contentWindow.postMessage({ type: "INIT_PORT", epoch: slot.epoch, bootSecret: slot.bootSecret, channelSecret: slot.bootSecret, artifactSha256: slot.artifactSha256, seq: ++seqRef.current }, "*", [channel.port2]);
    slot.port1 = channel.port1;
    setSlots((p) => [...p]);
  }, [nodeLabel, params.activeSubState, params.onQuoteText, params.onRegionClick, seqRef, setSlots, slotsRef]);

  const handleIframeLoad = useCallback(() => {
    const el = activeSlot?.iframeRef.current;
    if (activeSlot && el) handleSlotIframeLoad(activeSlot.slotId, el);
  }, [activeSlot, handleSlotIframeLoad]);

  const postToPort = useCallback((payload: Record<string, unknown>) => {
    if (!activeSlot?.port1) return;
    activeSlot.port1.postMessage({ ...payload, epoch: activeSlot.epoch, bootSecret: activeSlot.bootSecret, channelSecret: activeSlot.bootSecret, seq: ++seqRef.current });
  }, [activeSlot, seqRef]);

  const snapshot = buildActiveSlotSnapshot(activeSlot, resolvedSha256, fallbackRef);

  return {
    ...snapshot,
    handleIframeLoad,
    handleSlotIframeLoad,
    postApplyHtml: useCallback((html: string) => postToPort({ type: "APPLY_HTML", html }), [postToPort]),
    postFixtureHydrate: useCallback((variant: FixtureVariant, fixtureJson?: string) => postToPort({ type: "FLOW_PREVIEW_HYDRATE", variant, artifactSha256: snapshot.artifactSha256, fixtureJson }), [postToPort, snapshot.artifactSha256]),
    postCtaHoverSync: useCallback((ffId: string, hovered: boolean) => postToPort({ type: "CTA_HOVER_SYNC", ffId, hovered }), [postToPort]),
    postPatchByFfId: useCallback((ffId: string, patch: { textContent?: string; safeStyles?: Record<string, string> }) => postToPort({ type: "PATCH_BY_FF_ID", ffId, textContent: patch.textContent, safeStyles: patch.safeStyles }), [postToPort]),
    poolSlots: slots,
    activeSlot,
  };
}
