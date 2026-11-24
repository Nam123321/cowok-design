// @story 05.6
// @implements AC-02, AC-03 (Extracted Shared Component Gallery styled in active BrandProposalV1 & user themeMode)
import React from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";
import type {
  BrandModeTokens,
  BrandProposalV1,
  PopularComponentAuditItem,
} from "../../lib/validations/cognitive-studio";

const STROKE_MONO = 1.85;
const ICON_SM = 14;
const FONT_WEIGHT_SEMIBOLD = 600;
const MUTED_OPACITY = 0.8;
const LINE_HEIGHT_BODY = 1.4;

export interface SharedComponentGalleryProps {
  proposal: BrandProposalV1 | null;
  activeMode: BrandModeTokens | null;
  auditItems: PopularComponentAuditItem[];
}

function resolveRadiusPx(scale: BrandProposalV1["radiusScale"] | undefined): string {
  switch (scale) {
    case "sharp": return "2px";
    case "soft": return "14px";
    case "pill": return "999px";
    default: return "8px";
  }
}

function buildTokenVars(mode: BrandModeTokens | null, radius: string): React.CSSProperties {
  const tokens = mode?.tokens ?? {};
  return {
    backgroundColor: tokens["--bg-surface"] ?? "var(--bg-surface)",
    color: tokens["--text-primary"] ?? "var(--text-primary)",
    borderColor: tokens["--border-default"] ?? "var(--border-default)",
    borderRadius: radius,
  };
}

interface SpecimenStyleProps {
  kind: string;
  accent: string;
  onAccent: string;
  border: string;
  radius: string;
}

const DataTableSpecimen: React.FC<{ accent: string; border: string; radius: string }> = ({ accent, border, radius }) => (
  <div style={{ fontSize: "11px", border: `1px solid ${border}`, borderRadius: radius, overflow: "hidden" }}>
    <div style={{ display: "flex", justifyContent: "space-between", padding: "4px 8px", borderBottom: `1px solid ${border}`, fontWeight: FONT_WEIGHT_SEMIBOLD }}>
      <span>Item</span>
      <span>Status</span>
    </div>
    <div style={{ display: "flex", justifyContent: "space-between", padding: "4px 8px" }}>
      <span>Row A</span>
      <span style={{ color: accent }}>Active</span>
    </div>
  </div>
);

const ComponentPreviewSpecimen: React.FC<SpecimenStyleProps> = ({ kind, accent, onAccent, border, radius }) => {
  if (kind === "button") {
    return (
      <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
        <span style={{ padding: "5px 12px", borderRadius: radius, backgroundColor: accent, color: onAccent, fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>Primary</span>
        <span style={{ padding: "5px 12px", borderRadius: radius, border: `1px solid ${border}`, fontSize: "12px" }}>Secondary</span>
      </div>
    );
  }
  if (kind === "toast") {
    return (
      <div style={{ display: "flex", alignItems: "center", gap: "6px", padding: "6px 10px", borderRadius: radius, border: `1px solid ${accent}`, fontSize: "11.5px" }}>
        <BrandIcon name="check-circle" size={ICON_SM} strokeWidth={STROKE_MONO} color={accent} />
        <span>Saved changes</span>
      </div>
    );
  }
  if (kind === "confirm-dialog") {
    return (
      <div style={{ padding: "8px", borderRadius: radius, border: `1px solid ${border}`, fontSize: "11px" }}>
        <div style={{ fontWeight: FONT_WEIGHT_SEMIBOLD, marginBottom: "4px" }}>Confirm action?</div>
        <div style={{ display: "flex", gap: "6px" }}>
          <span style={{ padding: "2px 8px", borderRadius: radius, backgroundColor: accent, color: onAccent }}>Confirm</span>
          <span style={{ padding: "2px 8px", borderRadius: radius, border: `1px solid ${border}` }}>Cancel</span>
        </div>
      </div>
    );
  }
  if (kind === "data-table") {
    return <DataTableSpecimen accent={accent} border={border} radius={radius} />;
  }
  return (
    <div style={{ padding: "6px 10px", borderRadius: radius, border: `1px dashed ${border}`, fontSize: "11.5px" }}>
      {kind}
    </div>
  );
};

const GalleryCard: React.FC<{
  item: PopularComponentAuditItem;
  surfaceStyle: React.CSSProperties;
  accent: string;
  onAccent: string;
  border: string;
  radius: string;
}> = ({ item, surfaceStyle, accent, onAccent, border, radius }) => (
  <article
    data-testid={`gallery-card-${item.componentKind}`}
    style={{ ...surfaceStyle, borderStyle: "solid", borderWidth: "1px", padding: "12px", display: "flex", flexDirection: "column", gap: "10px" }}
  >
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}>
      <span style={{ fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>{item.componentKind}</span>
      <span style={{ fontSize: "10px", padding: "2px 6px", borderRadius: "var(--radius-pill)", border: `1px solid ${border}` }}>{item.category}</span>
    </div>
    <ComponentPreviewSpecimen kind={item.componentKind} accent={accent} onAccent={onAccent} border={border} radius={radius} />
    {item.reason && <p style={{ margin: 0, fontSize: "11px", opacity: MUTED_OPACITY, lineHeight: LINE_HEIGHT_BODY }}>{item.reason}</p>}
  </article>
);

export const SharedComponentGallery: React.FC<SharedComponentGalleryProps> = ({
  proposal,
  activeMode,
  auditItems,
}) => {
  const { t } = useTranslation();
  const radius = resolveRadiusPx(proposal?.radiusScale);
  const surfaceStyle = buildTokenVars(activeMode, radius);
  const accent = activeMode?.tokens["--action-primary"] ?? "var(--action-primary)";
  const onAccent = activeMode?.tokens["--text-on-accent"] ?? "var(--action-primary-fg)";
  const border = activeMode?.tokens["--border-default"] ?? "var(--border-default)";
  const visibleItems = auditItems.filter((item) => item.selected);

  if (visibleItems.length === 0) {
    return (
      <div data-testid="shared-component-gallery-empty" style={{ padding: "24px", textAlign: "center", color: "var(--text-secondary)", fontSize: "12.5px" }}>
        {t("brandStudio.galleryEmpty", "Chưa có component nào được chọn. Hãy bật ở tab Popular Audit.")}
      </div>
    );
  }

  return (
    <div data-testid="shared-component-gallery" style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(210px, 1fr))", gap: "12px", padding: "4px 0" }}>
      {visibleItems.map((item) => (
        <GalleryCard key={item.componentKind} item={item} surfaceStyle={surfaceStyle} accent={accent} onAccent={onAccent} border={border} radius={radius} />
      ))}
    </div>
  );
};
