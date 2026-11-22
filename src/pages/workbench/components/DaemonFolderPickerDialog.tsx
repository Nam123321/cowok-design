// @story 04.2
// @implements AC-04
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { useDaemonPickerState } from "../hooks/useDaemonPickerState";
import {
  extractDaemonErrorMessage,
  MANAGED_SLUG_REGEX,
  toManagedSlug,
} from "../hooks/useProjectTrashAndBrowse";
import {
  DaemonBrowseFsView,
  resolveBrowseTargetPath,
  validateSubfolderName,
} from "./DaemonBrowseFsView";

export interface DaemonFolderPickerDialogProps {
  isOpen: boolean;
  mode: "create" | "promote";
  projectId?: string;
  initialSlug?: string;
  initialTab?: "managed" | "browse";
  initialPath?: string;
  onClose: () => void;
  onSelectForCreate?: (selectedPath: string | null, managedSlug: string) => void;
  onConfirmPromote?: (projectId: string, targetDirectory: string) => Promise<void>;
}

const PICKER_Z_INDEX = 10000;
const HEADER_ICON_SIZE = 15;
const SEMIBOLD_WEIGHT = 600;
const MEDIUM_WEIGHT = 500;

const buildTabBtnStyle = (active: boolean): React.CSSProperties => ({
  padding: "8px", fontSize: "11.5px", fontWeight: active ? SEMIBOLD_WEIGHT : MEDIUM_WEIGHT, border: "none",
  borderBottom: active ? "2px solid var(--action-primary)" : "2px solid transparent",
  backgroundColor: active ? "var(--bg-selected)" : "transparent", color: "var(--text-primary)", cursor: "pointer",
});

const ManagedSlugTabView: React.FC<{
  slugInput: string;
  managedRoot: string;
  onChangeSlug: (val: string) => void;
}> = ({ slugInput, managedRoot, onChangeSlug }) => {
  const { t } = useTranslation();
  const isValid = MANAGED_SLUG_REGEX.test(slugInput.trim());
  const displayBase = managedRoot || "~/CowokProjects";
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
      <label htmlFor="pickerManagedSlugInput" style={{ fontSize: "11.5px", fontWeight: SEMIBOLD_WEIGHT, color: "var(--text-secondary)" }}>
        {t("workbench.managedSlugLabel", "Slug Workspace (~/CowokProjects/<slug>)")}
      </label>
      <input id="pickerManagedSlugInput" type="text" value={slugInput} onChange={(e) => onChangeSlug(e.target.value)} className="cowok-input" style={{ fontFamily: "var(--font-mono)" }} />
      <div style={{ fontSize: "11px", color: isValid ? "var(--text-muted)" : "var(--state-danger)" }}>
        {isValid ? t("workbench.managedSlugHint", "Chỉ gồm chữ thường a-z, số 0-9, '-' hoặc '_' (tối đa 63 ký tự)") : t("workbench.invalidSlugError", "Slug không hợp lệ. Phải khớp ^[a-z0-9][a-z0-9_-]{0,62}$")}
      </div>
      <div className="ff-metric-line" style={{ width: "fit-content" }}>
        <span>{t("workbench.managedSlugPreviewPrefix", "Đường dẫn chuẩn:")}</span>
        <strong>{`${displayBase}/${slugInput.trim() || "..."}`}</strong>
      </div>
    </div>
  );
};

function resolvePickedDirectory(s: ReturnType<typeof useDaemonPickerState>): string | null {
  const slug = s.slugInput.trim();
  if (s.activeTab === "managed") return s.browseData?.managed_root ? `${s.browseData.managed_root}/${slug}` : null;
  return resolveBrowseTargetPath(s.browseData, s.selectedPath, s.newSubfolderName);
}

function isExactHomeTarget(pickedDir: string | null, homeDir: string | null | undefined): boolean {
  if (!pickedDir || !homeDir) return false;
  const normPicked = pickedDir.replace(/\/+$/, "");
  const normHome = homeDir.replace(/\/+$/, "");
  return normPicked === normHome;
}

const PickerErrorBanner: React.FC<{ errorMsg: string; onResetRoot: () => void }> = ({ errorMsg, onResetRoot }) => {
  const { t } = useTranslation();
  return (
    <div role="alert" data-testid="daemon-picker-error" style={{ padding: "8px 10px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--state-danger-soft)", border: "1px solid var(--state-danger)", color: "var(--state-danger)", fontSize: "11.5px", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
      <span>{errorMsg}</span>
      <button type="button" onClick={onResetRoot} className="cowok-btn cowok-btn-secondary" style={{ fontSize: "10.5px", padding: "2px 6px", flexShrink: 0 }}>
        {t("workbench.pickerResetRootBtn", "Quay về ~/CowokProjects")}
      </button>
    </div>
  );
};

const PickerHeaderAndTabs: React.FC<{
  isManagedTab: boolean;
  onSelectTab: (tab: "managed" | "browse") => void;
  onClose: () => void;
}> = ({ isManagedTab, onSelectTab, onClose }) => {
  const { t } = useTranslation();
  return (
    <>
      <div style={{ padding: "12px 16px", borderBottom: "1px solid var(--border-subtle)", display: "flex", alignItems: "center", justifyContent: "space-between", backgroundColor: "var(--bg-workspace)" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "8px", fontSize: "13px", fontWeight: SEMIBOLD_WEIGHT, color: "var(--text-primary)" }}>
          <BrandIcon name="folder-git" size={HEADER_ICON_SIZE} color="var(--action-primary)" />
          <span>{t("workbench.pickerTitle", "Chọn Thư Mục Dự Án Qua Daemon")}</span>
        </div>
        <button type="button" onClick={onClose} aria-label={t("workbench.closeModal", "Đóng hộp thoại")} style={{ border: "none", background: "transparent", cursor: "pointer", color: "var(--text-muted)", fontSize: "16px" }}>×</button>
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", borderBottom: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-subtle)" }}>
        <button type="button" onClick={() => onSelectTab("managed")} style={buildTabBtnStyle(isManagedTab)}>{t("workbench.pickerCreateTab", "1-Click ~/CowokProjects/<slug>")}</button>
        <button type="button" onClick={() => onSelectTab("browse")} style={buildTabBtnStyle(!isManagedTab)}>{t("workbench.pickerBrowseTab", "Duyệt Thư Mục Máy Chủ")}</button>
      </div>
    </>
  );
};

const PickerFooterButtons: React.FC<{
  mode: "create" | "promote";
  canSubmit: boolean;
  onClose: () => void;
  onConfirm: () => void;
}> = ({ mode, canSubmit, onClose, onConfirm }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px", marginTop: "4px" }}>
      <button type="button" onClick={onClose} className="cowok-btn cowok-btn-secondary" style={{ padding: "6px 12px", fontSize: "12px" }}>
        {t("workbench.cancelBtn", "Hủy")}
      </button>
      <button id="confirmFolderPromoteBtn" type="button" disabled={!canSubmit} onClick={onConfirm} className="cowok-btn cowok-btn-primary" style={{ padding: "6px 14px", fontSize: "12px", fontWeight: SEMIBOLD_WEIGHT, backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)" }}>
        {mode === "create" ? t("workbench.pickerConfirmCreateBtn", "Chọn thư mục này") : t("workbench.pickerConfirmPromoteBtn", "Xác nhận & Gắn PathToken")}
      </button>
    </div>
  );
};

async function runPickerConfirm(args: {
  props: DaemonFolderPickerDialogProps;
  s: ReturnType<typeof useDaemonPickerState>;
  isManagedTab: boolean;
  pickedDir: string | null;
  t: (k: string, f: string) => string;
}) {
  const { props, s, isManagedTab, pickedDir, t } = args;
  s.setErrorMsg(null);
  if (props.mode === "create") {
    const slugToPass = isManagedTab
      ? s.slugInput.trim()
      : toManagedSlug(pickedDir?.split("/").filter(Boolean).pop() || s.slugInput);
    props.onSelectForCreate?.(pickedDir, slugToPass);
    props.onClose();
    return;
  }
  if (!props.projectId || !props.onConfirmPromote || !pickedDir) return;
  s.setIsSubmitting(true);
  try {
    await props.onConfirmPromote(props.projectId, pickedDir);
  } catch (err: unknown) {
    s.setErrorMsg(extractDaemonErrorMessage(err, t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")));
  } finally {
    s.setIsSubmitting(false);
  }
}

export const DaemonFolderPickerDialog: React.FC<DaemonFolderPickerDialogProps> = (props) => {
  const { t } = useTranslation();
  const s = useDaemonPickerState(props);
  if (!props.isOpen) return null;

  const isManagedTab = s.activeTab === "managed";
  const pickedDir = resolvePickedDirectory(s);
  const subfolderError = isManagedTab ? null : validateSubfolderName(s.newSubfolderName);
  const isHomeRootBlocked = !isManagedTab && isExactHomeTarget(pickedDir, s.browseData?.home_dir);
  const canSubmitBrowse = Boolean(pickedDir) && !subfolderError && !isHomeRootBlocked;
  const canSubmit = !s.isSubmitting && (isManagedTab ? MANAGED_SLUG_REGEX.test(s.slugInput.trim()) : canSubmitBrowse);

  return (
    <div id="daemonFolderPickerDialog" role="dialog" aria-modal="true" style={{ position: "fixed", inset: 0, zIndex: PICKER_Z_INDEX, backgroundColor: "var(--bg-overlay)", display: "flex", alignItems: "center", justifyContent: "center", padding: "16px" }}>
      <div style={{ width: "100%", maxWidth: "560px", backgroundColor: "var(--bg-surface)", borderRadius: "var(--radius-lg)", border: "1px solid var(--border-default)", boxShadow: "var(--shadow-lg)", overflow: "hidden" }}>
        <PickerHeaderAndTabs isManagedTab={isManagedTab} onSelectTab={s.setActiveTab} onClose={props.onClose} />
        <div style={{ padding: "16px", display: "flex", flexDirection: "column", gap: "12px" }}>
          {s.errorMsg && <PickerErrorBanner errorMsg={s.errorMsg} onResetRoot={() => void s.loadDirectory(undefined)} />}
          {isManagedTab ? (
            <ManagedSlugTabView slugInput={s.slugInput} managedRoot={s.browseData?.managed_root || "~/CowokProjects"} onChangeSlug={s.setSlugInput} />
          ) : (
            <DaemonBrowseFsView
              browseData={s.browseData} selectedPath={s.selectedPath} pathInput={s.pathInput}
              newSubfolderName={s.newSubfolderName} pickedDir={pickedDir} subfolderError={subfolderError}
              isHomeRootBlocked={isHomeRootBlocked} isLoading={s.isLoading} isPickingNative={s.isPickingNative}
              onChangePathInput={s.setPathInput} onSubmitPath={s.navigateToPathInput}
              onChangeSubfolder={s.setNewSubfolderName} onNavigate={(p) => void s.loadDirectory(p)}
              onSelectEntry={s.setSelectedPath} onOpenNativeFinder={() => void s.openNativeFinder()}
            />
          )}
          <PickerFooterButtons
            mode={props.mode} canSubmit={canSubmit} onClose={props.onClose}
            onConfirm={() => { if (canSubmit) void runPickerConfirm({ props, s, isManagedTab, pickedDir, t }); }}
          />
        </div>
      </div>
    </div>
  );
};
