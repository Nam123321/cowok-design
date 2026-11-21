// @story 04.6
// @implements AC-02, AC-04 (CasDropzonePanel [5.8] with Binary Intake & Paste Guard)
import React, { useState } from "react";
import { apiChatVault } from "@/api/chat-vault";
import { useTranslation } from "@/hooks/useTranslation";
import type { CasAttachmentDto } from "@/lib/validations/chat-vault";
import { ALLOWED_CHAT_ATTACHMENT_ACCEPT } from "./read-selected-chat-files";

export interface CasDropzonePanelProps {
  sessionId: string;
  projectId?: string | null;
  onUploaded?: (attachment: CasAttachmentDto) => void;
  onError?: (message: string) => void;
}

const DROPZONE_BASE_STYLE: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: "8px",
  padding: "8px 10px",
  borderRadius: "var(--radius-sm)",
  border: "1px dashed var(--border-strong)",
  backgroundColor: "var(--bg-canvas)",
  color: "var(--text-secondary)",
  fontSize: "11px",
};

async function runCasUploadBatch(params: {
  files: File[];
  sessionId: string;
  projectId?: string | null;
  setBusy: (b: boolean) => void;
  fallbackError: string;
  onUploaded?: (attachment: CasAttachmentDto) => void;
  onError?: (message: string) => void;
}): Promise<void> {
  if (params.files.length === 0) return;
  params.setBusy(true);
  try {
    for (const file of params.files) {
      const dto = await apiChatVault.uploadAttachment({
        sessionId: params.sessionId,
        projectId: params.projectId ?? null,
        file,
        filename: file.name,
      });
      params.onUploaded?.(dto);
    }
  } catch (err) {
    params.onError?.(err instanceof Error ? err.message : params.fallbackError);
  } finally {
    params.setBusy(false);
  }
}

export const CasDropzonePanel: React.FC<CasDropzonePanelProps> = (props) => {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const trigger = (files: File[]) =>
    !busy &&
    void runCasUploadBatch({
      files,
      sessionId: props.sessionId,
      projectId: props.projectId,
      setBusy,
      fallbackError: t("cas.uploadFailed", "Upload failed"),
      onUploaded: props.onUploaded,
      onError: props.onError,
    });

  return (
    <label
      data-ff-id="cas-file-dropzone"
      data-testid="cas-file-dropzone"
      onDragOver={(e) => e.preventDefault()}
      onDrop={(e) => { e.preventDefault(); trigger(Array.from(e.dataTransfer?.files || [])); }}
      onPaste={(e) => {
        const files = Array.from(e.clipboardData?.files || []);
        if (files.length === 0) return;
        e.preventDefault();
        trigger(files);
      }}
      style={{ ...DROPZONE_BASE_STYLE, cursor: busy ? "wait" : "pointer" }}
    >
      <span>{busy ? t("cas.uploading", "Uploading to CAS...") : t("cas.prompt", "Attach .csv / .tsv / .xlsx / .fig / .pdf / .png / .svg / .txt")}</span>
      <input
        type="file"
        multiple
        data-testid="cas-dropzone-file-input"
        accept={ALLOWED_CHAT_ATTACHMENT_ACCEPT}
        onChange={(e) => { const f = e.target.files ? Array.from(e.target.files) : []; e.target.value = ""; trigger(f); }}
        style={{ display: "none" }}
      />
    </label>
  );
};
