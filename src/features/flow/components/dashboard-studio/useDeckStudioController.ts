// @story 06.5
// @implements AC-02, AC-03, AC-04, AC-05 (Orchestration Controller v2 for Option D Dashboard Studio)
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { generateDeck, normalizeChartSource } from "../../../../api/deck-generation";
import type { CowokToastTone } from "../../../../components/ui/CowokToast";
import { useFormatter } from "../../../../hooks/useFormatter";
import { useTranslation } from "../../../../hooks/useTranslation";
import type {
  GenerateDeckResponse,
  NormalizeChartSourceResponse,
} from "../../../../lib/validations/deck-generation";
import {
  buildChartSourcePayload,
  buildEmbeddableDashboardHtml,
  buildMainChatSyncSummary,
  computeSourceDivergence,
  formatDeckGenError,
  formatMetricValue,
  normalizeResponseToRows,
  parseRawDelimitedRows,
  readSelectedChartFile,
} from "./studioDataUtils";
import {
  applyAiLens,
  applyWhatIfDelta,
  autoFixHonestDataIssues,
  buildRowCallout,
  computeRowDeltaPct,
  deriveBackendBrief,
  mapFfIdToControlView,
} from "./studioStorytellingUtils";
import { applySubChatCommand } from "./studioSubChatEngine";
import {
  createInitialSnapshot,
  type StudioAiLensId,
  type StudioControlView,
  type StudioDataRow,
  type StudioInteractionMode,
  type StudioSnapshot,
  type SubChatMessage,
  type UploadedFileState,
} from "./types";
import { useStudioHistory } from "./useStudioHistory";
import { useStudioLayout } from "./useStudioLayout";

export interface ToastNotice {
  tone: CowokToastTone;
  message: string;
}

export interface StudioControllerParams {
  isOpen: boolean;
  projectId: string;
  aspectRatio: "16:9" | "16:10" | "Auto";
  onClose: () => void;
  onDeckGenerated?: (response: GenerateDeckResponse) => void;
  onSyncToMainChat?: (summaryText: string) => void;
  onEmbedIntoActiveSlide?: (htmlSnippet: string, title: string) => void;
}

interface SourceActionDeps {
  projectId: string;
  snapshot: StudioSnapshot;
  commit: (updater: (prev: StudioSnapshot) => StudioSnapshot) => void;
  isDivergent: boolean;
  uploadedFile: UploadedFileState | null;
  pastedData: string;
  sourceRows: StudioDataRow[];
  fileInputRef: React.RefObject<HTMLInputElement | null>;
  setUploadedFile: (v: UploadedFileState | null) => void;
  setPastedData: (v: string) => void;
  setSourceRows: (rows: StudioDataRow[]) => void;
  setNormalizedChart: (v: NormalizeChartSourceResponse | null) => void;
  setSubmitting: (v: boolean) => void;
  setToast: (v: ToastNotice | null) => void;
}

const INITIAL_ASSISTANT_MSG: SubChatMessage = {
  id: "sys-welcome",
  role: "assistant",
  text: "Nhập yêu cầu hoặc chọn điểm trên biểu đồ (VD: 'Chuyển biểu đồ đường', 'Mục tiêu 1200', 'Giải thích Direct').",
};

function useEscapeClose(isOpen: boolean, onClose: () => void): void {
  useEffect(() => {
    if (!isOpen) return;
    const onKeyDown = (ev: KeyboardEvent) => { if (ev.key === "Escape") onClose(); };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isOpen, onClose]);
}

function useFileAndPasteActions(deps: SourceActionDeps) {
  const { t } = useTranslation();
  const handleFileChange = useCallback(async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const loaded = await readSelectedChartFile(file);
    if (!loaded.ok) { deps.setToast({ tone: "danger", message: t(loaded.key, loaded.fallback) }); return; }
    deps.setUploadedFile(loaded.state);
    deps.setNormalizedChart(null);
    deps.setToast(null);
    const parsed = loaded.state.inlineContent ? parseRawDelimitedRows(loaded.state.inlineContent) : [];
    if (parsed.length > 0) {
      deps.setSourceRows(parsed.map((r) => ({ ...r })));
      deps.commit((prev) => ({ ...prev, rows: parsed, whatIfRowId: null, whatIfDeltaPct: 0 }));
    }
  }, [deps, t]);

  const handleClearFile = useCallback(() => {
    deps.setUploadedFile(null);
    deps.setNormalizedChart(null);
    if (deps.fileInputRef.current) deps.fileInputRef.current.value = "";
  }, [deps]);

  const handleRawCsvChange = useCallback((val: string) => {
    deps.setPastedData(val);
    deps.setNormalizedChart(null);
    const parsed = parseRawDelimitedRows(val);
    if (parsed.length > 0) {
      deps.setSourceRows(parsed.map((r) => ({ ...r })));
      deps.commit((prev) => ({ ...prev, rows: parsed, whatIfRowId: null, whatIfDeltaPct: 0 }));
    }
  }, [deps]);

  const handleRevertToSource = useCallback(() => {
    deps.commit((prev) => ({ ...prev, rows: deps.sourceRows.map((r) => ({ ...r })), whatIfRowId: null, whatIfDeltaPct: 0 }));
    deps.setToast({ tone: "info", message: "Đã khôi phục dữ liệu về đúng tệp/nguồn gốc." });
  }, [deps]);

  return { handleFileChange, handleClearFile, handleRawCsvChange, handleRevertToSource };
}

function useNormalizePreviewAction(deps: SourceActionDeps) {
  const { t } = useTranslation();
  return useCallback(async () => {
    const chartSource = buildChartSourcePayload({
      uploadedFile: deps.uploadedFile, pastedData: deps.pastedData, rows: deps.snapshot.rows,
      isDivergent: deps.isDivergent, yAxisLabel: deps.snapshot.yAxisLabel,
      showComparison: deps.snapshot.showComparison, chartType: deps.snapshot.chartType, title: deps.snapshot.title,
    });
    if (!chartSource) return;
    deps.setSubmitting(true);
    deps.setToast(null);
    try {
      const normalized = await normalizeChartSource({ projectId: deps.projectId, ...chartSource });
      deps.setNormalizedChart(normalized);
      const nextRows = normalizeResponseToRows(normalized);
      if (nextRows.length > 0) {
        deps.setSourceRows(nextRows.map((r) => ({ ...r })));
        deps.commit((prev) => ({ ...prev, rows: nextRows, whatIfRowId: null, whatIfDeltaPct: 0 }));
      }
    } catch (err) {
      deps.setToast({ tone: "danger", message: formatDeckGenError(err, t("deckGen.errorGeneric", "Không thể chuẩn hóa dữ liệu biểu đồ.")) });
    } finally {
      deps.setSubmitting(false);
    }
  }, [deps, t]);
}

function useDeckGenerateAction(deps: {
  params: StudioControllerParams;
  snapshot: StudioSnapshot;
  messages: readonly SubChatMessage[];
  isDivergent: boolean;
  uploadedFile: UploadedFileState | null;
  pastedData: string;
  setGeneratedSummary: (v: GenerateDeckResponse | null) => void;
  setNormalizedChart: (v: NormalizeChartSourceResponse | null) => void;
  setSubmitting: (v: boolean) => void;
  setToast: (v: ToastNotice | null) => void;
}) {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });

  return useCallback(async (e: React.FormEvent) => {
    e.preventDefault();
    const snap = deps.snapshot;
    const derivedBrief = deriveBackendBrief(snap, deps.messages);
    if (!snap.title.trim() || !derivedBrief) {
      deps.setToast({ tone: "warning", message: t("deckGen.errorMissingBrief", "Vui lòng nhập tiêu đề cho bộ slide.") });
      return;
    }
    deps.setSubmitting(true);
    deps.setToast(null);
    try {
      const chartSource = buildChartSourcePayload({
        uploadedFile: deps.uploadedFile, pastedData: deps.pastedData, rows: snap.rows,
        isDivergent: deps.isDivergent, yAxisLabel: snap.yAxisLabel,
        showComparison: snap.showComparison, chartType: snap.chartType, title: snap.title,
      });
      const generated = await generateDeck({
        projectId: deps.params.projectId, title: snap.title.trim(), brief: derivedBrief,
        framePreset: snap.framePreset, aspectRatio: deps.params.aspectRatio, chartSource,
      });
      deps.setGeneratedSummary(generated);
      if (generated.normalizedChart) deps.setNormalizedChart(generated.normalizedChart);
      deps.params.onSyncToMainChat?.(buildMainChatSyncSummary(snap, "Đã lưu & tạo bộ Slide", generated.deck.slides.length));
      deps.setToast({ tone: "success", message: `${t("deckGen.successToast", "Đã tạo bộ slide thành công")} (${formatNumber(generated.deck.slides.length)} slides)` });
      deps.params.onDeckGenerated?.(generated);
    } catch (err) {
      deps.setToast({ tone: "danger", message: formatDeckGenError(err, t("deckGen.errorGeneric", "Không thể tạo bộ slide.")) });
    } finally {
      deps.setSubmitting(false);
    }
  }, [deps, formatNumber, t]);
}

function useStudioUiState(initialRows: StudioDataRow[]) {
  const [sourceRows, setSourceRows] = useState<StudioDataRow[]>(() => initialRows.map((r) => ({ ...r })));
  const [pastedData, setPastedData] = useState("");
  const [uploadedFile, setUploadedFile] = useState<UploadedFileState | null>(null);
  const [normalizedChart, setNormalizedChart] = useState<NormalizeChartSourceResponse | null>(null);
  const [generatedSummary, setGeneratedSummary] = useState<GenerateDeckResponse | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [toast, setToast] = useState<ToastNotice | null>(null);
  const [interactionMode, setInteractionMode] = useState<StudioInteractionMode>("interact");
  const [selectedFfId, setSelectedFfId] = useState<string | null>("kpi-revenue-card");
  const [controlView, setControlView] = useState<StudioControlView>("general");
  const [highlightedRowId, setHighlightedRowId] = useState<string | null>(null);
  const [subChatInput, setSubChatInput] = useState("");
  const [messages, setMessages] = useState<SubChatMessage[]>([INITIAL_ASSISTANT_MSG]);
  return {
    sourceRows, setSourceRows, pastedData, setPastedData, uploadedFile, setUploadedFile,
    normalizedChart, setNormalizedChart, generatedSummary, setGeneratedSummary,
    submitting, setSubmitting, toast, setToast, interactionMode, setInteractionMode,
    selectedFfId, setSelectedFfId, controlView, setControlView,
    highlightedRowId, setHighlightedRowId, subChatInput, setSubChatInput, messages, setMessages,
  };
}

function useStorytellingActions(
  history: ReturnType<typeof useStudioHistory>,
  ui: ReturnType<typeof useStudioUiState>,
  dispatchSubChatPrompt: (prompt: string) => void
) {
  const handleSelectElement = useCallback((ffId: string) => {
    ui.setSelectedFfId(ffId);
    const view = mapFfIdToControlView(ffId);
    if (view !== "general") ui.setControlView(view);
  }, [ui]);

  const handleApplyAiLens = useCallback((lensId: StudioAiLensId) => {
    const res = applyAiLens(history.snapshot, lensId);
    history.commit(() => res.nextSnapshot);
    ui.setHighlightedRowId(res.highlightRowId);
  }, [history, ui]);

  const handleAutoFixQa = useCallback(() => {
    history.commit((prev) => autoFixHonestDataIssues(prev));
    ui.setToast({ tone: "success", message: "Đã chuẩn hóa Action Title, Chiếu điểm & số liệu bằng 1 chạm." });
  }, [history, ui]);

  const handleToggleCallout = useCallback((row: StudioDataRow) => {
    history.commit((prev) => {
      const exists = prev.callouts.some((c) => c.rowId === row.id);
      const callouts = exists ? prev.callouts.filter((c) => c.rowId !== row.id) : [...prev.callouts, buildRowCallout(row, prev.unit)];
      return { ...prev, callouts };
    });
  }, [history]);

  const handleToggleSpotlight = useCallback((rowId: string) => {
    history.commit((prev) => ({ ...prev, spotlightRowId: prev.spotlightRowId === rowId ? null : rowId }));
  }, [history]);

  const handleAskAiExplainRow = useCallback((row: StudioDataRow) => {
    const pct = computeRowDeltaPct(row);
    const val = formatMetricValue(row.current, history.snapshot.unit);
    dispatchSubChatPrompt(`Giải thích vì sao ${row.label} đạt ${val} (${pct >= 0 ? "+" : ""}${pct}% vs kỳ trước)?`);
  }, [dispatchSubChatPrompt, history.snapshot.unit]);

  const handleApplyWhatIf = useCallback((rowId: string, deltaPct: number) => {
    history.commit((prev) => applyWhatIfDelta(ui.sourceRows, prev, rowId, deltaPct));
  }, [history, ui.sourceRows]);

  return {
    handleSelectElement, handleApplyAiLens, handleAutoFixQa,
    handleToggleCallout, handleToggleSpotlight, handleAskAiExplainRow, handleApplyWhatIf,
  };
}

function useSubChatAndEmbedActions(
  params: StudioControllerParams,
  history: ReturnType<typeof useStudioHistory>,
  ui: ReturnType<typeof useStudioUiState>
) {
  const dispatchSubChatPrompt = useCallback((rawPrompt: string) => {
    const prompt = rawPrompt.trim();
    if (!prompt) return;
    const res = applySubChatCommand(prompt, history.snapshot, ui.selectedFfId, history.snapshot.pins);
    history.commit(() => res.nextSnapshot);
    if (res.highlightRowId !== undefined) ui.setHighlightedRowId(res.highlightRowId);
    const stamp = Date.now();
    ui.setMessages((prev) => [...prev, { id: `u-${stamp}`, role: "user", text: prompt, targetFfId: ui.selectedFfId ?? undefined }, { id: `a-${stamp}`, role: "assistant", text: res.reply }]);
  }, [history, ui]);

  const handleSubChatSend = useCallback(() => {
    if (!ui.subChatInput.trim()) return;
    dispatchSubChatPrompt(ui.subChatInput);
    ui.setSubChatInput("");
  }, [dispatchSubChatPrompt, ui]);

  const handleEmbedIntoSlide = useCallback(() => {
    params.onEmbedIntoActiveSlide?.(buildEmbeddableDashboardHtml(history.snapshot), history.snapshot.title);
    params.onSyncToMainChat?.(buildMainChatSyncSummary(history.snapshot, "Đã gắn vào Slide hiện tại"));
    ui.setToast({ tone: "success", message: `Đã gắn dashboard "${history.snapshot.title}" vào Slide hiện tại.` });
  }, [history.snapshot, params, ui]);

  return { dispatchSubChatPrompt, handleSubChatSend, handleEmbedIntoSlide };
}

export function useDeckStudioController(params: StudioControllerParams) {
  const initial = useMemo(() => createInitialSnapshot(), []);
  const history = useStudioHistory(initial, params.isOpen);
  const layout = useStudioLayout();
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const ui = useStudioUiState(initial.rows);

  useEscapeClose(params.isOpen, params.onClose);
  const divergence = useMemo(() => computeSourceDivergence(ui.sourceRows, history.snapshot.rows), [ui.sourceRows, history.snapshot.rows]);
  const srcDeps: SourceActionDeps = {
    projectId: params.projectId, snapshot: history.snapshot, commit: history.commit, isDivergent: divergence.isDivergent,
    uploadedFile: ui.uploadedFile, pastedData: ui.pastedData, sourceRows: ui.sourceRows, fileInputRef,
    setUploadedFile: ui.setUploadedFile, setPastedData: ui.setPastedData, setSourceRows: ui.setSourceRows,
    setNormalizedChart: ui.setNormalizedChart, setSubmitting: ui.setSubmitting, setToast: ui.setToast,
  };
  const fileActions = useFileAndPasteActions(srcDeps);
  const handlePreviewChart = useNormalizePreviewAction(srcDeps);
  const handleSubmit = useDeckGenerateAction({
    params, snapshot: history.snapshot, messages: ui.messages, isDivergent: divergence.isDivergent,
    uploadedFile: ui.uploadedFile, pastedData: ui.pastedData,
    setGeneratedSummary: ui.setGeneratedSummary, setNormalizedChart: ui.setNormalizedChart,
    setSubmitting: ui.setSubmitting, setToast: ui.setToast,
  });
  const { dispatchSubChatPrompt, handleSubChatSend, handleEmbedIntoSlide } = useSubChatAndEmbedActions(params, history, ui);
  const storyActions = useStorytellingActions(history, ui, dispatchSubChatPrompt);

  return {
    history, layout, fileInputRef, ...ui, divergence,
    ...fileActions, ...storyActions, handlePreviewChart, handleSubChatSend, handleEmbedIntoSlide, handleSubmit,
  };
}
