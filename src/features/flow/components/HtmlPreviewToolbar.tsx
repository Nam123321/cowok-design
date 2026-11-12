// @story 04.1
// @implements AC-03, AC-07
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";

const ICON_SIZE_SM = 12;
const ICON_SIZE_MD = 13;

const VIEWPORT_LABELS: Record<"desktop" | "tablet" | "mobile", string> = {
  desktop: "1440",
  tablet: "768",
  mobile: "390",
};

export interface HtmlPreviewToolbarProps {
  pointerMode: "interact" | "click-dom" | "drag-box";
  onSetPointerMode: (mode: "interact" | "click-dom" | "drag-box") => void;
  isInlineCodeOpen: boolean;
  onToggleInlineCode: () => void;
  allowDesignCdn: boolean;
  onToggleDesignCdn: () => void;
  viewportPreset: "desktop" | "tablet" | "mobile";
  onSetViewportPreset: (vp: "desktop" | "tablet" | "mobile") => void;
  aspectRatio: "auto" | "16-9" | "16-10";
  onSetAspectRatio: (ar: "auto" | "16-9" | "16-10") => void;
  isPresenting: boolean;
  onTogglePresenting: () => void;
  isLeftStructureHidden: boolean;
  onToggleLeftStructureHidden: () => void;
}

const PointerModeGroup: React.FC<{
  pointerMode: "interact" | "click-dom" | "drag-box";
  onSetPointerMode: (mode: "interact" | "click-dom" | "drag-box") => void;
}> = ({ pointerMode, onSetPointerMode }) => {
  const { t } = useTranslation();
  const modes: Array<{ id: "interact" | "click-dom" | "drag-box"; icon: string; label: string }> = [
    { id: "interact", icon: "cursor-click", label: t("canvas.modeInteract", "Tương tác trực tiếp với HTML") },
    { id: "click-dom", icon: "crosshair", label: t("canvas.modeClickComponent", "Click chọn Component (data-ff-id) để bình luận") },
    { id: "drag-box", icon: "frame", label: t("canvas.modeMarqueeBox", "Kéo khung chọn vùng (Marquee Box) đính kèm vào Chat") },
  ];
  return (
    <div style={{ display: "inline-flex", alignItems: "center", backgroundColor: "var(--bg-workspace)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-xs)", padding: "2px", gap: "2px" }}>
      {modes.map((m) => (
        <button key={m.id} type="button" onClick={() => onSetPointerMode(m.id)} title={m.label} aria-label={m.label} style={{ width: "22px", height: "22px", border: "none", borderRadius: "4px", backgroundColor: pointerMode === m.id ? "var(--bg-selected)" : "transparent", color: pointerMode === m.id ? "var(--text-primary)" : "var(--text-secondary)", cursor: "pointer", display: "inline-flex", alignItems: "center", justifyContent: "center" }}>
          <BrandIcon name={m.icon} size={ICON_SIZE_SM} color="currentColor" />
        </button>
      ))}
    </div>
  );
};

const ViewportRightControls: React.FC<HtmlPreviewToolbarProps> = (p) => {
  const { t } = useTranslation();
  const expandTitle = p.isLeftStructureHidden
    ? t("canvas.restoreSplitView", "Thu gọn về chế độ Chia đôi (Hiện lại khung Cấu trúc)")
    : t("canvas.expandHtmlPane", "Mở rộng toàn khung Live HTML Preview (Thu gọn khung Cấu trúc)");
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "5px" }}>
      <div style={{ display: "inline-flex", alignItems: "center", backgroundColor: "var(--bg-workspace)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-xs)", padding: "2px", gap: "2px" }}>
        {(["desktop", "tablet", "mobile"] as const).map((vp) => (
          <button key={vp} type="button" onClick={() => p.onSetViewportPreset(vp)} title={`Viewport: ${vp}`} style={{ height: "20px", padding: "0 6px", border: "none", borderRadius: "4px", fontSize: "10px", fontFamily: "var(--font-mono)", backgroundColor: p.viewportPreset === vp ? "var(--bg-selected)" : "transparent", color: p.viewportPreset === vp ? "var(--text-primary)" : "var(--text-muted)", fontWeight: p.viewportPreset === vp ? 600 : 400, cursor: "pointer" }}>
            {VIEWPORT_LABELS[vp]}
          </button>
        ))}
      </div>
      <select value={p.aspectRatio} onChange={(e) => p.onSetAspectRatio(e.target.value as "auto" | "16-9" | "16-10")} aria-label="Aspect Ratio" style={{ height: "24px", padding: "0 6px", fontSize: "10.5px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)", color: "var(--text-secondary)", cursor: "pointer" }}>
        <option value="auto">Auto Fit</option>
        <option value="16-9">16:9 Deck</option>
        <option value="16-10">16:10 Studio</option>
      </select>
      <button type="button" onClick={p.onTogglePresenting} className="cowok-btn cowok-btn-primary" style={{ height: "24px", padding: "0 9px", fontSize: "11px", fontWeight: 600, borderRadius: "var(--radius-xs)", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", cursor: "pointer" }}>
        <span>{p.isPresenting ? t("canvas.exitPresent", "✕ Thoát") : t("canvas.presentButton", "▶ Present")}</span>
      </button>
      <button type="button" onClick={p.onToggleLeftStructureHidden} title={expandTitle} aria-label={t("canvas.toggleStructurePane", "Thu gọn / Mở rộng khung Live HTML Preview")} className="cowok-btn cowok-btn-ghost" style={{ width: "28px", height: "28px", padding: 0, borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: p.isLeftStructureHidden ? "var(--bg-selected)" : "var(--bg-workspace)", color: p.isLeftStructureHidden ? "var(--text-primary)" : "var(--text-secondary)" }}>
        <BrandIcon name={p.isLeftStructureHidden ? "minimize" : "maximize"} size={ICON_SIZE_MD} color="currentColor" />
      </button>
    </div>
  );
};

export const HtmlPreviewToolbar: React.FC<HtmlPreviewToolbarProps> = (props) => {
  const { t } = useTranslation();
  const codeTitle = t("canvas.toggleCodeEditor", "Mở / Đóng trình sửa mã nguồn trực tiếp");
  const cdnBtnStyle: React.CSSProperties = {
    height: "24px", padding: "0 8px", fontSize: "10.5px", fontWeight: props.allowDesignCdn ? 600 : 500,
    borderRadius: "var(--radius-xs)", border: props.allowDesignCdn ? "1px solid var(--action-primary)" : "1px solid var(--border-default)",
    backgroundColor: props.allowDesignCdn ? "var(--bg-selected)" : "var(--bg-workspace)",
    color: props.allowDesignCdn ? "var(--text-primary)" : "var(--text-muted)", cursor: "pointer", whiteSpace: "nowrap",
  };

  return (
    <div className="html-pane-toolbar" style={{ height: "35px", minHeight: "35px", padding: "0 8px", backgroundColor: "var(--bg-surface)", borderBottom: "1px solid var(--border-default)", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "4px" }}>
        <PointerModeGroup pointerMode={props.pointerMode} onSetPointerMode={props.onSetPointerMode} />
        <button type="button" onClick={props.onToggleInlineCode} title={codeTitle} aria-label={codeTitle} style={{ width: "28px", height: "28px", border: "1px solid var(--border-default)", borderRadius: "var(--radius-xs)", backgroundColor: props.isInlineCodeOpen ? "var(--bg-selected)" : "var(--bg-workspace)", color: props.isInlineCodeOpen ? "var(--text-primary)" : "var(--text-secondary)", cursor: "pointer", display: "inline-flex", alignItems: "center", justifyContent: "center" }}>
          <BrandIcon name="code" size={ICON_SIZE_MD} color="currentColor" />
        </button>
        <button type="button" data-testid="toggle-design-cdn-btn" aria-pressed={props.allowDesignCdn} onClick={props.onToggleDesignCdn} title={t("canvas.toggleDesignCdnHint", "Cho phép nạp chỉ-đọc Google Fonts / Tailwind / unpkg / jsDelivr (vẫn chặn connect-src)")} style={cdnBtnStyle}>
          {props.allowDesignCdn ? t("canvas.designCdnOn", "Design CDN: Bật") : t("canvas.designCdnOff", "Bật Design CDN")}
        </button>
      </div>
      <ViewportRightControls {...props} />
    </div>
  );
};
