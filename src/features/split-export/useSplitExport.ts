// @story 07.2
// @implements AC-01, AC-02, AC-03, AC-04
import { useCallback, useEffect, useState } from "react";
import {
  apiSplitExport,
  SplitExportApiClient,
  SplitExportApiError,
} from "../../api/split-export";
import { useTranslation } from "../../hooks/useTranslation";
import type {
  DeveloperHandoffData,
  ExportHistoryItem,
  HtmlBundleExportData,
  PptxExportData,
  QcGateSummary,
  VectorPdfExportData,
} from "../../lib/validations/split-export";

export type SplitExportActiveFormat =
  | null
  | "html"
  | "pdf"
  | "pptx"
  | "developer-handoff";

export interface SplitExportErrorNotice {
  kind: "qc_blocked" | "size_exceeded" | "generic";
  p0Count?: number;
  message: string;
}

export interface UseSplitExportOptions {
  projectId?: string;
  projectTitle?: string;
  previewHtml?: string;
  enabled?: boolean;
  client?: SplitExportApiClient;
  autoDownload?: boolean;
}

const DEFAULT_QC_SUMMARY: QcGateSummary = {
  status: "UNSCANNED",
  p0Count: 0,
  p1Count: 0,
  latestRunId: null,
};

function triggerBrowserBlobDownload(blob: Blob, fileName: string): void {
  if (typeof document === "undefined" || typeof URL === "undefined") return;
  if (typeof URL.createObjectURL !== "function") return;
  const objectUrl = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = objectUrl;
  link.download = fileName;
  link.style.display = "none";
  document.body.appendChild(link);
  link.click();
  link.remove();
  if (typeof URL.revokeObjectURL === "function") {
    URL.revokeObjectURL(objectUrl);
  }
}

function escapeHtmlText(raw: string): string {
  return raw
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function decodeBase64ToUint8Array(base64: string): Uint8Array {
  const binary = globalThis.atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

export function useSplitExport(options: UseSplitExportOptions) {
  const {
    projectId,
    projectTitle,
    previewHtml = "",
    enabled = true,
    client = apiSplitExport,
    autoDownload = true,
  } = options;
  const { t } = useTranslation();
  const resolvedProjectTitle =
    projectTitle || t("splitExport.defaultProjectTitle", "Bản trình chiếu");

  const [qcGate, setQcGate] = useState<QcGateSummary>(DEFAULT_QC_SUMMARY);
  const [pptxPluginInstalled, setPptxPluginInstalled] = useState<boolean>(false);
  const [history, setHistory] = useState<ExportHistoryItem[]>([]);
  const [overrideQcWarning, setOverrideQcWarning] = useState<boolean>(false);
  const [activeFormat, setActiveFormat] = useState<SplitExportActiveFormat>(null);
  const [htmlResult, setHtmlResult] = useState<HtmlBundleExportData | null>(null);
  const [pdfResult, setPdfResult] = useState<VectorPdfExportData | null>(null);
  const [pptxResult, setPptxResult] = useState<PptxExportData | null>(null);
  const [devHandoffResult, setDevHandoffResult] = useState<DeveloperHandoffData | null>(null);
  const [errorNotice, setErrorNotice] = useState<SplitExportErrorNotice | null>(null);

  // Reset cross-project state & explicit QC override whenever projectId or modal enabled state changes
  useEffect(() => {
    setOverrideQcWarning(false);
    setQcGate(DEFAULT_QC_SUMMARY);
    setPptxPluginInstalled(false);
    setHistory([]);
    setErrorNotice(null);
    setHtmlResult(null);
    setPdfResult(null);
    setPptxResult(null);
    setDevHandoffResult(null);
  }, [projectId, enabled]);

  const refreshHistory = useCallback(
    async (signal?: AbortSignal) => {
      if (!projectId || !enabled) return;
      try {
        const res = await client.getExportHistory(projectId, 20, signal);
        if (signal?.aborted) return;
        setQcGate(res.data.qcGate);
        setPptxPluginInstalled(res.data.pptxPluginInstalled);
        setHistory(res.data.items);
      } catch {
        // Keep default unscanned state if history endpoint is temporarily unreachable or aborted
      }
    },
    [client, enabled, projectId]
  );

  useEffect(() => {
    if (!projectId || !enabled) return;
    const controller = new AbortController();
    void refreshHistory(controller.signal);
    return () => {
      controller.abort();
    };
  }, [enabled, projectId, refreshHistory]);

  const mapExportError = useCallback((err: unknown): SplitExportErrorNotice => {
    if (err instanceof SplitExportApiError) {
      if (err.status === 409 || err.code === "QC_GATE_P0_BLOCKED") {
        const count = err.p0Count ?? qcGate.p0Count;
        setQcGate((prev) => ({
          ...prev,
          status: "FAIL",
          p0Count: Math.max(prev.p0Count, count || 1),
        }));
        return {
          kind: "qc_blocked",
          p0Count: Math.max(count || 1, 1),
          message: err.message,
        };
      }
      if (err.status === 413 || err.code === "ASSET_SIZE_LIMIT_EXCEEDED") {
        return {
          kind: "size_exceeded",
          message: err.message,
        };
      }
      return { kind: "generic", message: err.message };
    }
    return {
      kind: "generic",
      message:
        err instanceof Error && err.message
          ? err.message
          : t(
              "splitExport.genericExportError",
              "Không thể đóng gói tệp lúc này. Vui lòng thử lại sau giây lát."
            ),
    };
  }, [qcGate.p0Count, t]);

  const runOfflineHtmlExport = useCallback(async () => {
    if (!projectId) return null;
    setActiveFormat("html");
    setErrorNotice(null);
    try {
      const fallbackBody = t(
        "splitExport.offlineHtmlFallbackBody",
        "Bản trình chiếu ngoại tuyến độc lập."
      );
      const res = await client.exportOfflineHtml({
        projectId,
        title: resolvedProjectTitle,
        html:
          previewHtml.trim() ||
          `<section data-slide="1"><h1>${escapeHtmlText(resolvedProjectTitle)}</h1><p>${escapeHtmlText(fallbackBody)}</p></section>`,
        enablePresenter: true,
        overrideQcWarning,
      });
      setHtmlResult(res.data);
      if (autoDownload) {
        const blob = new Blob([res.data.htmlContent], {
          type: "text/html;charset=utf-8",
        });
        triggerBrowserBlobDownload(blob, res.data.fileName);
      }
      void refreshHistory();
      return res.data;
    } catch (err) {
      setErrorNotice(mapExportError(err));
      return null;
    } finally {
      setActiveFormat(null);
    }
  }, [
    autoDownload,
    client,
    mapExportError,
    overrideQcWarning,
    previewHtml,
    projectId,
    resolvedProjectTitle,
    refreshHistory,
    t,
  ]);

  const runVectorPdfExport = useCallback(async () => {
    if (!projectId) return null;
    setActiveFormat("pdf");
    setErrorNotice(null);
    try {
      const res = await client.exportVectorPdf({
        projectId,
        title: resolvedProjectTitle,
        html: previewHtml || undefined,
        overrideQcWarning,
      });
      setPdfResult(res.data);
      if (autoDownload) {
        const bytes = decodeBase64ToUint8Array(res.data.pdfBase64);
        const pdfBlob = new Blob([bytes.buffer as ArrayBuffer], {
          type: "application/pdf",
        });
        triggerBrowserBlobDownload(pdfBlob, res.data.fileName);
      }
      void refreshHistory();
      return res.data;
    } catch (err) {
      setErrorNotice(mapExportError(err));
      return null;
    } finally {
      setActiveFormat(null);
    }
  }, [
    autoDownload,
    client,
    mapExportError,
    overrideQcWarning,
    previewHtml,
    projectId,
    resolvedProjectTitle,
    refreshHistory,
  ]);

  const runPptxExport = useCallback(async () => {
    if (!projectId) return null;
    setActiveFormat("pptx");
    setErrorNotice(null);
    try {
      const res = await client.exportPptx({
        projectId,
        title: resolvedProjectTitle,
        overrideQcWarning,
      });
      setPptxPluginInstalled(res.pluginInstalled);
      setPptxResult(res.data);
      void refreshHistory();
      return res.data;
    } catch (err) {
      setErrorNotice(mapExportError(err));
      return null;
    } finally {
      setActiveFormat(null);
    }
  }, [client, mapExportError, overrideQcWarning, projectId, resolvedProjectTitle, refreshHistory]);

  const runDeveloperHandoffExport = useCallback(async () => {
    if (!projectId) return null;
    setActiveFormat("developer-handoff");
    setErrorNotice(null);
    try {
      const res = await client.exportDeveloperHandoff({
        projectId,
        projectName: resolvedProjectTitle,
        htmlSource: previewHtml || undefined,
        overrideQcWarning,
      });
      setDevHandoffResult(res.data);
      if (autoDownload) {
        const payload = JSON.stringify(
          {
            fileName: res.data.fileName,
            sha256: res.data.sha256,
            files: res.data.files,
          },
          null,
          2
        );
        const blob = new Blob([payload], { type: "application/json;charset=utf-8" });
        triggerBrowserBlobDownload(blob, res.data.fileName);
      }
      void refreshHistory();
      return res.data;
    } catch (err) {
      setErrorNotice(mapExportError(err));
      return null;
    } finally {
      setActiveFormat(null);
    }
  }, [
    autoDownload,
    client,
    mapExportError,
    overrideQcWarning,
    previewHtml,
    projectId,
    resolvedProjectTitle,
    refreshHistory,
  ]);

  return {
    qcGate,
    pptxPluginInstalled,
    history,
    overrideQcWarning,
    setOverrideQcWarning,
    activeFormat,
    htmlResult,
    pdfResult,
    pptxResult,
    devHandoffResult,
    errorNotice,
    clearErrorNotice: () => setErrorNotice(null),
    refreshHistory,
    runOfflineHtmlExport,
    runVectorPdfExport,
    runPptxExport,
    runDeveloperHandoffExport,
  };
}
