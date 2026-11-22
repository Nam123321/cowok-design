// @story 04.2
// @implements AC-04
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import type {
  FsBrowseEntry,
  FsBrowseQuickRoot,
  FsBrowseResponse,
} from "../../../lib/validations/harness-daemon";

const FOLDER_ICON_SIZE = 14;
const SEMIBOLD_WEIGHT = 600;
const SUBFOLDER_FORBIDDEN_CHARS = /[/\\]/;

export function validateSubfolderName(raw: string): string | null {
  const trimmed = raw.trim();
  if (!trimmed) return null;
  if (trimmed === "." || trimmed === ".." || trimmed.startsWith(".") || SUBFOLDER_FORBIDDEN_CHARS.test(trimmed)) {
    return "INVALID_SUBFOLDER";
  }
  return null;
}

export function resolveBrowseTargetPath(
  browseData: FsBrowseResponse | null,
  selectedPath: string,
  newSubfolderName: string
): string | null {
  const baseDir = (selectedPath || browseData?.current_path || "").trim();
  if (!baseDir) return null;
  const cleanSub = newSubfolderName.trim();
  if (!cleanSub) return baseDir;
  const normalizedBase = baseDir.endsWith("/") ? baseDir.slice(0, -1) : baseDir;
  return `${normalizedBase}/${cleanSub}`;
}

function buildQuickRootsList(browseData: FsBrowseResponse | null): FsBrowseQuickRoot[] {
  if (browseData?.quick_roots && browseData.quick_roots.length > 0) {
    return browseData.quick_roots;
  }
  const fallback: FsBrowseQuickRoot[] = [];
  if (browseData?.home_dir) {
    fallback.push({ label: "Home (~)", path: browseData.home_dir });
  }
  if (browseData?.managed_root) {
    fallback.push({ label: "~/CowokProjects", path: browseData.managed_root });
  }
  return fallback;
}

const NativeFinderButton: React.FC<{
  disabled: boolean;
  isPickingNative: boolean;
  onClick: () => void;
}> = ({ disabled, isPickingNative, onClick }) => {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      data-testid="daemon-picker-native-finder-btn"
      disabled={disabled}
      onClick={onClick}
      className="cowok-btn cowok-btn-secondary"
      style={{
        display: "inline-flex", alignItems: "center", gap: "5px",
        fontSize: "11px", fontWeight: SEMIBOLD_WEIGHT, padding: "3px 9px",
        borderColor: "var(--action-primary)", color: "var(--action-primary)", flexShrink: 0,
      }}
    >
      <BrandIcon name="folder" size={FOLDER_ICON_SIZE} color="currentColor" />
      <span>
        {isPickingNative
          ? t("workbench.pickerNativeFinderOpening", "Đang mở Finder...")
          : t("workbench.pickerNativeFinderBtn", "Mở Finder...")}
      </span>
    </button>
  );
};

const QuickRootBar: React.FC<{
  roots: FsBrowseQuickRoot[];
  currentPath: string;
  isLoading: boolean;
  isPickingNative: boolean;
  onNavigate: (path?: string) => void;
  onOpenNativeFinder: () => void;
}> = ({ roots, currentPath, isLoading, isPickingNative, onNavigate, onOpenNativeFinder }) => {
  const isDisabled = isLoading || isPickingNative;
  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px", flexWrap: "wrap" }}>
      <div style={{ display: "flex", flexWrap: "wrap", gap: "4px" }}>
        {roots.map((root, idx) => {
          const isCurrent = currentPath === root.path;
          return (
            <button
              key={root.path} type="button" disabled={isDisabled} data-testid={`fs-quick-root-${idx}`}
              onClick={() => onNavigate(root.path)} className="cowok-btn cowok-btn-secondary"
              style={{
                fontSize: "10.5px", padding: "2px 8px",
                borderColor: isCurrent ? "var(--action-primary)" : undefined,
                backgroundColor: isCurrent ? "var(--bg-selected)" : undefined,
                fontWeight: isCurrent ? SEMIBOLD_WEIGHT : undefined,
              }}
            >
              {root.label}
            </button>
          );
        })}
      </div>
      <NativeFinderButton disabled={isDisabled} isPickingNative={isPickingNative} onClick={onOpenNativeFinder} />
    </div>
  );
};

const PathAddressBar: React.FC<{
  pathInput: string;
  parentPath: string | null | undefined;
  isLoading: boolean;
  onChangePathInput: (val: string) => void;
  onSubmitPath: () => void;
  onNavigateParent: () => void;
}> = ({ pathInput, parentPath, isLoading, onChangePathInput, onSubmitPath, onNavigateParent }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
      <input
        type="text"
        data-testid="daemon-picker-path-input"
        value={pathInput}
        onChange={(e) => onChangePathInput(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            onSubmitPath();
          }
        }}
        placeholder={t("workbench.pickerPathPlaceholder", "~/Desktop/my-project hoặc /Users/...")}
        className="cowok-input"
        style={{ flex: 1, fontSize: "11px", fontFamily: "var(--font-mono)", padding: "4px 8px" }}
      />
      <button
        type="button"
        data-testid="daemon-picker-go-btn"
        disabled={isLoading || !pathInput.trim()}
        onClick={onSubmitPath}
        className="cowok-btn cowok-btn-secondary"
        style={{ fontSize: "11px", padding: "4px 8px", flexShrink: 0 }}
      >
        {t("workbench.pickerGoBtn", "Đi tới")}
      </button>
      <button
        type="button"
        data-testid="daemon-picker-up-btn"
        disabled={!parentPath || isLoading}
        onClick={onNavigateParent}
        className="cowok-btn cowok-btn-secondary"
        style={{ fontSize: "11px", padding: "4px 8px", flexShrink: 0 }}
      >
        {t("workbench.pickerParentBtn", "Lên cấp")}
      </button>
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
        display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px",
        padding: "6px 8px", borderRadius: "var(--radius-xs)", cursor: "pointer",
        backgroundColor: isSelected ? "var(--bg-selected)" : "transparent",
        border: isSelected ? "1px solid var(--action-primary)" : "1px solid transparent",
      }}
    >
      <span style={{ display: "flex", alignItems: "center", gap: "6px", fontSize: "12px", color: "var(--text-primary)", minWidth: 0 }}>
        <BrandIcon name={entry.is_git_repo ? "folder-git" : "folder"} size={FOLDER_ICON_SIZE} color="var(--action-primary)" />
        <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{entry.name}</span>
      </span>
      <span style={{ display: "inline-flex", alignItems: "center", gap: "6px", flexShrink: 0 }}>
        {entry.is_git_repo && <span className="ff-metric-line">{t("workbench.pickerGitRepoBadge", "Git")}</span>}
        <button
          type="button"
          data-testid={`fs-open-${entry.name}`}
          onClick={(e) => {
            e.stopPropagation();
            onOpen();
          }}
          className="cowok-btn cowok-btn-secondary"
          style={{ fontSize: "10px", padding: "1px 6px" }}
        >
          {t("workbench.pickerOpenDirBtn", "Mở")}
        </button>
      </span>
    </div>
  );
};

const TargetFolderSummary: React.FC<{
  pickedDir: string | null;
  newSubfolderName: string;
  subfolderError: string | null;
  isHomeRootBlocked: boolean;
  onChangeSubfolder: (val: string) => void;
}> = ({ pickedDir, newSubfolderName, subfolderError, isHomeRootBlocked, onChangeSubfolder }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "6px", paddingTop: "4px", borderTop: "1px solid var(--border-subtle)" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <label htmlFor="pickerNewSubfolderInput" style={{ fontSize: "11px", color: "var(--text-secondary)", flexShrink: 0 }}>
          {t("workbench.pickerNewSubfolderLabel", "Tạo thư mục con (tùy chọn):")}
        </label>
        <input
          id="pickerNewSubfolderInput"
          type="text"
          data-testid="daemon-picker-new-subfolder-input"
          value={newSubfolderName}
          onChange={(e) => onChangeSubfolder(e.target.value)}
          placeholder={t("workbench.pickerNewSubfolderPlaceholder", "Ví dụ: my-new-app (bỏ trống để chọn thư mục đang chọn)")}
          className="cowok-input"
          style={{ flex: 1, fontSize: "11px", fontFamily: "var(--font-mono)", padding: "3px 8px" }}
        />
      </div>
      {subfolderError && (
        <div role="alert" style={{ fontSize: "11px", color: "var(--state-danger)" }}>
          {t("workbench.pickerInvalidSubfolderError", "Tên thư mục con không hợp lệ (không chứa '/', '\\' hoặc bắt đầu bằng '.')")}
        </div>
      )}
      {isHomeRootBlocked && (
        <div role="status" data-testid="daemon-picker-home-root-hint" style={{ fontSize: "11px", color: "var(--state-warning)" }}>
          {t("workbench.pickerHomeRootBlockedHint", "Không thể đặt dự án trực tiếp tại thư mục gốc Home (~). Hãy chọn một thư mục con hoặc nhập tên thư mục con mới.")}
        </div>
      )}
      {pickedDir && (
        <div className="ff-metric-line" data-testid="daemon-picker-target-preview" style={{ maxWidth: "100%", overflow: "hidden" }}>
          <span>{t("workbench.pickerSelectedTargetPrefix", "Thư mục đích:")}</span>
          <strong style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{pickedDir}</strong>
        </div>
      )}
    </div>
  );
};

export interface DaemonBrowseFsViewProps {
  browseData: FsBrowseResponse | null;
  selectedPath: string;
  pathInput: string;
  newSubfolderName: string;
  pickedDir: string | null;
  subfolderError: string | null;
  isHomeRootBlocked: boolean;
  isLoading: boolean;
  isPickingNative: boolean;
  onChangePathInput: (val: string) => void;
  onSubmitPath: () => void;
  onChangeSubfolder: (val: string) => void;
  onNavigate: (path?: string) => void;
  onSelectEntry: (path: string) => void;
  onOpenNativeFinder: () => void;
}

export const DaemonBrowseFsView: React.FC<DaemonBrowseFsViewProps> = (p) => {
  const { t } = useTranslation();
  const entries = p.browseData?.entries ?? [];
  const quickRoots = buildQuickRootsList(p.browseData);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
      <QuickRootBar
        roots={quickRoots}
        currentPath={p.browseData?.current_path || ""}
        isLoading={p.isLoading}
        isPickingNative={p.isPickingNative}
        onNavigate={p.onNavigate}
        onOpenNativeFinder={p.onOpenNativeFinder}
      />
      <PathAddressBar
        pathInput={p.pathInput}
        parentPath={p.browseData?.parent_path}
        isLoading={p.isLoading}
        onChangePathInput={p.onChangePathInput}
        onSubmitPath={p.onSubmitPath}
        onNavigateParent={() => p.browseData?.parent_path && p.onNavigate(p.browseData.parent_path)}
      />
      <div style={{ maxHeight: "200px", overflowY: "auto", border: "1px solid var(--border-subtle)", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-workspace)", padding: "4px" }}>
        {p.isLoading && <div style={{ padding: "12px", fontSize: "11.5px", color: "var(--text-muted)" }}>{t("workbench.pickerLoading", "Đang đọc thư mục qua Daemon...")}</div>}
        {!p.isLoading && p.browseData && entries.length === 0 && (
          <div style={{ padding: "12px", fontSize: "11.5px", color: "var(--text-muted)" }}>{t("workbench.pickerEmptyDir", "Thư mục trống — có thể chọn trực tiếp làm Workspace")}</div>
        )}
        {!p.isLoading && entries.map((entry) => (
          <BrowseFsEntryRow
            key={entry.path}
            entry={entry}
            isSelected={p.selectedPath === entry.path}
            onSelect={() => p.onSelectEntry(entry.path)}
            onOpen={() => p.onNavigate(entry.path)}
          />
        ))}
      </div>
      <TargetFolderSummary
        pickedDir={p.pickedDir}
        newSubfolderName={p.newSubfolderName}
        subfolderError={p.subfolderError}
        isHomeRootBlocked={p.isHomeRootBlocked}
        onChangeSubfolder={p.onChangeSubfolder}
      />
    </div>
  );
};
