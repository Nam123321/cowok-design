// @story 05.2
import React, { useEffect, useMemo, useRef, useState } from "react";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import { MAX_PROPOSAL_TRADEOFFS, type DecideTradeoffInput, type ProposalTradeoff } from "../../../lib/validations/change-proposal";
import {
  computeAutoFlipPopoverPosition,
  resolveDesignerTalkHotkeyAction,
  shouldSuppressProposalHotkey,
  type ElementRectLike,
  type PopoverGeometry,
} from "../utils/diff-engine";

const DEFAULT_STAGE_WIDTH = 960;
const DEFAULT_STAGE_HEIGHT = 600;
const INITIAL_POPOVER_TOP = 48;
const INITIAL_POPOVER_LEFT = 16;
const INITIAL_POPOVER_MAX_H = 260;
const FALLBACK_ANCHOR_SIZE = 28;
const BADGE_MIN_OFFSET = 4;
const BADGE_RIGHT_INSET = 14;
const FALLBACK_ANCHOR_RECT: ElementRectLike = { left: INITIAL_POPOVER_LEFT, top: BADGE_RIGHT_INSET, width: FALLBACK_ANCHOR_SIZE, height: FALLBACK_ANCHOR_SIZE };
const INITIAL_POPOVER_GEOM: PopoverGeometry = { placement: "bottom", left: INITIAL_POPOVER_LEFT, top: INITIAL_POPOVER_TOP, maxHeight: INITIAL_POPOVER_MAX_H };

export interface DesignerTalkOverlayProps {
  isOpen: boolean;
  tradeoffs: ProposalTradeoff[];
  elementRects: Record<string, ElementRectLike>;
  drawerWidth?: number;
  isModalOpen?: boolean;
  isInlineEditorOpen?: boolean;
  onClose?: () => void;
  onDecide: (tradeoffId: string, input: DecideTradeoffInput) => Promise<void> | void;
}

function getBadgeClass(item: ProposalTradeoff, isActive: boolean): string {
  const active = isActive ? " cp-dt-badge--active" : "";
  let decision = "";
  if (item.decision === "agreed") decision = " cp-dt-badge--agreed";
  else if (item.decision === "disagreed_instead") decision = " cp-dt-badge--disagreed";
  return `cp-dt-badge${active}${decision}`;
}

function usePopoverGeometrySync(opts: {
  overlayRef: React.RefObject<HTMLDivElement | null>;
  isOpen: boolean;
  activeItem: ProposalTradeoff | null;
  elementRects: Record<string, ElementRectLike>;
  drawerWidth: number;
}): PopoverGeometry {
  const { overlayRef, isOpen, activeItem, elementRects, drawerWidth } = opts;
  const [popoverGeom, setPopoverGeom] = useState<PopoverGeometry>(INITIAL_POPOVER_GEOM);
  useEffect(() => {
    if (!isOpen || !activeItem) return;
    const rafId = requestAnimationFrame(() => {
      const box = overlayRef.current?.getBoundingClientRect();
      const width = box && box.width > 0 ? box.width : DEFAULT_STAGE_WIDTH;
      const height = box && box.height > 0 ? box.height : DEFAULT_STAGE_HEIGHT;
      const anchor = elementRects[activeItem.elementFfId] ?? FALLBACK_ANCHOR_RECT;
      setPopoverGeom(computeAutoFlipPopoverPosition(anchor, { width, height, drawerWidth }));
    });
    return () => cancelAnimationFrame(rafId);
  }, [activeItem, drawerWidth, elementRects, isOpen, overlayRef]);
  return popoverGeom;
}

function useTradeoffKeyboardNav(opts: {
  isOpen: boolean;
  boundedTradeoffs: ProposalTradeoff[];
  activeItem: ProposalTradeoff | null;
  isModalOpen: boolean;
  isInlineEditorOpen: boolean;
  setActiveId: (id: string) => void;
  onClose?: () => void;
  onDecide: (tradeoffId: string, input: DecideTradeoffInput) => Promise<void> | void;
}) {
  const { isOpen, boundedTradeoffs, activeItem, isModalOpen, isInlineEditorOpen, setActiveId, onClose, onDecide } = opts;
  useEffect(() => {
    if (!isOpen || boundedTradeoffs.length === 0) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (shouldSuppressProposalHotkey(e, { isModalOpen, isInlineEditorOpen })) return;
      const action = resolveDesignerTalkHotkeyAction(e.key);
      if (!action) return;
      if (action.type === "close") return void onClose?.();
      if (action.type === "step") {
        e.preventDefault();
        const curIdx = activeItem ? boundedTradeoffs.findIndex((x) => x.id === activeItem.id) : -1;
        return setActiveId(boundedTradeoffs[(curIdx + action.delta + boundedTradeoffs.length) % boundedTradeoffs.length].id);
      }
      if (action.type === "select_index") {
        const hit = boundedTradeoffs.find((x) => x.badgeIndex === action.badgeIndex);
        if (hit) setActiveId(hit.id);
        return;
      }
      if (activeItem) { e.preventDefault(); void onDecide(activeItem.id, { decision: action.decision }); }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [activeItem, boundedTradeoffs, isInlineEditorOpen, isModalOpen, isOpen, onClose, onDecide, setActiveId]);
}

const TradeoffPopoverCard: React.FC<{
  item: ProposalTradeoff;
  geom: PopoverGeometry;
  onClose?: () => void;
  onDecide: (tradeoffId: string, input: DecideTradeoffInput) => Promise<void> | void;
}> = ({ item, geom, onClose, onDecide }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const [askMode, setAskMode] = useState(false);
  const [askText, setAskText] = useState("");

  return (
    <div className="cp-dt-popover" role="region" aria-label={t("proposals.designerTalkTitle", "Designer Talk Trade-off")} data-testid="designer-talk-popover" data-placement={geom.placement} style={{ left: `${geom.left}px`, top: `${geom.top}px`, maxHeight: `${geom.maxHeight}px` }}>
      <div className="cp-dt-popover-header">
        <span className="cp-dt-index-pill">#{formatNumber(item.badgeIndex)}</span>
        <span className="cp-dt-ff-chip">{item.elementFfId}</span>
        <button type="button" className="cp-icon-close-btn" aria-label={t("common.close", "Close")} onClick={onClose}>✕</button>
      </div>
      <p className="cp-dt-note">{item.designNote}</p>
      <div className="cp-dt-instead"><strong>{t("proposals.insteadPrefix", "Instead")}:</strong> {item.insteadLabel}</div>
      <div className="cp-dt-actions">
        <button type="button" className="cp-dt-btn cp-dt-btn--agree" data-testid="dt-agree-btn" onClick={() => void onDecide(item.id, { decision: "agreed" })}>{t("proposals.agreeBtn", "Agree")}</button>
        <button type="button" className="cp-dt-btn cp-dt-btn--disagree" data-testid="dt-disagree-btn" onClick={() => void onDecide(item.id, { decision: "disagreed_instead" })}>{t("proposals.disagreeInsteadBtn", "Disagree (instead)")}</button>
        <button type="button" className="cp-dt-btn" data-testid="dt-ask-btn" onClick={() => setAskMode((v) => !v)}>{t("proposals.askBtn", "Ask")}</button>
        <button type="button" className="cp-dt-btn" data-testid="dt-skip-btn" onClick={() => void onDecide(item.id, { decision: "skipped" })}>{t("proposals.skipBtn", "Skip")}</button>
      </div>
      {askMode && (
        <form className="cp-dt-ask-form" onSubmit={(e) => { e.preventDefault(); if (!askText.trim()) return; void onDecide(item.id, { decision: "asked", askPrompt: askText.trim() }); setAskText(""); setAskMode(false); }}>
          <input type="text" className="cp-dt-ask-input" data-testid="dt-ask-input" value={askText} onChange={(e) => setAskText(e.target.value)} placeholder={t("proposals.askPlaceholder", "Ask about this trade-off...")} />
          <button type="submit" className="cp-dt-btn">{t("common.send", "Send")}</button>
        </form>
      )}
    </div>
  );
};

const DesignerTalkEmptyCard: React.FC<{ onClose?: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  return (
    <div className="cp-dt-overlay" data-testid="designer-talk-overlay" data-ff-id="designer-talk-overlay">
      <div className="cp-dt-popover" data-testid="designer-talk-empty-card" style={{ left: `${INITIAL_POPOVER_LEFT}px`, top: `${INITIAL_POPOVER_TOP}px` }}>
        <div className="cp-dt-popover-header">
          <span className="cp-dt-index-pill">T</span>
          <span className="cp-dt-ff-chip">{t("proposals.designerTalkTitle", "Designer Talk Trade-off")}</span>
          <button type="button" className="cp-icon-close-btn" aria-label={t("common.close", "Close")} onClick={onClose}>✕</button>
        </div>
        <p className="cp-dt-note">{t("proposals.designerTalkEmpty", "No design trade-offs recorded yet for this selection. Generate or edit a ChangeProposal first.")}</p>
      </div>
    </div>
  );
};

export const DesignerTalkOverlay: React.FC<DesignerTalkOverlayProps> = ({
  isOpen, tradeoffs, elementRects, drawerWidth = 0, isModalOpen = false, isInlineEditorOpen = false, onClose, onDecide,
}) => {
  const { formatNumber } = useFormatter();
  const overlayRef = useRef<HTMLDivElement>(null);
  const boundedTradeoffs = useMemo(() => tradeoffs.slice(0, MAX_PROPOSAL_TRADEOFFS), [tradeoffs]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const activeItem = useMemo(() => boundedTradeoffs.find((item) => item.id === activeId) ?? boundedTradeoffs[0] ?? null, [activeId, boundedTradeoffs]);
  const popoverGeom = usePopoverGeometrySync({ overlayRef, isOpen, activeItem, elementRects, drawerWidth });
  useTradeoffKeyboardNav({ isOpen, boundedTradeoffs, activeItem, isModalOpen, isInlineEditorOpen, setActiveId, onClose, onDecide });

  if (!isOpen) return null;
  if (boundedTradeoffs.length === 0) return <DesignerTalkEmptyCard onClose={onClose} />;
  const docked = boundedTradeoffs.filter((item) => !elementRects[item.elementFfId]);
  const anchored = boundedTradeoffs.filter((item) => Boolean(elementRects[item.elementFfId]));

  return (
    <div ref={overlayRef} className="cp-dt-overlay" data-testid="designer-talk-overlay" data-ff-id="designer-talk-overlay">
      {docked.length > 0 && (
        <div className="cp-dt-dock">
          {docked.map((item) => (
            <button key={item.id} type="button" className={getBadgeClass(item, activeItem?.id === item.id)} data-testid={`designer-talk-badge-${item.badgeIndex}`} data-ff-id={item.elementFfId} onClick={() => setActiveId(item.id)}>
              {formatNumber(item.badgeIndex)}
            </button>
          ))}
        </div>
      )}
      {anchored.map((item) => {
        const rect = elementRects[item.elementFfId];
        return (
          <button key={item.id} type="button" className={getBadgeClass(item, activeItem?.id === item.id)} data-testid={`designer-talk-badge-${item.badgeIndex}`} data-ff-id={item.elementFfId} style={{ left: `${Math.max(BADGE_MIN_OFFSET, rect.left + rect.width - BADGE_RIGHT_INSET)}px`, top: `${Math.max(BADGE_MIN_OFFSET, rect.top)}px` }} onClick={() => setActiveId(item.id)}>
            {formatNumber(item.badgeIndex)}
          </button>
        );
      })}
      {activeItem && <TradeoffPopoverCard key={activeItem.id} item={activeItem} geom={popoverGeom} onClose={onClose} onDecide={onDecide} />}
    </div>
  );
};
