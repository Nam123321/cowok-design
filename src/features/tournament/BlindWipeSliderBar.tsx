// @story 10.3
// @implements AC-02 (Blind Wipe, Side-by-Side, 2x2 Grid & Onion-Skin Comparison Arena with 5-Icon Main Canvas Responsive Viewport Bar)
import React, { useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type { ViewportPreset } from "../../lib/validations/screen-fixture";
import type { GeneratedVariant } from "../../lib/validations/tournament";
import {
  formatVariantDiversityBadge,
  normalizeTournamentViewport,
  type TournamentViewportMode,
} from "./useDesignTournament";

export type TournamentCompareMode = "wipe" | "side-by-side" | "grid" | "onion";

const DEFAULT_SPLIT_PERCENT = 50;
const MIN_SPLIT_PERCENT = 5;
const MAX_SPLIT_PERCENT = 95;
const PERCENT_MAX = 100;
const DEFAULT_ONION_OPACITY = 50;
const MIN_ONION_OPACITY = 0;
const DESKTOP_FRAME_WIDTH_PX = 1440;
const TABLET_FRAME_WIDTH_PX = 768;
const FOLD_INNER_FRAME_WIDTH_PX = 712;
const MOBILE_FRAME_WIDTH_PX = 390;
const FOLD_OUTER_FRAME_WIDTH_PX = 344;
const PREVIEW_HEIGHT_PX = 340;
const GRID_PREVIEW_HEIGHT_PX = 220;
const WIPE_HANDLE_WIDTH_PX = 4;
const ICON_SIZE_SM = 13;
const ICON_STROKE_MONO = 1.85;
const FONT_MEDIUM = 500;
const FONT_SEMIBOLD = 600;
const Z_OVERLAY = 2;
const Z_HANDLE = 3;
const Z_PILL = 4;

interface ViewportPillOption {
  id: ViewportPreset;
  icon: "monitor" | "tablet" | "fold-inner" | "smartphone" | "fold-outer";
  testId: string;
  labelKey: string;
  labelFallback: string;
  widthPx: number;
}

const VIEWPORT_PILL_OPTIONS: ReadonlyArray<ViewportPillOption> = [
  { id: "desktop-1440", icon: "monitor", testId: "tournament-viewport-desktop", labelKey: "tournament.viewportDesktop", labelFallback: "Màn hình Desktop ({{width}}px)", widthPx: DESKTOP_FRAME_WIDTH_PX },
  { id: "tablet-768", icon: "tablet", testId: "tournament-viewport-tablet", labelKey: "tournament.viewportTablet", labelFallback: "Màn hình Tablet ({{width}}px)", widthPx: TABLET_FRAME_WIDTH_PX },
  { id: "fold-inner-712", icon: "fold-inner", testId: "tournament-viewport-fold-inner", labelKey: "tournament.viewportFoldInner", labelFallback: "Màn hình mở gập ({{width}}px)", widthPx: FOLD_INNER_FRAME_WIDTH_PX },
  { id: "mobile-390", icon: "smartphone", testId: "tournament-viewport-mobile", labelKey: "tournament.viewportMobile", labelFallback: "Màn hình Mobile ({{width}}px)", widthPx: MOBILE_FRAME_WIDTH_PX },
  { id: "fold-outer-344", icon: "fold-outer", testId: "tournament-viewport-fold-outer", labelKey: "tournament.viewportFoldOuter", labelFallback: "Màn hình ngoài gập ({{width}}px)", widthPx: FOLD_OUTER_FRAME_WIDTH_PX },
];

const COMPARE_MODE_OPTIONS: ReadonlyArray<{ id: TournamentCompareMode; testId: string; labelKey: string; labelFallback: string }> = [
  { id: "wipe", testId: "compare-mode-wipe", labelKey: "tournament.compareModeWipe", labelFallback: "Kéo rèm" },
  { id: "side-by-side", testId: "compare-mode-side-by-side", labelKey: "tournament.compareModeSideBySide", labelFallback: "Song song" },
  { id: "grid", testId: "compare-mode-grid", labelKey: "tournament.compareModeGrid", labelFallback: "Lưới 2×2" },
  { id: "onion", testId: "compare-mode-onion", labelKey: "tournament.compareModeOnion", labelFallback: "Chồng mờ" },
];

export interface BlindWipeSliderBarProps {
  leftVariant: GeneratedVariant;
  rightVariant: GeneratedVariant;
  viewportMode: TournamentViewportMode;
  onViewportModeChange: (mode: TournamentViewportMode) => void;
  allVariants: ReadonlyArray<GeneratedVariant>;
  onCompareVariantChange: (variantId: string) => void;
  onSelectVariant?: (variantId: string) => void;
}

const PILL_GROUP_STYLE: React.CSSProperties = {
  display: "inline-flex", alignItems: "center", backgroundColor: "var(--bg-workspace)",
  border: "1px solid var(--border-default)", borderRadius: "var(--radius-xs)", padding: "2px", gap: "2px", flexWrap: "nowrap",
};
const FRAME_STYLE: React.CSSProperties = {
  position: "absolute", inset: 0, width: "100%", height: "100%", border: "none", backgroundColor: "var(--bg-card)",
};
const LABEL_PILL_STYLE: React.CSSProperties = {
  position: "absolute", top: "10px", zIndex: Z_PILL, padding: "4px 10px", borderRadius: "999px",
  backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", color: "var(--text-primary)",
  fontSize: "11px", fontWeight: FONT_SEMIBOLD, boxShadow: "var(--shadow-sm)",
};
const STAGE_BOX_BASE: React.CSSProperties = {
  position: "relative", maxWidth: "100%", height: `${PREVIEW_HEIGHT_PX}px`, overflow: "hidden",
  borderRadius: "var(--radius-sm)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-card)",
};

function wrapOpaquePreviewHtml(rawHtml: string): string {
  const baseStyle =
    "<style>html,body{background:#ffffff;color:#0f172a;font-family:system-ui,-apple-system,sans-serif;margin:0;padding:38px 16px 16px;box-sizing:border-box;min-height:100%;}</style>";
  if (rawHtml.toLowerCase().includes("<head>")) return rawHtml.replace(/<head>/i, `<head>${baseStyle}`);
  return `<!doctype html><html><head>${baseStyle}</head><body>${rawHtml}</body></html>`;
}

function resolveStageWidth(preset: ViewportPreset): string {
  if (preset === "tablet-768") return `${TABLET_FRAME_WIDTH_PX}px`;
  if (preset === "fold-inner-712") return `${FOLD_INNER_FRAME_WIDTH_PX}px`;
  if (preset === "mobile-390") return `var(--mobile-judge-viewport, ${MOBILE_FRAME_WIDTH_PX}px)`;
  if (preset === "fold-outer-344") return `${FOLD_OUTER_FRAME_WIDTH_PX}px`;
  return "100%";
}

const ViewportPillBar: React.FC<{ activePreset: ViewportPreset; onChange: (mode: TournamentViewportMode) => void }> = ({
  activePreset,
  onChange,
}) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  return (
    <div style={PILL_GROUP_STYLE} role="group" aria-label={t("tournament.viewportAria", "Chế độ khung nhìn")} data-testid="tournament-viewport-group">
      {VIEWPORT_PILL_OPTIONS.map((item) => {
        const active = activePreset === item.id;
        const label = t(item.labelKey, item.labelFallback, { width: formatNumber(item.widthPx) });
        return (
          <button
            key={item.id} type="button" data-testid={item.testId} aria-pressed={active} aria-label={label} title={label}
            onClick={() => onChange(item.id)}
            style={{
              width: "26px", height: "24px", padding: 0, border: "none", borderRadius: "var(--radius-xs)",
              backgroundColor: active ? "var(--bg-selected)" : "transparent", color: active ? "var(--action-primary)" : "var(--text-muted)",
              display: "inline-flex", alignItems: "center", justifyContent: "center", cursor: "pointer",
            }}
          >
            <BrandIcon name={item.icon} size={ICON_SIZE_SM} strokeWidth={ICON_STROKE_MONO} color="currentColor" />
          </button>
        );
      })}
    </div>
  );
};

const CompareModeSwitcher: React.FC<{ mode: TournamentCompareMode; onChange: (next: TournamentCompareMode) => void }> = ({
  mode,
  onChange,
}) => {
  const { t } = useTranslation();
  return (
    <div role="group" aria-label={t("tournament.compareModeAria", "Chế độ so sánh giao diện")} data-testid="tournament-compare-mode-switcher" style={PILL_GROUP_STYLE}>
      {COMPARE_MODE_OPTIONS.map((opt) => {
        const active = mode === opt.id;
        return (
          <button
            key={opt.id} type="button" data-testid={opt.testId} aria-pressed={active} onClick={() => onChange(opt.id)}
            style={{
              height: "24px", padding: "0 8px", border: "none", borderRadius: "var(--radius-xs)",
              backgroundColor: active ? "var(--bg-selected)" : "transparent", color: active ? "var(--action-primary)" : "var(--text-muted)",
              fontSize: "11px", fontWeight: active ? FONT_SEMIBOLD : FONT_MEDIUM, cursor: "pointer", whiteSpace: "nowrap",
            }}
          >
            {t(opt.labelKey, opt.labelFallback)}
          </button>
        );
      })}
    </div>
  );
};

const ComparisonHeaderBar: React.FC<{
  compareMode: TournamentCompareMode;
  onCompareModeChange: (m: TournamentCompareMode) => void;
  compareChoices: ReadonlyArray<GeneratedVariant>;
  rightVariantId: string;
  onCompareVariantChange: (id: string) => void;
  activePreset: ViewportPreset;
  onViewportModeChange: (m: TournamentViewportMode) => void;
}> = (p) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", flexWrap: "wrap", gap: "8px" }}>
      <CompareModeSwitcher mode={p.compareMode} onChange={p.onCompareModeChange} />
      <div style={{ display: "inline-flex", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
        {p.compareMode !== "grid" && p.compareChoices.length > 0 && (
          <label style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "11.5px", color: "var(--text-muted)" }}>
            <span>{t("tournament.compareAgainst", "So với")}:</span>
            <select
              data-testid="blind-wipe-compare-select" value={p.rightVariantId} onChange={(e) => p.onCompareVariantChange(e.target.value)}
              style={{ padding: "4px 8px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-card)", color: "var(--text-primary)", fontSize: "11.5px" }}
            >
              {p.compareChoices.map((v) => <option key={v.id} value={v.id}>{v.displayLabel}</option>)}
            </select>
          </label>
        )}
        <ViewportPillBar activePreset={p.activePreset} onChange={p.onViewportModeChange} />
      </div>
    </div>
  );
};

const WipeComparisonStage: React.FC<{
  leftVariant: GeneratedVariant;
  rightVariant: GeneratedVariant;
  stageWidth: string;
  splitPercent: number;
  onSplitChange: (val: number) => void;
}> = ({ leftVariant, rightVariant, stageWidth, splitPercent, onSplitChange }) => {
  const { t } = useTranslation();
  return (
    <>
      <div style={{ display: "flex", justifyContent: "center", width: "100%" }}>
        <div data-testid="blind-wipe-stage" style={{ ...STAGE_BOX_BASE, width: stageWidth }}>
          <span style={{ ...LABEL_PILL_STYLE, left: "10px" }} data-testid="blind-wipe-left-label">{leftVariant.displayLabel}</span>
          <span style={{ ...LABEL_PILL_STYLE, right: "10px" }} data-testid="blind-wipe-right-label">{rightVariant.displayLabel}</span>
          <iframe title={rightVariant.displayLabel} srcDoc={wrapOpaquePreviewHtml(rightVariant.html)} sandbox="" style={FRAME_STYLE} />
          <div data-testid="blind-wipe-clip-overlay" style={{ position: "absolute", inset: 0, backgroundColor: "var(--bg-card)", clipPath: `inset(0 ${PERCENT_MAX - splitPercent}% 0 0)`, zIndex: Z_OVERLAY }}>
            <iframe title={leftVariant.displayLabel} srcDoc={wrapOpaquePreviewHtml(leftVariant.html)} sandbox="" style={FRAME_STYLE} />
          </div>
          <div aria-hidden="true" style={{ position: "absolute", top: 0, bottom: 0, left: `${splitPercent}%`, width: `var(--wipe-handle-width, ${WIPE_HANDLE_WIDTH_PX}px)`, transform: "translateX(-50%)", backgroundColor: "var(--action-primary)", zIndex: Z_HANDLE, pointerEvents: "none" }} />
        </div>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
        <span style={{ fontSize: "11px", color: "var(--text-muted)", minWidth: "68px" }}>{leftVariant.displayLabel}</span>
        <input type="range" min={MIN_SPLIT_PERCENT} max={MAX_SPLIT_PERCENT} value={splitPercent} data-testid="blind-wipe-range-input" aria-label={t("tournament.wipeRangeAria", "Kéo để so sánh hai phương án")} onChange={(e) => onSplitChange(Number(e.target.value))} style={{ flex: 1, accentColor: "var(--action-primary)", cursor: "ew-resize" }} />
        <span style={{ fontSize: "11px", color: "var(--text-muted)", minWidth: "68px", textAlign: "right" }}>{rightVariant.displayLabel}</span>
      </div>
    </>
  );
};

const SideBySideStage: React.FC<{ leftVariant: GeneratedVariant; rightVariant: GeneratedVariant; stageWidth: string }> = ({
  leftVariant,
  rightVariant,
  stageWidth,
}) => (
  <div data-testid="compare-stage-side-by-side" style={{ display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: "10px", width: "100%" }}>
    {[{ variant: leftVariant, testId: "side-by-side-left" }, { variant: rightVariant, testId: "side-by-side-right" }].map(({ variant, testId }) => (
      <div key={testId} data-testid={testId} style={{ display: "flex", justifyContent: "center", minWidth: 0 }}>
        <div style={{ ...STAGE_BOX_BASE, width: stageWidth }}>
          <span style={{ ...LABEL_PILL_STYLE, left: "10px" }}>{variant.displayLabel}</span>
          <iframe title={variant.displayLabel} srcDoc={wrapOpaquePreviewHtml(variant.html)} sandbox="" style={FRAME_STYLE} />
        </div>
      </div>
    ))}
  </div>
);

const GridVariantCard: React.FC<{
  variant: GeneratedVariant;
  isSelected: boolean;
  stageWidth: string;
  onSelectVariant?: (variantId: string) => void;
}> = ({ variant, isSelected, stageWidth, onSelectVariant }) => {
  useTranslation();
  const badge = formatVariantDiversityBadge(variant);
  return (
    <div data-testid={`compare-grid-card-${variant.id}`} style={{ display: "flex", flexDirection: "column", borderRadius: "var(--radius-sm)", border: isSelected ? "1.5px solid var(--action-primary)" : "1px solid var(--border-default)", backgroundColor: "var(--bg-card)", overflow: "hidden", minWidth: 0 }}>
      <button
        type="button" data-testid={`compare-grid-select-${variant.id}`} aria-pressed={isSelected} onClick={() => onSelectVariant?.(variant.id)}
        style={{
          display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px", padding: "6px 10px", border: "none",
          borderBottom: "1px solid var(--border-subtle)", backgroundColor: isSelected ? "var(--bg-selected)" : "var(--bg-surface)",
          color: isSelected ? "var(--action-primary)" : "var(--text-primary)", fontSize: "11.5px", fontWeight: FONT_SEMIBOLD,
          cursor: onSelectVariant ? "pointer" : "default", textAlign: "left",
        }}
      >
        <span>{variant.displayLabel}</span>
        <span title={badge.tooltip} style={{ padding: "2px 6px", borderRadius: "999px", fontSize: "10px", backgroundColor: badge.passed ? "var(--state-success-soft, var(--bg-card))" : "var(--state-warning-soft, var(--bg-workspace))", color: badge.passed ? "var(--state-success)" : "var(--state-warning)" }}>
          {badge.label}
        </span>
      </button>
      <div style={{ display: "flex", justifyContent: "center", width: "100%", backgroundColor: "var(--bg-canvas)" }}>
        <div style={{ position: "relative", width: stageWidth, maxWidth: "100%", height: `${GRID_PREVIEW_HEIGHT_PX}px`, overflow: "hidden" }}>
          <iframe title={variant.displayLabel} srcDoc={wrapOpaquePreviewHtml(variant.html)} sandbox="" style={FRAME_STYLE} />
        </div>
      </div>
    </div>
  );
};

const GridOverviewStage: React.FC<{
  variants: ReadonlyArray<GeneratedVariant>;
  activeVariantId: string;
  stageWidth: string;
  onSelectVariant?: (variantId: string) => void;
}> = ({ variants, activeVariantId, stageWidth, onSelectVariant }) => (
  <div data-testid="compare-stage-grid" style={{ display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: "10px", width: "100%" }}>
    {variants.map((variant) => (
      <GridVariantCard key={variant.id} variant={variant} isSelected={variant.id === activeVariantId} stageWidth={stageWidth} onSelectVariant={onSelectVariant} />
    ))}
  </div>
);

const OnionSkinStage: React.FC<{
  leftVariant: GeneratedVariant;
  rightVariant: GeneratedVariant;
  stageWidth: string;
  opacityPercent: number;
  onOpacityChange: (val: number) => void;
}> = ({ leftVariant, rightVariant, stageWidth, opacityPercent, onOpacityChange }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const formattedPct = `${formatNumber(opacityPercent)}%`;
  return (
    <>
      <div style={{ display: "flex", justifyContent: "center", width: "100%" }}>
        <div data-testid="compare-stage-onion" style={{ ...STAGE_BOX_BASE, width: stageWidth }}>
          <span style={{ ...LABEL_PILL_STYLE, left: "10px" }}>{`${leftVariant.displayLabel} (${formattedPct})`}</span>
          <span style={{ ...LABEL_PILL_STYLE, right: "10px" }}>{`${rightVariant.displayLabel} ${t("tournament.baseBadge", "(nền)")}`}</span>
          <iframe title={rightVariant.displayLabel} srcDoc={wrapOpaquePreviewHtml(rightVariant.html)} sandbox="" style={FRAME_STYLE} />
          <div data-testid="onion-overlay-layer" style={{ position: "absolute", inset: 0, opacity: opacityPercent / PERCENT_MAX, zIndex: Z_OVERLAY, pointerEvents: "none" }}>
            <iframe title={leftVariant.displayLabel} srcDoc={wrapOpaquePreviewHtml(leftVariant.html)} sandbox="" style={FRAME_STYLE} />
          </div>
        </div>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
        <span style={{ fontSize: "11px", color: "var(--text-muted)", minWidth: "74px" }}>{t("tournament.onionOpacityLabel", "Độ mờ lớp trên")}</span>
        <input type="range" min={MIN_ONION_OPACITY} max={PERCENT_MAX} value={opacityPercent} data-testid="onion-opacity-range-input" aria-label={t("tournament.onionOpacityAria", "Độ mờ lớp chồng so sánh")} onChange={(e) => onOpacityChange(Number(e.target.value))} style={{ flex: 1, accentColor: "var(--action-primary)", cursor: "ew-resize" }} />
        <span data-testid="onion-opacity-value" style={{ fontSize: "11px", color: "var(--text-secondary)", fontVariantNumeric: "tabular-nums", minWidth: "36px", textAlign: "right" }}>
          {formattedPct}
        </span>
      </div>
    </>
  );
};

export const BlindWipeSliderBar: React.FC<BlindWipeSliderBarProps> = ({
  leftVariant,
  rightVariant,
  viewportMode,
  onViewportModeChange,
  allVariants,
  onCompareVariantChange,
  onSelectVariant,
}) => {
  const [compareMode, setCompareMode] = useState<TournamentCompareMode>("wipe");
  const [splitPercent, setSplitPercent] = useState<number>(DEFAULT_SPLIT_PERCENT);
  const [onionOpacity, setOnionOpacity] = useState<number>(DEFAULT_ONION_OPACITY);
  const activePreset = normalizeTournamentViewport(viewportMode);
  const stageWidth = resolveStageWidth(activePreset);
  const compareChoices = allVariants.filter((v) => v.id !== leftVariant.id);

  return (
    <section
      data-ff-id="tournament-blind-wipe-slider" data-viewport-mode={activePreset} data-compare-mode={compareMode}
      style={{ display: "flex", flexDirection: "column", gap: "12px", padding: "14px", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-md)" }}
    >
      <ComparisonHeaderBar
        compareMode={compareMode} onCompareModeChange={setCompareMode} compareChoices={compareChoices}
        rightVariantId={rightVariant.id} onCompareVariantChange={onCompareVariantChange}
        activePreset={activePreset} onViewportModeChange={onViewportModeChange}
      />
      {compareMode === "wipe" && <WipeComparisonStage leftVariant={leftVariant} rightVariant={rightVariant} stageWidth={stageWidth} splitPercent={splitPercent} onSplitChange={setSplitPercent} />}
      {compareMode === "side-by-side" && <SideBySideStage leftVariant={leftVariant} rightVariant={rightVariant} stageWidth={stageWidth} />}
      {compareMode === "grid" && <GridOverviewStage variants={allVariants} activeVariantId={leftVariant.id} stageWidth={stageWidth} onSelectVariant={onSelectVariant} />}
      {compareMode === "onion" && <OnionSkinStage leftVariant={leftVariant} rightVariant={rightVariant} stageWidth={stageWidth} opacityPercent={onionOpacity} onOpacityChange={setOnionOpacity} />}
    </section>
  );
};
