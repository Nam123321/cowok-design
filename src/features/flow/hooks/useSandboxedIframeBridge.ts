// @story 04.1, 04.5, 05.1, 06.1
// @implements AC-02, AC-03, AC-05
import React, { useCallback, useEffect, useRef, useState } from "react";
import {
  computeArtifactSha256Sync,
  generateBootSecret,
  generateEpochNonce,
  prepareHtmlForApplyBridge,
  wrapUserHtmlSrcDoc,
  type SandboxSubStateLabels,
} from "../utils/html-sandbox-wrapper";
import type { FixtureVariant } from "../../../lib/validations/screen-fixture";
import { PortReadyBootstrapSchema, type ChartSpecV12, type IframeToHostDeckMessage } from "../../../lib/validations/presentation";
import {
  MAX_OUTBOUND_QUEUE_FRAMES,
  POOL_MAX_SLOTS,
  buildActiveSlotSnapshot,
  createKeepAliveSlot,
  enqueueOutboundFrame,
  evictOldestSlotIfFull,
  isSandboxFrameAllowedToBind,
  processValidSlotMessage,
  type BridgeCallbackParams,
  type ElementInspectMetadata,
  type ElementRectBox,
  type IframeKeepAliveSlot,
  type ProtocolLifecycleStatus,
  type SpatialTargetSelection,
} from "../utils/sandbox-bridge-protocol";

export { POOL_MAX_SLOTS, MAX_OUTBOUND_QUEUE_FRAMES };
export type { ElementInspectMetadata, ElementRectBox, IframeKeepAliveSlot, ProtocolLifecycleStatus, SpatialTargetSelection };
export const PROTOCOL_RETRY_DELAY_MS = 500;
export const MAX_PROTOCOL_RETRIES = 3;

export interface SandboxedBridgeParams extends BridgeCallbackParams {
  projectId?: string;
  activeTabId: string;
  selectedNodeId?: string | null;
  selectedNodeKey?: string | null;
  structureLens: "userflow" | "slide-cms";
  activeSlideId: string;
  effectiveCustomHtml?: string;
  rawHtml: string;
  /** Optional committed artifact hash/key to keep `targetCacheKey` invariant during fast `PATCH_BY_FF_ID` typing */
  committedSlideCacheKey?: string;
  /** Priority slide IDs pinned inside `POOL_MAX_SLOTS = 5` (e.g., audience active, presenter active, presenter next) */
  pinnedSlideIds?: string[];
  subStateLabels?: SandboxSubStateLabels;
}

function useKeepAlivePoolState(opts: {
  projectId: string;
  nodeId: string;
  rawHtml: string;
  structureLens: "userflow" | "slide-cms";
  committedSlideCacheKey?: string;
  pinnedSlideIds?: string[];
  activeSubState: FixtureVariant;
  subStateLabels?: SandboxSubStateLabels;
}) {
  const { projectId, nodeId, rawHtml, structureLens, committedSlideCacheKey, pinnedSlideIds, activeSubState, subStateLabels } = opts;
  const resolvedSha256 = computeArtifactSha256Sync(rawHtml);
  const [slots, setSlots] = useState<IframeKeepAliveSlot[]>([]);
  const [activeSlotKey, setActiveSlotKey] = useState<string>("");
  const seqRef = useRef(0);
  const slotsRef = useRef<IframeKeepAliveSlot[]>([]);
  slotsRef.current = slots;

  const stableCacheSeed = committedSlideCacheKey || (structureLens === "slide-cms" ? "slide-live-stable" : resolvedSha256);
  const targetCacheKey = `${projectId}::${nodeId}::${stableCacheSeed}`;

  useEffect(() => {
    const existing = slotsRef.current.find((s) => s.cacheKey === targetCacheKey);
    if (existing) { existing.lastUsedAt = Date.now(); setActiveSlotKey(targetCacheKey); return; }
    const nextSlots = evictOldestSlotIfFull(slotsRef.current, pinnedSlideIds);
    nextSlots.push(createKeepAliveSlot({ cacheKey: targetCacheKey, projectId, nodeId, artifactSha256: resolvedSha256, rawHtml, subStateLabels }));
    setSlots(nextSlots);
    setActiveSlotKey(targetCacheKey);
  }, [nodeId, pinnedSlideIds, projectId, rawHtml, resolvedSha256, subStateLabels, targetCacheKey]);

  const activeSlot = slots.find((s) => s.cacheKey === activeSlotKey) || slots[0] || null;
  const activeSlotIdRef = useRef<string | null>(null);
  activeSlotIdRef.current = activeSlot?.slotId ?? null;
  useEffect(() => {
    if (!activeSlot?.port1) return;
    activeSlot.port1.postMessage({
      type: "FLOW_PREVIEW_HYDRATE", epoch: activeSlot.epoch, bootSecret: activeSlot.bootSecret,
      channelSecret: activeSlot.channelSecret, variant: activeSubState, artifactSha256: activeSlot.artifactSha256, seq: ++seqRef.current,
    });
  }, [activeSlot?.artifactSha256, activeSlot?.bootSecret, activeSlot?.channelSecret, activeSlot?.epoch, activeSlot?.port1, activeSubState]);

  useEffect(() => () => { slotsRef.current.forEach((s) => s.port1?.close()); }, []);
  return { resolvedSha256, slots, setSlots, slotsRef, seqRef, activeSlot, activeSlotIdRef };
}

function advanceSlotBootstrapRetry(
  slot: IframeKeepAliveSlot,
  setProtocolError: React.Dispatch<React.SetStateAction<string | null>>,
  setSlots: React.Dispatch<React.SetStateAction<IframeKeepAliveSlot[]>>
): void {
  if (slot.isPortVerified) return;
  slot.port1?.close();
  slot.port1 = null;
  if (slot.retryCount < MAX_PROTOCOL_RETRIES) {
    slot.retryCount += 1;
    slot.epoch = generateEpochNonce();
    slot.bootSecret = generateBootSecret();
    slot.channelSecret = generateBootSecret();
    slot.scriptNonce = generateBootSecret();
    slot.initPortDispatched = false;
    slot.wrappedSrcDoc = wrapUserHtmlSrcDoc(slot.rawHtml, {
      epoch: slot.epoch,
      bootSecret: slot.bootSecret,
      scriptNonce: slot.scriptNonce,
      artifactSha256: slot.artifactSha256,
      subStateLabels: slot.subStateLabels,
    });
    slot.protocolStatus = "BOOTSTRAPPING";
  } else {
    slot.protocolStatus = "PROTOCOL_FAILED";
    setProtocolError("E_DECK_PROTOCOL_INVALID");
  }
  setSlots((prev) => [...prev]);
}

function useRetryTimersMap() {
  const retryTimersRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());
  const clearSlotRetryTimer = useCallback((slotId: string) => {
    const existing = retryTimersRef.current.get(slotId);
    if (existing !== undefined) {
      clearTimeout(existing);
      retryTimersRef.current.delete(slotId);
    }
  }, []);
  const setSlotRetryTimer = useCallback((slotId: string, fn: () => void) => {
    clearSlotRetryTimer(slotId);
    retryTimersRef.current.set(slotId, setTimeout(fn, PROTOCOL_RETRY_DELAY_MS));
  }, [clearSlotRetryTimer]);
  useEffect(() => () => {
    retryTimersRef.current.forEach((timerId) => clearTimeout(timerId));
    retryTimersRef.current.clear();
  }, []);
  return { clearSlotRetryTimer, setSlotRetryTimer };
}

function isOriginAllowedForReady(origin: string): boolean {
  if (origin === "null" || origin === "") return true;
  return typeof window !== "undefined" && origin === window.location.origin;
}

function findMatchingPortReadySlot(ev: MessageEvent, slots: IframeKeepAliveSlot[]): { slot: IframeKeepAliveSlot; iframe: HTMLIFrameElement } | null {
  if (!isOriginAllowedForReady(ev.origin)) return null;
  const parsed = PortReadyBootstrapSchema.safeParse(ev.data);
  if (!parsed.success) return null;
  const match = slots.find((s) => s.epoch === parsed.data.epoch && s.bootSecret === parsed.data.bootSecret && !s.isPortVerified);
  const iframe = match?.iframeRef.current;
  if (!match || !iframe) return null;
  if (iframe.contentWindow && ev.source !== iframe.contentWindow) return null;
  return { slot: match, iframe };
}

function useSlotBootstrapHandshake(opts: {
  nodeLabel: string;
  slotsRef: React.MutableRefObject<IframeKeepAliveSlot[]>;
  activeSlotIdRef: React.MutableRefObject<string | null>;
  latestParamsRef: React.MutableRefObject<SandboxedBridgeParams>;
  outboundQueueRef: React.MutableRefObject<Array<Record<string, unknown>>>;
  seqRef: React.MutableRefObject<number>;
  setSlots: React.Dispatch<React.SetStateAction<IframeKeepAliveSlot[]>>;
  setProtocolError: React.Dispatch<React.SetStateAction<string | null>>;
}) {
  const { nodeLabel, slotsRef, activeSlotIdRef, latestParamsRef, outboundQueueRef, seqRef, setSlots, setProtocolError } = opts;
  const { clearSlotRetryTimer, setSlotRetryTimer } = useRetryTimersMap();

  const sendInitPortToSlot = useCallback((slot: IframeKeepAliveSlot, iframeEl: HTMLIFrameElement) => {
    if (slot.isPortVerified || slot.initPortDispatched) return;
    if (!isSandboxFrameAllowedToBind(iframeEl) || typeof MessageChannel === "undefined" || !iframeEl.contentWindow) return;
    slot.initPortDispatched = true;
    slot.port1?.close();
    const channel = new MessageChannel();
    channel.port1.onmessage = (ev) => processValidSlotMessage(ev, {
      slot, channel, nodeLabel, latestParamsRef, outboundQueueRef,
      clearRetryTimer: () => clearSlotRetryTimer(slot.slotId),
      isActiveSlot: () => activeSlotIdRef.current === slot.slotId,
      setProtocolError, seqRef, refreshSlots: () => setSlots((p) => [...p]),
    });
    iframeEl.contentWindow.postMessage({ type: "INIT_PORT", epoch: slot.epoch, bootSecret: slot.bootSecret, channelSecret: slot.channelSecret, artifactSha256: slot.artifactSha256, seq: ++seqRef.current }, "*", [channel.port2]);
    slot.port1 = channel.port1;
    setSlotRetryTimer(slot.slotId, () => advanceSlotBootstrapRetry(slot, setProtocolError, setSlots));
    setSlots((p) => [...p]);
  }, [activeSlotIdRef, clearSlotRetryTimer, latestParamsRef, nodeLabel, outboundQueueRef, seqRef, setProtocolError, setSlotRetryTimer, setSlots]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const onMessage = (ev: MessageEvent) => {
      const matched = findMatchingPortReadySlot(ev, slotsRef.current);
      if (matched) sendInitPortToSlot(matched.slot, matched.iframe);
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [sendInitPortToSlot, slotsRef]);

  return sendInitPortToSlot;
}

function useBridgePostActions(
  postToPort: (payload: Record<string, unknown>) => void,
  artifactSha256: string,
  activeSlot: IframeKeepAliveSlot | null
) {
  const postApplyHtml = useCallback(
    (html: string) => {
      const prepared = activeSlot?.scriptNonce ? prepareHtmlForApplyBridge(html, activeSlot.scriptNonce) : html;
      if (activeSlot) {
        activeSlot.rawHtml = html;
        if (activeSlot.outboundQueue.length > 0) {
          activeSlot.outboundQueue = activeSlot.outboundQueue.filter((frame) => frame.type !== "PATCH_BY_FF_ID");
        }
      }
      postToPort({ type: "APPLY_HTML", html: prepared });
    },
    [activeSlot, postToPort]
  );
  const postFixtureHydrate = useCallback(
    (variant: FixtureVariant, fixtureJson?: string) => postToPort({ type: "FLOW_PREVIEW_HYDRATE", variant, artifactSha256, fixtureJson }),
    [artifactSha256, postToPort]
  );
  const postCtaHoverSync = useCallback((ffId: string, hovered: boolean) => postToPort({ type: "CTA_HOVER_SYNC", ffId, hovered }), [postToPort]);
  const postPatchByFfId = useCallback(
    (ffId: string, patch: { textContent?: string; safeStyles?: Record<string, string>; bulletItems?: string[]; chartsJson?: string; slideUid?: string; slotKind?: string; revision?: number }) =>
      postToPort({ type: "PATCH_BY_FF_ID", ffId, slotKind: patch.slotKind || "text", slideUid: patch.slideUid, revision: patch.revision, textContent: patch.textContent, bulletItems: patch.bulletItems, chartsJson: patch.chartsJson, safeStyles: patch.safeStyles }),
    [postToPort]
  );
  const postDeckKeyNav = useCallback((direction: "prev" | "next" | "goto", targetIndex?: number) => postToPort({ type: "DECK_KEY_NAV", direction, targetIndex }), [postToPort]);
  const postDeckBlackout = useCallback((mode: "none" | "black" | "white") => postToPort({ type: "DECK_BLACKOUT_SET", mode }), [postToPort]);
  const postRender2DChart = useCallback(
    (opts: { slideUid: string; ffId: string; spec: ChartSpecV12; ariaLabel: string }) =>
      postToPort({ type: "RENDER_2D_CHART", slideUid: opts.slideUid, ffId: opts.ffId, spec: opts.spec, ariaLabel: opts.ariaLabel }),
    [postToPort]
  );
  return { postApplyHtml, postFixtureHydrate, postCtaHoverSync, postPatchByFfId, postDeckKeyNav, postDeckBlackout, postRender2DChart };
}

export function useSandboxedIframeBridge(params: SandboxedBridgeParams) {
  const latestParamsRef = useRef(params);
  latestParamsRef.current = params;
  const projectId = params.projectId || "default-proj";
  const nodeId = params.structureLens === "slide-cms" ? params.activeSlideId : (params.selectedNodeId || params.activeSlideId);
  const nodeLabel = params.structureLens === "slide-cms" ? params.activeSlideId : (params.selectedNodeKey || params.selectedNodeId || params.activeSlideId);
  const fallbackRef = useRef<HTMLIFrameElement>(null);
  const pool = useKeepAlivePoolState({
    projectId, nodeId, rawHtml: params.rawHtml, structureLens: params.structureLens,
    committedSlideCacheKey: params.committedSlideCacheKey, pinnedSlideIds: params.pinnedSlideIds,
    activeSubState: params.activeSubState, subStateLabels: params.subStateLabels,
  });
  const { activeSlot, activeSlotIdRef, resolvedSha256, slots, setSlots, slotsRef, seqRef } = pool;
  const outboundQueueRef = useRef<Array<Record<string, unknown>>>([]);
  const [protocolError, setProtocolError] = useState<string | null>(null);
  const sendInitPortToSlot = useSlotBootstrapHandshake({ nodeLabel, slotsRef, activeSlotIdRef, latestParamsRef, outboundQueueRef, seqRef, setSlots, setProtocolError });

  const handleSlotIframeLoad = useCallback((slotId: string, iframeEl: HTMLIFrameElement | null) => {
    const slot = iframeEl ? slotsRef.current.find((s) => s.slotId === slotId) : undefined;
    if (slot && iframeEl) sendInitPortToSlot(slot, iframeEl);
  }, [sendInitPortToSlot, slotsRef]);

  const postToPort = useCallback((payload: Record<string, unknown>) => {
    if (!activeSlot) return;
    if (!activeSlot.port1 || !activeSlot.isPortVerified) {
      enqueueOutboundFrame(activeSlot.outboundQueue, payload);
      return;
    }
    activeSlot.port1.postMessage({ ...payload, epoch: activeSlot.epoch, bootSecret: activeSlot.bootSecret, channelSecret: activeSlot.channelSecret, seq: ++seqRef.current });
  }, [activeSlot, seqRef]);

  const snapshot = buildActiveSlotSnapshot(activeSlot, resolvedSha256, fallbackRef, protocolError);
  const postActions = useBridgePostActions(postToPort, snapshot.artifactSha256, activeSlot);
  useEffect(() => {
    if (params.structureLens !== "slide-cms" || !activeSlot || !params.effectiveCustomHtml) return;
    if (activeSlot.rawHtml === params.effectiveCustomHtml) return;
    postActions.postApplyHtml(params.effectiveCustomHtml);
  }, [activeSlot, params.effectiveCustomHtml, params.structureLens, postActions]);
  const handleIframeLoad = useCallback(() => {
    if (activeSlot?.iframeRef.current) handleSlotIframeLoad(activeSlot.slotId, activeSlot.iframeRef.current);
  }, [activeSlot, handleSlotIframeLoad]);
  return { ...snapshot, ...postActions, targetCacheKey: activeSlot?.cacheKey ?? "", handleIframeLoad, handleSlotIframeLoad, poolSlots: slots, activeSlot };
}
