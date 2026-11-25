// @story 07.2, 07.3
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
  PptxExportMode,
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

const HTTP_CONFLICT = 409;
const HTTP_PAYLOAD_TOO_LARGE = 413;
const DEFAULT_HISTORY_LIMIT = 20;
const JSON_INDENT_SPACES = 2;
const PPTX_MIME =
  "application/vnd.openxmlformats-officedocument.presentationml.presentation";

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

function downloadBase64Blob(base64: string, mimeType: string, fileName: string): void {
  const bytes = decodeBase64ToUint8Array(base64);
  const blob = new Blob([bytes.buffer as ArrayBuffer], { type: mimeType });
  triggerBrowserBlobDownload(blob, fileName);
}

function buildFallbackDeckHtml(
  previewHtml: string,
  title: string,
  fallbackBody: string
): string {
  const trimmed = previewHtml.trim();
  if (trimmed) return trimmed;
  return `<section data-slide="1"><h1>${escapeHtmlText(title)}</h1><p>${escapeHtmlText(fallbackBody)}</p></section>`;
}

function toSplitExportErrorNotice(
  err: unknown,
  currentP0Count: number,
  genericFallback: string,
  onQcBlocked: (p0Count: number) => void
): SplitExportErrorNotice {
  if (err instanceof SplitExportApiError) {
    if (err.status === HTTP_CONFLICT || err.code === "QC_GATE_P0_BLOCKED") {
      const count = Math.max(err.p0Count ?? currentP0Count, 1);
      onQcBlocked(count);
      return { kind: "qc_blocked", p0Count: count, message: err.message };
    }
    if (err.status === HTTP_PAYLOAD_TOO_LARGE || err.code === "ASSET_SIZE_LIMIT_EXCEEDED") {
      return { kind: "size_exceeded", message: err.message };
    }
    return { kind: "generic", message: err.message };
  }
  const message = err instanceof Error && err.message ? err.message : genericFallback;
  return { kind: "generic", message };
}

interface ExportRunContext {
  projectId?: string;
  resolvedTitle: string;
  previewHtml: string;
  overrideQcWarning: boolean;
  locale: string;
  pptxExportMode: PptxExportMode;
  autoDownload: boolean;
  client: SplitExportApiClient;
  t: (key: string, defaultVal?: string) => string;
  setActiveFormat: (f: SplitExportActiveFormat) => void;
  setErrorNotice: (n: SplitExportErrorNotice | null) => void;
  setHtmlResult: (d: HtmlBundleExportData | null) => void;
  setPdfResult: (d: VectorPdfExportData | null) => void;
  setPptxResult: (d: PptxExportData | null) => void;
  setPptxPluginInstalled: (v: boolean) => void;
  setPptxExportMode: (m: PptxExportMode) => void;
  setDevHandoffResult: (d: DeveloperHandoffData | null) => void;
  mapError: (err: unknown) => SplitExportErrorNotice;
  refreshHistory: () => Promise<void>;
}

async function runGuardedExport<T>(
  ctx: ExportRunContext,
  format: Exclude<SplitExportActiveFormat, null>,
  work: (projectId: string) => Promise<T>
): Promise<T | null> {
  if (!ctx.projectId) return null;
  ctx.setActiveFormat(format);
  ctx.setErrorNotice(null);
  try {
    const result = await work(ctx.projectId);
    void ctx.refreshHistory();
    return result;
  } catch (err) {
    ctx.setErrorNotice(ctx.mapError(err));
    return null;
  } finally {
    ctx.setActiveFormat(null);
  }
}

function executeOfflineHtml(ctx: ExportRunContext) {
  return runGuardedExport(ctx, "html", async (projectId) => {
    const fallback = ctx.t("splitExport.offlineHtmlFallbackBody", "Bản trình chiếu ngoại tuyến độc lập.");
    const res = await ctx.client.exportOfflineHtml({
      projectId,
      title: ctx.resolvedTitle,
      html: buildFallbackDeckHtml(ctx.previewHtml, ctx.resolvedTitle, fallback),
      enablePresenter: true,
      overrideQcWarning: ctx.overrideQcWarning,
    });
    ctx.setHtmlResult(res.data);
    if (ctx.autoDownload) {
      triggerBrowserBlobDownload(
        new Blob([res.data.htmlContent], { type: "text/html;charset=utf-8" }),
        res.data.fileName
      );
    }
    return res.data;
  });
}

function executeVectorPdf(ctx: ExportRunContext) {
  return runGuardedExport(ctx, "pdf", async (projectId) => {
    const res = await ctx.client.exportVectorPdf({
      projectId,
      title: ctx.resolvedTitle,
      html: ctx.previewHtml || undefined,
      overrideQcWarning: ctx.overrideQcWarning,
    });
    ctx.setPdfResult(res.data);
    if (ctx.autoDownload) {
      downloadBase64Blob(res.data.pdfBase64, "application/pdf", res.data.fileName);
    }
    return res.data;
  });
}

function executePptx(ctx: ExportRunContext, modeOverride?: PptxExportMode) {
  return runGuardedExport(ctx, "pptx", async (projectId) => {
    const effectiveMode = modeOverride ?? ctx.pptxExportMode;
    if (modeOverride) ctx.setPptxExportMode(modeOverride);
    const fallback = ctx.t("splitExport.offlineHtmlFallbackBody", "Bản trình chiếu ngoại tuyến độc lập.");
    const res = await ctx.client.exportPptx({
      projectId,
      title: ctx.resolvedTitle,
      html: buildFallbackDeckHtml(ctx.previewHtml, ctx.resolvedTitle, fallback),
      exportMode: effectiveMode,
      locale: ctx.locale,
      overrideQcWarning: ctx.overrideQcWarning,
    });
    ctx.setPptxPluginInstalled(res.pluginInstalled);
    ctx.setPptxResult(res.data);
    if (ctx.autoDownload && res.data.pptxBase64 && res.data.fileName) {
      downloadBase64Blob(res.data.pptxBase64, PPTX_MIME, res.data.fileName);
    }
    return res.data;
  });
}

function executeDeveloperHandoff(ctx: ExportRunContext) {
  return runGuardedExport(ctx, "developer-handoff", async (projectId) => {
    const res = await ctx.client.exportDeveloperHandoff({
      projectId,
      projectName: ctx.resolvedTitle,
      htmlSource: ctx.previewHtml || undefined,
      overrideQcWarning: ctx.overrideQcWarning,
    });
    ctx.setDevHandoffResult(res.data);
    if (ctx.autoDownload) {
      const payload = JSON.stringify(
        { fileName: res.data.fileName, sha256: res.data.sha256, files: res.data.files },
        null,
        JSON_INDENT_SPACES
      );
      triggerBrowserBlobDownload(
        new Blob([payload], { type: "application/json;charset=utf-8" }),
        res.data.fileName
      );
    }
    return res.data;
  });
}

function useSplitExportState(projectId?: string, enabled = true) {
  const [qcGate, setQcGate] = useState<QcGateSummary>(DEFAULT_QC_SUMMARY);
  const [pptxPluginInstalled, setPptxPluginInstalled] = useState<boolean>(true);
  const [pptxExportMode, setPptxExportMode] = useState<PptxExportMode>("editable");
  const [history, setHistory] = useState<ExportHistoryItem[]>([]);
  const [overrideQcWarning, setOverrideQcWarning] = useState<boolean>(false);
  const [activeFormat, setActiveFormat] = useState<SplitExportActiveFormat>(null);
  const [htmlResult, setHtmlResult] = useState<HtmlBundleExportData | null>(null);
  const [pdfResult, setPdfResult] = useState<VectorPdfExportData | null>(null);
  const [pptxResult, setPptxResult] = useState<PptxExportData | null>(null);
  const [devHandoffResult, setDevHandoffResult] = useState<DeveloperHandoffData | null>(null);
  const [errorNotice, setErrorNotice] = useState<SplitExportErrorNotice | null>(null);

  useEffect(() => {
    setOverrideQcWarning(false);
    setQcGate(DEFAULT_QC_SUMMARY);
    setPptxPluginInstalled(true);
    setPptxExportMode("editable");
    setHistory([]);
    setErrorNotice(null);
    setHtmlResult(null);
    setPdfResult(null);
    setPptxResult(null);
    setDevHandoffResult(null);
  }, [projectId, enabled]);

  return {
    qcGate, setQcGate, pptxPluginInstalled, setPptxPluginInstalled,
    pptxExportMode, setPptxExportMode, history, setHistory,
    overrideQcWarning, setOverrideQcWarning, activeFormat, setActiveFormat,
    htmlResult, setHtmlResult, pdfResult, setPdfResult,
    pptxResult, setPptxResult, devHandoffResult, setDevHandoffResult,
    errorNotice, setErrorNotice,
  };
}

interface SplitExportHistoryParams {
  projectId?: string;
  enabled: boolean;
  client: SplitExportApiClient;
  setQcGate: (v: QcGateSummary) => void;
  setPptxPluginInstalled: (v: boolean) => void;
  setHistory: (v: ExportHistoryItem[]) => void;
}

function useSplitExportHistory({
  projectId,
  enabled,
  client,
  setQcGate,
  setPptxPluginInstalled,
  setHistory,
}: SplitExportHistoryParams) {
  const refreshHistory = useCallback(async (signal?: AbortSignal) => {
    if (!projectId || !enabled) return;
    try {
      const res = await client.getExportHistory(projectId, DEFAULT_HISTORY_LIMIT, signal);
      if (signal?.aborted) return;
      setQcGate(res.data.qcGate);
      setPptxPluginInstalled(res.data.pptxPluginInstalled);
      setHistory(res.data.items);
    } catch {
      // Keep default state when history endpoint is temporarily unreachable or aborted
    }
  }, [client, enabled, projectId, setHistory, setPptxPluginInstalled, setQcGate]);

  useEffect(() => {
    if (!projectId || !enabled) return;
    const controller = new AbortController();
    void refreshHistory(controller.signal);
    return () => controller.abort();
  }, [enabled, projectId, refreshHistory]);

  return refreshHistory;
}

export function useSplitExport(options: UseSplitExportOptions) {
  const { projectId, projectTitle, previewHtml = "", enabled = true, client = apiSplitExport, autoDownload = true } = options;
  const { t, locale } = useTranslation();
  const resolvedTitle = projectTitle || t("splitExport.defaultProjectTitle", "Bản trình chiếu");
  const st = useSplitExportState(projectId, enabled);
  const { setQcGate, setPptxPluginInstalled, setHistory } = st;
  const refreshHistory = useSplitExportHistory({ projectId, enabled, client, setQcGate, setPptxPluginInstalled, setHistory });

  const mapError = useCallback((err: unknown) => toSplitExportErrorNotice(
    err,
    st.qcGate.p0Count,
    t("splitExport.genericExportError", "Không thể đóng gói tệp lúc này. Vui lòng thử lại sau giây lát."),
    (count) => setQcGate((prev) => ({ ...prev, status: "FAIL", p0Count: Math.max(prev.p0Count, count) }))
  ), [setQcGate, st.qcGate.p0Count, t]);

  const runCtx: ExportRunContext = {
    projectId, resolvedTitle, previewHtml, overrideQcWarning: st.overrideQcWarning, locale,
    pptxExportMode: st.pptxExportMode, autoDownload, client, t,
    setActiveFormat: st.setActiveFormat, setErrorNotice: st.setErrorNotice,
    setHtmlResult: st.setHtmlResult, setPdfResult: st.setPdfResult,
    setPptxResult: st.setPptxResult, setPptxPluginInstalled, setPptxExportMode: st.setPptxExportMode,
    setDevHandoffResult: st.setDevHandoffResult, mapError, refreshHistory,
  };

  return {
    qcGate: st.qcGate, pptxPluginInstalled: st.pptxPluginInstalled, pptxExportMode: st.pptxExportMode,
    setPptxExportMode: st.setPptxExportMode, history: st.history, overrideQcWarning: st.overrideQcWarning,
    setOverrideQcWarning: st.setOverrideQcWarning, activeFormat: st.activeFormat, htmlResult: st.htmlResult,
    pdfResult: st.pdfResult, pptxResult: st.pptxResult, devHandoffResult: st.devHandoffResult,
    errorNotice: st.errorNotice, clearErrorNotice: () => st.setErrorNotice(null), refreshHistory,
    runOfflineHtmlExport: () => executeOfflineHtml(runCtx), runVectorPdfExport: () => executeVectorPdf(runCtx),
    runPptxExport: (mode?: PptxExportMode) => executePptx(runCtx, mode),
    runDeveloperHandoffExport: () => executeDeveloperHandoff(runCtx),
  };
}
