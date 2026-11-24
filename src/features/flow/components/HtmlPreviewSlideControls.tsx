// @story 04.1, 06.1, 06.5
// @implements AC-01, AC-02, AC-06 (Responsive Viewport Switcher, Slide Aspect/Zoom Controls & AI CSV/PDF Deck Generator Trigger)
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type { GenerateDeckResponse } from "../../../lib/validations/deck-generation";
import type { ViewportPreset } from "../../../lib/validations/screen-fixture";
import { DeckCsvGeneratorModal } from "./DeckCsvGeneratorModal";
import { CompactOptionDropdown, TOOLBAR_BTN_BASE, TOOLBAR_ICON_BTN_BASE } from "./HtmlPreviewToolbarControls";

const ICON_SIZE_SM = 12;
const DESKTOP_PX = 1440;
const TABLET_PX = 768;
const MOBILE_PX = 390;
const PAN_SCALE_PCT = 100;
const DISABLED_OPACITY = 0.45;
const FONT_WEIGHT_MEDIUM = 500;
const FONT_WEIGHT_SEMIBOLD = 600;

const VIEWPORT_ITEMS: ReadonlyArray<{
  preset: ViewportPreset;
  icon: "monitor" | "tablet" | "smartphone";
  pxWidth: number;
  i18nKey: string;
  defaultLabel: string;
}> = [
  { preset: "desktop-1440", icon: "monitor", pxWidth: DESKTOP_PX, i18nKey: "canvas.viewportDesktop", defaultLabel: "Desktop" },
  { preset: "tablet-768", icon: "tablet", pxWidth: TABLET_PX, i18nKey: "canvas.viewportTablet", defaultLabel: "Tablet" },
  { preset: "mobile-390", icon: "smartphone", pxWidth: MOBILE_PX, i18nKey: "canvas.viewportMobile", defaultLabel: "Mobile" },
];

export const TOOLBAR_PILL_GROUP_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  backgroundColor: "var(--bg-workspace)",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-xs)",
  padding: "2px",
  gap: "2px",
  flexWrap: "nowrap",
};

const COMPACT_PILL_BTN_STYLE = (active: boolean): React.CSSProperties => ({
  height: "20px",
  padding: "0 6px",
  borderRadius: "var(--radius-xs)",
  border: "none",
  fontSize: "10.5px",
  fontWeight: active ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM,
  fontVariantNumeric: "tabular-nums",
  cursor: "pointer",
  backgroundColor: active ? "var(--bg-selected)" : "transparent",
  color: active ? "var(--action-primary)" : "var(--text-muted)",
});

export const ViewportSwitcherControl: React.FC<{
  activePreset: ViewportPreset;
  isReadOnly?: boolean;
  isCompact: boolean;
  onSetViewportPreset: (vp: ViewportPreset) => void;
}> = ({ activePreset, isReadOnly, isCompact, onSetViewportPreset }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const currentItem = VIEWPORT_ITEMS.find((item) => item.preset === activePreset) ?? VIEWPORT_ITEMS[0];
  const aria = t("canvas.viewportSwitcherAria", "Kích thước màn hình");
  if (isCompact) {
    return (
      <CompactOptionDropdown
        testId="viewport-preset-dropdown"
        ariaLabel={aria}
        activeLabel={t(currentItem.i18nKey, currentItem.defaultLabel)}
        activeIcon={currentItem.icon}
        disabled={isReadOnly}
        value={activePreset}
        options={VIEWPORT_ITEMS.map(({ preset, icon, pxWidth, i18nKey, defaultLabel }) => ({
          value: preset,
          icon,
          label: `${t(i18nKey, defaultLabel)} (${formatNumber(pxWidth)}px)`,
          testId: `viewport-btn-${preset}`,
        }))}
        onChange={onSetViewportPreset}
      />
    );
  }
  return (
    <div style={TOOLBAR_PILL_GROUP_STYLE} role="group" aria-label={aria}>
      {VIEWPORT_ITEMS.map(({ preset, icon, pxWidth, i18nKey, defaultLabel }) => {
        const isActive = activePreset === preset;
        const vpTitle = `${t(i18nKey, defaultLabel)} (${formatNumber(pxWidth)}px)`;
        return (
          <button
            key={preset}
            type="button"
            disabled={isReadOnly}
            data-testid={`viewport-btn-${preset}`}
            aria-pressed={isActive}
            onClick={() => onSetViewportPreset(preset)}
            title={vpTitle}
            aria-label={vpTitle}
            style={{
              ...TOOLBAR_ICON_BTN_BASE,
              height: "20px",
              backgroundColor: isActive ? "var(--bg-selected)" : "transparent",
              color: isActive ? "var(--action-primary)" : "var(--text-muted)",
              opacity: isReadOnly ? DISABLED_OPACITY : 1,
              cursor: isReadOnly ? "not-allowed" : "pointer",
            }}
          >
            <BrandIcon name={icon} size={ICON_SIZE_SM} color="currentColor" />
          </button>
        );
      })}
    </div>
  );
};

const AspectControl: React.FC<{
  aspectRatio: "auto" | "16-9" | "16-10";
  isCompact: boolean;
  onSetAspectRatio: (ar: "auto" | "16-9" | "16-10") => void;
}> = ({ aspectRatio, isCompact, onSetAspectRatio }) => {
  const { t } = useTranslation();
  const labelMap: Record<"auto" | "16-9" | "16-10", string> = {
    "16-9": "16:9",
    "16-10": "16:10",
    auto: t("canvas.aspectAutoShort", "Auto"),
  };
  const options = (["16-9", "16-10", "auto"] as const).map((ar) => ({ value: ar, label: labelMap[ar], testId: `aspect-pill-${ar}` }));
  const aria = t("canvas.aspectRatio", "Tỷ lệ khung hình");
  if (isCompact) {
    return (
      <CompactOptionDropdown
        testId="aspect-ratio-dropdown"
        ariaLabel={aria}
        activeLabel={labelMap[aspectRatio]}
        isMono
        value={aspectRatio}
        options={options}
        onChange={onSetAspectRatio}
      />
    );
  }
  return (
    <div style={TOOLBAR_PILL_GROUP_STYLE} role="group" aria-label={aria} data-testid="aspect-ratio-pill-group">
      {options.map((opt) => (
        <button key={opt.value} type="button" data-testid={opt.testId} aria-pressed={aspectRatio === opt.value} onClick={() => onSetAspectRatio(opt.value)} style={COMPACT_PILL_BTN_STYLE(aspectRatio === opt.value)}>
          {opt.label}
        </button>
      ))}
    </div>
  );
};

const ZoomControl: React.FC<{
  zoomMode?: "fit" | "pan-100";
  isCompact: boolean;
  onSetZoomMode?: (mode: "fit" | "pan-100") => void;
}> = ({ zoomMode, isCompact, onSetZoomMode }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  if (!onSetZoomMode) return null;
  const activeZoom = zoomMode ?? "fit";
  const options = [
    { value: "fit" as const, label: t("presentation.zoomFit", "Vừa khung"), testId: "zoom-pill-fit" },
    { value: "pan-100" as const, label: `${formatNumber(PAN_SCALE_PCT)}%`, testId: "zoom-pill-pan-100" },
  ];
  const activeLabel = options.find((z) => z.value === activeZoom)?.label ?? options[0].label;
  const aria = t("presentation.zoomGroupAria", "Tỷ lệ hiển thị");
  return (
    <div data-testid="zoom-pill-scale" style={{ display: "inline-flex" }}>
      {isCompact ? (
        <CompactOptionDropdown testId="zoom-mode-dropdown" ariaLabel={aria} activeLabel={activeLabel} isMono value={activeZoom} options={options} onChange={onSetZoomMode} />
      ) : (
        <div style={TOOLBAR_PILL_GROUP_STYLE} role="group" aria-label={aria} data-testid="zoom-pill-group">
          {options.map((z) => (
            <button key={z.value} type="button" data-testid={z.testId} aria-pressed={activeZoom === z.value} onClick={() => onSetZoomMode(z.value)} style={COMPACT_PILL_BTN_STYLE(activeZoom === z.value)}>
              {z.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
};

function toDeckAspectRatio(ar: "auto" | "16-9" | "16-10"): "16:9" | "16:10" | "Auto" {
  if (ar === "16-10") return "16:10";
  if (ar === "auto") return "Auto";
  return "16:9";
}

function buildDeckTriggerStyle(
  isOpen: boolean,
  isCompact: boolean,
  hasProject: boolean
): React.CSSProperties {
  return {
    ...TOOLBAR_BTN_BASE,
    width: "auto",
    height: "24px",
    padding: isCompact ? "0 6px" : "0 8px",
    gap: "4px",
    border: "1px solid var(--border-default)",
    borderRadius: "var(--radius-xs)",
    backgroundColor: isOpen ? "var(--bg-selected)" : "var(--bg-workspace)",
    color: isOpen ? "var(--action-primary)" : "var(--text-secondary)",
    fontSize: "10.5px",
    fontWeight: FONT_WEIGHT_SEMIBOLD,
    whiteSpace: "nowrap",
    flexShrink: 0,
    opacity: hasProject ? 1 : DISABLED_OPACITY,
    cursor: hasProject ? "pointer" : "not-allowed",
  };
}

export const SlideAspectAndZoomControls: React.FC<{
  aspectRatio: "auto" | "16-9" | "16-10";
  zoomMode?: "fit" | "pan-100";
  isCompact: boolean;
  projectId?: string;
  onSetAspectRatio: (ar: "auto" | "16-9" | "16-10") => void;
  onSetZoomMode?: (mode: "fit" | "pan-100") => void;
  onDeckGenerated?: (response: GenerateDeckResponse) => void;
  onSyncToMainChat?: (summaryText: string) => void;
  onEmbedIntoActiveSlide?: (htmlSnippet: string, title: string) => void;
}> = (p) => {
  const { t } = useTranslation();
  const [isGeneratorOpen, setIsGeneratorOpen] = useState(false);
  const resolvedProjectId = p.projectId?.trim() || "";
  const hasProject = resolvedProjectId.length > 0;
  const triggerLabel = t("deckGen.openGeneratorBtn", "AI Deck");
  const triggerTitle = t("deckGen.openGeneratorTooltip", "Tạo bộ Slide đa trang từ Brief & CSV/TSV/PDF (stageshow)");
  return (
    <div style={{ display: "inline-flex", alignItems: "center", gap: "4px", flexWrap: "nowrap" }}>
      <AspectControl aspectRatio={p.aspectRatio} isCompact={p.isCompact} onSetAspectRatio={p.onSetAspectRatio} />
      <ZoomControl zoomMode={p.zoomMode} isCompact={p.isCompact} onSetZoomMode={p.onSetZoomMode} />
      <button type="button" disabled={!hasProject} data-testid="open-deck-csv-generator-btn" aria-label={triggerTitle} title={triggerTitle} onClick={() => hasProject && setIsGeneratorOpen(true)} style={buildDeckTriggerStyle(isGeneratorOpen, p.isCompact, hasProject)}>
        <BrandIcon name="sparkles" size={ICON_SIZE_SM} color="currentColor" />
        {!p.isCompact && <span>{triggerLabel}</span>}
      </button>
      {hasProject && (
        <DeckCsvGeneratorModal isOpen={isGeneratorOpen} projectId={resolvedProjectId} aspectRatio={toDeckAspectRatio(p.aspectRatio)} onClose={() => setIsGeneratorOpen(false)} onDeckGenerated={p.onDeckGenerated} onSyncToMainChat={p.onSyncToMainChat} onEmbedIntoActiveSlide={p.onEmbedIntoActiveSlide} />
      )}
    </div>
  );
};
