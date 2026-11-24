// @story 05.6
// @implements AC-01, AC-02, AC-03, AC-05, AC-06, [5.26] StudioDrawerShell, PAT-09, CMP-17
import React, { useCallback, useId, useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import {
  StudioCanvasPinsLayer,
  StudioDrawerShell,
  StudioMarqueeOverlay,
  StudioSpatialActionBar,
  StudioThreeTierToolbar,
  useStudioDrawerLayout,
  useStudioMarqueeSelection,
  type StudioPinnedComment,
  type StudioPointerMode,
  type StudioSpatialCoreAction,
  type StudioSpatialTarget,
  type StudioSubChatMessage,
  type StudioSubState,
  type StudioViewportPreset,
} from "../../components/ui/studio-drawer";
import { useTranslation } from "../../hooks/useTranslation";
import type { BrandProposalV1, TokenizedPagePreview } from "../../lib/validations/cognitive-studio";
import { interpretBrandSubChatCommand } from "./brandSubChatEngine";
import { HeritagePackPicker } from "./HeritagePackPicker";
import { PopularComponentAuditTable } from "./PopularComponentAuditTable";
import { SharedComponentGallery } from "./SharedComponentGallery";
import type { useBrandStudioController } from "./useBrandStudioController";

type StudioTab = "sketch-pages" | "component-gallery" | "popular-audit";
type Controller = ReturnType<typeof useBrandStudioController>;

const TOAST_Z = 62;
const STROKE_MONO = 1.85;
const ICON_SM = 14;
const FONT_WEIGHT_MEDIUM = 500;
const FONT_WEIGHT_SEMIBOLD = 600;
const LINE_HEIGHT_BODY = 1.4;
const MAX_PREVIEW_PAGES = 3;
const MICROS_PER_MS = 1000;
const MS_DECIMALS = 2;
const BRAND_MASTER_WIDTH_PX = 330;
const AVAILABLE_USER_MODES: readonly string[] = ["warm", "light", "dim", "dark", "night"];
const RADIUS_OPTIONS: readonly BrandProposalV1["radiusScale"][] = ["sharp", "balanced", "soft"];
const DENSITY_OPTIONS: readonly BrandProposalV1["density"][] = ["compact", "comfortable", "airy"];

const VIEWPORT_GRID_STYLES: Readonly<Record<StudioViewportPreset, React.CSSProperties>> = {
  "fold-outer-344": { maxWidth: "344px", margin: "0 auto", display: "flex", flexDirection: "column", gap: "12px" },
  "mobile-390": { maxWidth: "390px", margin: "0 auto", display: "flex", flexDirection: "column", gap: "12px" },
  "fold-inner-712": { maxWidth: "712px", margin: "0 auto", display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: "12px" },
  "tablet-768": { maxWidth: "768px", margin: "0 auto", display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))", gap: "12px" },
  "desktop-1440": { display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))", gap: "12px" },
};

export interface BrandVisualStudioModalProps {
  controller: Controller;
}

const ProposalCard: React.FC<{
  proposal: BrandProposalV1;
  activeModeId: string;
  isSelected: boolean;
  onSelect: () => void;
}> = ({ proposal, activeModeId, isSelected, onSelect }) => {
  const mode = proposal.themeModes.find((m) => m.modeId === activeModeId) ?? proposal.themeModes[0];
  const swatches = [
    mode?.tokens["--bg-canvas"], mode?.tokens["--bg-surface"],
    mode?.tokens["--action-primary"], mode?.tokens["--text-primary"],
  ].filter(Boolean) as string[];
  return (
    <button
      type="button"
      data-testid={`brand-proposal-card-${proposal.id}`}
      aria-pressed={isSelected}
      onClick={onSelect}
      style={{
        textAlign: "left", padding: "10px 12px", borderRadius: "var(--radius-sm)",
        border: isSelected ? "1.5px solid var(--action-primary)" : "1px solid var(--border-default)",
        backgroundColor: isSelected ? "var(--bg-elevated)" : "var(--bg-surface)",
        color: "var(--text-primary)", display: "flex", flexDirection: "column", gap: "6px", cursor: "pointer",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <strong style={{ fontSize: "12.5px" }}>{proposal.brandName}</strong>
        <span style={{ fontSize: "10px", color: "var(--text-secondary)" }}>{`${proposal.radiusScale} · ${proposal.density}`}</span>
      </div>
      <p style={{ margin: 0, fontSize: "11px", color: "var(--text-secondary)", lineHeight: LINE_HEIGHT_BODY }}>{proposal.rationale}</p>
      <div style={{ display: "flex", gap: "4px", alignItems: "center", flexWrap: "wrap" }}>
        {swatches.map((hex, i) => (
          <span key={`${proposal.id}-sw-${i}`} style={{ width: "14px", height: "14px", borderRadius: "50%", backgroundColor: hex, border: "1px solid var(--border-default)" }} />
        ))}
        {proposal.inspirationRefs.map((ref) => (
          <span key={ref} style={{ fontSize: "10px", padding: "1px 6px", borderRadius: "var(--radius-pill)", backgroundColor: "var(--bg-canvas)", color: "var(--text-secondary)" }}>{ref}</span>
        ))}
      </div>
    </button>
  );
};

const ModeSwitcherGroup: React.FC<{ controller: Controller; label: string }> = ({ controller, label }) => {
  if (controller.requestedModes.length <= 1) return null;
  return (
    <div role="group" data-testid="dynamic-theme-mode-switcher" aria-label={label} style={{ display: "inline-flex", gap: "3px", padding: "2px", borderRadius: "var(--radius-pill)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)" }}>
      {controller.requestedModes.map((modeId) => {
        const active = controller.activeModeId === modeId;
        return (
          <button
            key={modeId} type="button" data-testid={`studio-mode-switch-${modeId}`} aria-pressed={active}
            onClick={() => controller.selectMode(modeId)}
            style={{ height: "20px", padding: "0 8px", borderRadius: "var(--radius-pill)", border: "none", backgroundColor: active ? "var(--action-primary)" : "transparent", color: active ? "var(--action-primary-fg)" : "var(--text-secondary)", fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer" }}
          >
            {modeId}
          </button>
        );
      })}
    </div>
  );
};

const BrandModesAndRefsControls: React.FC<{ controller: Controller }> = ({ controller }) => {
  const { t } = useTranslation();
  return (
    <>
      <div style={{ display: "flex", flexDirection: "column", gap: "5px" }}>
        <span style={{ fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-secondary)" }}>
          {t("brandStudio.userThemeModesLabel", "Chế độ màu dự án (tùy chọn)")}
        </span>
        <div style={{ display: "flex", flexWrap: "wrap", gap: "5px" }}>
          {AVAILABLE_USER_MODES.map((modeId) => {
            const on = controller.requestedModes.includes(modeId);
            return (
              <button key={modeId} type="button" data-testid={`studio-request-mode-${modeId}`} aria-pressed={on} onClick={() => controller.toggleRequestedMode(modeId)} style={{ height: "23px", padding: "0 8px", borderRadius: "var(--radius-pill)", border: on ? "1px solid var(--action-primary)" : "1px solid var(--border-default)", backgroundColor: on ? "var(--bg-selected)" : "var(--bg-surface)", color: on ? "var(--action-primary)" : "var(--text-secondary)", fontSize: "11px", fontWeight: on ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM, cursor: "pointer" }}>
                {modeId}
              </button>
            );
          })}
        </div>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: "5px" }}>
        <label htmlFor="brand-studio-ref-apps" style={{ fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-secondary)" }}>
          {t("brandStudio.referenceAppsLabel", "Ứng dụng tham chiếu")}
        </label>
        <div style={{ display: "flex", gap: "6px" }}>
          <input id="brand-studio-ref-apps" type="text" data-testid="brand-studio-reference-apps-input" value={controller.referenceAppsText} onChange={(e) => controller.setReferenceAppsText(e.target.value)} placeholder="Linear, Stripe, Notion" style={{ flex: 1, minWidth: 0, height: "28px", padding: "0 8px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)", fontSize: "11.5px" }} />
          <button type="button" data-testid="brand-studio-regenerate-btn" disabled={controller.isLoadingStudio} onClick={() => { void controller.fetchBrandProposals({ refsText: controller.referenceAppsText }); }} style={{ height: "28px", padding: "0 10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--action-primary)", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer" }}>
            {t("brandStudio.regenerateBtn", "Tạo lại")}
          </button>
        </div>
      </div>
    </>
  );
};

const BrandMasterSidebar: React.FC<{
  controller: Controller;
  activeProposal: BrandProposalV1 | null;
  showHeritage: boolean;
  onToggleHeritage: () => void;
}> = ({ controller, activeProposal, showHeritage, onToggleHeritage }) => {
  const { t } = useTranslation();
  return (
    <div style={{ flex: 1, overflowY: "auto", padding: "10px 12px", display: "flex", flexDirection: "column", gap: "10px", minHeight: 0 }}>
      <BrandModesAndRefsControls controller={controller} />
      <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
        {controller.proposals.map((prop) => (
          <ProposalCard key={prop.id} proposal={prop} activeModeId={controller.activeModeId} isSelected={prop.id === activeProposal?.id} onSelect={() => controller.selectProposal(prop.id)} />
        ))}
      </div>
      <button type="button" data-testid="toggle-heritage-picker-btn" onClick={onToggleHeritage} style={{ height: "28px", padding: "0 10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)", fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer" }}>
        {t("brandStudio.inheritHeritageBtn", "Kế thừa Heritage Pack")}
      </button>
      {showHeritage && (
        <HeritagePackPicker packs={controller.heritagePacks} entryMode={controller.entryMode} onChangeEntryMode={controller.setEntryMode} onSavePack={controller.saveCurrentHeritagePack} onApplyPack={controller.applyHeritagePack} />
      )}
    </div>
  );
};

const StudioTabsSlot: React.FC<{ activeTab: StudioTab; onSelectTab: (tab: StudioTab) => void }> = ({ activeTab, onSelectTab }) => {
  const { t } = useTranslation();
  const tabs: readonly { id: StudioTab; label: string; title?: string }[] = [
    {
      id: "sketch-pages",
      label: t("brandStudio.tabSketchPages", "Xem trên màn hình"),
      title: t("brandStudio.tabSketchPagesTooltip", "Áp diện mạo mới lên tối đa 3 màn hình tiêu biểu của dự án để kiểm tra thực tế"),
    },
    {
      id: "component-gallery",
      label: t("brandStudio.tabGallery", "Thư viện thành phần"),
    },
    {
      id: "popular-audit",
      label: t("brandStudio.tabAudit", "Thành phần đề xuất"),
    },
  ];
  return (
    <div role="tablist" style={{ display: "inline-flex", gap: "4px", flexWrap: "wrap" }}>
      {tabs.map((tab) => {
        const active = activeTab === tab.id;
        return (
          <button key={tab.id} type="button" role="tab" aria-selected={active} title={tab.title ?? tab.label} data-testid={`studio-tab-${tab.id}`} onClick={() => onSelectTab(tab.id)} style={{ height: "24px", padding: "0 10px", borderRadius: "var(--radius-pill)", border: active ? "1px solid var(--action-primary)" : "1px solid var(--border-default)", backgroundColor: active ? "var(--bg-selected)" : "var(--bg-workspace)", color: active ? "var(--action-primary)" : "var(--text-secondary)", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer", whiteSpace: "nowrap" }}>
            {tab.label}
          </button>
        );
      })}
    </div>
  );
};

const BrandContextualTier3Strip: React.FC<{
  proposal: BrandProposalV1 | null;
  onPatchStyle: (patch: Partial<Pick<BrandProposalV1, "radiusScale" | "density">>) => void;
}> = ({ proposal, onPatchStyle }) => {
  if (!proposal) return null;
  const chipStyle = (active: boolean): React.CSSProperties => ({
    height: "20px", padding: "0 6px", borderRadius: "var(--radius-xs)",
    border: active ? "1px solid var(--action-primary)" : "1px solid var(--border-default)",
    backgroundColor: active ? "var(--bg-selected)" : "var(--bg-workspace)",
    color: active ? "var(--action-primary)" : "var(--text-secondary)",
    fontSize: "10px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer",
  });
  return (
    <>
      <div style={{ display: "inline-flex", alignItems: "center", gap: "5px", flexWrap: "wrap" }}>
        <span style={{ fontSize: "10px", color: "var(--text-muted)" }}>Radius:</span>
        {RADIUS_OPTIONS.map((r) => (
          <button key={r} type="button" data-testid={`brand-radius-${r}`} aria-pressed={proposal.radiusScale === r} onClick={() => onPatchStyle({ radiusScale: r })} style={chipStyle(proposal.radiusScale === r)}>{r}</button>
        ))}
        <span style={{ fontSize: "10px", color: "var(--text-muted)", marginLeft: "6px" }}>Density:</span>
        {DENSITY_OPTIONS.map((d) => (
          <button key={d} type="button" data-testid={`brand-density-${d}`} aria-pressed={proposal.density === d} onClick={() => onPatchStyle({ density: d })} style={chipStyle(proposal.density === d)}>{d}</button>
        ))}
      </div>
      <span style={{ fontSize: "10px", fontFamily: "var(--font-mono)", color: "var(--text-muted)", whiteSpace: "nowrap" }}>
        {`${proposal.fontPairing.heading.split(",")[0]} · ${proposal.fontPairing.body.split(",")[0]}`}
      </span>
    </>
  );
};

const SketchPagesGrid: React.FC<{
  pages: TokenizedPagePreview[];
  pointerMode: StudioPointerMode;
  viewportPreset: StudioViewportPreset;
  selectedFfId: string | null;
}> = ({ pages, pointerMode, viewportPreset, selectedFfId }) => (
  <div data-testid="sketch-pages-preview-grid" style={VIEWPORT_GRID_STYLES[viewportPreset]}>
    {pages.map((p) => {
      const ffId = `sketch-page:${p.screenId}`;
      const isSel = selectedFfId === ffId;
      return (
        <div key={p.screenId} data-ff-id={ffId} data-ff-label={p.title} style={{ border: isSel ? "1.5px solid var(--action-primary)" : "1px solid var(--border-default)", borderRadius: "var(--radius-sm)", overflow: "hidden", backgroundColor: "var(--bg-elevated)", cursor: pointerMode === "select-region" ? "crosshair" : "default" }}>
          <div style={{ padding: "6px 10px", fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)", borderBottom: "1px solid var(--border-default)", display: "flex", justifyContent: "space-between" }}>
            <span>{p.title}</span>
            <span style={{ color: "var(--text-secondary)", fontSize: "10.5px" }}>{p.pageRole}</span>
          </div>
          <iframe title={p.title} sandbox="" srcDoc={p.renderedHtml} data-testid={`sketch-preview-iframe-${p.screenId}`} style={{ width: "100%", height: "240px", border: "none", display: "block", pointerEvents: pointerMode === "interact" ? "auto" : "none" }} />
        </div>
      );
    })}
  </div>
);

const BrandSubStateNotice: React.FC<{ subState: StudioSubState; onReset: () => void }> = ({ subState, onReset }) => (
  <div data-testid={`brand-studio-substate-${subState.toLowerCase()}`} style={{ padding: "32px 16px", textAlign: "center", borderRadius: "var(--radius-sm)", border: "1px dashed var(--border-default)", backgroundColor: "var(--bg-elevated)", display: "flex", flexDirection: "column", alignItems: "center", gap: "8px" }}>
    <strong style={{ fontSize: "13px", color: "var(--text-primary)" }}>{`Trạng thái xem trước: ${subState}`}</strong>
    <button type="button" onClick={onReset} style={{ height: "26px", padding: "0 10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--action-primary)", backgroundColor: "var(--bg-surface)", color: "var(--action-primary)", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer" }}>
      Quay về DEFAULT
    </button>
  </div>
);

const StudioToastReceipt: React.FC<{ message: string | null; onDismiss: () => void }> = ({ message, onDismiss }) => {
  const { t } = useTranslation();
  if (!message) return null;
  return (
    <div role="status" aria-live="polite" data-testid="brand-studio-toast" style={{ position: "fixed", right: "18px", bottom: "18px", zIndex: TOAST_Z, display: "inline-flex", alignItems: "center", gap: "10px", padding: "8px 12px", borderRadius: "var(--radius-sm)", border: "1px solid var(--action-primary)", backgroundColor: "var(--bg-elevated)", color: "var(--text-primary)", boxShadow: "var(--shadow-lg)", fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
      <BrandIcon name="check-circle" size={ICON_SM} strokeWidth={STROKE_MONO} color="var(--action-primary)" />
      <span>{message}</span>
      <button type="button" aria-label={t("common.close", "Đóng")} data-testid="brand-studio-toast-close" onClick={onDismiss} style={{ border: "none", background: "transparent", color: "var(--text-secondary)", cursor: "pointer", padding: "0 2px" }}>
        <BrandIcon name="x" size={ICON_SM} strokeWidth={STROKE_MONO} />
      </button>
    </div>
  );
};

const INITIAL_BRAND_CHAT_MSG: StudioSubChatMessage = {
  id: "brand-welcome",
  role: "assistant",
  text: "Nhắn trực tiếp tại đây hoặc chọn/kéo vùng trên canvas để ghim bình luận — mọi thay đổi tự đồng bộ về Chat chính.",
};

type BrandChatActionOpts = {
  controller: Controller;
  activeProposal: BrandProposalV1 | null;
  selectedFfId: string | null;
  attachmentChips: string[];
  syncToMainChat: boolean;
  subChatInput: string;
  pinnedCount: number;
  expandSubChatPanel: () => void;
  onInspectTarget: () => void;
  setMessages: React.Dispatch<React.SetStateAction<StudioSubChatMessage[]>>;
  setPinnedComments: React.Dispatch<React.SetStateAction<StudioPinnedComment[]>>;
  setSubChatInput: React.Dispatch<React.SetStateAction<string>>;
  setAttachmentChips: React.Dispatch<React.SetStateAction<string[]>>;
  setSpatialTarget: React.Dispatch<React.SetStateAction<StudioSpatialTarget | null>>;
};

function useBrandStudioChatActions(opts: BrandChatActionOpts) {
  const { controller, activeProposal, selectedFfId, attachmentChips, syncToMainChat, subChatInput, pinnedCount, expandSubChatPanel, onInspectTarget, setMessages, setPinnedComments, setSubChatInput, setAttachmentChips, setSpatialTarget } = opts;
  const handleSendSubChat = useCallback(() => {
    const chipCtx = attachmentChips.length > 0 ? `[${attachmentChips.join(", ")}] ` : "";
    const fullPrompt = `${chipCtx}${subChatInput.trim()}`.trim();
    if (!fullPrompt) return;
    const res = interpretBrandSubChatCommand({ rawPrompt: fullPrompt, proposals: controller.proposals, activeProposal, activeModeId: controller.activeModeId, selectedFfId });
    controller.applySubChatBatch(res);
    const stamp = Date.now();
    setMessages((prev) => [...prev, { id: `bu-${stamp}`, role: "user", text: fullPrompt, scopeFfId: selectedFfId, syncedToMain: syncToMainChat }, { id: `ba-${stamp}`, role: "assistant", text: res.reply, syncedToMain: syncToMainChat }]);
    if (syncToMainChat) controller.syncToMainChat?.(res.syncSummary);
    setSubChatInput("");
    setAttachmentChips([]);
  }, [activeProposal, attachmentChips, controller, selectedFfId, setAttachmentChips, setMessages, setSubChatInput, subChatInput, syncToMainChat]);

  const handleSubmitSpatialComment = useCallback((target: StudioSpatialTarget, commentText: string) => {
    const nextNum = pinnedCount + 1;
    const label = target.label ?? target.ffId;
    setPinnedComments((prev) => [...prev, { id: `brand-pin-${Date.now()}-${nextNum}`, num: nextNum, ffId: target.ffId, label, text: commentText, x: target.x, y: target.y, kind: target.kind ?? "element", regionRect: target.regionRect }]);
    expandSubChatPanel();
    setMessages((prev) => [...prev, { id: `bc-${Date.now()}`, role: "user", kind: "comment", pinNumber: nextNum, text: `[${label}] ${commentText}`, scopeFfId: target.ffId, syncedToMain: syncToMainChat }]);
    if (syncToMainChat) controller.syncToMainChat?.(`[Brand Studio · Ghim #${nextNum} · ${label}] ${commentText}`);
    setSpatialTarget(null);
  }, [controller, expandSubChatPanel, pinnedCount, setMessages, setPinnedComments, setSpatialTarget, syncToMainChat]);

  const handleSpatialCoreAction = useCallback((action: StudioSpatialCoreAction, target: StudioSpatialTarget) => {
    const label = target.label ?? target.ffId;
    if (action === "inspect") onInspectTarget();
    else if (action === "ask-subchat" || action === "edit") {
      expandSubChatPanel();
      setAttachmentChips((prev) => Array.from(new Set([...prev, `⌖ ${label}`])));
      if (action === "edit") setSubChatInput(`Đổi bo góc mềm và mật độ gọn cho ${label}`);
    }
    setSpatialTarget(null);
  }, [expandSubChatPanel, onInspectTarget, setAttachmentChips, setSpatialTarget, setSubChatInput]);

  return { handleSendSubChat, handleSubmitSpatialComment, handleSpatialCoreAction };
}

function useBrandStudioWorkspaceState(
  controller: Controller,
  activeProposal: BrandProposalV1 | null,
  expandSubChatPanel: () => void,
  onInspectTarget: () => void
) {
  const [pointerMode, setPointerMode] = useState<StudioPointerMode>("interact");
  const [subState, setSubState] = useState<StudioSubState>("DEFAULT");
  const [viewportPreset, setViewportPreset] = useState<StudioViewportPreset>("desktop-1440");
  const [selectedFfId, setSelectedFfId] = useState<string | null>(null);
  const [spatialTarget, setSpatialTarget] = useState<StudioSpatialTarget | null>(null);
  const [pinnedComments, setPinnedComments] = useState<StudioPinnedComment[]>([]);
  const [attachmentChips, setAttachmentChips] = useState<string[]>([]);
  const [syncToMainChat, setSyncToMainChat] = useState(true);
  const [subChatInput, setSubChatInput] = useState("");
  const [messages, setMessages] = useState<StudioSubChatMessage[]>([INITIAL_BRAND_CHAT_MSG]);
  const actions = useBrandStudioChatActions({
    controller, activeProposal, selectedFfId, attachmentChips, syncToMainChat, subChatInput,
    pinnedCount: pinnedComments.length, expandSubChatPanel, onInspectTarget,
    setMessages, setPinnedComments, setSubChatInput, setAttachmentChips, setSpatialTarget,
  });
  const handleSelectTarget = useCallback((t: StudioSpatialTarget) => { setSelectedFfId(t.ffId); setSpatialTarget(t); }, []);
  const handleAttachFiles = useCallback((names: string[]) => setAttachmentChips((prev) => Array.from(new Set([...prev, ...names]))), []);
  const handleRemoveChip = useCallback((chip: string) => setAttachmentChips((prev) => prev.filter((c) => c !== chip)), []);
  return {
    pointerMode, setPointerMode, subState, setSubState, viewportPreset, setViewportPreset,
    selectedFfId, spatialTarget, setSpatialTarget, pinnedComments, attachmentChips,
    syncToMainChat, toggleSyncToMainChat: () => setSyncToMainChat((v) => !v),
    subChatInput, setSubChatInput, messages, ...actions, handleSelectTarget, handleAttachFiles, handleRemoveChip,
  };
}

const BrandDetailActiveTabBody: React.FC<{
  controller: Controller;
  ws: ReturnType<typeof useBrandStudioWorkspaceState>;
  activeTab: StudioTab;
  activeProposal: BrandProposalV1 | null;
  pages: TokenizedPagePreview[];
}> = ({ controller, ws, activeTab, activeProposal, pages }) => {
  const activeModeObj = activeProposal?.themeModes.find((m) => m.modeId === controller.activeModeId) ?? activeProposal?.themeModes[0] ?? null;
  if (ws.subState !== "DEFAULT") return <BrandSubStateNotice subState={ws.subState} onReset={() => ws.setSubState("DEFAULT")} />;
  return (
    <>
      {activeTab === "sketch-pages" && <SketchPagesGrid pages={pages} pointerMode={ws.pointerMode} viewportPreset={ws.viewportPreset} selectedFfId={ws.selectedFfId} />}
      {activeTab === "component-gallery" && <SharedComponentGallery proposal={activeProposal} activeMode={activeModeObj} auditItems={controller.auditItems} />}
      {activeTab === "popular-audit" && <PopularComponentAuditTable items={controller.auditItems} onToggleItem={controller.toggleAuditComponent} />}
    </>
  );
};

const BrandDetailPaneContent: React.FC<{
  controller: Controller;
  ws: ReturnType<typeof useBrandStudioWorkspaceState>;
  activeTab: StudioTab;
  onSelectTab: (tab: StudioTab) => void;
  onExpandSubChat: () => void;
  activeProposal: BrandProposalV1 | null;
  pages: TokenizedPagePreview[];
}> = ({ controller, ws, activeTab, onSelectTab, onExpandSubChat, activeProposal, pages }) => {
  const { t } = useTranslation();
  const marquee = useStudioMarqueeSelection({ pointerMode: ws.pointerMode, onSelectTarget: ws.handleSelectTarget });
  return (
    <>
      <StudioThreeTierToolbar
        pointerMode={ws.pointerMode} pinCount={ws.pinnedComments.length} isCommentsHighlighted={ws.pinnedComments.length > 0}
        onPointerModeChange={ws.setPointerMode} onToggleComments={() => { onExpandSubChat(); ws.setPointerMode("select-region"); }}
        row1Right={<StudioTabsSlot activeTab={activeTab} onSelectTab={onSelectTab} />}
        subState={ws.subState} onSubStateChange={ws.setSubState}
        row2LeftExtra={<ModeSwitcherGroup controller={controller} label={t("brandStudio.modeSwitcherLabel", "Chuyển chế độ màu dự án")} />}
        viewportPreset={ws.viewportPreset} onViewportPresetChange={ws.setViewportPreset}
        row3Strip={<BrandContextualTier3Strip proposal={activeProposal} onPatchStyle={controller.updateActiveProposalStyle} />}
      />
      <div ref={marquee.containerRef} data-testid="brand-studio-detail-canvas" onMouseDown={marquee.handleCanvasMouseDown} onContextMenu={marquee.handleCanvasContextMenu} style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "14px", position: "relative", userSelect: ws.pointerMode === "select-region" ? "none" : "auto" }}>
        <StudioMarqueeOverlay rect={marquee.marqueeRect} />
        <StudioCanvasPinsLayer pins={ws.pinnedComments} onSelectPin={() => onExpandSubChat()} />
        <StudioSpatialActionBar target={ws.spatialTarget} onCoreAction={ws.handleSpatialCoreAction} onSubmitComment={ws.handleSubmitSpatialComment} onClose={() => ws.setSpatialTarget(null)} />
        <BrandDetailActiveTabBody controller={controller} ws={ws} activeTab={activeTab} activeProposal={activeProposal} pages={pages} />
      </div>
    </>
  );
};

export const BrandVisualStudioModal: React.FC<BrandVisualStudioModalProps> = ({ controller }) => {
  const titleId = useId();
  const { t } = useTranslation();
  const layout = useStudioDrawerLayout(BRAND_MASTER_WIDTH_PX);
  const [activeTab, setActiveTab] = useState<StudioTab>("sketch-pages");
  const [showHeritage, setShowHeritage] = useState(false);

  const activeProposal = controller.proposals.find((p) => p.id === controller.selectedProposalId) ?? controller.proposals[0] ?? null;
  const pages = (controller.previewBundle?.pages ?? []).slice(0, MAX_PREVIEW_PAGES);
  const handleInspectTarget = useCallback(() => setActiveTab("component-gallery"), []);
  const ws = useBrandStudioWorkspaceState(controller, activeProposal, layout.expandSubChatPanel, handleInspectTarget);
  const renderMs = controller.previewBundle ? (controller.previewBundle.totalRenderTimeMicros / MICROS_PER_MS).toFixed(MS_DECIMALS) : "0.95";

  return (
    <StudioDrawerShell
      isOpen={controller.isStudioOpen}
      title={t("brandStudio.modalTitle", "Studio Diện mạo & Component dùng chung")}
      titleId={titleId} testId="brand-visual-studio-modal" titleTestId="brand-studio-drawer-title"
      layout={layout} closeButtonTestId="close-brand-studio-btn" onClose={controller.closeStudio}
      metricLine={
        <span data-testid="brand-studio-metric-line" style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
          <span data-testid="brand-studio-latency" style={{ fontSize: "11px", color: "var(--text-secondary)", fontVariantNumeric: "tabular-nums" }}>
            {`${renderMs}ms · WCAG AA >= 4.5:1`}
          </span>
        </span>
      }
      headerActions={
        <button type="button" data-testid="apply-brand-id-btn" onClick={controller.applySelectedBrand} style={{ height: "28px", padding: "0 12px", borderRadius: "var(--radius-xs)", border: "1px solid var(--action-primary)", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer", whiteSpace: "nowrap" }}>
          {t("brandStudio.applyBrandBtn", "Áp dụng Brand ID")}
        </button>
      }
      masterContent={<BrandMasterSidebar controller={controller} activeProposal={activeProposal} showHeritage={showHeritage} onToggleHeritage={() => setShowHeritage((v) => !v)} />}
      subChat={{
        selectedFfId: ws.selectedFfId, pinCount: ws.pinnedComments.length, inputValue: ws.subChatInput, messages: ws.messages,
        attachmentChips: ws.attachmentChips, onRemoveAttachmentChip: ws.handleRemoveChip, onAttachFiles: ws.handleAttachFiles,
        syncToMainChatEnabled: ws.syncToMainChat, onToggleSyncToMainChat: ws.toggleSyncToMainChat,
        onInputChange: ws.setSubChatInput, onSend: ws.handleSendSubChat,
      }}
      detailContent={<BrandDetailPaneContent controller={controller} ws={ws} activeTab={activeTab} onSelectTab={setActiveTab} onExpandSubChat={layout.expandSubChatPanel} activeProposal={activeProposal} pages={pages} />}
      toastSlot={<StudioToastReceipt message={controller.toastNotice} onDismiss={controller.clearToast} />}
    />
  );
};
