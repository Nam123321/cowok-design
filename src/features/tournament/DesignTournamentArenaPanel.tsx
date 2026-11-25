// @story 10.3
// @implements AC-01, AC-02, AC-03, AC-04 (CMP-07 DesignTournamentArena Panel with Anonymized Variants, Blind Wipe, RubricShield Summary, Cross-Variant Borrow & /crystallize)
import React from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";
import type {
  BlindRubricSummary,
  GeneratedVariant,
  SelectTournamentVariantData,
  StartTournamentData,
} from "../../lib/validations/tournament";
import { BlindWipeSliderBar } from "./BlindWipeSliderBar";
import { CrossVariantBorrowForm } from "./CrossVariantBorrowForm";
import { CrystallizeActionButton } from "./CrystallizeActionButton";
import {
  type DesignTournamentPort,
  formatVariantDiversityBadge,
  useDesignTournament,
} from "./useDesignTournament";

const FONT_MEDIUM = 500;
const FONT_SEMIBOLD = 600;
const HEADER_ICON_SIZE = 16;
const MAX_PACKAGE_NAME_LEN = 48;

export interface DesignTournamentArenaPanelProps {
  projectId: string;
  initialPrompt?: string;
  initialRun?: StartTournamentData | null;
  port?: DesignTournamentPort;
  onCommittedHtml?: (mergedHtml: string, result: SelectTournamentVariantData) => void;
  onFocusSubChat?: () => void;
}

const PANEL_STYLE: React.CSSProperties = {
  display: "flex", flexDirection: "column", gap: "14px", padding: "16px",
  backgroundColor: "var(--bg-canvas)", border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-lg)", color: "var(--text-primary)",
};

const VariantOptionButton: React.FC<{
  variant: GeneratedVariant;
  isActive: boolean;
  isRec: boolean;
  onSelect: (id: string) => void;
}> = ({ variant, isActive, isRec, onSelect }) => {
  const { t } = useTranslation();
  const badge = formatVariantDiversityBadge(variant);
  return (
    <button
      type="button" role="tab" aria-selected={isActive} data-testid={`tournament-option-tab-${variant.id}`}
      onClick={() => onSelect(variant.id)}
      style={{
        display: "inline-flex", alignItems: "center", gap: "8px", padding: "8px 12px", borderRadius: "var(--radius-sm)",
        border: isActive ? "1.5px solid var(--action-primary)" : "1px solid var(--border-default)",
        backgroundColor: isActive ? "var(--bg-selected)" : "var(--bg-surface)",
        color: isActive ? "var(--action-primary)" : "var(--text-primary)",
        fontSize: "12.5px", fontWeight: isActive ? FONT_SEMIBOLD : FONT_MEDIUM, cursor: "pointer",
      }}
    >
      <span>{variant.displayLabel}</span>
      {isRec && (
        <span style={{ padding: "2px 6px", borderRadius: "999px", backgroundColor: "var(--bg-card)", border: "1px solid var(--border-default)", fontSize: "10.5px", color: "var(--action-primary)" }}>
          {t("tournament.recommendedPill", "Gợi ý hàng đầu")}
        </span>
      )}
      <span
        data-testid={`tournament-diversity-badge-${variant.id}`} title={badge.tooltip}
        style={{
          padding: "2px 6px", borderRadius: "999px", fontSize: "10.5px",
          backgroundColor: badge.passed ? "var(--state-success-soft, var(--bg-card))" : "var(--state-warning-soft, var(--bg-workspace))",
          color: badge.passed ? "var(--state-success)" : "var(--state-warning)",
        }}
      >
        {badge.label}
      </span>
    </button>
  );
};

export const VariantOptionTabs: React.FC<{
  variants: ReadonlyArray<GeneratedVariant>;
  selectedId: string;
  recommendedId: string;
  onSelect: (id: string) => void;
}> = ({ variants, selectedId, recommendedId, onSelect }) => {
  const { t } = useTranslation();
  return (
    <div role="tablist" aria-label={t("tournament.variantTabsAria", "Danh sách phương án thiết kế")} style={{ display: "flex", flexWrap: "wrap", gap: "8px" }}>
      {variants.map((variant) => (
        <VariantOptionButton key={variant.id} variant={variant} isActive={variant.id === selectedId} isRec={variant.id === recommendedId} onSelect={onSelect} />
      ))}
    </div>
  );
};

export const RubricSummaryBar: React.FC<{ summary: BlindRubricSummary }> = ({ summary }) => {
  const { t } = useTranslation();
  return (
    <div
      data-testid="tournament-rubric-summary"
      style={{
        display: "flex", alignItems: "center", justifyContent: "space-between", flexWrap: "wrap", gap: "8px",
        padding: "10px 12px", borderRadius: "var(--radius-sm)", backgroundColor: "var(--bg-surface)",
        border: "1px solid var(--border-default)", fontSize: "12px",
      }}
    >
      <span style={{ color: "var(--text-primary)", fontWeight: FONT_MEDIUM }}>
        {t("tournament.blindJudgeSummary", "Đánh giá mù hai chiều gợi ý")}:{" "}
        <strong>{summary.recommendedDisplayLabel}</strong>
      </span>
      <span style={{ color: "var(--text-muted)", fontSize: "11.5px" }}>
        {summary.allPairsSwapConsistent
          ? t("tournament.swapConsistent", "Kết quả nhất quán khi đảo vị trí so sánh")
          : t("tournament.swapTieBreak", "Đã áp dụng tiêu chí kỹ thuật để phân định hòa")}
      </span>
    </div>
  );
};

const ArenaHeaderBar: React.FC<{
  prompt: string;
  isLaunching: boolean;
  onPromptChange: (val: string) => void;
  onLaunch: () => void;
}> = ({ prompt, isLaunching, onPromptChange, onLaunch }) => {
  const { t } = useTranslation();
  return (
    <header style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "10px", flexWrap: "wrap" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <BrandIcon name="layers" size={HEADER_ICON_SIZE} color="var(--action-primary)" />
        <div>
          <h2 style={{ margin: 0, fontSize: "15px", fontWeight: FONT_SEMIBOLD, color: "var(--text-primary)" }}>
            {t("tournament.arenaTitle", "Đấu trường so sánh phương án thiết kế")}
          </h2>
          <p style={{ margin: "2px 0 0", fontSize: "11.5px", color: "var(--text-muted)" }}>
            {t("tournament.arenaSubtitle", "Khám phá nhiều bố cục khác biệt rõ ràng và kết hợp chi tiết tốt nhất")}
          </p>
        </div>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: "8px", flexWrap: "wrap", maxWidth: "100%", flex: "1 1 260px", justifyContent: "flex-end" }}>
        <input
          type="text" data-testid="tournament-prompt-input" aria-label={t("tournament.promptAria", "Mô tả ý tưởng màn hình")}
          value={prompt} onChange={(e) => onPromptChange(e.target.value)}
          placeholder={t("tournament.promptPlaceholder", "Mô tả màn hình muốn so sánh phương án...")}
          style={{ flex: "1 1 180px", minWidth: 0, maxWidth: "100%", padding: "7px 10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-card)", color: "var(--text-primary)", fontSize: "12.5px", boxSizing: "border-box" }}
        />
        <button
          type="button" disabled={isLaunching || !prompt.trim()} data-testid="tournament-launch-btn" onClick={onLaunch}
          style={{ padding: "8px 14px", borderRadius: "var(--radius-xs)", border: "none", backgroundColor: "var(--action-primary)", color: "var(--text-on-action)", fontSize: "12.5px", fontWeight: FONT_SEMIBOLD, cursor: isLaunching ? "wait" : "pointer" }}
        >
          {isLaunching ? t("tournament.launchingBtn", "Đang tạo phương án...") : t("tournament.launchBtn", "Tạo phương án so sánh")}
        </button>
      </div>
    </header>
  );
};

const ArenaReceipts: React.FC<{
  run: StartTournamentData | null;
  lastCommit: SelectTournamentVariantData | null;
  crystallizedName: string | null;
}> = ({ run, lastCommit, crystallizedName }) => {
  const { t } = useTranslation();
  const findLabel = (id: string | null | undefined, fallback: string) =>
    id ? run?.variants.find((v) => v.id === id)?.displayLabel ?? fallback : null;
  const committedLabel = findLabel(lastCommit?.winningVariantId, t("tournament.selectedFallbackLabel", "Phương án đã chọn"));
  const borrowedLabel = findLabel(lastCommit?.borrowedFromVariantId, t("tournament.borrowedFallbackLabel", "phương án khác"));
  return (
    <>
      {lastCommit && committedLabel && (
        <div data-testid="tournament-commit-receipt" style={{ padding: "8px 12px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-selected)", color: "var(--text-primary)", fontSize: "12px" }}>
          {t("tournament.commitSuccess", "Đã chốt phương án thành công")}: <strong>{committedLabel}</strong>
          {borrowedLabel ? ` (${t("tournament.commitBorrowedPrefix", "kết hợp chi tiết từ")} ${borrowedLabel})` : ""}
        </div>
      )}
      {crystallizedName && (
        <div data-testid="tournament-crystallize-receipt" style={{ padding: "8px 12px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", color: "var(--text-primary)", fontSize: "12px" }}>
          {t("tournament.crystallizeSuccess", "Đã kết tinh gói tái sử dụng")}: <strong>{crystallizedName}</strong>
        </div>
      )}
    </>
  );
};

export const DesignTournamentArenaPanel: React.FC<DesignTournamentArenaPanelProps> = (props) => {
  const { t } = useTranslation();
  const state = useDesignTournament(props);
  const donorVariants = state.run ? state.run.variants.filter((v) => v.id !== state.selectedVariantId) : [];

  return (
    <div data-ff-id="design-tournament-arena" data-testid="design-tournament-arena" style={PANEL_STYLE}>
      <ArenaHeaderBar prompt={state.prompt} isLaunching={state.isLaunching} onPromptChange={state.setPrompt} onLaunch={() => void state.launchTournament()} />
      {state.errorMessage && (
        <div role="alert" data-testid="tournament-error-banner" style={{ padding: "8px 12px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-surface)", border: "1px solid var(--state-danger)", color: "var(--state-danger)", fontSize: "12px" }}>
          {state.errorMessage}
        </div>
      )}
      {state.run && state.activeVariant && state.compareVariant && (
        <>
          <VariantOptionTabs variants={state.run.variants} selectedId={state.selectedVariantId} recommendedId={state.run.rubricSummary.recommendedVariantId} onSelect={state.switchActiveVariant} />
          <RubricSummaryBar summary={state.run.rubricSummary} />
          <BlindWipeSliderBar leftVariant={state.activeVariant} rightVariant={state.compareVariant} viewportMode={state.viewportMode} onViewportModeChange={state.setViewportMode} allVariants={state.run.variants} onCompareVariantChange={state.setCompareVariantId} onSelectVariant={state.switchActiveVariant} />
          <CrossVariantBorrowForm activeVariant={state.activeVariant} donorVariants={donorVariants} isCommitting={state.isCommitting} onConfirm={(payload) => void state.confirmSelection(payload)} onFocusSubChat={props.onFocusSubChat} />
          <CrystallizeActionButton key={state.run.runId} defaultName={state.prompt.slice(0, MAX_PACKAGE_NAME_LEN) || t("tournament.defaultPackageName", "Mẫu giao diện mới")} isCrystallizing={state.isCrystallizing} onCrystallize={(kind, name) => void state.crystallizeWinner(kind, name)} />
        </>
      )}
      <ArenaReceipts run={state.run} lastCommit={state.lastCommit} crystallizedName={state.lastCrystallize?.name ?? null} />
    </div>
  );
};
