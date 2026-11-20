// @story 06.1, 06.3
// @implements AC-05
import React from "react";
import { IframeToHostMessageSchema } from "./html-sandbox-wrapper";
import {
  IframeToHostDeckMessageSchema,
  type IframeToHostDeckMessage,
} from "../../../lib/validations/presentation";
import type { ElementInspectMetadata } from "../../../lib/validations/flow-workbench";
import type { FixtureVariant } from "../../../lib/validations/screen-fixture";
import type {
  ElementRectBox,
  IframeKeepAliveSlot,
  SpatialTargetSelection,
} from "./sandbox-bridge-protocol";

export const MAX_OUTBOUND_QUEUE_FRAMES = 32;

export interface BridgeCallbackParams {
  activeSubState: FixtureVariant;
  onRegionClick: (regionId: string, label: string, ffId?: string, domHash?: string, rect?: ElementRectBox) => void;
  onSpatialTargetSelect?: (target: SpatialTargetSelection) => void;
  onQuoteText?: (text: string) => void;
  onDeckProtocolEvent?: (msg: IframeToHostDeckMessage) => void;
}

export interface InboundHandlerDeps {
  slot: IframeKeepAliveSlot;
  channel: MessageChannel;
  nodeLabel: string;
  latestParamsRef: React.MutableRefObject<BridgeCallbackParams>;
  outboundQueueRef: React.MutableRefObject<Array<Record<string, unknown>>>;
  clearRetryTimer: () => void;
  setProtocolError: React.Dispatch<React.SetStateAction<string | null>>;
  seqRef: React.MutableRefObject<number>;
  refreshSlots: () => void;
  isActiveSlot?: () => boolean;
}

function handlePortAck(deps: InboundHandlerDeps, artifactSha256: string): void {
  const { slot, channel, clearRetryTimer, setProtocolError, seqRef, refreshSlots } = deps;
  if (artifactSha256 !== slot.artifactSha256) {
    channel.port1.close();
    return;
  }
  clearRetryTimer();
  slot.isPortVerified = true;
  slot.protocolStatus = "PORT_BOUND";
  if (!deps.isActiveSlot || deps.isActiveSlot()) setProtocolError(null);
  const queued = slot.outboundQueue.splice(0, MAX_OUTBOUND_QUEUE_FRAMES);
  for (const frame of queued) {
    channel.port1.postMessage({
      ...frame,
      epoch: slot.epoch,
      bootSecret: slot.bootSecret,
      channelSecret: slot.channelSecret,
      seq: ++seqRef.current,
    });
  }
  refreshSlots();
}

function recordRectAndBuildTarget(
  deps: InboundHandlerDeps,
  msg: {
    selector: string;
    elementFfId?: string;
    rect?: ElementRectBox;
    domHash?: string;
    tagName?: string;
    metadata?: ElementInspectMetadata;
    clientX?: number;
    clientY?: number;
    altKey?: boolean;
    shiftKey?: boolean;
    metaKey?: boolean;
    ctrlKey?: boolean;
  },
  triggerSource: "select-click" | "interact-contextmenu"
): { ffId: string; label: string; target: SpatialTargetSelection } {
  const ffId = msg.elementFfId || msg.selector;
  const fallbackLeft = triggerSource === "interact-contextmenu" ? (msg.clientX ?? 16) : 16;
  const fallbackTop = triggerSource === "interact-contextmenu" ? (msg.clientY ?? 16) : 16;
  const rect = msg.rect ?? { left: fallbackLeft, top: fallbackTop, width: 120, height: 32 };
  if (msg.rect) {
    deps.slot.elementRects = { ...deps.slot.elementRects, [ffId]: msg.rect };
    deps.refreshSlots();
  }
  const label = `@[Element ${deps.nodeLabel} • #${ffId}]`;
  const target: SpatialTargetSelection = {
    targetType: "html_element",
    triggerSource,
    selector: msg.selector,
    label,
    elementFfId: ffId,
    coveredFfIds: [ffId],
    domHash: msg.domHash,
    rect,
    metadata: msg.metadata ?? { tagName: msg.tagName },
    ...(triggerSource === "select-click"
      ? { modifiers: { altKey: msg.altKey, shiftKey: msg.shiftKey, metaKey: msg.metaKey, ctrlKey: msg.ctrlKey } }
      : {}),
  };
  return { ffId, label, target };
}

function handleElementInteraction(
  deps: InboundHandlerDeps,
  msg: Extract<ReturnType<typeof IframeToHostMessageSchema.parse>, { type: "ELEMENT_CLICKED" | "ELEMENT_CONTEXT_MENU" }>
): void {
  const isClick = msg.type === "ELEMENT_CLICKED";
  const { ffId, label, target } = recordRectAndBuildTarget(
    deps,
    msg,
    isClick ? "select-click" : "interact-contextmenu"
  );
  if (deps.isActiveSlot && !deps.isActiveSlot()) return;
  if (isClick) {
    deps.latestParamsRef.current.onRegionClick(msg.selector, label, ffId, msg.domHash, msg.rect);
  }
  deps.latestParamsRef.current.onSpatialTargetSelect?.(target);
}

function dispatchSlotMessageByType(
  msg: ReturnType<typeof IframeToHostMessageSchema.parse>,
  deps: InboundHandlerDeps
): void {
  const { slot, channel, latestParamsRef, seqRef, refreshSlots } = deps;
  const activeSubState = latestParamsRef.current.activeSubState;
  if (msg.type === "PORT_ACK") return handlePortAck(deps, msg.artifactSha256);
  if (msg.type === "READY" && activeSubState && activeSubState !== "DEFAULT") {
    channel.port1.postMessage({
      type: "FLOW_PREVIEW_HYDRATE",
      epoch: slot.epoch,
      bootSecret: slot.bootSecret,
      channelSecret: slot.channelSecret,
      variant: activeSubState,
      artifactSha256: slot.artifactSha256,
      seq: ++seqRef.current,
    });
    return;
  }
  if (msg.type === "ELEMENT_RECTS") {
    slot.elementRects = msg.rects;
    refreshSlots();
    return;
  }
  if (msg.type === "ELEMENT_CLICKED" || msg.type === "ELEMENT_CONTEXT_MENU") {
    return handleElementInteraction(deps, msg);
  }
  if (msg.type === "TEXT_QUOTE" && (!deps.isActiveSlot || deps.isActiveSlot())) {
    latestParamsRef.current.onQuoteText?.(msg.text);
  }
}

function tryHandleDeckProtocolMessage(raw: Record<string, unknown>, deps: InboundHandlerDeps): boolean {
  const deckParsed = IframeToHostDeckMessageSchema.safeParse(raw);
  if (!deckParsed.success) return false;
  if (!deps.slot.isPortVerified || deckParsed.data.channelSecret !== deps.slot.channelSecret) {
    deps.channel.port1.close();
    return true;
  }
  if (deckParsed.data.seq <= deps.slot.lastAcceptedSeq) return true;
  deps.slot.lastAcceptedSeq = deckParsed.data.seq;
  if (!deps.isActiveSlot || deps.isActiveSlot()) {
    deps.latestParamsRef.current.onDeckProtocolEvent?.(deckParsed.data);
  }
  return true;
}

export function processValidSlotMessage(ev: MessageEvent, deps: InboundHandlerDeps): void {
  const raw = ev.data as Record<string, unknown> | null;
  if (!raw || typeof raw !== "object") return;
  const bootAckOk = raw.bootSecretAck === deps.slot.bootSecret;
  const channelOk = typeof raw.channelSecret === "string" && raw.channelSecret === deps.slot.channelSecret;
  if (raw.epoch !== deps.slot.epoch || !bootAckOk || !channelOk) {
    deps.channel.port1.close();
    return;
  }
  if (tryHandleDeckProtocolMessage(raw, deps)) return;
  const parsed = IframeToHostMessageSchema.safeParse(raw);
  if (!parsed.success) return;
  if (!deps.slot.isPortVerified && parsed.data.type !== "PORT_ACK") {
    deps.channel.port1.close();
    return;
  }
  const eventSeq = parsed.data.seq;
  if (typeof eventSeq === "number") {
    if (eventSeq <= deps.slot.lastAcceptedSeq) return;
    deps.slot.lastAcceptedSeq = eventSeq;
  } else if (parsed.data.type !== "PORT_ACK") {
    return;
  }
  dispatchSlotMessageByType(parsed.data, deps);
}
