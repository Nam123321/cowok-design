// @story 10.3
// @implements AC-01, AC-02, AC-03, AC-04 (Design Tournament Arena State Hook with Monotonic generationEpoch Fencing)
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createTournamentApiClient, TournamentApiClient, TournamentApiError } from "../../api/tournament";
import { getGlobalLocale, translateGlobal } from "../../hooks/useTranslation";
import { normalizeViewportPreset, type ViewportPreset } from "../../lib/validations/screen-fixture";
import type {
  CrystallizeData,
  CrystallizePackageKind,
  CrystallizeRequest,
  CrystallizeResponse,
  GeneratedVariant,
  ListTournamentRunsResponse,
  PersistedTournamentRun,
  SelectTournamentVariantData,
  SelectTournamentVariantRequest,
  SelectTournamentVariantResponse,
  StartTournamentData,
  StartTournamentRequest,
  StartTournamentResponse,
} from "../../lib/validations/tournament";

export type TournamentViewportMode = "desktop" | "mobile" | ViewportPreset;
export type TournamentContextTabKind = "html" | "slide" | "md" | "code";

const AST_PERCENT_SCALE = 100;
const DEFAULT_RECENT_RUNS_LIMIT = 10;
const EPOCH_SECONDS_CUTOFF = 10_000_000_000;
const MS_PER_SECOND = 1000;
const SUBCHAT_SYNC_EVENT = "cowok:studio-subchat-sync";
const MAX_CRYSTALLIZE_NAME_LEN = 48;
const MAX_SUBCHAT_PROMPT_LEN = 4096;
const DIRECT_BORROW_INTENT_RE =
  /(?:^|[\s,.;:!?()[\]{}"'「」（）])(chốt|áp dụng|ghép ngay|mượn trực tiếp|commit|apply|borrow)(?=$|[\s,.;:!?()[\]{}"'「」（）])|(採用|確定)/i;

const CONTEXT_PROMPT_TEMPLATES: Record<
  TournamentContextTabKind,
  { key: string; fallback: string; titleFallbackKey?: string; titleFallback: string }
> = {
  md: { key: "tournament.contextPromptMd", fallback: "Tạo 4 phương án giao diện từ đặc tả {{title}}", titleFallback: "DESIGN.md" },
  html: { key: "tournament.contextPromptHtml", fallback: "Tạo 4 phương án tối ưu bố cục cho {{title}}", titleFallbackKey: "tournament.contextCurrentScreen", titleFallback: "màn hình hiện tại" },
  slide: { key: "tournament.contextPromptHtml", fallback: "Tạo 4 phương án tối ưu bố cục cho {{title}}", titleFallbackKey: "tournament.contextCurrentScreen", titleFallback: "màn hình hiện tại" },
  code: { key: "tournament.contextPromptCode", fallback: "Tạo 4 phương án giao diện tương thích với {{title}}", titleFallbackKey: "tournament.contextCurrentCode", titleFallback: "mã nguồn hiện tại" },
};

const CRYSTALLIZE_PREFIX_RULES: ReadonlyArray<{ prefix: string; kind: CrystallizePackageKind }> = [
  { prefix: "brand-kit ", kind: "BRAND_KIT" },
  { prefix: "brand_kit ", kind: "BRAND_KIT" },
  { prefix: "skill-plugin ", kind: "SKILL_PLUGIN" },
  { prefix: "skill_plugin ", kind: "SKILL_PLUGIN" },
];

export interface VariantDiversityBadge {
  diversityPct: number;
  similarityPct: number;
  label: string;
  tooltip: string;
  passed: boolean;
}

export function normalizeTournamentViewport(mode: TournamentViewportMode): ViewportPreset {
  return normalizeViewportPreset(mode);
}

function formatLocaleInt(value: number): string {
  try {
    return new Intl.NumberFormat(getGlobalLocale()).format(value);
  } catch {
    return String(value);
  }
}

export function formatVariantDiversityBadge(
  variant: Pick<GeneratedVariant, "astSimilarity" | "cloneDistancePassed">
): VariantDiversityBadge {
  const similarityPct = Math.max(0, Math.min(AST_PERCENT_SCALE, Math.round(variant.astSimilarity * AST_PERCENT_SCALE)));
  const diversityPct = Math.max(0, Math.min(AST_PERCENT_SCALE, Math.round((1 - variant.astSimilarity) * AST_PERCENT_SCALE)));
  const formattedSim = formatLocaleInt(similarityPct);
  const formattedDiv = formatLocaleInt(diversityPct);
  const tooltip = translateGlobal(
    "tournament.diversityTooltip",
    "Đo thực tế bằng khoảng cách cấu trúc DOM/CSS (AST Jaccard) trên Rust — độ tương đồng tối đa {{similarityPct}}% (ngưỡng đạt <= 78%)",
    { similarityPct: formattedSim }
  );
  const label = variant.cloneDistancePassed
    ? translateGlobal("tournament.diversityDistinct", "Khác biệt {{pct}}%", { pct: formattedDiv })
    : translateGlobal("tournament.diversityDuplicate", "Trùng lặp {{pct}}%", { pct: formattedSim });
  return { diversityPct, similarityPct, label, tooltip, passed: variant.cloneDistancePassed };
}

export function buildContextualTournamentPrompt(
  activeTabTitle?: string,
  activeTabKind?: TournamentContextTabKind
): string {
  const cleanTitle = activeTabTitle?.trim() ?? "";
  if (activeTabKind) {
    const spec = CONTEXT_PROMPT_TEMPLATES[activeTabKind];
    const fallbackTitle = spec.titleFallbackKey ? translateGlobal(spec.titleFallbackKey, spec.titleFallback) : spec.titleFallback;
    return translateGlobal(spec.key, spec.fallback, { title: cleanTitle || fallbackTitle });
  }
  if (cleanTitle) {
    return translateGlobal("tournament.contextPromptTitled", "Tạo 4 phương án tối ưu bố cục cho {{title}}", { title: cleanTitle });
  }
  return translateGlobal("tournament.contextPromptDefault", "Tạo 4 phương án bố cục tối ưu cho không gian làm việc hiện tại");
}

export function formatTournamentRunTimestamp(createdAt: number): string {
  const tsMs = createdAt < EPOCH_SECONDS_CUTOFF ? createdAt * MS_PER_SECOND : createdAt;
  try {
    return new Intl.DateTimeFormat(getGlobalLocale(), {
      hour: "2-digit",
      minute: "2-digit",
      day: "2-digit",
      month: "2-digit",
    }).format(new Date(tsMs));
  } catch {
    return "";
  }
}

export function resolvePersistedWinnerLabel(run: PersistedTournamentRun): string | null {
  if (!run.winningVariantId) return null;
  const found = run.variants.find((v) => v.id === run.winningVariantId);
  return found ? found.displayLabel : translateGlobal("tournament.committedFallbackLabel", "Phương án đã chốt");
}

export function parseSubChatDirectBorrow(
  promptText: string,
  variants: ReadonlyArray<GeneratedVariant>,
  activeVariantId: string
): GeneratedVariant | null {
  const normalized = promptText.toLowerCase();
  if (!DIRECT_BORROW_INTENT_RE.test(normalized)) return null;
  return variants.find((v) => v.id !== activeVariantId && normalized.includes(v.displayLabel.toLowerCase())) ?? null;
}

export const findDirectBorrowIntent = parseSubChatDirectBorrow;

export function parseCrystallizeSlashCommand(
  input: string,
  fallbackName: string
): { kind: CrystallizePackageKind; name: string } | null {
  const match = /^\/crystallize(?:\s+([\s\S]*))?$/i.exec(input.trim());
  if (!match) return null;
  const rawArgs = (match[1] ?? "").trim();
  if (!rawArgs) return { kind: "STARTER_TEMPLATE", name: fallbackName };
  const lower = rawArgs.toLowerCase();
  const matchedRule = CRYSTALLIZE_PREFIX_RULES.find((rule) => lower.startsWith(rule.prefix));
  if (matchedRule) {
    return { kind: matchedRule.kind, name: rawArgs.slice(rawArgs.indexOf(" ") + 1).trim() || fallbackName };
  }
  return { kind: "STARTER_TEMPLATE", name: rawArgs.slice(0, MAX_CRYSTALLIZE_NAME_LEN) };
}

export interface SubChatTournamentActions {
  prompt: string;
  run: StartTournamentData | null;
  activeVariant: GeneratedVariant | null;
  setPrompt: (value: string) => void;
  launchTournament: (overridePrompt?: string) => Promise<StartTournamentData | null>;
  confirmSelection: (input?: ConfirmSelectionInput) => Promise<SelectTournamentVariantData | null>;
  crystallizeWinner: (kind: CrystallizePackageKind, name: string, description?: string) => Promise<CrystallizeData | null>;
}

async function handleSubChatCrystallize(
  cmd: { kind: CrystallizePackageKind; name: string },
  state: SubChatTournamentActions
): Promise<string> {
  if (!state.run) {
    return translateGlobal("tournament.subChatCrystallizeNoRun", "Vui lòng khởi tạo một phiên so sánh trước khi dùng lệnh /crystallize.");
  }
  const pkg = await state.crystallizeWinner(cmd.kind, cmd.name);
  return pkg
    ? translateGlobal("tournament.subChatCrystallizeSuccess", 'Đã kết tinh gói tái sử dụng "{{name}}" thành công.', { name: pkg.name })
    : translateGlobal("tournament.subChatCrystallizeFailed", "Chưa thể đóng gói mẫu tái sử dụng lúc này. Vui lòng thử lại.");
}

async function handleSubChatDirectBorrow(
  fullPrompt: string,
  donor: GeneratedVariant,
  active: GeneratedVariant,
  state: SubChatTournamentActions
): Promise<string> {
  const committed = await state.confirmSelection({ borrowFromVariantId: donor.id, borrowSelectorOrInstruction: fullPrompt });
  return committed
    ? translateGlobal("tournament.subChatBorrowSuccess", "Đã áp dụng ghép chi tiết từ {{donor}} vào {{target}} và chốt phương án.", { donor: donor.displayLabel, target: active.displayLabel })
    : translateGlobal("tournament.subChatBorrowFailed", "Chưa thể áp dụng ghép chi tiết lúc này. Vui lòng thử lại hoặc điều chỉnh hướng dẫn.");
}

function resolveSubChatBaseContext(state: SubChatTournamentActions, activeTabTitle?: string): string {
  if (state.run) return state.prompt.trim() || state.run.prompt;
  if (activeTabTitle) return translateGlobal("tournament.subChatContextPrefix", "Bối cảnh {{title}}", { title: activeTabTitle });
  return "";
}

export async function executeTournamentSubChatInstruction(
  fullPrompt: string,
  state: SubChatTournamentActions,
  activeTabTitle?: string
): Promise<string> {
  const fallbackName = state.prompt.slice(0, MAX_CRYSTALLIZE_NAME_LEN) || translateGlobal("tournament.defaultPackageName", "Mẫu giao diện mới");
  const crystallizeCmd = parseCrystallizeSlashCommand(fullPrompt, fallbackName);
  if (crystallizeCmd) return handleSubChatCrystallize(crystallizeCmd, state);
  const directDonor = state.run && state.activeVariant ? parseSubChatDirectBorrow(fullPrompt, state.run.variants, state.activeVariant.id) : null;
  if (directDonor && state.activeVariant) return handleSubChatDirectBorrow(fullPrompt, directDonor, state.activeVariant, state);
  const baseCtx = resolveSubChatBaseContext(state, activeTabTitle);
  const refinedPrompt = (baseCtx ? `${baseCtx} — ${fullPrompt}` : fullPrompt).slice(0, MAX_SUBCHAT_PROMPT_LEN);
  state.setPrompt(fullPrompt);
  const nextRun = await state.launchTournament(refinedPrompt);
  if (!nextRun) {
    return translateGlobal("tournament.subChatLaunchFailed", "Chưa thể khởi chạy vòng so sánh mới. Vui lòng kiểm tra kết nối hoặc thử lại.");
  }
  return translateGlobal(
    "tournament.subChatLaunchSuccess",
    "Đã tạo vòng so sánh mới với {{count}} phương án. Gợi ý hàng đầu: {{recommended}} — bạn có thể kéo thanh so sánh mù bên phải để đối chiếu.",
    { count: formatLocaleInt(nextRun.variants.length), recommended: nextRun.rubricSummary.recommendedDisplayLabel }
  );
}

export function dispatchTournamentSubChatSync(detail: {
  projectId: string;
  prompt: string;
  activeVariantLabel: string | null;
  runId: string | null;
}): void {
  if (typeof window === "undefined") return;
  window.dispatchEvent(new CustomEvent(SUBCHAT_SYNC_EVENT, { detail: { source: "design-tournament", ...detail } }));
}

export interface DesignTournamentPort {
  startTournament(payload: StartTournamentRequest): Promise<StartTournamentResponse>;
  selectWinner(payload: SelectTournamentVariantRequest, options?: { signal?: AbortSignal }): Promise<SelectTournamentVariantResponse>;
  crystallize(payload: CrystallizeRequest): Promise<CrystallizeResponse>;
  listRuns?(projectId: string, limit?: number): Promise<ListTournamentRunsResponse>;
}

const TOURNAMENT_ERROR_MAP: Record<string, { key: string; fallback: string }> = {
  STALE_GENERATION_EPOCH: { key: "tournament.errStaleGenerationEpoch", fallback: "Phương án bạn chọn đã cũ hơn lựa chọn mới nhất. Vui lòng thử lại." },
  BORROW_ZONE_NOT_FOUND: { key: "tournament.errBorrowZoneNotFound", fallback: "Không tìm thấy vùng giao diện phù hợp để mượn từ phương án nguồn." },
  CSS_SECURITY_BLOCKED: { key: "tournament.errCssSecurityBlocked", fallback: "Nội dung giao diện chứa mã HTML/CSS không an toàn và đã bị chặn." },
  E_MXSS_BLOCKED: { key: "tournament.errCssSecurityBlocked", fallback: "Nội dung giao diện chứa mã HTML/CSS không an toàn và đã bị chặn." },
  INSUFFICIENT_TOURNAMENT_VARIANTS: { key: "tournament.errInsufficientVariants", fallback: "Không đủ phương án hoàn thành trong thời gian cho phép. Vui lòng thử lại." },
  TOURNAMENT_LEASE_CONFLICT: { key: "tournament.errLeaseConflict", fallback: "Xung đột tài nguyên phiên so sánh. Vui lòng thử lại sau giây lát." },
  SEALED_EVAL_TAMPER_ERROR: { key: "tournament.errSealedEvalTamper", fallback: "Hành động bị chặn bởi lớp bảo vệ bộ đánh giá niêm phong." },
  HUMAN_APPROVAL_REQUIRED: { key: "tournament.errHumanApprovalRequired", fallback: "Thay đổi kỹ năng chỉ được phép lưu bản nháp chờ phê duyệt thủ công." },
  PROJECT_NOT_FOUND: { key: "tournament.errProjectNotFound", fallback: "Không tìm thấy dự án tương ứng." },
  TOURNAMENT_RUN_NOT_FOUND: { key: "tournament.errRunNotFound", fallback: "Không tìm thấy phiên so sánh tương ứng." },
  TOURNAMENT_VARIANT_NOT_FOUND: { key: "tournament.errVariantNotFound", fallback: "Không tìm thấy phương án thiết kế tương ứng." },
  VARIANT_ARTIFACT_NOT_FOUND: { key: "tournament.errArtifactNotFound", fallback: "Không thể đọc tệp giao diện của phương án đã chọn." },
  STORAGE_BACKPRESSURE: { key: "tournament.errStorageBackpressure", fallback: "Hệ thống đang bận xử lý dữ liệu. Vui lòng thử lại sau giây lát." },
  PAYLOAD_TOO_LARGE: { key: "tournament.errPayloadTooLarge", fallback: "Nội dung gửi lên vượt quá giới hạn dung lượng cho phép." },
  UNAUTHORIZED: { key: "tournament.errUnauthorized", fallback: "Phiên đăng nhập không hợp lệ hoặc đã hết hạn." },
  FORBIDDEN: { key: "tournament.errForbidden", fallback: "Bạn không có quyền thực hiện thao tác này." },
};

export function formatTournamentApiError(err: unknown, fallback: string): string {
  if (err instanceof TournamentApiError) {
    const entry = TOURNAMENT_ERROR_MAP[err.code];
    if (entry) return translateGlobal(entry.key, entry.fallback);
  }
  return fallback;
}

function isAbortError(err: unknown): boolean {
  return (err instanceof DOMException && err.name === "AbortError") || (err instanceof Error && err.name === "AbortError");
}

function invokeSelectWinner(
  apiPort: DesignTournamentPort,
  payload: SelectTournamentVariantRequest,
  signal: AbortSignal
): Promise<SelectTournamentVariantResponse> {
  return apiPort instanceof TournamentApiClient || apiPort.selectWinner.length >= 2
    ? apiPort.selectWinner(payload, { signal })
    : apiPort.selectWinner(payload);
}

export interface UseDesignTournamentOptions {
  projectId: string;
  initialPrompt?: string;
  initialRun?: StartTournamentData | null;
  port?: DesignTournamentPort;
  onCommittedHtml?: (mergedHtml: string, result: SelectTournamentVariantData) => void;
  loadRecentRunsOnMount?: boolean;
}

export interface ConfirmSelectionInput {
  borrowFromVariantId?: string;
  borrowSelectorOrInstruction?: string;
}

function useRecentRunsLoader(apiPort: DesignTournamentPort, projectId: string, loadOnMount: boolean) {
  const [recentRuns, setRecentRuns] = useState<PersistedTournamentRun[]>([]);
  const [isLoadingRuns, setIsLoadingRuns] = useState(false);
  const isMountedRef = useRef(true);

  useEffect(() => {
    isMountedRef.current = true;
    return () => { isMountedRef.current = false; };
  }, []);

  const refreshRecentRuns = useCallback(async (): Promise<PersistedTournamentRun[]> => {
    if (typeof apiPort.listRuns !== "function" || !projectId.trim()) return [];
    setIsLoadingRuns(true);
    try {
      const res = await apiPort.listRuns(projectId, DEFAULT_RECENT_RUNS_LIMIT);
      if (isMountedRef.current) setRecentRuns(res.data.runs);
      return res.data.runs;
    } catch {
      return [];
    } finally {
      if (isMountedRef.current) setIsLoadingRuns(false);
    }
  }, [apiPort, projectId]);

  useEffect(() => {
    if (loadOnMount) void refreshRecentRuns();
  }, [loadOnMount, refreshRecentRuns]);

  return { recentRuns, isLoadingRuns, refreshRecentRuns };
}

function useTournamentCommitAction(params: {
  apiPort: DesignTournamentPort;
  projectId: string;
  run: StartTournamentData | null;
  selectedVariantId: string;
  generationEpochRef: React.MutableRefObject<number>;
  commitAbortRef: React.MutableRefObject<AbortController | null>;
  onCommittedHtml?: (mergedHtml: string, result: SelectTournamentVariantData) => void;
  onAfterCommit: (data: SelectTournamentVariantData) => void;
  onError: (msg: string | null) => void;
}) {
  const [isCommitting, setIsCommitting] = useState(false);

  const confirmSelection = useCallback(async (input: ConfirmSelectionInput = {}) => {
    if (!params.run || !params.selectedVariantId) return null;
    params.commitAbortRef.current?.abort();
    const controller = new AbortController();
    params.commitAbortRef.current = controller;
    const commitEpoch = params.generationEpochRef.current;
    setIsCommitting(true);
    params.onError(null);
    try {
      const payload: SelectTournamentVariantRequest = {
        projectId: params.projectId, runId: params.run.runId, winningVariantId: params.selectedVariantId, generationEpoch: commitEpoch,
        borrowFromVariantId: input.borrowFromVariantId || undefined, borrowSelectorOrInstruction: input.borrowSelectorOrInstruction?.trim() || undefined,
      };
      const res = await invokeSelectWinner(params.apiPort, payload, controller.signal);
      if (controller.signal.aborted || params.generationEpochRef.current !== commitEpoch) return null;
      params.onAfterCommit(res.data);
      params.onCommittedHtml?.(res.data.mergedHtml, res.data);
      return res.data;
    } catch (err) {
      if (controller.signal.aborted || isAbortError(err) || params.generationEpochRef.current !== commitEpoch) return null;
      params.onError(formatTournamentApiError(err, translateGlobal("tournament.errCommitFailed", "Không thể xác nhận phương án đã chọn")));
      return null;
    } finally {
      if (params.commitAbortRef.current === controller) params.commitAbortRef.current = null;
      if (params.generationEpochRef.current === commitEpoch) setIsCommitting(false);
    }
  }, [params]);

  return { isCommitting, setIsCommitting, confirmSelection };
}

function useTournamentCrystallizeAction(params: {
  apiPort: DesignTournamentPort;
  projectId: string;
  run: StartTournamentData | null;
  selectedVariantId: string;
  lastCommit: SelectTournamentVariantData | null;
  onSuccess: (data: CrystallizeData) => void;
  onError: (msg: string | null) => void;
}) {
  const [isCrystallizing, setIsCrystallizing] = useState(false);

  const crystallizeWinner = useCallback(async (packageKind: CrystallizePackageKind, name: string, description?: string) => {
    const cleanName = name.trim();
    if (!cleanName) return null;
    setIsCrystallizing(true);
    params.onError(null);
    try {
      const activeVariant = params.run?.variants.find((v) => v.id === params.selectedVariantId);
      const res = await params.apiPort.crystallize({
        projectId: params.projectId, runId: params.run?.runId, variantId: activeVariant?.id,
        packageKind, name: cleanName, description: description?.trim() || undefined,
        htmlSnapshot: params.lastCommit?.mergedHtml ?? activeVariant?.html,
      });
      params.onSuccess(res.data);
      return res.data;
    } catch (err) {
      params.onError(formatTournamentApiError(err, translateGlobal("tournament.errCrystallizeFailed", "Không thể đóng gói mẫu tái sử dụng")));
      return null;
    } finally {
      setIsCrystallizing(false);
    }
  }, [params]);

  return { isCrystallizing, crystallizeWinner };
}

function useTournamentLaunchAction(params: {
  apiPort: DesignTournamentPort;
  projectId: string;
  prompt: string;
  commitAbortRef: React.MutableRefObject<AbortController | null>;
  onResetCommit: () => void;
  onError: (msg: string | null) => void;
  onRunStarted: (nextRun: StartTournamentData) => void;
}) {
  const [isLaunching, setIsLaunching] = useState(false);

  const launchTournament = useCallback(async (overridePrompt?: string) => {
    const effectivePrompt = (overridePrompt ?? params.prompt).trim();
    if (!effectivePrompt) return null;
    params.commitAbortRef.current?.abort();
    params.commitAbortRef.current = null;
    params.onResetCommit();
    setIsLaunching(true);
    params.onError(null);
    try {
      const { data: nextRun } = await params.apiPort.startTournament({ projectId: params.projectId, prompt: effectivePrompt });
      params.onRunStarted(nextRun);
      return nextRun;
    } catch (err) {
      params.onError(formatTournamentApiError(err, translateGlobal("tournament.errLaunchFailed", "Không thể khởi chạy so sánh phương án")));
      return null;
    } finally {
      setIsLaunching(false);
    }
  }, [params]);

  return { isLaunching, launchTournament };
}

function resolveInitialVariantIds(initialRun: StartTournamentData | null): { selected: string; compare: string } {
  if (!initialRun) return { selected: "", compare: "" };
  const selected = initialRun.rubricSummary.recommendedVariantId || initialRun.variants[0]?.id || "";
  const compare = initialRun.variants.find((v) => v.id !== selected)?.id ?? initialRun.variants[0]?.id ?? "";
  return { selected, compare };
}

function useTournamentSelectionState(initialRun: StartTournamentData | null, commitAbortRef: React.MutableRefObject<AbortController | null>, setIsCommitting: (v: boolean) => void, setErrorMessage: (m: string | null) => void) {
  const initialIds = resolveInitialVariantIds(initialRun);
  const [run, setRun] = useState<StartTournamentData | null>(initialRun);
  const [selectedVariantId, setSelectedVariantId] = useState<string>(initialIds.selected);
  const [compareVariantId, setCompareVariantId] = useState<string>(initialIds.compare);
  const [generationEpoch, setGenerationEpoch] = useState<number>(initialRun?.generationEpoch ?? 1);
  const generationEpochRef = useRef<number>(initialRun?.generationEpoch ?? 1);

  const applyStartedRun = useCallback((nextRun: StartTournamentData) => {
    const nextIds = resolveInitialVariantIds(nextRun);
    generationEpochRef.current = nextRun.generationEpoch;
    setRun(nextRun);
    setGenerationEpoch(nextRun.generationEpoch);
    setSelectedVariantId(nextIds.selected);
    setCompareVariantId(nextIds.compare);
  }, []);

  const switchActiveVariant = useCallback((nextVariantId: string) => {
    if (!nextVariantId) return;
    commitAbortRef.current?.abort();
    commitAbortRef.current = null;
    generationEpochRef.current += 1;
    setGenerationEpoch(generationEpochRef.current);
    setSelectedVariantId(nextVariantId);
    setIsCommitting(false);
    setErrorMessage(null);
    if (run && nextVariantId === compareVariantId) {
      const alt = run.variants.find((v) => v.id !== nextVariantId);
      if (alt) setCompareVariantId(alt.id);
    }
  }, [commitAbortRef, compareVariantId, run, setErrorMessage, setIsCommitting]);

  const activeVariant = useMemo(() => run?.variants.find((v) => v.id === selectedVariantId) ?? null, [run, selectedVariantId]);
  const compareVariant = useMemo(() => run?.variants.find((v) => v.id === compareVariantId) ?? null, [run, compareVariantId]);
  return { run, selectedVariantId, compareVariantId, setCompareVariantId, generationEpoch, generationEpochRef, activeVariant, compareVariant, applyStartedRun, switchActiveVariant };
}

export function useDesignTournament(options: UseDesignTournamentOptions) {
  const { projectId, initialPrompt = "", initialRun = null, onCommittedHtml, loadRecentRunsOnMount = false } = options;
  const apiPort = useMemo<DesignTournamentPort>(() => options.port ?? createTournamentApiClient(), [options.port]);
  const [prompt, setPrompt] = useState(initialPrompt);
  const [viewportMode, setViewportMode] = useState<TournamentViewportMode>("desktop-1440");
  const [lastCommit, setLastCommit] = useState<SelectTournamentVariantData | null>(null);
  const [lastCrystallize, setLastCrystallize] = useState<CrystallizeData | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const commitAbortRef = useRef<AbortController | null>(null);
  const setCommittingRef = useRef<(v: boolean) => void>(() => {});
  const { recentRuns, isLoadingRuns, refreshRecentRuns } = useRecentRunsLoader(apiPort, projectId, loadRecentRunsOnMount);
  const sel = useTournamentSelectionState(initialRun, commitAbortRef, (v) => setCommittingRef.current(v), setErrorMessage);

  useEffect(() => () => { commitAbortRef.current?.abort(); commitAbortRef.current = null; }, []);

  const { isCommitting, setIsCommitting, confirmSelection } = useTournamentCommitAction({
    apiPort, projectId, run: sel.run, selectedVariantId: sel.selectedVariantId, generationEpochRef: sel.generationEpochRef, commitAbortRef, onCommittedHtml,
    onAfterCommit: (data) => { setLastCommit(data); void refreshRecentRuns(); }, onError: setErrorMessage,
  });
  setCommittingRef.current = setIsCommitting;

  const { isCrystallizing, crystallizeWinner } = useTournamentCrystallizeAction({
    apiPort, projectId, run: sel.run, selectedVariantId: sel.selectedVariantId, lastCommit, onSuccess: setLastCrystallize, onError: setErrorMessage,
  });
  const { isLaunching, launchTournament } = useTournamentLaunchAction({
    apiPort, projectId, prompt, commitAbortRef, onResetCommit: () => setIsCommitting(false), onError: setErrorMessage,
    onRunStarted: (nextRun) => { sel.applyStartedRun(nextRun); setLastCommit(null); setLastCrystallize(null); void refreshRecentRuns(); },
  });

  return {
    prompt, setPrompt, run: sel.run, recentRuns, isLoadingRuns, refreshRecentRuns, selectedVariantId: sel.selectedVariantId,
    compareVariantId: sel.compareVariantId, setCompareVariantId: sel.setCompareVariantId, activeVariant: sel.activeVariant,
    compareVariant: sel.compareVariant, viewportMode, setViewportMode, generationEpoch: sel.generationEpoch,
    isLaunching, isCommitting, isCrystallizing, lastCommit, lastCrystallize, errorMessage,
    launchTournament, switchActiveVariant: sel.switchActiveVariant, confirmSelection, crystallizeWinner,
  };
}
