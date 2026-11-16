// @story 04.1, 04.5
// @implements AC-02, AC-03 (128-bit CSPRNG bootSecret & Monotonic Epoch Sandbox Bridge Hook)
import { useCallback, useEffect, useRef, useState } from "react";
import { IframeToHostMessageSchema, computeArtifactSha256Sync, generateBootSecret, generateEpochNonce } from "../utils/html-sandbox-wrapper";
import type { FixtureVariant } from "../../../lib/validations/screen-fixture";

export interface ElementRectBox {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface SandboxedBridgeParams {
  activeTabId: string;
  selectedNodeId?: string | null;
  selectedNodeKey?: string | null;
  structureLens: "userflow" | "slide-cms";
  activeSlideId: string;
  activeSubState: FixtureVariant;
  effectiveCustomHtml?: string;
  rawHtml: string;
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
    // Opaque origin SecurityError is expected for allow-scripts srcDoc iframes.
  }
  return true;
}

function dispatchVerifiedIframeMessage(args: {
  ev: MessageEvent;
  epoch: number;
  bootSecret: string;
  artifactSha256: string;
  nodeLabel: string;
  port1: MessagePort;
  setIsPortVerified: (v: boolean) => void;
  setElementRects: React.Dispatch<React.SetStateAction<Record<string, ElementRectBox>>>;
  onRegionClick: SandboxedBridgeParams["onRegionClick"];
  onQuoteText?: (text: string) => void;
}): void {
  const parsed = IframeToHostMessageSchema.safeParse(args.ev.data);
  if (!parsed.success || parsed.data.epoch !== args.epoch) { args.port1.close(); return; }
  const msg = parsed.data;
  if (!msg.bootSecretAck || msg.bootSecretAck !== args.bootSecret) { args.port1.close(); return; }
  if (msg.type === "PORT_ACK") {
    if (msg.artifactSha256 === args.artifactSha256) args.setIsPortVerified(true);
    else args.port1.close();
    return;
  }
  if (msg.type === "READY") return;
  if (msg.type === "ELEMENT_RECTS") {
    args.setElementRects((prev) => ({ ...prev, ...msg.rects }));
    return;
  }
  if (msg.type === "ELEMENT_CLICKED") {
    const ffId = msg.elementFfId || msg.selector;
    if (msg.rect) args.setElementRects((prev) => ({ ...prev, [ffId]: msg.rect! }));
    args.onRegionClick(msg.selector, `@[Element ${args.nodeLabel} • #${ffId}]`, ffId, msg.domHash, msg.rect);
    return;
  }
  if (msg.type === "TEXT_QUOTE") args.onQuoteText?.(msg.text);
}

function useBridgeEpochRotation(args: {
  deps: readonly unknown[];
  port1Ref: React.MutableRefObject<MessagePort | null>;
  boundEpochRef: React.MutableRefObject<number | null>;
  setIsPortVerified: (v: boolean) => void;
  setElementRects: React.Dispatch<React.SetStateAction<Record<string, ElementRectBox>>>;
  setEpoch: (v: number) => void;
  setBootSecret: (v: string) => void;
}) {
  const isFirstMountRef = useRef(true);
  useEffect(() => {
    if (isFirstMountRef.current) { isFirstMountRef.current = false; return; }
    args.port1Ref.current?.close();
    args.port1Ref.current = null;
    args.boundEpochRef.current = null;
    args.setIsPortVerified(false);
    args.setElementRects({});
    args.setEpoch(generateEpochNonce());
    args.setBootSecret(generateBootSecret());
  }, args.deps);

  useEffect(() => () => { args.port1Ref.current?.close(); args.port1Ref.current = null; }, [args.port1Ref]);
}

export function useSandboxedIframeBridge(params: SandboxedBridgeParams) {
  const [epoch, setEpoch] = useState<number>(() => generateEpochNonce());
  const [bootSecret, setBootSecret] = useState<string>(() => generateBootSecret());
  const [isPortVerified, setIsPortVerified] = useState(false);
  const [elementRects, setElementRects] = useState<Record<string, ElementRectBox>>({});
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const port1Ref = useRef<MessagePort | null>(null);
  const boundEpochRef = useRef<number | null>(null);
  const artifactSha256 = computeArtifactSha256Sync(params.rawHtml);

  useBridgeEpochRotation({
    deps: [params.activeTabId, params.selectedNodeId, params.structureLens, params.activeSlideId, params.activeSubState, params.effectiveCustomHtml, params.rawHtml],
    port1Ref, boundEpochRef, setIsPortVerified, setElementRects, setEpoch, setBootSecret,
  });

  const handleIframeLoad = useCallback(() => {
    const iframeEl = iframeRef.current;
    if (!iframeEl || !isSandboxFrameAllowedToBind(iframeEl) || boundEpochRef.current === epoch) return;
    if (typeof MessageChannel === "undefined" || !iframeEl.contentWindow) return;
    port1Ref.current?.close();
    const channel = new MessageChannel();
    const nodeLabel = params.selectedNodeKey || params.selectedNodeId || "SCR-02";
    channel.port1.onmessage = (ev: MessageEvent) =>
      dispatchVerifiedIframeMessage({ ev, epoch, bootSecret, artifactSha256, nodeLabel, port1: channel.port1, setIsPortVerified, setElementRects, onRegionClick: params.onRegionClick, onQuoteText: params.onQuoteText });
    iframeEl.contentWindow.postMessage({ type: "INIT_PORT", epoch, bootSecret, artifactSha256 }, "*", [channel.port2]);
    port1Ref.current = channel.port1;
    boundEpochRef.current = epoch;
  }, [artifactSha256, bootSecret, epoch, params]);

  const postApplyHtml = useCallback((html: string) => port1Ref.current?.postMessage({ type: "APPLY_HTML", epoch, bootSecret, html }), [bootSecret, epoch]);
  const postFixtureHydrate = useCallback(
    (variant: FixtureVariant, fixtureJson: string) => port1Ref.current?.postMessage({ type: "FLOW_PREVIEW_HYDRATE", epoch, bootSecret, variant, artifactSha256, fixtureJson }),
    [artifactSha256, bootSecret, epoch]
  );

  return { epoch, bootSecret, artifactSha256, isPortVerified, elementRects, iframeRef, handleIframeLoad, postApplyHtml, postFixtureHydrate };
}
