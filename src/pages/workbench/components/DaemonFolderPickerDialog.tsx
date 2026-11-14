// @story 04.2
// @implements AC-04
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import type { FsBrowseEntry, FsBrowseResponse } from "../../../lib/validations/harness-daemon";
import { useDaemonPickerState } from "../hooks/useDaemonPickerState";
import {
  extractDaemonErrorMessage,
  MANAGED_SLUG_REGEX,
} from "../hooks/useProjectTrashAndBrowse";

export interface DaemonFolderPickerDialogProps {
  isOpen: boolean;
  mode: "create" | "promote";
  projectId?: string;
  initialSlug?: string;
  onClose: () => void;
  onSelectForCreate?: (selectedPath: string | null, managedSlug: string) => void;
  onConfirmPromote?: (projectId: string, targetDirectory: string) => Promise<void>;
}

const PICKER_Z_INDEX = 10000;
const FOLDER_ICON_SIZE = 14;
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

const BrowseFsEntryRow: React.FC<{
  entry: FsBrowseEntry;
  isSelected: boolean;
  onSelect: () => void;
  onOpen: () => void;
}> = ({ entry, isSelected, onSelect, onOpen }) => {
  const { t } = useTranslation();
  return (
    <div
      role="button"
      onClick={onSelect}
      onDoubleClick={onOpen}
      data-testid={`fs-entry-${entry.name}`}
      style={{
        display: "flex", alignItems: "center", justifyContent: "space-between", padding: "6px 8px", borderRadius: "var(--radius-xs)",
        cursor: "pointer", backgroundColor: isSelected ? "var(--bg-selected)" : "transparent",
        border: isSelected ? "1px solid var(--action-primary)" : "1px solid transparent",
      }}
    >
      <span style={{ display: "flex", alignItems: "center", gap: "6px", fontSize: "12px", color: "var(--text-primary)" }}>
        <BrandIcon name={entry.is_git_repo ? "folder-git" : "folder"} size={FOLDER_ICON_SIZE} color="var(--action-primary)" />
        <span>{entry.name}</span>
      </span>
      {entry.is_git_repo && <span className="ff-metric-line">{t("workbench.pickerGitRepoBadge", "Git")}</span>}
    </div>
  );
};

const BrowseFsTabView: React.FC<{
  browseData: FsBrowseResponse | null;
  selectedPath: string;
  isLoading: boolean;
  onNavigate: (path?: string) => void;
  onSelectEntry: (path: string) => void;
}> = ({ browseData, selectedPath, isLoading, onNavigate, onSelectEntry }) => {
  const { t } = useTranslation();
  const entries = browseData?.entries ?? [];
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}>
        <span style={{ fontSize: "11px", fontFamily: "var(--font-mono)", color: "var(--text-secondary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {browseData?.current_path || t("workbench.pickerLoading", "Đang đọc thư mục qua Daemon...")}
        </span>
        <div style={{ display: "flex", gap: "4px", flexShrink: 0 }}>
          <button type="button" disabled={!browseData?.parent_path || isLoading} onClick={() => browseData?.parent_path && onNavigate(browseData.parent_path)} className="cowok-btn cowok-btn-secondary" style={{ fontSize: "11px", padding: "3px 8px" }}>
            {t("workbench.pickerParentBtn", "Lên cấp")}
          </button>
          <button type="button" disabled={isLoading} onClick={() => onNavigate(undefined)} className="cowok-btn cowok-btn-secondary" style={{ fontSize: "11px", padding: "3px 8px" }}>
            {t("workbench.pickerResetRootBtn", "Quay về ~/CowokProjects")}
          </button>
        </div>
      </div>
      <div style={{ maxHeight: "210px", overflowY: "auto", border: "1px solid var(--border-subtle)", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-workspace)", padding: "4px" }}>
        {isLoading && <div style={{ padding: "12px", fontSize: "11.5px", color: "var(--text-muted)" }}>{t("workbench.pickerLoading", "Đang đọc thư mục qua Daemon...")}</div>}
        {!isLoading && browseData && entries.length === 0 && (
          <div style={{ padding: "12px", fontSize: "11.5px", color: "var(--text-muted)" }}>{t("workbench.pickerEmptyDir", "Thư mục trống — có thể chọn trực tiếp làm Workspace")}</div>
        )}
        {!isLoading && entries.map((entry) => (
          <BrowseFsEntryRow key={entry.path} entry={entry} isSelected={selectedPath === entry.path} onSelect={() => onSelectEntry(entry.path)} onOpen={() => onNavigate(entry.path)} />
        ))}
      </div>
    </div>
  );
};

function resolvePickedDirectory(s: ReturnType<typeof useDaemonPickerState>): string | null {
  const slug = s.slugInput.trim();
  if (s.activeTab === "managed") return s.browseData?.managed_root ? `${s.browseData.managed_root}/${slug}` : null;
  return s.selectedPath || s.browseData?.current_path || null;
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

export const DaemonFolderPickerDialog: React.FC<DaemonFolderPickerDialogProps> = (props) => {
  const { t } = useTranslation();
  const s = useDaemonPickerState(props);
  if (!props.isOpen) return null;

  const isManagedTab = s.activeTab === "managed";
  const pickedDir = resolvePickedDirectory(s);
  const canSubmit = !s.isSubmitting && (isManagedTab ? MANAGED_SLUG_REGEX.test(s.slugInput.trim()) : Boolean(pickedDir));

  const handleConfirm = async () => {
    if (!canSubmit) return;
    s.setErrorMsg(null);
    if (props.mode === "create") {
      props.onSelectForCreate?.(pickedDir, s.slugInput.trim());
      props.onClose();
      return;
    }
    if (!props.projectId || !props.onConfirmPromote || !pickedDir) return;
    s.setIsSubmitting(true);
    try { await props.onConfirmPromote(props.projectId, pickedDir); } catch (err: unknown) {
      s.setErrorMsg(extractDaemonErrorMessage(err, t("workbench.daemonActionError", "Lỗi đồng bộ Daemon SQLite V7")));
    } finally { s.setIsSubmitting(false); }
  };

  return (
    <div id="daemonFolderPickerDialog" role="dialog" aria-modal="true" style={{ position: "fixed", inset: 0, zIndex: PICKER_Z_INDEX, backgroundColor: "var(--bg-overlay)", display: "flex", alignItems: "center", justifyContent: "center", padding: "16px" }}>
      <div style={{ width: "100%", maxWidth: "540px", backgroundColor: "var(--bg-surface)", borderRadius: "var(--radius-lg)", border: "1px solid var(--border-default)", boxShadow: "var(--shadow-lg)", overflow: "hidden" }}>
        <PickerHeaderAndTabs isManagedTab={isManagedTab} onSelectTab={s.setActiveTab} onClose={props.onClose} />
        <div style={{ padding: "16px", display: "flex", flexDirection: "column", gap: "12px" }}>
          {s.errorMsg && <PickerErrorBanner errorMsg={s.errorMsg} onResetRoot={() => void s.loadDirectory(undefined)} />}
          {isManagedTab ? <ManagedSlugTabView slugInput={s.slugInput} managedRoot={s.browseData?.managed_root || "~/CowokProjects"} onChangeSlug={s.setSlugInput} /> : <BrowseFsTabView browseData={s.browseData} selectedPath={s.selectedPath} isLoading={s.isLoading} onNavigate={(p) => void s.loadDirectory(p)} onSelectEntry={s.setSelectedPath} />}
          <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px", marginTop: "4px" }}>
            <button type="button" onClick={props.onClose} className="cowok-btn cowok-btn-secondary" style={{ padding: "6px 12px", fontSize: "12px" }}>{t("workbench.cancelBtn", "Hủy")}</button>
            <button id="confirmFolderPromoteBtn" type="button" disabled={!canSubmit} onClick={() => void handleConfirm()} className="cowok-btn cowok-btn-primary" style={{ padding: "6px 14px", fontSize: "12px", fontWeight: SEMIBOLD_WEIGHT, backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)" }}>
              {props.mode === "create" ? t("workbench.pickerConfirmCreateBtn", "Chọn thư mục này") : t("workbench.pickerConfirmPromoteBtn", "Xác nhận & Gắn PathToken")}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
