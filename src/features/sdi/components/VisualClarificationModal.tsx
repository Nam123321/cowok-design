// @story 05.1, 05.4
// @implements Zero-Tailwind Semantic Pure CSS Visual Clarification Modal + Cognitive Phase 2 Taste, Theme Modes & Popular FE Components
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type { ClarificationVariant } from "../../../lib/validations/sdi";
import type { LiveDirectionOption } from "../../../lib/validations/sketch-polish";
import type { PopularComponentId, ThemeModeOption } from "../../sketch-polish/useSketchPolishController";
import { ClarificationCard } from "./ClarificationCard";

const MODAL_Z_INDEX = 50;
const DISABLED_OPACITY = 0.5;
const ICON_HEADER_SIZE = 16;
const ICON_CHECK_SIZE = 12;
const STROKE_WIDTH_MONO = 1.85;
const FONT_WEIGHT_BOLD = 700;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_MEDIUM = 500;
const LINE_HEIGHT_TIGHT = 1.35;
const LINE_HEIGHT_CARD = 1.45;
const LINE_HEIGHT_BODY = 1.5;

const THEME_MODE_ITEMS: readonly { id: ThemeModeOption; key: string; fallback: string }[] = [
  { id: "light", key: "sketchPolish.modeLight", fallback: "Sáng (Light)" },
  { id: "warm", key: "sketchPolish.modeWarm", fallback: "Ấm (Warm)" },
  { id: "dark", key: "sketchPolish.modeDark", fallback: "Tối (Dark)" },
  { id: "dim", key: "sketchPolish.modeDim", fallback: "Dịu (Dim)" },
];

const POPULAR_COMPONENT_ITEMS: readonly { id: PopularComponentId; key: string; fallback: string }[] = [
  { id: "toast", key: "sketchPolish.compToast", fallback: "Thông báo (Toast)" },
  { id: "confirm-dialog", key: "sketchPolish.compConfirmDialog", fallback: "Hộp thoại xác nhận" },
  { id: "tooltip", key: "sketchPolish.compTooltip", fallback: "Chú giải (Tooltip)" },
  { id: "dropdown", key: "sketchPolish.compDropdown", fallback: "Menu thả xuống" },
  { id: "empty-error", key: "sketchPolish.compEmptyError", fallback: "Trạng thái rỗng / lỗi" },
];

const MODAL_OVERLAY_STYLE: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  zIndex: MODAL_Z_INDEX,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  backgroundColor: "var(--bg-overlay)",
  padding: "16px",
};

const MODAL_CARD_STYLE: React.CSSProperties = {
  width: "100%",
  maxWidth: "700px",
  maxHeight: "90vh",
  overflowY: "auto",
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-lg)",
  padding: "20px",
  boxShadow: "var(--shadow-lg)",
  display: "flex",
  flexDirection: "column",
  gap: "14px",
};

const INPUT_BASE_STYLE: React.CSSProperties = {
  flex: 1,
  height: "34px",
  padding: "0 12px",
  fontSize: "12.5px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-elevated)",
  color: "var(--text-primary)",
};

function buildPillStyle(isActive: boolean): React.CSSProperties {
  return {
    display: "inline-flex",
    alignItems: "center",
    gap: "5px",
    height: "26px",
    padding: "0 10px",
    borderRadius: "var(--radius-pill)",
    border: isActive ? "1px solid var(--human-border, var(--human-accent))" : "1px solid var(--border-default)",
    backgroundColor: isActive ? "var(--human-soft)" : "var(--bg-elevated)",
    color: isActive ? "var(--human-accent)" : "var(--text-secondary)",
    fontSize: "11.5px",
    fontWeight: isActive ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM,
    cursor: "pointer",
  };
}

interface VisualClarificationModalProps {
  isOpen: boolean;
  variants: ClarificationVariant[];
  onConfirm: (selectedVariantId: string) => void;
  onClose: () => void;
  liveDirections?: LiveDirectionOption[];
  activeDirectionId?: string;
  generationEpoch?: number;
  goalPrompt?: string;
  clarifyLocale?: "vi" | "en";
  referenceApps?: string;
  themeModes?: ThemeModeOption[];
  selectedPopularComponents?: PopularComponentId[];
  onChangeGoalPrompt?: (val: string) => void;
  onChangeClarifyLocale?: (loc: "vi" | "en") => void;
  onChangeReferenceApps?: (val: string) => void;
  onToggleThemeMode?: (mode: ThemeModeOption) => void;
  onTogglePopularComponent?: (compId: PopularComponentId) => void;
  onSwitchLiveDirection?: (directionId: string) => void;
}

const ModalHeader: React.FC<{ isLiveDirectionMode: boolean }> = ({ isLiveDirectionMode }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
      <h2 style={{ margin: 0, fontSize: "16px", fontWeight: FONT_WEIGHT_BOLD, color: "var(--text-primary)", display: "flex", alignItems: "center", gap: "8px", lineHeight: LINE_HEIGHT_TIGHT }}>
        <BrandIcon name="sparkles" size={ICON_HEADER_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="var(--human-accent)" />
        <span>
          {isLiveDirectionMode
            ? t("sketchPolish.clarifyModalTitle", "Bước 2 · Chọn diện mạo & thành phần giao diện")
            : t("sdi.clarificationTitle", "Chọn hướng thiết kế phù hợp")}
        </span>
      </h2>
      <p style={{ margin: 0, fontSize: "12.5px", color: "var(--text-secondary)", lineHeight: LINE_HEIGHT_BODY }}>
        {isLiveDirectionMode
          ? t("sketchPolish.clarifyModalDesc", "Chia sẻ phong cách bạn yêu thích, chế độ màu mong muốn và các thành phần cần có trước khi chuyển bản phác thảo sang giao diện chính thức.")
          : t("sdi.clarificationSimpleDesc", "Chọn một phương án gần nhất với ý tưởng của bạn để tiếp tục.")}
      </p>
    </div>
  );
};

const ModalFooter: React.FC<{
  selectedId: string | null;
  isLiveDirectionMode: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}> = ({ selectedId, isLiveDirectionMode, onCancel, onConfirm }) => {
  const { t } = useTranslation();
  const isDisabled = !selectedId;
  return (
    <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px", paddingTop: "12px", borderTop: "1px solid var(--border-default)" }}>
      <button type="button" onClick={onCancel} style={{ padding: "7px 14px", fontSize: "12.5px", fontWeight: FONT_WEIGHT_MEDIUM, borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "transparent", color: "var(--text-secondary)", cursor: "pointer" }}>
        {t("common.cancel", "Hủy bỏ")}
      </button>
      <button type="button" data-testid="clarify-confirm-btn" disabled={isDisabled} onClick={onConfirm} style={{ padding: "7px 16px", fontSize: "12.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, borderRadius: "var(--radius-xs)", border: "1px solid var(--human-border, var(--human-accent))", backgroundColor: "var(--human-soft)", color: "var(--human-accent)", cursor: isDisabled ? "not-allowed" : "pointer", opacity: isDisabled ? DISABLED_OPACITY : 1, display: "inline-flex", alignItems: "center", gap: "6px" }}>
        <BrandIcon name="check" size={ICON_CHECK_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="currentColor" />
        <span>
          {isLiveDirectionMode
            ? t("sketchPolish.confirmApplyStyleBtn", "Áp dụng diện mạo & Hoàn thiện")
            : t("sdi.confirmContinueBtn", "Áp dụng phong cách này")}
        </span>
      </button>
    </div>
  );
};

const GoalPromptRow: React.FC<{
  goalPrompt?: string;
  clarifyLocale?: "vi" | "en";
  onChangeGoalPrompt?: (val: string) => void;
  onChangeClarifyLocale?: (loc: "vi" | "en") => void;
}> = ({ goalPrompt, clarifyLocale, onChangeGoalPrompt, onChangeClarifyLocale }) => {
  const { t } = useTranslation();
  if (!onChangeGoalPrompt) return null;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "5px" }}>
      <label htmlFor="clarify-goal-input" style={{ fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
        {t("sketchPolish.goalPromptLabel", "Mục tiêu màn hình này")}
      </label>
      <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
        <input id="clarify-goal-input" type="text" value={goalPrompt ?? ""} onChange={(e) => onChangeGoalPrompt(e.target.value)} placeholder={t("sketchPolish.goalPromptPlaceholder", "VD: Trang thanh toán đơn giản, rõ ràng cho khách hàng...")} data-testid="clarify-goal-input" style={INPUT_BASE_STYLE} />
        {onChangeClarifyLocale && (
          <select value={clarifyLocale ?? "vi"} onChange={(e) => onChangeClarifyLocale(e.target.value === "en" ? "en" : "vi")} data-testid="clarify-locale-select" aria-label={t("sketchPolish.contentLanguageLabel", "Ngôn ngữ hiển thị")} style={{ height: "34px", padding: "0 10px", fontSize: "12.5px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-elevated)", color: "var(--text-primary)" }}>
            <option value="vi">Tiếng Việt</option>
            <option value="en">English</option>
          </select>
        )}
      </div>
    </div>
  );
};

const ReferenceAppsRow: React.FC<{
  referenceApps?: string;
  onChangeReferenceApps?: (val: string) => void;
}> = ({ referenceApps, onChangeReferenceApps }) => {
  const { t } = useTranslation();
  if (!onChangeReferenceApps) return null;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "5px" }}>
      <label htmlFor="clarify-reference-apps-input" style={{ fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
        {t("sketchPolish.referenceAppsLabel", "Ứng dụng / Website bạn thích phong cách (tuỳ chọn)")}
      </label>
      <input id="clarify-reference-apps-input" type="text" value={referenceApps ?? ""} onChange={(e) => onChangeReferenceApps(e.target.value)} placeholder={t("sketchPolish.referenceAppsPlaceholder", "VD: Linear, Stripe, Notion, Airbnb...")} data-testid="clarify-reference-apps-input" style={INPUT_BASE_STYLE} />
    </div>
  );
};

const GoalAndReferenceInputs: React.FC<{
  goalPrompt?: string;
  clarifyLocale?: "vi" | "en";
  referenceApps?: string;
  onChangeGoalPrompt?: (val: string) => void;
  onChangeClarifyLocale?: (loc: "vi" | "en") => void;
  onChangeReferenceApps?: (val: string) => void;
}> = ({ goalPrompt, clarifyLocale, referenceApps, onChangeGoalPrompt, onChangeClarifyLocale, onChangeReferenceApps }) => {
  if (!onChangeGoalPrompt && !onChangeReferenceApps) return null;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
      <GoalPromptRow goalPrompt={goalPrompt} clarifyLocale={clarifyLocale} onChangeGoalPrompt={onChangeGoalPrompt} onChangeClarifyLocale={onChangeClarifyLocale} />
      <ReferenceAppsRow referenceApps={referenceApps} onChangeReferenceApps={onChangeReferenceApps} />
    </div>
  );
};

const ThemeModesSelector: React.FC<{
  themeModes?: ThemeModeOption[];
  onToggleThemeMode?: (mode: ThemeModeOption) => void;
}> = ({ themeModes, onToggleThemeMode }) => {
  const { t } = useTranslation();
  if (!themeModes || !onToggleThemeMode) return null;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
      <span style={{ fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
        {t("sketchPolish.themeModesLabel", "Chế độ màu sắc của dự án (chọn 1 hoặc nhiều)")}
      </span>
      <div style={{ display: "flex", flexWrap: "wrap", gap: "6px" }} data-testid="clarify-theme-modes">
        {THEME_MODE_ITEMS.map((item) => {
          const active = themeModes.includes(item.id);
          return (
            <button
              key={item.id}
              type="button"
              aria-pressed={active}
              data-testid={`theme-mode-pill-${item.id}`}
              style={buildPillStyle(active)}
              onClick={() => onToggleThemeMode(item.id)}
            >
              {active && <BrandIcon name="check" size={ICON_CHECK_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="currentColor" />}
              <span>{t(item.key, item.fallback)}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
};

const PopularComponentsChecklist: React.FC<{
  selected?: PopularComponentId[];
  onToggle?: (compId: PopularComponentId) => void;
}> = ({ selected, onToggle }) => {
  const { t } = useTranslation();
  if (!selected || !onToggle) return null;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
      <span style={{ fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
        {t("sketchPolish.popularComponentsLabel", "Thành phần phổ biến thường dùng (chọn phần dự án bạn cần)")}
      </span>
      <div style={{ display: "flex", flexWrap: "wrap", gap: "6px" }} data-testid="popular-components-checklist">
        {POPULAR_COMPONENT_ITEMS.map((item) => {
          const active = selected.includes(item.id);
          return (
            <button
              key={item.id}
              type="button"
              aria-pressed={active}
              data-testid={`popular-comp-${item.id}`}
              style={buildPillStyle(active)}
              onClick={() => onToggle(item.id)}
            >
              {active && <BrandIcon name="check" size={ICON_CHECK_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="currentColor" />}
              <span>{t(item.key, item.fallback)}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
};

function resolveLocalizedDirectionCopy(
  dir: LiveDirectionOption,
  t: (key: string, fallback: string) => string
): { label: string; summary: string; rationale: string; icon: "file-text" | "brand-agent" } {
  if (dir.id === "dir-editorial-sketch") {
    return {
      label: t("sketchPolish.dirSketchTitle", "Ấm áp & Tối giản"),
      summary: t("sketchPolish.dirSketchDesc", "Nền giấy sáng dịu mắt, nhấn mạnh cấu trúc nội dung và khoảng thở rộng rãi cho trải nghiệm đọc tập trung."),
      rationale: t("sketchPolish.dirSketchRationale", "Cảm hứng: Notion & Editorial Workspace"),
      icon: "file-text",
    };
  }
  if (dir.id === "dir-polish-workbench") {
    return {
      label: t("sketchPolish.dirPolishTitle", "Hiện đại & Sắc nét"),
      summary: t("sketchPolish.dirPolishDesc", "Bố cục thẻ số liệu rõ ràng, tương phản cao chuẩn dễ đọc, sẵn sàng trình bày và bàn giao phát triển."),
      rationale: t("sketchPolish.dirPolishRationale", "Cảm hứng: Linear & Stripe Dashboard"),
      icon: "brand-agent",
    };
  }
  return { label: dir.label, summary: dir.summary, rationale: dir.skillSlug, icon: "brand-agent" };
}

const DirectionPillCard: React.FC<{
  dir: LiveDirectionOption;
  isActive: boolean;
  onSelect: (directionId: string) => void;
}> = ({ dir, isActive, onSelect }) => {
  const { t } = useTranslation();
  const copy = resolveLocalizedDirectionCopy(dir, t);
  return (
    <button
      type="button"
      aria-pressed={isActive}
      data-testid={`live-direction-pill-${dir.id}`}
      style={{
        display: "flex", flexDirection: "column", alignItems: "flex-start", gap: "6px",
        padding: "13px", borderRadius: "var(--radius-sm)", textAlign: "left", cursor: "pointer",
        border: isActive ? "1.5px solid var(--human-border, var(--human-accent))" : "1px solid var(--border-default)",
        backgroundColor: isActive ? "var(--human-soft)" : "var(--bg-elevated)", color: "var(--text-primary)",
        lineHeight: LINE_HEIGHT_CARD,
      }}
      onClick={() => onSelect(dir.id)}
    >
      <span style={{ display: "flex", alignItems: "center", justifyContent: "space-between", width: "100%", gap: "8px" }}>
        <strong style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "13px" }}>
          <span aria-hidden="true" style={{ width: "10px", height: "10px", borderRadius: "50%", backgroundColor: dir.accentHex, border: "1px solid var(--border-strong)", flexShrink: 0 }} />
          <span>{copy.label}</span>
        </strong>
        {isActive && (
          <span style={{ fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--human-accent)" }}>
            {t("sketchPolish.selectedBadge", "Đang chọn")}
          </span>
        )}
      </span>
      <span style={{ fontSize: "11px", color: "var(--human-accent)", fontWeight: FONT_WEIGHT_MEDIUM }}>{copy.rationale}</span>
      <span style={{ fontSize: "12px", color: "var(--text-secondary)", lineHeight: LINE_HEIGHT_BODY }}>{copy.summary}</span>
    </button>
  );
};

const LiveDirectionSwitcherSection: React.FC<{
  directions: LiveDirectionOption[];
  activeDirectionId?: string;
  generationEpoch?: number;
  goalPrompt?: string;
  clarifyLocale?: "vi" | "en";
  referenceApps?: string;
  themeModes?: ThemeModeOption[];
  selectedPopularComponents?: PopularComponentId[];
  onChangeGoalPrompt?: (val: string) => void;
  onChangeClarifyLocale?: (loc: "vi" | "en") => void;
  onChangeReferenceApps?: (val: string) => void;
  onToggleThemeMode?: (mode: ThemeModeOption) => void;
  onTogglePopularComponent?: (compId: PopularComponentId) => void;
  onSwitchLiveDirection?: (directionId: string) => void;
}> = (props) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
      <GoalAndReferenceInputs
        goalPrompt={props.goalPrompt}
        clarifyLocale={props.clarifyLocale}
        referenceApps={props.referenceApps}
        onChangeGoalPrompt={props.onChangeGoalPrompt}
        onChangeClarifyLocale={props.onChangeClarifyLocale}
        onChangeReferenceApps={props.onChangeReferenceApps}
      />
      <ThemeModesSelector themeModes={props.themeModes} onToggleThemeMode={props.onToggleThemeMode} />
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: "10px" }} data-testid="live-direction-pills">
        {props.directions.map((dir) => (
          <DirectionPillCard
            key={dir.id}
            dir={dir}
            isActive={props.activeDirectionId === dir.id}
            onSelect={(id) => props.onSwitchLiveDirection?.(id)}
          />
        ))}
      </div>
      <PopularComponentsChecklist selected={props.selectedPopularComponents} onToggle={props.onTogglePopularComponent} />
      {typeof props.generationEpoch === "number" && (
        <span data-testid="clarify-generation-epoch" style={{ fontSize: "11px", color: "var(--text-muted)" }}>
          {`${t("sketchPolish.generationEpochLabel", "Lần thử")}: #${formatNumber(props.generationEpoch)}`}
        </span>
      )}
    </div>
  );
};

const VariantGridSection: React.FC<{
  variants: ClarificationVariant[];
  selectedId: string | null;
  onSelect: (id: string) => void;
}> = ({ variants, selectedId, onSelect }) => {
  if (variants.length === 0) return null;
  return (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))", gap: "10px" }}>
      {variants.map((v) => (
        <ClarificationCard key={v.id} variant={v} isSelected={selectedId === v.id} onSelect={onSelect} />
      ))}
    </div>
  );
};

export const VisualClarificationModal: React.FC<VisualClarificationModalProps> = (props) => {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  if (!props.isOpen) return null;
  const effectiveSelected = selectedId ?? props.activeDirectionId ?? null;
  const isLiveDirectionMode = Boolean(props.liveDirections && props.liveDirections.length > 0);
  const handleSelectDirection = (dirId: string) => {
    setSelectedId(dirId);
    props.onSwitchLiveDirection?.(dirId);
  };
  return (
    <div role="dialog" aria-modal="true" data-ff-id="visual-clarification-modal" data-testid="visual-clarification-modal" style={MODAL_OVERLAY_STYLE}>
      <div style={MODAL_CARD_STYLE}>
        <ModalHeader isLiveDirectionMode={isLiveDirectionMode} />
        {props.liveDirections && props.liveDirections.length > 0 && (
          <LiveDirectionSwitcherSection
            directions={props.liveDirections}
            activeDirectionId={effectiveSelected ?? props.activeDirectionId}
            generationEpoch={props.generationEpoch}
            goalPrompt={props.goalPrompt}
            clarifyLocale={props.clarifyLocale}
            referenceApps={props.referenceApps}
            themeModes={props.themeModes}
            selectedPopularComponents={props.selectedPopularComponents}
            onChangeGoalPrompt={props.onChangeGoalPrompt}
            onChangeClarifyLocale={props.onChangeClarifyLocale}
            onChangeReferenceApps={props.onChangeReferenceApps}
            onToggleThemeMode={props.onToggleThemeMode}
            onTogglePopularComponent={props.onTogglePopularComponent}
            onSwitchLiveDirection={handleSelectDirection}
          />
        )}
        <VariantGridSection variants={props.variants} selectedId={selectedId} onSelect={setSelectedId} />
        <ModalFooter
          selectedId={effectiveSelected}
          isLiveDirectionMode={isLiveDirectionMode}
          onCancel={() => { setSelectedId(null); props.onClose(); }}
          onConfirm={() => { if (!effectiveSelected) return; setSelectedId(null); props.onConfirm(effectiveSelected); }}
        />
      </div>
    </div>
  );
};
