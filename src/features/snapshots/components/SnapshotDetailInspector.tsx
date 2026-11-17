// @story 05.3
import React, { useMemo, useState } from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import type {
  ChatArtifactSnapshotResponse,
  VersionSnapshotResponse,
} from "../../../lib/validations/version-snapshot";
import { parseTurnManifestFiles } from "../utils/snapshot-engine";

export type SelectedHistoryEntry =
  | { kind: "snapshot"; item: VersionSnapshotResponse }
  | { kind: "turn"; item: ChatArtifactSnapshotResponse };

export type DetailViewMode = "html" | "userflow" | "files";

export interface SnapshotDetailInspectorProps {
  selected: SelectedHistoryEntry;
  artifactFormat?: "dag" | "slide-cms" | "md" | "assets" | "code";
  isLocked?: boolean;
  onCloseDetail: () => void;
  onRequestRevertSnapshot: (snapshot: VersionSnapshotResponse) => void;
  onPreviewTurn?: (turn: ChatArtifactSnapshotResponse) => void;
  onForkTurn?: (turn: ChatArtifactSnapshotResponse) => void;
}

const HASH_SHORT_LEN = 12;
const MAX_FLOW_PREVIEW_CARDS = 4;

function escapeHtmlText(text: string): string {
  return text.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}

function extractPreviewHtml(selected: SelectedHistoryEntry): string {
  if (selected.kind === "turn") {
    const files = parseTurnManifestFiles(selected.item.manifestJson);
    const htmlFile = files.find((f) => f.path.endsWith(".html")) || files[0];
    if (htmlFile?.content) return htmlFile.content;
    const label = escapeHtmlText(selected.item.nodeId || selected.item.label || "Snapshot");
    return `<main style="font-family:sans-serif;padding:24px"><h3>${label}</h3><p>${files.length} files captured</p></main>`;
  }
  const snap = selected.item;
  const htmlFile = snap.files.find((f) => f.path.endsWith(".html")) || snap.files[0];
  if (htmlFile?.content) return htmlFile.content;
  return `<main style="font-family:sans-serif;padding:24px"><h3>${escapeHtmlText(snap.label)}</h3><p>${escapeHtmlText(snap.description || "")}</p></main>`;
}

function resolveFileList(selected: SelectedHistoryEntry): Array<{ path: string; size: number }> {
  if (selected.kind === "turn") {
    return parseTurnManifestFiles(selected.item.manifestJson).map((f) => ({ path: f.path, size: f.content.length }));
  }
  return selected.item.files.map((f) => ({ path: f.path, size: f.sizeBytes ?? f.content.length }));
}

const DetailHeaderSwitcher: React.FC<{
  title: string;
  hash: string;
  isSlideDeck: boolean;
  effectiveMode: DetailViewMode;
  fileCount: number;
  onSetMode: (m: DetailViewMode) => void;
  onCloseDetail: () => void;
}> = ({ title, hash, isSlideDeck, effectiveMode, fileCount, onSetMode, onCloseDetail }) => {
  const { t } = useTranslation();
  return (
    <div className="snap-detail-header">
      <div className="snap-item-tag">{title} · <code>{hash}</code></div>
      <div className="snap-item-actions">
        <div className="snap-detail-switcher" role="tablist" data-testid="detail-view-switcher">
          <button type="button" role="tab" aria-selected={effectiveMode === "html"} className={`snap-btn snap-btn--compact ${effectiveMode === "html" ? "snap-btn--primary" : "snap-btn--ghost"}`} data-testid="detail-mode-html" onClick={() => onSetMode("html")}>
            {isSlideDeck ? t("snapshots.slidePreviewTab", "Slide 16:9") : t("snapshots.htmlPreviewTab", "HTML")}
          </button>
          {!isSlideDeck && (
            <button type="button" role="tab" aria-selected={effectiveMode === "userflow"} className={`snap-btn snap-btn--compact ${effectiveMode === "userflow" ? "snap-btn--primary" : "snap-btn--ghost"}`} data-testid="detail-mode-userflow" onClick={() => onSetMode("userflow")}>
              {t("snapshots.userflowTab", "Userflow")}
            </button>
          )}
          <button type="button" role="tab" aria-selected={effectiveMode === "files"} className={`snap-btn snap-btn--compact ${effectiveMode === "files" ? "snap-btn--primary" : "snap-btn--ghost"}`} data-testid="detail-mode-files" onClick={() => onSetMode("files")}>
            {t("snapshots.filesTab", "Files")} ({fileCount})
          </button>
        </div>
        <button type="button" className="snap-drawer-close-btn" onClick={onCloseDetail} title={t("common.close", "Close")}>✕</button>
      </div>
    </div>
  );
};

const DetailStageViewport: React.FC<{
  title: string;
  previewDoc: string;
  selected: SelectedHistoryEntry;
  effectiveMode: DetailViewMode;
  isSlideDeck: boolean;
  files: Array<{ path: string; size: number }>;
}> = ({ title, previewDoc, selected, effectiveMode, isSlideDeck, files }) => {
  const { t } = useTranslation();
  const nodeName = selected.kind === "turn" ? (selected.item.nodeId || "SCR-01") : selected.item.label;
  return (
    <div className="snap-detail-stage">
      {effectiveMode === "html" && (
        <iframe title={title} sandbox="allow-scripts" srcDoc={previewDoc} className="snap-detail-iframe" data-testid="detail-html-iframe" />
      )}
      {effectiveMode === "userflow" && !isSlideDeck && (
        <div className="snap-flow-preview-grid" data-testid="detail-userflow-preview">
          <div className="snap-flow-preview-node">
            <strong>{nodeName}</strong>
            <span className="snap-meta-label">{t("snapshots.flowSnapshotReadonly", "Captured screen graph state (read-only)")}</span>
          </div>
          {files.slice(0, MAX_FLOW_PREVIEW_CARDS).map((f) => (
            <div key={f.path} className="snap-flow-preview-node">
              <code>{f.path}</code>
              <span className="snap-meta-label">{f.size} {t("snapshots.charsUnit", "chars")}</span>
            </div>
          ))}
        </div>
      )}
      {effectiveMode === "files" && (
        <ul className="snap-checklist" data-testid="detail-files-list">
          {files.map((f) => (
            <li key={f.path} className="snap-checklist-item">
              <code>{f.path}</code>
              <span>{f.size} {t("snapshots.charsUnit", "chars")}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};

const DetailFooterActions: React.FC<{
  selected: SelectedHistoryEntry;
  isLocked: boolean;
  onRequestRevertSnapshot: (snapshot: VersionSnapshotResponse) => void;
  onPreviewTurn?: (turn: ChatArtifactSnapshotResponse) => void;
  onForkTurn?: (turn: ChatArtifactSnapshotResponse) => void;
}> = ({ selected, isLocked, onRequestRevertSnapshot, onPreviewTurn, onForkTurn }) => {
  const { t } = useTranslation();
  return (
    <div className="snap-detail-footer">
      {selected.kind === "turn" ? (
        <>
          {onPreviewTurn && (
            <button type="button" className="snap-btn snap-btn--compact" onClick={() => onPreviewTurn(selected.item)}>
              {t("snapshots.previewTurnBtn", "Preview on Stage")}
            </button>
          )}
          {onForkTurn && (
            <button type="button" className="snap-btn snap-btn--primary snap-btn--compact" onClick={() => onForkTurn(selected.item)}>
              {t("snapshots.forkTurnBtn", "Fork Session")}
            </button>
          )}
        </>
      ) : (
        <button type="button" disabled={isLocked} className="snap-btn snap-btn--primary snap-btn--compact" data-testid="detail-revert-btn" onClick={() => onRequestRevertSnapshot(selected.item)}>
          {t("snapshots.revertBtn", "Revert")}
        </button>
      )}
    </div>
  );
};

export const SnapshotDetailInspector: React.FC<SnapshotDetailInspectorProps> = ({
  selected,
  artifactFormat = "dag",
  isLocked = false,
  onCloseDetail,
  onRequestRevertSnapshot,
  onPreviewTurn,
  onForkTurn,
}) => {
  const isSlideDeck = artifactFormat === "slide-cms";
  const [viewMode, setViewMode] = useState<DetailViewMode>("html");
  const effectiveMode: DetailViewMode = isSlideDeck && viewMode === "userflow" ? "html" : viewMode;
  const files = useMemo(() => resolveFileList(selected), [selected]);
  const previewDoc = useMemo(() => extractPreviewHtml(selected), [selected]);
  const title = selected.kind === "snapshot"
    ? selected.item.label
    : (selected.item.label || `Turn ${selected.item.contentHash.slice(0, HASH_SHORT_LEN)}`);
  const hash = (selected.kind === "snapshot" ? selected.item.treeSha256 : selected.item.contentHash).slice(0, HASH_SHORT_LEN);

  return (
    <section className="snap-detail-pane" data-testid="version-detail-pane" aria-label={title}>
      <DetailHeaderSwitcher title={title} hash={hash} isSlideDeck={isSlideDeck} effectiveMode={effectiveMode} fileCount={files.length} onSetMode={setViewMode} onCloseDetail={onCloseDetail} />
      <DetailStageViewport title={title} previewDoc={previewDoc} selected={selected} effectiveMode={effectiveMode} isSlideDeck={isSlideDeck} files={files} />
      <DetailFooterActions selected={selected} isLocked={isLocked} onRequestRevertSnapshot={onRequestRevertSnapshot} onPreviewTurn={onPreviewTurn} onForkTurn={onForkTurn} />
    </section>
  );
};
