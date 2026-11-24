// @story 05.6
// @implements AC-04, AC-06 (Bidirectional Hidden Spec Drawer — RightDrawer [5.25] + FE Data Contract Tab + OCC 409 STALE_MAPPING)
import React, { useEffect, useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";
import type { FeDataContractDraft, HiddenSpecKey } from "../../lib/validations/cognitive-studio";
import type { useBrandStudioController } from "./useBrandStudioController";

type DrawerTab = "specs" | "fe-contract";
type Controller = ReturnType<typeof useBrandStudioController>;

const DRAWER_Z = 56;
const STROKE_MONO = 1.85;
const ICON_SM = 14;
const FONT_WEIGHT_SEMIBOLD = 600;
const COPY_FEEDBACK_MS = 1800;
const REVOKE_OBJECT_URL_MS = 1000;
const EDITOR_ROWS = 16;

export interface HiddenSpecDrawerProps {
  controller: Controller;
}

const FeContractFieldsTable: React.FC<{ draft: FeDataContractDraft }> = ({ draft }) => (
  <div style={{ border: "1px solid var(--border-default)", borderRadius: "var(--radius-xs)", overflow: "hidden" }}>
    <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "11.5px" }}>
      <thead>
        <tr style={{ backgroundColor: "var(--bg-elevated)", borderBottom: "1px solid var(--border-default)", textAlign: "left" }}>
          <th style={{ padding: "6px 8px" }}>Field</th>
          <th style={{ padding: "6px 8px" }}>Type</th>
          <th style={{ padding: "6px 8px" }}>ff-id</th>
        </tr>
      </thead>
      <tbody>
        {draft.viewModelFields.map((f) => (
          <tr key={f.ffId} style={{ borderBottom: "1px solid var(--border-default)" }}>
            <td style={{ padding: "6px 8px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>{f.fieldName}</td>
            <td style={{ padding: "6px 8px", color: "var(--text-secondary)" }}>{f.inferredType}</td>
            <td style={{ padding: "6px 8px", color: "var(--text-secondary)" }}>{f.ffId}</td>
          </tr>
        ))}
      </tbody>
    </table>
  </div>
);

function triggerFeContractDownload(draftTsSource: string): boolean {
  if (typeof document === "undefined" || typeof URL === "undefined" || typeof URL.createObjectURL !== "function") {
    return false;
  }
  const content = draftTsSource || "// No screen fields extracted yet.\n";
  const blob = new Blob([content], { type: "text/typescript;charset=utf-8" });
  const href = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = href;
  anchor.download = "fe-data-contract.draft.ts";
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  setTimeout(() => URL.revokeObjectURL(href), REVOKE_OBJECT_URL_MS);
  return true;
}

const ICON_BTN_STYLE = (active: boolean): React.CSSProperties => ({
  width: "28px",
  height: "28px",
  padding: 0,
  borderRadius: "var(--radius-xs)",
  border: active ? "1px solid var(--action-primary)" : "1px solid var(--border-default)",
  backgroundColor: active ? "var(--bg-selected)" : "var(--bg-surface)",
  color: active ? "var(--action-primary)" : "var(--text-secondary)",
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  cursor: "pointer",
});

const FeContractActionsRow: React.FC<{
  diskPath: string;
  folderStatus: string | null;
  copied: boolean;
  onCopy: () => void;
  onOpenFolder: () => void;
}> = ({ diskPath, folderStatus, copied, onCopy, onOpenFolder }) => {
  const { t } = useTranslation();
  const copyTooltip = copied
    ? t("brandStudio.copiedBtn", "Đã sao chép")
    : t("brandStudio.copyFeContractTooltip", "Sao chép nội dung fe-data-contract.draft.ts");
  const openFolderTooltip = t("brandStudio.openSpecsFolderTooltip", "Mở thư mục chứa file trên máy (.cowok-design/specs)");
  return (
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "8px" }}>
      <div style={{ display: "flex", flexDirection: "column", minWidth: 0 }}>
        <span style={{ fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>fe-data-contract.draft.ts</span>
        <span data-testid="fe-contract-disk-path" style={{ fontSize: "10.5px", fontFamily: "var(--font-mono)", color: "var(--text-muted)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {folderStatus ?? diskPath}
        </span>
      </div>
      <div style={{ display: "inline-flex", alignItems: "center", gap: "6px", flexShrink: 0 }}>
        <button type="button" data-testid="copy-fe-contract-btn" title={copyTooltip} aria-label={copyTooltip} onClick={onCopy} style={ICON_BTN_STYLE(copied)}>
          <BrandIcon name={copied ? "check" : "copy"} size={ICON_SM} strokeWidth={STROKE_MONO} color="currentColor" />
        </button>
        <button type="button" data-testid="open-fe-contract-folder-btn" title={openFolderTooltip} aria-label={openFolderTooltip} onClick={onOpenFolder} style={ICON_BTN_STYLE(Boolean(folderStatus))}>
          <BrandIcon name="folder-open" size={ICON_SM} strokeWidth={STROKE_MONO} color="currentColor" />
        </button>
      </div>
    </div>
  );
};

const FeDataContractTabView: React.FC<{
  draft: FeDataContractDraft | null;
  draftTsSource: string;
  diskPath: string;
  onOpenFolder: (fileName?: string) => Promise<{ opened: boolean; path: string } | null>;
}> = ({ draft, draftTsSource, diskPath, onOpenFolder }) => {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  const [folderStatus, setFolderStatus] = useState<string | null>(null);

  const handleOpenFolder = async () => {
    const result = await onOpenFolder("fe-data-contract.draft.ts");
    if (result) setFolderStatus(result.opened ? t("brandStudio.folderOpenedHint", "Đã mở thư mục chứa file") : result.path);
    else if (triggerFeContractDownload(draftTsSource)) setFolderStatus(t("brandStudio.exportedBtn", "Đã tải bản sao .draft.ts"));
    setTimeout(() => setFolderStatus(null), COPY_FEEDBACK_MS);
  };

  const handleCopy = async () => {
    try {
      if (typeof navigator !== "undefined" && navigator.clipboard) await navigator.clipboard.writeText(draftTsSource);
      setCopied(true);
      setTimeout(() => setCopied(false), COPY_FEEDBACK_MS);
    } catch { /* clipboard unavailable */ }
  };

  return (
    <div data-testid="fe-data-contract-panel" style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
      <div role="note" data-testid="fe-contract-boundary-banner" style={{ padding: "8px 10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--action-primary)", backgroundColor: "var(--bg-elevated)", fontSize: "11.5px", color: "var(--text-primary)", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
        {draft?.boundaryNotice ?? "FE_REQUIRED_VIEWMODEL_AND_ACTIONS_ONLY"}
      </div>
      {draft && <FeContractFieldsTable draft={draft} />}
      <FeContractActionsRow diskPath={diskPath} folderStatus={folderStatus} copied={copied} onCopy={() => { void handleCopy(); }} onOpenFolder={() => { void handleOpenFolder(); }} />
      <pre data-testid="fe-contract-ts-preview" style={{ margin: 0, padding: "10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-canvas)", color: "var(--text-primary)", fontSize: "11px", overflowX: "auto", maxHeight: "260px" }}>
        {draftTsSource || "// No screen fields extracted yet."}
      </pre>
    </div>
  );
};

const DrawerHeader: React.FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  return (
    <header style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "12px 14px", borderBottom: "1px solid var(--border-default)", backgroundColor: "var(--bg-elevated)" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <BrandIcon name="file-text" size={ICON_SM} strokeWidth={STROKE_MONO} color="var(--action-primary)" />
        <strong style={{ fontSize: "13.5px", color: "var(--text-primary)" }}>
          {t("brandStudio.hiddenSpecsTitle", "Đặc tả ẩn 01..09 & FE Contract")}
        </strong>
      </div>
      <button type="button" aria-label={t("common.close", "Đóng")} data-testid="close-hidden-spec-drawer-btn" onClick={onClose} style={{ height: "28px", width: "28px", display: "inline-flex", alignItems: "center", justifyContent: "center", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-secondary)", cursor: "pointer" }}>
        <BrandIcon name="x" size={ICON_SM} strokeWidth={STROKE_MONO} />
      </button>
    </header>
  );
};

const DrawerTabs: React.FC<{ activeTab: DrawerTab; onChange: (t: DrawerTab) => void }> = ({ activeTab, onChange }) => {
  const { t } = useTranslation();
  const tabBtnStyle = (selected: boolean): React.CSSProperties => ({
    height: "26px",
    padding: "0 10px",
    borderRadius: "var(--radius-pill)",
    border: selected ? "1px solid var(--action-primary)" : "1px solid var(--border-default)",
    backgroundColor: "var(--bg-surface)",
    color: selected ? "var(--action-primary)" : "var(--text-secondary)",
    fontSize: "11.5px",
    fontWeight: FONT_WEIGHT_SEMIBOLD,
    cursor: "pointer",
  });
  return (
    <div role="tablist" style={{ display: "flex", gap: "6px", padding: "8px 14px", borderBottom: "1px solid var(--border-default)" }}>
      <button type="button" role="tab" aria-selected={activeTab === "specs"} data-testid="drawer-tab-specs" onClick={() => onChange("specs")} style={tabBtnStyle(activeTab === "specs")}>
        {t("brandStudio.tabSpecs0109", "Đặc tả 01..09")}
      </button>
      <button type="button" role="tab" aria-selected={activeTab === "fe-contract"} data-testid="drawer-tab-fe-contract" onClick={() => onChange("fe-contract")} style={tabBtnStyle(activeTab === "fe-contract")}>
        {t("brandStudio.tabFeContract", "FE Data Contract")}
      </button>
    </div>
  );
};

const HiddenSpecsEditorPane: React.FC<{ hiddenSpec: Controller["hiddenSpec"] }> = ({ hiddenSpec }) => {
  const { t } = useTranslation();
  const activeDoc = hiddenSpec.specs.find((s) => s.specKey === hiddenSpec.selectedSpecKey);
  return (
    <>
      <select aria-label={t("brandStudio.selectSpecLabel", "Chọn tài liệu đặc tả")} data-testid="hidden-spec-select" value={hiddenSpec.selectedSpecKey} onChange={(e) => hiddenSpec.selectSpecKey(e.target.value as HiddenSpecKey)} style={{ height: "32px", padding: "0 8px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-elevated)", color: "var(--text-primary)", fontSize: "12px" }}>
        {hiddenSpec.specs.map((s) => (
          <option key={s.specKey} value={s.specKey}>{`${s.title} (rev ${s.revision})`}</option>
        ))}
      </select>
      {hiddenSpec.occConflict && (
        <div role="alert" data-testid="hidden-spec-occ-conflict-banner" style={{ padding: "8px 10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--state-error)", backgroundColor: "var(--bg-elevated)", color: "var(--state-error)", fontSize: "11.5px", display: "flex", flexDirection: "column", gap: "6px" }}>
          <span>{`STALE_MAPPING (rev ${hiddenSpec.occConflict.expectedRevision} vs ${hiddenSpec.occConflict.currentRevision})`}</span>
          <button type="button" data-testid="reload-hidden-spec-btn" onClick={() => { void hiddenSpec.loadHiddenSpecs(); }} style={{ alignSelf: "flex-start", height: "24px", padding: "0 10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--state-error)", backgroundColor: "var(--bg-surface)", color: "var(--state-error)", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer" }}>
            {t("brandStudio.reloadLatestBtn", "Tải lại bản mới")}
          </button>
        </div>
      )}
      <textarea data-testid="hidden-spec-markdown-editor" value={hiddenSpec.draftMarkdown} onChange={(e) => hiddenSpec.setDraftMarkdown(e.target.value)} rows={EDITOR_ROWS} style={{ width: "100%", flex: 1, minHeight: "280px", padding: "10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-canvas)", color: "var(--text-primary)", fontFamily: "var(--font-mono, monospace)", fontSize: "12px", boxSizing: "border-box" }} />
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <span style={{ fontSize: "11px", color: "var(--text-secondary)" }}>{`rev ${activeDoc?.revision ?? 0}`}</span>
        <button type="button" data-testid="save-hidden-spec-btn" disabled={hiddenSpec.isSavingSpec} onClick={() => { void hiddenSpec.saveActiveSpec(); }} style={{ height: "30px", padding: "0 14px", borderRadius: "var(--radius-xs)", border: "1px solid var(--action-primary)", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer" }}>
          {t("brandStudio.saveSpecBtn", "Lưu đặc tả")}
        </button>
      </div>
    </>
  );
};

export const HiddenSpecDrawer: React.FC<HiddenSpecDrawerProps> = ({ controller }) => {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<DrawerTab>("specs");

  useEffect(() => {
    if (!controller.isHiddenSpecOpen) return;
    const onKeyDown = (evt: KeyboardEvent) => {
      if (evt.key === "Escape") controller.closeHiddenSpec();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [controller.isHiddenSpecOpen, controller.closeHiddenSpec]);

  if (!controller.isHiddenSpecOpen) return null;

  return (
    <aside role="dialog" aria-label={t("brandStudio.hiddenSpecsTitle", "Đặc tả ẩn 01..09")} data-testid="hidden-spec-drawer" style={{ position: "fixed", top: 0, right: 0, bottom: 0, width: "min(480px, 100vw)", zIndex: DRAWER_Z, backgroundColor: "var(--bg-surface)", borderLeft: "1px solid var(--border-default)", boxShadow: "var(--shadow-lg)", display: "flex", flexDirection: "column" }}>
      <DrawerHeader onClose={controller.closeHiddenSpec} />
      <DrawerTabs activeTab={activeTab} onChange={setActiveTab} />
      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "14px", display: "flex", flexDirection: "column", gap: "10px" }}>
        {activeTab === "specs" ? (
          <HiddenSpecsEditorPane hiddenSpec={controller.hiddenSpec} />
        ) : (
          <FeDataContractTabView
            draft={controller.feContract}
            draftTsSource={controller.feContractTs}
            diskPath={controller.feContractDiskPath}
            onOpenFolder={controller.openSpecsFolder}
          />
        )}
      </div>
    </aside>
  );
};
