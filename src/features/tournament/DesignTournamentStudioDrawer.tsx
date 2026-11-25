// @story 10.3
// @implements [5.26] StudioDrawerShell, PAT-09, CMP-07 Design Tournament Master-Detail Studio Drawer with Scoped Sub-Chat & Smart Contextual Launcher
import React, { useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import {
  STUDIO_FONT_MEDIUM,
  STUDIO_FONT_SEMIBOLD,
  STUDIO_ICON_MD,
  STUDIO_ICON_SM,
  STUDIO_STROKE_MONO,
  StudioDrawerShell,
  type StudioSubChatMessage,
  useStudioDrawerLayout,
} from "../../components/ui/studio-drawer";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type {
  CrystallizePackageKind,
  GeneratedVariant,
  PersistedTournamentRun,
  SelectTournamentVariantData,
} from "../../lib/validations/tournament";
import { BlindWipeSliderBar } from "./BlindWipeSliderBar";
import { CrossVariantBorrowForm } from "./CrossVariantBorrowForm";
import { CrystallizeActionButton } from "./CrystallizeActionButton";
import { RubricSummaryBar } from "./DesignTournamentArenaPanel";
import {
  buildContextualTournamentPrompt,
  type DesignTournamentPort,
  dispatchTournamentSubChatSync,
  executeTournamentSubChatInstruction,
  formatTournamentRunTimestamp,
  formatVariantDiversityBadge,
  resolvePersistedWinnerLabel,
  useDesignTournament,
} from "./useDesignTournament";

export { buildContextualTournamentPrompt };

const TOURNAMENT_MASTER_WIDTH_PX = 388;
const MAX_PACKAGE_NAME_LEN = 48;
const RECENT_RUNS_DISPLAY_LIMIT = 5;
const BODY_LINE_HEIGHT = 1.5;
const CRYSTALLIZE_POPOVER_Z = 25;
const DISABLED_BTN_OPACITY = 0.45;

export interface DesignTournamentStudioDrawerProps {
  isOpen: boolean;
  onClose: () => void;
  projectId: string;
  activeTabTitle?: string;
  activeTabKind?: "html" | "slide" | "md" | "code";
  port?: DesignTournamentPort;
  onCommittedHtml?: (mergedHtml: string, result: SelectTournamentVariantData) => void;
  onActiveRunCountChange?: (count: number) => void;
}

type TournamentHookState = ReturnType<typeof useDesignTournament>;

const CARD_STYLE: React.CSSProperties = {
  display: "flex", flexDirection: "column", gap: "8px", padding: "14px",
  borderRadius: "var(--radius-md)", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)",
};
const PRIMARY_BTN_STYLE: React.CSSProperties = {
  display: "inline-flex", alignItems: "center", gap: "6px", padding: "7px 12px", borderRadius: "var(--radius-xs)",
  border: "none", backgroundColor: "var(--action-primary)", color: "var(--text-on-action)",
  fontSize: "12px", fontWeight: STUDIO_FONT_SEMIBOLD, whiteSpace: "nowrap",
};
const SECONDARY_BTN_STYLE: React.CSSProperties = {
  padding: "6px 12px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-card)", color: "var(--text-primary)", fontSize: "11.5px", fontWeight: STUDIO_FONT_SEMIBOLD, cursor: "pointer",
};
const CARD_TITLE_STYLE: React.CSSProperties = { margin: 0, fontSize: "13px", fontWeight: STUDIO_FONT_SEMIBOLD, color: "var(--text-primary)" };
const CARD_BODY_STYLE: React.CSSProperties = { margin: 0, fontSize: "12px", color: "var(--text-secondary)", lineHeight: BODY_LINE_HEIGHT };
const PILL_BASE_STYLE: React.CSSProperties = { padding: "2px 7px", borderRadius: "999px", fontSize: "10px", fontWeight: STUDIO_FONT_SEMIBOLD };

const MasterOptionCard: React.FC<{ variant: GeneratedVariant; isActive: boolean; isRecommended: boolean; onSelect: (id: string) => void }> = ({
  variant,
  isActive,
  isRecommended,
  onSelect,
}) => {
  const { t } = useTranslation();
  const badge = formatVariantDiversityBadge(variant);
  return (
    <button
      type="button" role="tab" aria-selected={isActive} data-testid={`tournament-option-tab-${variant.id}`} onClick={() => onSelect(variant.id)}
      style={{
        display: "flex", flexDirection: "column", alignItems: "flex-start", gap: "6px", padding: "10px",
        borderRadius: "var(--radius-sm)", cursor: "pointer", textAlign: "left", fontSize: "12px",
        border: isActive ? "1.5px solid var(--action-primary)" : "1px solid var(--border-default)",
        backgroundColor: isActive ? "var(--bg-selected)" : "var(--bg-surface)",
        color: isActive ? "var(--action-primary)" : "var(--text-primary)", fontWeight: isActive ? STUDIO_FONT_SEMIBOLD : STUDIO_FONT_MEDIUM,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", width: "100%", gap: "4px" }}>
        <span>{variant.displayLabel}</span>
        {isRecommended && (
          <span style={{ ...PILL_BASE_STYLE, backgroundColor: "var(--bg-card)", border: "1px solid var(--border-default)", color: "var(--action-primary)" }}>
            {t("tournament.recommendedPill", "Gợi ý hàng đầu")}
          </span>
        )}
      </div>
      <span
        data-testid={`tournament-diversity-badge-${variant.id}`} title={badge.tooltip}
        style={{ ...PILL_BASE_STYLE, backgroundColor: badge.passed ? "var(--state-success-soft, var(--bg-card))" : "var(--state-warning-soft, var(--bg-workspace))", color: badge.passed ? "var(--state-success)" : "var(--state-warning)" }}
      >
        {badge.label}
      </span>
    </button>
  );
};

const MasterOptionCardList: React.FC<{ variants: ReadonlyArray<GeneratedVariant>; selectedId: string; recommendedId: string; onSelect: (id: string) => void }> = ({
  variants,
  selectedId,
  recommendedId,
  onSelect,
}) => {
  const { t } = useTranslation();
  return (
    <div role="tablist" aria-label={t("tournament.variantTabsAria", "Danh sách phương án thiết kế")} style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "8px" }}>
      {variants.map((variant) => (
        <MasterOptionCard key={variant.id} variant={variant} isActive={variant.id === selectedId} isRecommended={variant.id === recommendedId} onSelect={onSelect} />
      ))}
    </div>
  );
};

const TournamentMasterPane: React.FC<{ state: TournamentHookState; onFocusSubChat: () => void }> = ({
  state,
  onFocusSubChat,
}) => {
  const { t } = useTranslation();
  const donorVariants = state.run ? state.run.variants.filter((v) => v.id !== state.selectedVariantId) : [];
  return (
    <div data-testid="tournament-master-controls" style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "12px", display: "flex", flexDirection: "column", gap: "12px" }}>
      {state.errorMessage && (
        <div role="alert" data-testid="tournament-error-banner" style={{ padding: "8px 10px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-surface)", border: "1px solid var(--state-danger)", color: "var(--state-danger)", fontSize: "11.5px" }}>
          {state.errorMessage}
        </div>
      )}
      {state.run && state.activeVariant ? (
        <>
          <MasterOptionCardList variants={state.run.variants} selectedId={state.selectedVariantId} recommendedId={state.run.rubricSummary.recommendedVariantId} onSelect={state.switchActiveVariant} />
          <div style={{ minWidth: 0, overflowX: "auto" }}>
            <CrossVariantBorrowForm activeVariant={state.activeVariant} donorVariants={donorVariants} isCommitting={state.isCommitting} onConfirm={(payload) => void state.confirmSelection(payload)} onFocusSubChat={onFocusSubChat} />
          </div>
        </>
      ) : (
        <div style={{ padding: "12px", borderRadius: "var(--radius-sm)", border: "1px dashed var(--border-default)", backgroundColor: "var(--bg-surface)", fontSize: "11.5px", color: "var(--text-secondary)", lineHeight: BODY_LINE_HEIGHT }}>
          {t("tournament.masterIdleHint", "Dùng nút khởi tạo nhanh theo bối cảnh ở khung phải, hoặc nhắn yêu cầu thiết kế trực tiếp ở Sub-Chat bên dưới để bắt đầu vòng so sánh.")}
        </div>
      )}
    </div>
  );
};

const CrystallizeHeaderTrigger: React.FC<{ enabled: boolean; isOpen: boolean; onToggle: () => void }> = ({ enabled, isOpen, onToggle }) => {
  const { t } = useTranslation();
  return (
    <button
      type="button" disabled={!enabled} aria-expanded={isOpen}
      data-ff-id="crystallize-action-button" data-testid="tournament-crystallize-trigger-btn"
      title={t("tournament.crystallizeTriggerTooltip", "Đóng gói phương án đang chọn thành mẫu tái sử dụng")}
      onClick={onToggle}
      style={{
        display: "inline-flex", alignItems: "center", gap: "5px", padding: "4px 9px", borderRadius: "var(--radius-sm)",
        border: isOpen ? "1px solid var(--action-primary)" : "1px solid var(--border-default)",
        backgroundColor: isOpen ? "var(--bg-selected)" : "var(--bg-workspace)",
        color: isOpen ? "var(--action-primary)" : "var(--text-primary)", fontSize: "11px", fontWeight: STUDIO_FONT_SEMIBOLD,
        opacity: enabled ? 1 : DISABLED_BTN_OPACITY, cursor: enabled ? "pointer" : "not-allowed", whiteSpace: "nowrap",
      }}
    >
      <BrandIcon name="layers" size={STUDIO_ICON_SM} strokeWidth={STUDIO_STROKE_MONO} color="currentColor" />
      <span>{t("tournament.crystallizeShortBtn", "Kết tinh mẫu")}</span>
    </button>
  );
};

const CrystallizePopoverPanel: React.FC<{
  runId: string;
  defaultName: string;
  isCrystallizing: boolean;
  onClose: () => void;
  onCrystallize: (kind: CrystallizePackageKind, name: string) => void;
}> = ({ runId, defaultName, isCrystallizing, onClose, onCrystallize }) => {
  const { t } = useTranslation();
  return (
    <div
      role="region" aria-label={t("tournament.crystallizePopoverAria", "Kết tinh mẫu tái sử dụng")} data-testid="tournament-crystallize-popover"
      style={{
        position: "absolute", top: "10px", right: "14px", zIndex: CRYSTALLIZE_POPOVER_Z, width: "min(460px, calc(100% - 28px))",
        display: "flex", flexDirection: "column", gap: "8px", padding: "12px", borderRadius: "var(--radius-md)",
        backgroundColor: "var(--bg-elevated)", border: "1px solid var(--border-default)", boxShadow: "var(--shadow-lg)",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
        <div style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
          <BrandIcon name="layers" size={STUDIO_ICON_SM} strokeWidth={STUDIO_STROKE_MONO} color="var(--action-primary)" />
          <span style={{ fontSize: "12px", fontWeight: STUDIO_FONT_SEMIBOLD, color: "var(--text-primary)" }}>
            {t("tournament.crystallizePopoverTitle", "Kết tinh phương án thành mẫu tái sử dụng")}
          </span>
        </div>
        <button
          type="button" aria-label={t("common.close", "Đóng")} data-testid="tournament-crystallize-popover-close" onClick={onClose}
          style={{ display: "inline-flex", alignItems: "center", justifyContent: "center", padding: "4px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)", color: "var(--text-secondary)", cursor: "pointer" }}
        >
          <BrandIcon name="x" size={STUDIO_ICON_SM} strokeWidth={STUDIO_STROKE_MONO} color="currentColor" />
        </button>
      </div>
      <CrystallizeActionButton key={runId} defaultName={defaultName} isCrystallizing={isCrystallizing} onCrystallize={onCrystallize} />
    </div>
  );
};

const RecentRunRow: React.FC<{ item: PersistedTournamentRun }> = ({ item }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const winnerLabel = resolvePersistedWinnerLabel(item);
  const countText = t("tournament.variantsCount", "{{count}} phương án", { count: formatNumber(item.variants.length) });
  const timestamp = formatTournamentRunTimestamp(item.createdAt);
  const statusText = winnerLabel
    ? t("tournament.recentRunWinner", "Đã chốt: {{winner}}", { winner: winnerLabel })
    : t("tournament.recentRunUncommitted", "Chưa chốt");
  const pillStyle: React.CSSProperties = {
    ...PILL_BASE_STYLE,
    backgroundColor: winnerLabel ? "var(--state-success-soft, var(--bg-selected))" : "var(--bg-subtle)",
    color: winnerLabel ? "var(--state-success)" : "var(--text-muted)",
  };
  return (
    <li data-testid={`tournament-recent-run-${item.id}`} style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", padding: "7px 10px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-card)", border: "1px solid var(--border-subtle)", fontSize: "11.5px" }}>
      <span style={{ color: "var(--text-secondary)" }}>{timestamp ? `${countText} · ${timestamp}` : countText}</span>
      <span style={pillStyle}>{statusText}</span>
    </li>
  );
};

const RecentRunsHistoryCard: React.FC<{ recentRuns: ReadonlyArray<PersistedTournamentRun>; isLoadingRuns: boolean }> = ({
  recentRuns,
  isLoadingRuns,
}) => {
  const { t } = useTranslation();
  const visibleRuns = recentRuns.slice(0, RECENT_RUNS_DISPLAY_LIMIT);
  let body: React.ReactNode;
  if (isLoadingRuns) body = <p style={{ margin: 0, fontSize: "12px", color: "var(--text-muted)" }}>{t("tournament.loadingRecentRuns", "Đang tải lịch sử phiên gần đây...")}</p>;
  else if (visibleRuns.length === 0) body = <p style={{ margin: 0, fontSize: "12px", color: "var(--text-muted)" }}>{t("tournament.noRecentRuns", "Chưa có phiên so sánh nào được lưu cho dự án này.")}</p>;
  else body = <ul style={{ listStyle: "none", margin: 0, padding: 0, display: "flex", flexDirection: "column", gap: "6px" }}>{visibleRuns.map((item) => <RecentRunRow key={item.id} item={item} />)}</ul>;
  return (
    <article data-testid="tournament-recent-runs-card" style={CARD_STYLE}>
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <BrandIcon name="refresh-cw" size={STUDIO_ICON_MD} strokeWidth={STUDIO_STROKE_MONO} color="var(--action-primary)" />
        <h3 style={CARD_TITLE_STYLE}>{t("tournament.recentRunsTitle", "Phiên đấu trường gần đây")}</h3>
      </div>
      {body}
    </article>
  );
};

const QuickStartContextCard: React.FC<{ contextualPrompt: string; activeTabTitle?: string; isLaunching: boolean; onQuickStart: () => void }> = ({
  contextualPrompt,
  activeTabTitle,
  isLaunching,
  onQuickStart,
}) => {
  const { t } = useTranslation();
  return (
    <article style={CARD_STYLE}>
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <BrandIcon name="sparkles" size={STUDIO_ICON_MD} strokeWidth={STUDIO_STROKE_MONO} color="var(--action-primary)" />
        <h3 style={CARD_TITLE_STYLE}>{t("tournament.quickStartTitle", "Khởi tạo từ bối cảnh hiện tại")}</h3>
      </div>
      <p style={CARD_BODY_STYLE}>
        {activeTabTitle
          ? t("tournament.quickStartFromTabDesc", "Đề xuất tự động từ thẻ đang mở ({{title}}):", { title: activeTabTitle })
          : t("tournament.quickStartFallbackDesc", "Khởi chạy ngay 4 phương án thiết kế khác biệt rõ ràng:")}
      </p>
      <div data-testid="tournament-contextual-prompt-preview" style={{ padding: "8px 10px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-card)", border: "1px solid var(--border-subtle)", fontSize: "12px", color: "var(--text-primary)" }}>
        {contextualPrompt}
      </div>
      <div>
        <button type="button" disabled={isLaunching} data-testid="tournament-quick-start-btn" onClick={onQuickStart} style={{ ...PRIMARY_BTN_STYLE, cursor: isLaunching ? "wait" : "pointer" }}>
          <BrandIcon name="layers" size={STUDIO_ICON_SM} strokeWidth={STUDIO_STROKE_MONO} color="currentColor" />
          <span>{isLaunching ? t("tournament.launchingBtn", "Đang tạo phương án...") : t("tournament.quickStartCta", "Tạo 4 phương án ngay")}</span>
        </button>
      </div>
    </article>
  );
};

const SubChatEntryCard: React.FC<{ onFocusSubChat: () => void }> = ({ onFocusSubChat }) => {
  const { t } = useTranslation();
  return (
    <article style={CARD_STYLE}>
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <BrandIcon name="message-square" size={STUDIO_ICON_MD} strokeWidth={STUDIO_STROKE_MONO} color="var(--action-primary)" />
        <h3 style={CARD_TITLE_STYLE}>{t("tournament.subChatCardTitle", "Hoặc nhắn yêu cầu trực tiếp ở Sub-Chat bên trái")}</h3>
      </div>
      <p style={CARD_BODY_STYLE}>
        {t("tournament.subChatCardDesc", "Trao đổi trực tiếp với Agent về phong cách, mật độ thông tin hoặc yêu cầu kết hợp chi tiết trước khi sinh phương án.")}
      </p>
      <div>
        <button type="button" data-testid="tournament-focus-subchat-btn" onClick={onFocusSubChat} style={SECONDARY_BTN_STYLE}>
          {t("tournament.openSubChatBtn", "Mở Sub-Chat bên trái")}
        </button>
      </div>
    </article>
  );
};

const TournamentSmartLauncher: React.FC<{
  contextualPrompt: string;
  activeTabTitle?: string;
  isLaunching: boolean;
  recentRuns: ReadonlyArray<PersistedTournamentRun>;
  isLoadingRuns: boolean;
  onQuickStart: () => void;
  onFocusSubChat: () => void;
}> = (props) => (
  <div data-testid="tournament-smart-launcher" style={{ display: "flex", flexDirection: "column", gap: "12px", maxWidth: "760px", width: "100%", margin: "0 auto" }}>
    <QuickStartContextCard contextualPrompt={props.contextualPrompt} activeTabTitle={props.activeTabTitle} isLaunching={props.isLaunching} onQuickStart={props.onQuickStart} />
    <RecentRunsHistoryCard recentRuns={props.recentRuns} isLoadingRuns={props.isLoadingRuns} />
    <SubChatEntryCard onFocusSubChat={props.onFocusSubChat} />
  </div>
);

const TournamentReceiptBanners: React.FC<{ state: TournamentHookState }> = ({ state }) => {
  const { t } = useTranslation();
  const findLabel = (id: string | null | undefined, fallback: string) =>
    id ? state.run?.variants.find((v) => v.id === id)?.displayLabel ?? fallback : null;
  const committedLabel = findLabel(state.lastCommit?.winningVariantId, t("tournament.selectedFallbackLabel", "Phương án đã chọn"));
  const borrowedLabel = findLabel(state.lastCommit?.borrowedFromVariantId, t("tournament.borrowedFallbackLabel", "phương án khác"));
  return (
    <>
      {state.lastCommit && committedLabel && (
        <div data-testid="tournament-commit-receipt" style={{ padding: "8px 12px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-selected)", color: "var(--text-primary)", fontSize: "12px" }}>
          {t("tournament.commitSuccess", "Đã chốt phương án thành công")}: <strong>{committedLabel}</strong>
          {borrowedLabel ? ` (${t("tournament.commitBorrowedPrefix", "kết hợp chi tiết từ")} ${borrowedLabel})` : ""}
        </div>
      )}
      {state.lastCrystallize && (
        <div data-testid="tournament-crystallize-receipt" style={{ padding: "8px 12px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", color: "var(--text-primary)", fontSize: "12px" }}>
          {t("tournament.crystallizeSuccess", "Đã kết tinh gói tái sử dụng")}: <strong>{state.lastCrystallize.name}</strong>
        </div>
      )}
    </>
  );
};

function useTournamentSubChat(projectId: string, state: TournamentHookState, activeTabTitle?: string) {
  const { t } = useTranslation();
  const [inputValue, setInputValue] = useState("");
  const [conversationMessages, setConversationMessages] = useState<StudioSubChatMessage[]>([]);
  const [attachmentChips, setAttachmentChips] = useState<string[]>([]);
  const [syncToMainChat, setSyncToMainChat] = useState(true);

  const messages = useMemo<StudioSubChatMessage[]>(
    () => [
      {
        id: "tournament-welcome-msg",
        role: "assistant",
        text: t(
          "tournament.subChatWelcome",
          'Chào bạn! Nhắn yêu cầu thiết kế, tinh chỉnh (VD: "Giữ thanh điều hướng trái của Option B nhưng làm gọn KPI như Option A") hoặc gõ /crystallize <tên mẫu> để đóng gói phương án.'
        ),
      },
      ...conversationMessages,
    ],
    [conversationMessages, t]
  );

  const handleAttachFiles = useCallback((names: string[]) => setAttachmentChips((prev) => Array.from(new Set([...prev, ...names]))), []);
  const handleRemoveChip = useCallback((chip: string) => setAttachmentChips((prev) => prev.filter((c) => c !== chip)), []);
  const toggleSyncToMainChat = useCallback(() => setSyncToMainChat((v) => !v), []);

  const handleSend = useCallback(async () => {
    const chipCtx = attachmentChips.length > 0 ? `[${attachmentChips.join(", ")}] ` : "";
    const fullPrompt = `${chipCtx}${inputValue.trim()}`.trim();
    if (!fullPrompt) return;
    const scopeLabel = state.activeVariant?.displayLabel ?? activeTabTitle ?? null;
    setInputValue("");
    setAttachmentChips([]);
    setConversationMessages((prev) => [...prev, { id: `tu-${Date.now()}`, role: "user", text: fullPrompt, scopeFfId: scopeLabel, syncedToMain: syncToMainChat }]);
    if (syncToMainChat) {
      dispatchTournamentSubChatSync({ projectId, prompt: fullPrompt, activeVariantLabel: state.activeVariant?.displayLabel ?? null, runId: state.run?.runId ?? null });
    }
    const reply = await executeTournamentSubChatInstruction(fullPrompt, state, activeTabTitle);
    setConversationMessages((prev) => [...prev, { id: `ta-${Date.now()}`, role: "assistant", text: reply, syncedToMain: syncToMainChat }]);
  }, [activeTabTitle, attachmentChips, inputValue, projectId, state, syncToMainChat]);

  return { inputValue, setInputValue, messages, attachmentChips, syncToMainChat, toggleSyncToMainChat, handleAttachFiles, handleRemoveChip, handleSend };
}

const TournamentDetailPane: React.FC<{
  state: TournamentHookState;
  contextualPrompt: string;
  activeTabTitle?: string;
  isCrystallizeOpen: boolean;
  defaultPackageName: string;
  onCloseCrystallize: () => void;
  onCrystallizeSubmit: (kind: CrystallizePackageKind, name: string) => void;
  onQuickStart: () => void;
  onFocusSubChat: () => void;
}> = (p) => (
  <div data-ff-id="design-tournament-arena" data-testid="design-tournament-arena" style={{ position: "relative", flex: 1, minHeight: 0, overflowY: "auto", padding: "14px", display: "flex", flexDirection: "column", gap: "12px", backgroundColor: "var(--bg-canvas)" }}>
    {p.isCrystallizeOpen && p.state.run && (
      <CrystallizePopoverPanel runId={p.state.run.runId} defaultName={p.defaultPackageName} isCrystallizing={p.state.isCrystallizing} onClose={p.onCloseCrystallize} onCrystallize={p.onCrystallizeSubmit} />
    )}
    {p.state.run && p.state.activeVariant && p.state.compareVariant ? (
      <>
        <RubricSummaryBar summary={p.state.run.rubricSummary} />
        <BlindWipeSliderBar leftVariant={p.state.activeVariant} rightVariant={p.state.compareVariant} viewportMode={p.state.viewportMode} onViewportModeChange={p.state.setViewportMode} allVariants={p.state.run.variants} onCompareVariantChange={p.state.setCompareVariantId} onSelectVariant={p.state.switchActiveVariant} />
        <TournamentReceiptBanners state={p.state} />
      </>
    ) : (
      <TournamentSmartLauncher contextualPrompt={p.contextualPrompt} activeTabTitle={p.activeTabTitle} isLaunching={p.state.isLaunching} recentRuns={p.state.recentRuns} isLoadingRuns={p.state.isLoadingRuns} onQuickStart={p.onQuickStart} onFocusSubChat={p.onFocusSubChat} />
    )}
  </div>
);

const SUBCHAT_TEXTAREA_SELECTOR = '[data-testid="studio-sub-chat-pane"] textarea, [data-testid="studio-subchat-panel"] textarea, [data-testid="studio-subchat-input"]';

function focusSubChatTextarea(): void {
  if (typeof document === "undefined") return;
  const target = document.querySelector<HTMLTextAreaElement>(SUBCHAT_TEXTAREA_SELECTOR);
  if (target) target.focus();
  else if (typeof window !== "undefined") window.requestAnimationFrame(() => document.querySelector<HTMLTextAreaElement>(SUBCHAT_TEXTAREA_SELECTOR)?.focus());
}

function useTournamentDrawerBindings(props: DesignTournamentStudioDrawerProps) {
  const { isOpen, projectId, activeTabTitle, activeTabKind, port, onCommittedHtml, onActiveRunCountChange } = props;
  const { locale } = useTranslation();
  const layout = useStudioDrawerLayout(TOURNAMENT_MASTER_WIDTH_PX);
  const contextualPrompt = useMemo(() => buildContextualTournamentPrompt(activeTabTitle, activeTabKind), [activeTabKind, activeTabTitle, locale]);
  const state = useDesignTournament({ projectId, initialPrompt: contextualPrompt, port, onCommittedHtml, loadRecentRunsOnMount: isOpen });
  const subChat = useTournamentSubChat(projectId, state, activeTabTitle);
  const prevContextualPromptRef = useRef(contextualPrompt);
  const [isCrystallizeOpen, setIsCrystallizeOpen] = useState(false);

  useEffect(() => {
    if (!state.run && (!state.prompt.trim() || state.prompt === prevContextualPromptRef.current)) state.setPrompt(contextualPrompt);
    prevContextualPromptRef.current = contextualPrompt;
  }, [contextualPrompt, state]);

  useEffect(() => { if (!state.run) setIsCrystallizeOpen(false); }, [state.run]);

  const totalRunCount = useMemo(() => {
    const ids = new Set<string>(state.recentRuns.map((r) => r.id));
    if (state.run?.runId) ids.add(state.run.runId);
    return ids.size;
  }, [state.recentRuns, state.run?.runId]);

  useEffect(() => { onActiveRunCountChange?.(totalRunCount); }, [onActiveRunCountChange, totalRunCount]);

  const handleQuickStart = useCallback(() => { state.setPrompt(contextualPrompt); void state.launchTournament(contextualPrompt); }, [contextualPrompt, state]);
  const handleFocusSubChat = useCallback(() => {
    layout.expandSubChatPanel();
    if (!state.run && !subChat.inputValue.trim()) subChat.setInputValue(contextualPrompt);
    focusSubChatTextarea();
  }, [contextualPrompt, layout, state.run, subChat]);
  const handleCrystallizeSubmit = useCallback(async (kind: CrystallizePackageKind, name: string) => {
    const pkg = await state.crystallizeWinner(kind, name);
    if (pkg) setIsCrystallizeOpen(false);
  }, [state]);

  return { layout, contextualPrompt, state, subChat, isCrystallizeOpen, setIsCrystallizeOpen, totalRunCount, handleQuickStart, handleFocusSubChat, handleCrystallizeSubmit };
}

export const DesignTournamentStudioDrawer: React.FC<DesignTournamentStudioDrawerProps> = (props) => {
  const { isOpen, onClose, activeTabTitle } = props;
  const titleId = useId();
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const ctrl = useTournamentDrawerBindings(props);
  const { state, subChat } = ctrl;
  const metricText = state.run
    ? t("tournament.headerMetricActive", "{{count}} phương án · {{recommended}}", {
        count: formatNumber(state.run.variants.length),
        recommended: state.run.rubricSummary.recommendedDisplayLabel,
      })
    : t("tournament.headerMetricRecent", "{{count}} phiên gần đây", {
        count: formatNumber(ctrl.totalRunCount),
      });
  const defaultPackageName = state.prompt.slice(0, MAX_PACKAGE_NAME_LEN) || t("tournament.defaultPackageName", "Mẫu giao diện mới");

  return (
    <StudioDrawerShell
      isOpen={isOpen} title={t("tournament.arenaTitle", "Đấu trường so sánh phương án thiết kế")} titleId={titleId}
      testId="design-tournament-studio-drawer" titleTestId="tournament-drawer-title" iconName="layers"
      layout={ctrl.layout} closeButtonTestId="tournament-drawer-close-btn" onClose={onClose}
      headerActions={<CrystallizeHeaderTrigger enabled={Boolean(state.run)} isOpen={ctrl.isCrystallizeOpen} onToggle={() => ctrl.setIsCrystallizeOpen((v) => !v)} />}
      mobileControlLabel={t("tournament.mobileControlTab", "Điều khiển & Chat")}
      mobileCanvasLabel={t("tournament.mobileCanvasTab", "So sánh Mù")}
      metricLine={<span data-testid="tournament-drawer-metric-line" style={{ fontSize: "11px", color: "var(--text-secondary)", fontVariantNumeric: "tabular-nums" }}>{metricText}</span>}
      masterContent={<TournamentMasterPane state={state} onFocusSubChat={ctrl.handleFocusSubChat} />}
      subChat={{
        selectedFfId: state.activeVariant?.displayLabel ?? activeTabTitle ?? null, pinCount: 0,
        inputValue: subChat.inputValue, messages: subChat.messages, attachmentChips: subChat.attachmentChips,
        onAttachFiles: subChat.handleAttachFiles, onRemoveAttachmentChip: subChat.handleRemoveChip,
        placeholder: t("tournament.subChatPlaceholder", "VD: Giữ thanh điều hướng trái của Option B nhưng làm gọn KPI như Option A..."),
        syncToMainChatEnabled: subChat.syncToMainChat, onToggleSyncToMainChat: subChat.toggleSyncToMainChat,
        onInputChange: subChat.setInputValue, onSend: () => void subChat.handleSend(),
      }}
      detailContent={
        <TournamentDetailPane
          state={state} contextualPrompt={ctrl.contextualPrompt} activeTabTitle={activeTabTitle}
          isCrystallizeOpen={ctrl.isCrystallizeOpen} defaultPackageName={defaultPackageName}
          onCloseCrystallize={() => ctrl.setIsCrystallizeOpen(false)}
          onCrystallizeSubmit={(kind, name) => void ctrl.handleCrystallizeSubmit(kind, name)}
          onQuickStart={ctrl.handleQuickStart} onFocusSubChat={ctrl.handleFocusSubChat}
        />
      }
    />
  );
};
