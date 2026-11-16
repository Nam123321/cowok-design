// @story 04.1, 04.5
// @implements AC-01, AC-02, AC-03 (State-Hydrated Preview Toolbar)
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { normalizeViewportPreset, type FixtureVariant, type ViewportPreset } from "../../../lib/validations/screen-fixture";
import { PointerModeGroup, SubStateSwitcher } from "./HtmlPreviewToolbarControls";

const ICON_SIZE_SM = 12;
const ICON_SIZE_MD = 13;
type ToolbarViewportPreset = ViewportPreset | "desktop" | "tablet" | "mobile";

const VIEWPORT_LABELS: Record<ViewportPreset, string> = {
  "desktop-1440": "1440",
  "tablet-768": "768",
  "mobile-390": "390",
};

const VP_GROUP_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  backgroundColor: "var(--bg-workspace)",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-xs)",
  padding: "2px",
  gap: "2px",
};

export interface HtmlPreviewToolbarProps {
  pointerMode: "interact" | "click-dom" | "drag-box";
  onSetPointerMode: (mode: "interact" | "click-dom" | "drag-box") => void;
  activeSubState?: FixtureVariant;
  onSelectSubState?: (variant: FixtureVariant) => void;
  isInlineCodeOpen: boolean;
  dirtyBuffer?: boolean;
  onToggleInlineCode: () => void;
  viewportPreset: ToolbarViewportPreset;
  onSetViewportPreset: (vp: ViewportPreset) => void;
  aspectRatio: "auto" | "16-9" | "16-10";
  onSetAspectRatio: (ar: "auto" | "16-9" | "16-10") => void;
  isPresenting: boolean;
  onTogglePresenting: () => void;
  isLeftStructureHidden: boolean;
  onToggleLeftStructureHidden: () => void;
  isCommentsDrawerOpen?: boolean;
  commentCount?: number;
  onToggleCommentsDrawer?: () => void;
}

const ViewportRightControls: React.FC<HtmlPreviewToolbarProps> = (p) => {
  const { t } = useTranslation();
  const activePreset = normalizeViewportPreset(p.viewportPreset);
  const expandTitle = p.isLeftStructureHidden ? t("canvas.restoreSplitView", "Thu gọn về chế độ Chia đôi") : t("canvas.expandHtmlPane", "Mở rộng khung Live HTML Preview");
  const presentTitle = p.isPresenting ? t("canvas.exitPresent", "Thoát trình chiếu (Present)") : t("canvas.presentButton", "Trình chiếu (Present)");

  return (
    <div style={{ display: "flex", alignItems: "center", gap: "5px" }}>
      <div style={VP_GROUP_STYLE}>
        {(["desktop-1440", "tablet-768", "mobile-390"] as const).map((vp) => (
          <button key={vp} type="button" data-testid={`viewport-btn-${vp}`} onClick={() => p.onSetViewportPreset(vp)} title={`Viewport: ${vp}`} style={{ height: "20px", padding: "0 6px", border: "none", borderRadius: "4px", fontSize: "10px", fontFamily: "var(--font-mono)", backgroundColor: activePreset === vp ? "var(--bg-selected)" : "transparent", color: activePreset === vp ? "var(--text-primary)" : "var(--text-muted)", fontWeight: activePreset === vp ? 600 : 400, cursor: "pointer" }}>
            {VIEWPORT_LABELS[vp]}
          </button>
        ))}
      </div>
      <select value={p.aspectRatio} onChange={(e) => p.onSetAspectRatio(e.target.value as "auto" | "16-9" | "16-10")} aria-label={t("canvas.aspectRatio", "Tỷ lệ khung hình")} style={{ height: "24px", padding: "0 6px", fontSize: "10.5px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)", color: "var(--text-secondary)", cursor: "pointer" }}>
        <option value="auto">{t("canvas.aspectAutoFit", "Auto Fit")}</option>
        <option value="16-9">{t("canvas.aspectDeck169", "16:9 Deck")}</option>
        <option value="16-10">{t("canvas.aspectStudio1610", "16:10 Studio")}</option>
      </select>
      {p.onToggleCommentsDrawer && (
        <button type="button" data-testid="toggle-comments-drawer-btn" aria-pressed={Boolean(p.isCommentsDrawerOpen)} onClick={p.onToggleCommentsDrawer} title={t("comments.drawerTitle", "Bình luận ghim trên phần tử")} style={{ height: "24px", padding: "0 7px", display: "inline-flex", alignItems: "center", gap: "4px", borderRadius: "var(--radius-xs)", border: p.isCommentsDrawerOpen ? "1px solid var(--action-primary)" : "1px solid var(--border-default)", backgroundColor: p.isCommentsDrawerOpen ? "var(--bg-selected)" : "var(--bg-workspace)", color: p.isCommentsDrawerOpen ? "var(--text-primary)" : "var(--text-secondary)", fontSize: "10.5px", fontFamily: "var(--font-mono)", cursor: "pointer" }}>
          <BrandIcon name="message-square" size={ICON_SIZE_SM} color="currentColor" />
          <span>{p.commentCount ?? 0}</span>
        </button>
      )}
      <button type="button" data-testid="toggle-present-btn" onClick={p.onTogglePresenting} title={presentTitle} aria-label={presentTitle} aria-pressed={p.isPresenting} className="cowok-btn cowok-btn-primary" style={{ width: "28px", height: "28px", padding: 0, display: "inline-flex", alignItems: "center", justifyContent: "center", borderRadius: "var(--radius-xs)", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", cursor: "pointer" }}>
        <BrandIcon name={p.isPresenting ? "close" : "play"} size={ICON_SIZE_SM} color="currentColor" />
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

  return (
    <div className="html-pane-toolbar" style={{ minHeight: "36px", padding: "4px 8px", backgroundColor: "var(--bg-surface)", borderBottom: "1px solid var(--border-default)", display: "flex", alignItems: "center", justifyContent: "space-between", flexWrap: "wrap", gap: "6px" }}>
      <div style={{ display: "flex", alignItems: "center", flexWrap: "wrap", gap: "5px" }}>
        <PointerModeGroup pointerMode={props.pointerMode} onSetPointerMode={props.onSetPointerMode} />
        <SubStateSwitcher activeSubState={props.activeSubState} onSelectSubState={props.onSelectSubState} />
        <button type="button" data-testid="toggle-inline-code-btn" onClick={props.onToggleInlineCode} title={codeTitle} aria-label={codeTitle} style={{ position: "relative", width: "28px", height: "28px", border: "1px solid var(--border-default)", borderRadius: "var(--radius-xs)", backgroundColor: props.isInlineCodeOpen ? "var(--bg-selected)" : "var(--bg-workspace)", color: props.isInlineCodeOpen ? "var(--text-primary)" : "var(--text-secondary)", cursor: "pointer", display: "inline-flex", alignItems: "center", justifyContent: "center" }}>
          <BrandIcon name="code" size={ICON_SIZE_MD} color="currentColor" />
          {props.dirtyBuffer && <span data-testid="toolbar-dirty-indicator" style={{ position: "absolute", top: "3px", right: "3px", width: "6px", height: "6px", borderRadius: "50%", backgroundColor: "var(--agent-accent)" }} />}
        </button>
      </div>
      <ViewportRightControls {...props} />
    </div>
  );
};
