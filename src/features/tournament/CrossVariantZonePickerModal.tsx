// @story 10.3
// @implements AC-02 (Visual Cross-Variant Zone Picker Popup with Safe Static DOMParser Thumbnails & Multi-Block Selection)
import React, { useEffect, useId, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type { GeneratedVariant } from "../../lib/validations/tournament";

const MODAL_Z_INDEX = 80;
const MODAL_ICON_SIZE = 14;
const FONT_MEDIUM = 500;
const FONT_SEMIBOLD = 600;
const MAX_EXCERPT_LEN = 64;
const THUMB_HEIGHT_PX = 108;
const BODY_LINE_HEIGHT = 1.45;

const EXCLUDED_ZONE_TAGS = new Set(["html", "head", "body", "main", "style", "meta", "script"]);

const ZONE_LABEL_FALLBACKS: Record<string, string> = {
  "tournament.zoneLabelNav": "Thanh điều hướng",
  "tournament.zoneLabelKpi": "Khối chỉ số KPI",
  "tournament.zoneLabelTable": "Bảng dữ liệu",
  "tournament.zoneLabelHeader": "Phần đầu trang",
  "tournament.zoneLabelFooter": "Chân trang",
  "tournament.zoneLabelForm": "Biểu mẫu & Nút hành động",
  "tournament.zoneLabelCard": "Thẻ nổi bật",
  "tournament.zoneLabelSection": "Khối nội dung",
};

export interface DonorVisualZone {
  ffId: string;
  tag: string;
  labelKey: string;
  excerpt: string;
  previewHtml: string;
}

export interface CrossVariantZonePickerModalProps {
  isOpen: boolean;
  donorVariant: GeneratedVariant | null;
  initialSelectedFfIds: ReadonlyArray<string>;
  onClose: () => void;
  onApply: (selectedFfIds: string[]) => void;
}

export function getZoneLabelFallback(labelKey: string): string {
  return ZONE_LABEL_FALLBACKS[labelKey] ?? ZONE_LABEL_FALLBACKS["tournament.zoneLabelSection"];
}

const ZONE_LABEL_RULES: ReadonlyArray<{ key: string; tags: ReadonlyArray<string>; pattern: RegExp }> = [
  { key: "tournament.zoneLabelNav", tags: ["nav", "aside"], pattern: /nav|sidebar|rail|menu/ },
  { key: "tournament.zoneLabelKpi", tags: [], pattern: /kpi|metric|stat/ },
  { key: "tournament.zoneLabelTable", tags: ["table"], pattern: /table|ledger|grid/ },
  { key: "tournament.zoneLabelHeader", tags: ["header", "h1"], pattern: /hero|header|banner|heading/ },
  { key: "tournament.zoneLabelFooter", tags: ["footer"], pattern: /footer/ },
  { key: "tournament.zoneLabelForm", tags: ["form", "fieldset", "button"], pattern: /form|cta|btn|action/ },
  { key: "tournament.zoneLabelCard", tags: ["article", "figure"], pattern: /card|bento|spotlight|callout/ },
];

function inferZoneLabelKey(tag: string, ffId: string, className: string): string {
  const hint = `${ffId} ${className}`.toLowerCase();
  const matched = ZONE_LABEL_RULES.find((rule) => rule.tags.includes(tag) || rule.pattern.test(hint));
  return matched ? matched.key : "tournament.zoneLabelSection";
}

function buildMiniPreviewDoc(donorStyles: string, element: Element): string {
  const clone = element.cloneNode(true) as Element;
  clone.querySelectorAll("script").forEach((node) => node.remove());
  const baseReset =
    "html,body{box-sizing:border-box;margin:0;padding:10px;background:#ffffff;color:#0f172a;font-family:system-ui,-apple-system,sans-serif;pointer-events:none;overflow:hidden;}";
  return `<!doctype html><html><head><style>${baseReset}</style>${donorStyles}</head><body>${clone.outerHTML}</body></html>`;
}

function formatZoneExcerpt(rawText: string): string {
  const compact = rawText.replace(/\s+/g, " ").trim();
  if (compact.length <= MAX_EXCERPT_LEN) return compact;
  return `${compact.slice(0, MAX_EXCERPT_LEN - 1)}…`;
}

export function extractDonorVisualZones(html: string): DonorVisualZone[] {
  if (!html.trim() || typeof DOMParser === "undefined") return [];
  const doc = new DOMParser().parseFromString(html, "text/html");
  const donorStyles = Array.from(doc.querySelectorAll("style"))
    .map((styleEl) => `<style>${(styleEl.textContent ?? "").replace(/<\/style/gi, "")}</style>`)
    .join("");
  const seen = new Set<string>();
  const zones: DonorVisualZone[] = [];

  for (const el of Array.from(doc.querySelectorAll("[data-ff-id]"))) {
    const tag = el.tagName.toLowerCase();
    const ffId = (el.getAttribute("data-ff-id") ?? "").trim();
    if (!ffId || EXCLUDED_ZONE_TAGS.has(tag) || seen.has(ffId)) continue;
    seen.add(ffId);
    zones.push({
      ffId,
      tag,
      labelKey: inferZoneLabelKey(tag, ffId, el.getAttribute("class") ?? ""),
      excerpt: formatZoneExcerpt(el.textContent ?? ""),
      previewHtml: buildMiniPreviewDoc(donorStyles, el),
    });
  }
  return zones;
}

const OVERLAY_STYLE: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  zIndex: MODAL_Z_INDEX,
  backgroundColor: "var(--bg-overlay)",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  padding: "16px",
};

const DIALOG_STYLE: React.CSSProperties = {
  width: "min(720px, 100%)",
  maxHeight: "min(84vh, 720px)",
  display: "flex",
  flexDirection: "column",
  backgroundColor: "var(--bg-surface)",
  color: "var(--text-primary)",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-md)",
  boxShadow: "var(--shadow-lg)",
  overflow: "hidden",
};

const THUMB_BOX_STYLE: React.CSSProperties = {
  position: "relative",
  height: `${THUMB_HEIGHT_PX}px`,
  width: "100%",
  overflow: "hidden",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-subtle)",
  backgroundColor: "var(--bg-card)",
};

const THUMB_IFRAME_STYLE: React.CSSProperties = {
  width: "200%",
  height: "200%",
  transform: "scale(0.5)",
  transformOrigin: "top left",
  border: "none",
  pointerEvents: "none",
};

const ZonePickerHeader: React.FC<{
  titleId: string;
  donorLabel: string;
  onClose: () => void;
}> = ({ titleId, donorLabel, onClose }) => {
  const { t } = useTranslation();
  return (
    <header style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between", gap: "10px", padding: "14px 16px", borderBottom: "1px solid var(--border-default)", backgroundColor: "var(--bg-elevated, var(--bg-surface))" }}>
      <div style={{ display: "flex", flexDirection: "column", gap: "3px" }}>
        <h3 id={titleId} style={{ margin: 0, fontSize: "14px", fontWeight: FONT_SEMIBOLD, color: "var(--text-primary)" }}>
          {t("tournament.zonePickerTitle", "Chọn khối muốn mượn từ {{label}}", { label: donorLabel })}
        </h3>
        <p style={{ margin: 0, fontSize: "12px", color: "var(--text-secondary)", lineHeight: BODY_LINE_HEIGHT }}>
          {t("tournament.zonePickerSubtitle", "Tích chọn một hoặc nhiều khối bên dưới để ghép vào phương án đang chọn.")}
        </p>
      </div>
      <button
        type="button"
        aria-label={t("common.close", "Đóng")}
        data-testid="zone-picker-close-btn"
        onClick={onClose}
        style={{ display: "inline-flex", alignItems: "center", justifyContent: "center", padding: "5px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-card)", color: "var(--text-secondary)", cursor: "pointer" }}
      >
        <BrandIcon name="x" size={MODAL_ICON_SIZE} color="currentColor" />
      </button>
    </header>
  );
};

const ZonePreviewCard: React.FC<{
  zone: DonorVisualZone;
  isSelected: boolean;
  onToggle: (ffId: string) => void;
}> = ({ zone, isSelected, onToggle }) => {
  const { t } = useTranslation();
  const label = t(zone.labelKey, getZoneLabelFallback(zone.labelKey));
  return (
    <div
      role="button"
      tabIndex={0}
      aria-pressed={isSelected}
      data-testid={`zone-picker-card-${zone.ffId}`}
      onClick={() => onToggle(zone.ffId)}
      onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onToggle(zone.ffId); } }}
      style={{
        display: "flex", flexDirection: "column", gap: "8px", padding: "10px", borderRadius: "var(--radius-sm)",
        border: isSelected ? "1.5px solid var(--action-primary)" : "1px solid var(--border-default)",
        backgroundColor: isSelected ? "var(--bg-selected)" : "var(--bg-card)", cursor: "pointer", textAlign: "left",
      }}
    >
      <div style={THUMB_BOX_STYLE}>
        <iframe title={label} sandbox="" srcDoc={zone.previewHtml} style={THUMB_IFRAME_STYLE} />
      </div>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}>
        <label onClick={(e) => e.stopPropagation()} style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "12px", fontWeight: FONT_SEMIBOLD, color: "var(--text-primary)", cursor: "pointer", minWidth: 0 }}>
          <input type="checkbox" data-testid={`zone-picker-checkbox-${zone.ffId}`} checked={isSelected} onChange={() => onToggle(zone.ffId)} style={{ accentColor: "var(--action-primary)", cursor: "pointer" }} />
          <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{label}</span>
        </label>
        <span style={{ padding: "1px 6px", borderRadius: "999px", fontSize: "10px", fontWeight: FONT_MEDIUM, backgroundColor: "var(--bg-subtle, var(--bg-surface))", color: "var(--text-muted)", flexShrink: 0 }}>
          {`<${zone.tag}>`}
        </span>
      </div>
      {zone.excerpt && (
        <span style={{ fontSize: "11px", color: "var(--text-muted)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {zone.excerpt}
        </span>
      )}
    </div>
  );
};

const ZonePickerFooter: React.FC<{
  selectedCount: number;
  onCancel: () => void;
  onApply: () => void;
}> = ({ selectedCount, onCancel, onApply }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  return (
    <footer style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "10px", padding: "12px 16px", borderTop: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", flexWrap: "wrap" }}>
      <span data-testid="zone-picker-selected-count" style={{ padding: "3px 9px", borderRadius: "999px", fontSize: "11.5px", fontWeight: FONT_SEMIBOLD, backgroundColor: "var(--bg-selected)", color: "var(--action-primary)", fontVariantNumeric: "tabular-nums" }}>
        {t("tournament.zonePickerSelectedCount", "Đã chọn {{count}} khối", { count: formatNumber(selectedCount) })}
      </span>
      <div style={{ display: "inline-flex", alignItems: "center", gap: "8px" }}>
        <button type="button" data-testid="zone-picker-cancel-btn" onClick={onCancel} style={{ padding: "7px 12px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-card)", color: "var(--text-primary)", fontSize: "12px", fontWeight: FONT_SEMIBOLD, cursor: "pointer" }}>
          {t("tournament.zonePickerCancel", "Hủy")}
        </button>
        <button type="button" data-testid="zone-picker-apply-btn" onClick={onApply} style={{ padding: "7px 14px", borderRadius: "var(--radius-xs)", border: "none", backgroundColor: "var(--action-primary)", color: "var(--text-on-action)", fontSize: "12px", fontWeight: FONT_SEMIBOLD, cursor: "pointer" }}>
          {t("tournament.zonePickerApply", "Áp dụng")}
        </button>
      </div>
    </footer>
  );
};

function useZonePickerEscape(isOpen: boolean, onClose: () => void): void {
  useEffect(() => {
    if (!isOpen) return;
    const onKey = (ev: KeyboardEvent) => {
      if (ev.key !== "Escape" || ev.isComposing) return;
      ev.preventDefault();
      ev.stopPropagation();
      onClose();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [isOpen, onClose]);
}

export const CrossVariantZonePickerModal: React.FC<CrossVariantZonePickerModalProps> = ({
  isOpen,
  donorVariant,
  initialSelectedFfIds,
  onClose,
  onApply,
}) => {
  const titleId = useId();
  const { t } = useTranslation();
  const [draftSelected, setDraftSelected] = useState<string[]>([...initialSelectedFfIds]);
  const zones = useMemo(() => (donorVariant ? extractDonorVisualZones(donorVariant.html) : []), [donorVariant]);

  useEffect(() => {
    if (isOpen) setDraftSelected([...initialSelectedFfIds]);
  }, [donorVariant?.id, initialSelectedFfIds, isOpen]);

  useZonePickerEscape(isOpen && Boolean(donorVariant), onClose);

  if (!isOpen || !donorVariant) return null;

  const toggleZone = (ffId: string) =>
    setDraftSelected((prev) => (prev.includes(ffId) ? prev.filter((id) => id !== ffId) : [...prev, ffId]));

  const modalContent = (
    <div style={OVERLAY_STYLE} onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <div role="dialog" aria-modal="true" aria-labelledby={titleId} data-testid="cross-variant-zone-picker-modal" style={DIALOG_STYLE}>
        <ZonePickerHeader titleId={titleId} donorLabel={donorVariant.displayLabel} onClose={onClose} />
        <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "14px 16px" }}>
          {zones.length === 0 ? (
            <p style={{ margin: 0, fontSize: "12px", color: "var(--text-muted)" }}>
              {t("tournament.zonePickerEmpty", "Không tìm thấy khối giao diện có thể mượn trong phương án này.")}
            </p>
          ) : (
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(240px, 1fr))", gap: "12px" }}>
              {zones.map((zone) => (
                <ZonePreviewCard key={zone.ffId} zone={zone} isSelected={draftSelected.includes(zone.ffId)} onToggle={toggleZone} />
              ))}
            </div>
          )}
        </div>
        <ZonePickerFooter selectedCount={draftSelected.length} onCancel={onClose} onApply={() => onApply(draftSelected)} />
      </div>
    </div>
  );

  return typeof document !== "undefined" ? createPortal(modalContent, document.body) : modalContent;
};
