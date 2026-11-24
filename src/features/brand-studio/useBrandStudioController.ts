// @story 05.6
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06
import { useCallback, useEffect, useRef, useState } from "react";
import {
  CognitiveStudioApiError,
  generateBrandProposals,
  getFeDataContract,
  getHiddenSpecs,
  listHeritagePacks,
  putHiddenSpec,
  revealSpecsFolder,
  saveHeritagePack,
  scanSketchComponentsApi,
} from "../../api/cognitive-studio";
import type {
  AdaptiveEntryMode,
  BrandPreviewBundle,
  BrandProposalV1,
  FeDataContractDraft,
  HeritageDesignPack,
  HiddenSpecDocument,
  HiddenSpecKey,
  PopularComponentAuditItem,
} from "../../lib/validations/cognitive-studio";

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const DEFAULT_SINGLE_MODE: readonly string[] = ["warm"];
const FALLBACK_SKETCH_PAGES = 3;
const FALLBACK_SHA_LENGTH = 64;
const FALLBACK_PAGE_RENDER_MICROS = 420;
const FALLBACK_TOTAL_RENDER_MICROS = 950;
const MAX_REFERENCE_APPS = 6;
const MIN_REQUESTED_MODES = 1;
const MAX_REQUESTED_MODES = 6;

type ProposalStyleOverride = Partial<Pick<BrandProposalV1, "radiusScale" | "density">>;

export interface BrandSubChatBatchMutation {
  nextProposalId?: string;
  nextModeId?: string;
  nextReferenceAppsText?: string;
  radiusScale?: BrandProposalV1["radiusScale"];
  density?: BrandProposalV1["density"];
}

export interface OccConflictState {
  specKey: HiddenSpecKey;
  expectedRevision: number;
  currentRevision: number;
  message: string;
}

export interface BrandStudioControllerParams {
  projectId?: string;
  screenId?: string;
  locale: "vi" | "en";
  initialThemeModes?: string[];
  onApplyBrandTokens?: (proposal: BrandProposalV1, activeModeId: string) => void;
  onSyncToMainChat?: (summaryText: string) => void;
}

function escapeHtml(raw: string): string {
  return raw
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function buildFallbackModeTokens(modeId: string): Record<string, string> {
  const isDark = modeId === "dark" || modeId === "night";
  return {
    "--bg-canvas": isDark ? "#0D1016" : "#F6F2EA",
    "--bg-surface": isDark ? "#151923" : "#FDFBF7",
    "--text-primary": isDark ? "#F3F4F6" : "#23211D",
    "--border-default": isDark ? "#2A3140" : "#DDD6C7",
    "--action-primary": isDark ? "#93C5FD" : "#1D4ED8",
    "--text-on-accent": isDark ? "#0B0E14" : "#FFFFFF",
  };
}

function buildFallbackProposals(modes: string[]): BrandProposalV1[] {
  const effectiveModes = modes.length > 0 ? modes : [...DEFAULT_SINGLE_MODE];
  return [
    {
      id: "brand-atelier-precision",
      brandName: "Precision Atelier",
      rationale: "Crisp 1px borders, balanced hierarchy, and WCAG AA contrast.",
      inspirationRefs: ["Linear", "Stripe"],
      themeModes: effectiveModes.map((m) => ({
        modeId: m,
        label: m.charAt(0).toUpperCase() + m.slice(1),
        tokens: buildFallbackModeTokens(m),
        wcagAaVerified: true,
      })),
      fontPairing: { heading: "Inter, sans-serif", body: "Inter, sans-serif", mono: "JetBrains Mono, monospace" },
      radiusScale: "balanced",
      density: "comfortable",
      componentStyleOverrides: {},
    },
  ];
}

function buildFallbackPreview(proposal: BrandProposalV1, activeMode: string): BrandPreviewBundle {
  const safeMode = escapeHtml(activeMode);
  const safeName = escapeHtml(proposal.brandName);
  return {
    proposalId: proposal.id,
    activeModeId: activeMode,
    requestedModes: proposal.themeModes.map((m) => m.modeId),
    totalSketchPages: FALLBACK_SKETCH_PAGES,
    cappedToRepresentative: false,
    pages: [
      {
        screenId: "sketch-hero",
        title: "Hero Overview",
        pageRole: "hero",
        modeId: activeMode,
        renderedHtml: `<html data-brand-mode="${safeMode}"><body><main><h1>${safeName}</h1></main></body></html>`,
        contentSha256: "0".repeat(FALLBACK_SHA_LENGTH),
        wcagAaVerified: true,
        contrastReports: [],
        renderTimeMicros: FALLBACK_PAGE_RENDER_MICROS,
      },
    ],
    totalRenderTimeMicros: FALLBACK_TOTAL_RENDER_MICROS,
  };
}

function parseReferenceApps(raw: string): string[] {
  return raw
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s.length > 0)
    .slice(0, MAX_REFERENCE_APPS);
}

function useHiddenSpecSave(opts: {
  projectId: string | undefined;
  isValidProject: boolean;
  selectedSpecKey: HiddenSpecKey;
  draftMarkdown: string;
  specsRef: React.MutableRefObject<HiddenSpecDocument[]>;
  setSpecs: React.Dispatch<React.SetStateAction<HiddenSpecDocument[]>>;
  setOccConflict: React.Dispatch<React.SetStateAction<OccConflictState | null>>;
  setIsSavingSpec: React.Dispatch<React.SetStateAction<boolean>>;
}) {
  const { projectId, isValidProject, selectedSpecKey, draftMarkdown, specsRef, setSpecs, setOccConflict, setIsSavingSpec } = opts;
  return useCallback(async (overrideRevision?: number) => {
    if (!isValidProject || !projectId) return false;
    const current = specsRef.current.find((s) => s.specKey === selectedSpecKey);
    const expectedRevision = overrideRevision ?? current?.revision ?? 0;
    setIsSavingSpec(true);
    try {
      const res = await putHiddenSpec({ projectId, specKey: selectedSpecKey, contentMarkdown: draftMarkdown, expectedRevision });
      setOccConflict(null);
      setSpecs((prev) => prev.map((s) => (s.specKey === res.data.specKey ? res.data : s)));
      return true;
    } catch (err) {
      if (err instanceof CognitiveStudioApiError && err.code === "STALE_MAPPING") {
        setOccConflict({
          specKey: selectedSpecKey,
          expectedRevision,
          currentRevision: Number(err.details?.currentRevision ?? expectedRevision + 1),
          message: err.message,
        });
      }
      return false;
    } finally {
      setIsSavingSpec(false);
    }
  }, [draftMarkdown, isValidProject, projectId, selectedSpecKey, setIsSavingSpec, setOccConflict, setSpecs, specsRef]);
}

function useHiddenSpecsState(projectId: string | undefined, isValidProject: boolean) {
  const [specs, setSpecs] = useState<HiddenSpecDocument[]>([]);
  const [selectedSpecKey, setSelectedSpecKey] = useState<HiddenSpecKey>("02_BRAND_IDENTITY");
  const [draftMarkdown, setDraftMarkdown] = useState("");
  const [isSavingSpec, setIsSavingSpec] = useState(false);
  const [occConflict, setOccConflict] = useState<OccConflictState | null>(null);
  const specsRef = useRef<HiddenSpecDocument[]>([]);
  specsRef.current = specs;

  const loadHiddenSpecs = useCallback(async () => {
    if (!isValidProject || !projectId) return;
    try {
      const res = await getHiddenSpecs(projectId);
      setSpecs(res.data.specs);
      setOccConflict(null);
      const active = res.data.specs.find((s) => s.specKey === selectedSpecKey) ?? res.data.specs[0];
      if (active) {
        setSelectedSpecKey(active.specKey);
        setDraftMarkdown(active.contentMarkdown);
      }
    } catch { /* keep existing state */ }
  }, [isValidProject, projectId, selectedSpecKey]);

  const selectSpecKey = useCallback((key: HiddenSpecKey) => {
    setSelectedSpecKey(key);
    setOccConflict(null);
    const found = specsRef.current.find((s) => s.specKey === key);
    if (found) setDraftMarkdown(found.contentMarkdown);
  }, []);

  const saveActiveSpec = useHiddenSpecSave({
    projectId, isValidProject, selectedSpecKey, draftMarkdown, specsRef, setSpecs, setOccConflict, setIsSavingSpec,
  });

  return {
    specs, selectedSpecKey, draftMarkdown, setDraftMarkdown, isSavingSpec, occConflict,
    loadHiddenSpecs, selectSpecKey, saveActiveSpec, clearOccConflict: () => setOccConflict(null),
  };
}

interface FetchProposalsOptions {
  modes?: string[];
  activeMode?: string;
  proposalId?: string;
  refsText?: string;
}

interface BrandFetcherOpts {
  projectId: string | undefined;
  isValidProject: boolean;
  locale: "vi" | "en";
  requestedModesRef: React.MutableRefObject<string[]>;
  activeModeRef: React.MutableRefObject<string>;
  selectedProposalRef: React.MutableRefObject<string>;
  referenceAppsRef: React.MutableRefObject<string>;
  styleOverridesRef: React.MutableRefObject<Record<string, ProposalStyleOverride>>;
  setProposals: React.Dispatch<React.SetStateAction<BrandProposalV1[]>>;
  setActiveModeId: React.Dispatch<React.SetStateAction<string>>;
  setSelectedProposalId: React.Dispatch<React.SetStateAction<string>>;
  setPreviewBundle: React.Dispatch<React.SetStateAction<BrandPreviewBundle | null>>;
  setIsLoadingStudio: React.Dispatch<React.SetStateAction<boolean>>;
}

function mergeStyleOverrides(
  proposals: BrandProposalV1[],
  overrides: Record<string, ProposalStyleOverride>
): BrandProposalV1[] {
  return proposals.map((p) => {
    const ov = overrides[p.id];
    return ov ? { ...p, ...ov } : p;
  });
}

function applyBrandFallback(opts: BrandFetcherOpts, modes: string[], mode: string): void {
  const fb = mergeStyleOverrides(buildFallbackProposals(modes), opts.styleOverridesRef.current);
  opts.setProposals(fb);
  opts.setActiveModeId(mode);
  opts.setPreviewBundle(buildFallbackPreview(fb[0]!, mode));
}

function useSequencedBrandFetcher(opts: BrandFetcherOpts) {
  const seqRef = useRef(0);
  const abortRef = useRef<AbortController | null>(null);
  useEffect(() => () => { abortRef.current?.abort(); }, []);

  return useCallback(async (callOpts?: FetchProposalsOptions) => {
    const modes = callOpts?.modes ?? opts.requestedModesRef.current;
    const curMode = opts.activeModeRef.current;
    const mode = callOpts?.activeMode ?? (modes.includes(curMode) ? curMode : modes[0] ?? "warm");
    const seq = ++seqRef.current;
    abortRef.current?.abort();
    if (!opts.isValidProject || !opts.projectId) {
      applyBrandFallback(opts, modes, mode);
      return;
    }
    const controller = new AbortController();
    abortRef.current = controller;
    opts.setIsLoadingStudio(true);
    try {
      const res = await generateBrandProposals({
        projectId: opts.projectId,
        referenceApps: parseReferenceApps(callOpts?.refsText ?? opts.referenceAppsRef.current),
        themeModes: modes,
        locale: opts.locale,
        activeModeId: mode,
        selectedProposalId: callOpts?.proposalId ?? opts.selectedProposalRef.current,
      }, controller.signal);
      if (seq !== seqRef.current || controller.signal.aborted) return;
      opts.setProposals(mergeStyleOverrides(res.data.proposals, opts.styleOverridesRef.current));
      opts.setActiveModeId(res.data.activeModeId);
      opts.setSelectedProposalId(res.data.selectedProposalId);
      opts.setPreviewBundle(res.data.previewBundle);
    } catch {
      if (seq === seqRef.current && !controller.signal.aborted) applyBrandFallback(opts, modes, mode);
    } finally {
      if (seq === seqRef.current) opts.setIsLoadingStudio(false);
    }
  }, [opts]);
}

function useSequencedComponentScanner(opts: {
  projectId: string | undefined;
  isValidProject: boolean;
  setAuditItems: React.Dispatch<React.SetStateAction<PopularComponentAuditItem[]>>;
  setExtractedKinds: React.Dispatch<React.SetStateAction<string[]>>;
}) {
  const seqRef = useRef(0);
  const abortRef = useRef<AbortController | null>(null);

  useEffect(() => () => { abortRef.current?.abort(); }, []);

  return useCallback(async (selectedKinds?: string[]) => {
    if (!opts.isValidProject || !opts.projectId) return;
    const seq = ++seqRef.current;
    abortRef.current?.abort();
    const controller = new AbortController();
    abortRef.current = controller;
    try {
      const res = await scanSketchComponentsApi(
        { projectId: opts.projectId, portalId: "SalesApp", surface: "fe", selectedKinds },
        controller.signal
      );
      if (seq !== seqRef.current || controller.signal.aborted) return;
      opts.setAuditItems(res.data.auditItems);
      opts.setExtractedKinds(res.data.extractedKinds);
    } catch { /* preserve existing audit items on abort/offline */ }
  }, [opts]);
}

function useStudioSideData(projectId: string | undefined, screenId: string, isValidProject: boolean) {
  const [feContract, setFeContract] = useState<FeDataContractDraft | null>(null);
  const [feContractTs, setFeContractTs] = useState("");
  const [feContractDiskPath, setFeContractDiskPath] = useState(".cowok-design/specs/fe-data-contract.draft.ts");
  const [heritagePacks, setHeritagePacks] = useState<HeritageDesignPack[]>([]);

  const loadFeContract = useCallback(async () => {
    if (!isValidProject || !projectId) return;
    try {
      const res = await getFeDataContract(projectId, screenId);
      setFeContract(res.data.draft);
      setFeContractTs(res.data.draftTsSource);
      if (res.data.diskPath) setFeContractDiskPath(res.data.diskPath);
    } catch { /* preserve existing contract */ }
  }, [isValidProject, projectId, screenId]);

  const loadHeritagePacksList = useCallback(async () => {
    if (!isValidProject) return;
    try {
      const res = await listHeritagePacks();
      setHeritagePacks(res.data.packs);
    } catch { /* ignore offline */ }
  }, [isValidProject]);

  const openSpecsFolder = useCallback(async (fileName = "fe-data-contract.draft.ts"): Promise<{ opened: boolean; path: string } | null> => {
    if (!isValidProject || !projectId) return null;
    try {
      const res = await revealSpecsFolder({ projectId, fileName });
      return { opened: res.data.opened, path: res.data.relativePath };
    } catch {
      return null;
    }
  }, [isValidProject, projectId]);

  return { feContract, feContractTs, feContractDiskPath, heritagePacks, setHeritagePacks, loadFeContract, loadHeritagePacksList, openSpecsFolder };
}

function useStudioCoreState(initialThemeModes?: string[]) {
  const [isStudioOpen, setIsStudioOpen] = useState(false);
  const [isHiddenSpecOpen, setIsHiddenSpecOpen] = useState(false);
  const [requestedModes, setRequestedModes] = useState<string[]>(
    initialThemeModes && initialThemeModes.length > 0 ? initialThemeModes : [...DEFAULT_SINGLE_MODE]
  );
  const [activeModeId, setActiveModeId] = useState<string>(requestedModes[0] ?? "warm");
  const [referenceAppsText, setReferenceAppsText] = useState("Linear, Stripe");
  const [proposals, setProposals] = useState<BrandProposalV1[]>(() => buildFallbackProposals(requestedModes));
  const [selectedProposalId, setSelectedProposalId] = useState<string>("brand-atelier-precision");
  const [previewBundle, setPreviewBundle] = useState<BrandPreviewBundle | null>(null);
  const [auditItems, setAuditItems] = useState<PopularComponentAuditItem[]>([]);
  const [extractedKinds, setExtractedKinds] = useState<string[]>([]);
  const [entryMode, setEntryMode] = useState<AdaptiveEntryMode>("GREENFIELD");
  const [isLoadingStudio, setIsLoadingStudio] = useState(false);
  const [toastNotice, setToastNotice] = useState<string | null>(null);

  return {
    isStudioOpen, setIsStudioOpen, isHiddenSpecOpen, setIsHiddenSpecOpen,
    requestedModes, setRequestedModes, activeModeId, setActiveModeId,
    referenceAppsText, setReferenceAppsText, proposals, setProposals,
    selectedProposalId, setSelectedProposalId, previewBundle, setPreviewBundle,
    auditItems, setAuditItems, extractedKinds, setExtractedKinds,
    entryMode, setEntryMode, isLoadingStudio, setIsLoadingStudio, toastNotice, setToastNotice,
  };
}

function useStudioRefs(core: ReturnType<typeof useStudioCoreState>) {
  const requestedModesRef = useRef(core.requestedModes);
  requestedModesRef.current = core.requestedModes;
  const activeModeRef = useRef(core.activeModeId);
  activeModeRef.current = core.activeModeId;
  const selectedProposalRef = useRef(core.selectedProposalId);
  selectedProposalRef.current = core.selectedProposalId;
  const referenceAppsRef = useRef(core.referenceAppsText);
  referenceAppsRef.current = core.referenceAppsText;
  const auditItemsRef = useRef(core.auditItems);
  auditItemsRef.current = core.auditItems;
  const styleOverridesRef = useRef<Record<string, ProposalStyleOverride>>({});
  return { requestedModesRef, activeModeRef, selectedProposalRef, referenceAppsRef, auditItemsRef, styleOverridesRef };
}

interface StudioMutationsOpts {
  projectId: string | undefined;
  isValidProject: boolean;
  core: ReturnType<typeof useStudioCoreState>;
  refs: ReturnType<typeof useStudioRefs>;
  setHeritagePacks: React.Dispatch<React.SetStateAction<HeritageDesignPack[]>>;
  fetchBrandProposals: (o?: FetchProposalsOptions) => Promise<void>;
  runComponentScan: (selectedKinds?: string[]) => Promise<void>;
  onApplyBrandTokens?: (proposal: BrandProposalV1, activeModeId: string) => void;
  onSyncToMainChat?: (summaryText: string) => void;
}

function useHeritagePackActions(opts: StudioMutationsOpts) {
  const { projectId, isValidProject, core, refs, setHeritagePacks, fetchBrandProposals } = opts;

  const saveCurrentHeritagePack = useCallback(async (name: string, nextEntryMode: AdaptiveEntryMode) => {
    if (!isValidProject || !projectId) return;
    const activeProposal = core.proposals.find((p) => p.id === core.selectedProposalId) ?? core.proposals[0];
    const selectedComps = refs.auditItemsRef.current.filter((i) => i.selected).map((i) => i.componentKind);
    const res = await saveHeritagePack({
      sourceProjectId: projectId,
      name,
      entryMode: nextEntryMode,
      brandIdentity: activeProposal ? { proposalId: activeProposal.id, brandName: activeProposal.brandName } : {},
      componentRegistry: { components: selectedComps },
      presentationTheme: { aspectRatio: "16:9", activeModeId: core.activeModeId },
      themeModes: refs.requestedModesRef.current,
    });
    setHeritagePacks((prev) => [res.data.pack, ...prev]);
    core.setToastNotice(`Saved Heritage Pack: ${res.data.pack.name}`);
  }, [core, isValidProject, projectId, refs, setHeritagePacks]);

  const applyHeritagePack = useCallback((pack: HeritageDesignPack) => {
    const nextMode = pack.themeModes[0] ?? "warm";
    refs.requestedModesRef.current = pack.themeModes;
    refs.activeModeRef.current = nextMode;
    core.setRequestedModes(pack.themeModes);
    core.setEntryMode(pack.entryMode);
    core.setActiveModeId(nextMode);
    void fetchBrandProposals({ modes: pack.themeModes, activeMode: nextMode });
    core.setToastNotice(`Loaded Heritage Pack: ${pack.name}`);
  }, [core, fetchBrandProposals, refs]);

  return { saveCurrentHeritagePack, applyHeritagePack };
}

function appendBoundedMode(modes: string[], modeId: string): string[] {
  if (modes.includes(modeId)) return modes;
  return modes.length >= MAX_REQUESTED_MODES ? [...modes.slice(1), modeId] : [...modes, modeId];
}

function recordProposalStyleOverride(
  opts: StudioMutationsOpts,
  targetId: string,
  patch: ProposalStyleOverride
): void {
  const clean: ProposalStyleOverride = {};
  if (patch.radiusScale) clean.radiusScale = patch.radiusScale;
  if (patch.density) clean.density = patch.density;
  if (Object.keys(clean).length === 0) return;
  opts.refs.styleOverridesRef.current[targetId] = { ...(opts.refs.styleOverridesRef.current[targetId] ?? {}), ...clean };
  opts.core.setProposals((prev) => prev.map((p) => (p.id === targetId ? { ...p, ...clean } : p)));
}

function useBrandProposalMutations(opts: StudioMutationsOpts) {
  const { core, refs, fetchBrandProposals } = opts;

  const updateActiveProposalStyle = useCallback((patch: ProposalStyleOverride) => {
    recordProposalStyleOverride(opts, refs.selectedProposalRef.current, patch);
  }, [opts, refs]);

  const applySubChatBatch = useCallback((batch: BrandSubChatBatchMutation) => {
    const targetId = batch.nextProposalId ?? refs.selectedProposalRef.current;
    if (batch.nextProposalId) {
      refs.selectedProposalRef.current = batch.nextProposalId;
      core.setSelectedProposalId(batch.nextProposalId);
    }
    recordProposalStyleOverride(opts, targetId, { radiusScale: batch.radiusScale, density: batch.density });
    const nextModes = batch.nextModeId ? appendBoundedMode(refs.requestedModesRef.current, batch.nextModeId) : refs.requestedModesRef.current;
    const nextActive = batch.nextModeId ?? refs.activeModeRef.current;
    if (batch.nextModeId) {
      refs.requestedModesRef.current = nextModes;
      refs.activeModeRef.current = nextActive;
      core.setRequestedModes(nextModes);
      core.setActiveModeId(nextActive);
    }
    if (batch.nextReferenceAppsText) {
      refs.referenceAppsRef.current = batch.nextReferenceAppsText;
      core.setReferenceAppsText(batch.nextReferenceAppsText);
    }
    if (batch.nextProposalId || batch.nextModeId || batch.nextReferenceAppsText) {
      void fetchBrandProposals({ modes: nextModes, activeMode: nextActive, proposalId: targetId, refsText: batch.nextReferenceAppsText ?? refs.referenceAppsRef.current });
    }
  }, [core, fetchBrandProposals, opts, refs]);

  const selectMode = useCallback((modeId: string) => {
    refs.activeModeRef.current = modeId;
    core.setActiveModeId(modeId);
    void fetchBrandProposals({ activeMode: modeId });
  }, [core, fetchBrandProposals, refs]);

  const selectProposal = useCallback((proposalId: string) => {
    refs.selectedProposalRef.current = proposalId;
    core.setSelectedProposalId(proposalId);
    void fetchBrandProposals({ proposalId });
  }, [core, fetchBrandProposals, refs]);

  return { updateActiveProposalStyle, applySubChatBatch, selectMode, selectProposal };
}

function useStudioMutations(opts: StudioMutationsOpts) {
  const { core, refs, fetchBrandProposals, runComponentScan, onApplyBrandTokens, onSyncToMainChat } = opts;
  const heritageActions = useHeritagePackActions(opts);
  const proposalActions = useBrandProposalMutations(opts);

  const toggleRequestedMode = useCallback((modeId: string) => {
    const prev = refs.requestedModesRef.current;
    const next = prev.includes(modeId)
      ? (prev.length > MIN_REQUESTED_MODES ? prev.filter((m) => m !== modeId) : prev)
      : appendBoundedMode(prev, modeId);
    const nextActive = next.includes(refs.activeModeRef.current) ? refs.activeModeRef.current : next[0]!;
    refs.requestedModesRef.current = next;
    refs.activeModeRef.current = nextActive;
    core.setRequestedModes(next);
    core.setActiveModeId(nextActive);
    void fetchBrandProposals({ modes: next, activeMode: nextActive });
  }, [core, fetchBrandProposals, refs]);

  const toggleAuditComponent = useCallback((componentKind: string) => {
    const next = refs.auditItemsRef.current.map((it) =>
      it.componentKind === componentKind ? { ...it, selected: !it.selected } : it
    );
    refs.auditItemsRef.current = next;
    core.setAuditItems(next);
    void runComponentScan(next.filter((i) => i.selected).map((i) => i.componentKind));
  }, [core, refs, runComponentScan]);

  const applySelectedBrand = useCallback(() => {
    const activeProposal = core.proposals.find((p) => p.id === core.selectedProposalId) ?? core.proposals[0];
    if (activeProposal) {
      onApplyBrandTokens?.(activeProposal, core.activeModeId);
      onSyncToMainChat?.(`[Brand Studio] Đã áp dụng ${activeProposal.brandName} (${core.activeModeId} · ${activeProposal.radiusScale} · ${activeProposal.density})`);
      core.setToastNotice(`Applied ${activeProposal.brandName} (${core.activeModeId})`);
    }
    core.setIsStudioOpen(false);
  }, [core, onApplyBrandTokens, onSyncToMainChat]);

  return { toggleRequestedMode, toggleAuditComponent, applySelectedBrand, ...proposalActions, ...heritageActions };
}

export function useBrandStudioController(params: BrandStudioControllerParams) {
  const { projectId, screenId = "default", locale, initialThemeModes, onApplyBrandTokens, onSyncToMainChat } = params;
  const isValidProject = Boolean(projectId && UUID_RE.test(projectId));
  const core = useStudioCoreState(initialThemeModes);
  const refs = useStudioRefs(core);
  const hiddenSpec = useHiddenSpecsState(projectId, isValidProject);
  const sideData = useStudioSideData(projectId, screenId, isValidProject);

  const fetchBrandProposals = useSequencedBrandFetcher({
    projectId, isValidProject, locale, ...refs,
    setProposals: core.setProposals, setActiveModeId: core.setActiveModeId,
    setSelectedProposalId: core.setSelectedProposalId, setPreviewBundle: core.setPreviewBundle,
    setIsLoadingStudio: core.setIsLoadingStudio,
  });
  const runComponentScan = useSequencedComponentScanner({
    projectId, isValidProject, setAuditItems: core.setAuditItems, setExtractedKinds: core.setExtractedKinds,
  });

  useEffect(() => {
    if (core.isStudioOpen) void Promise.all([fetchBrandProposals(), runComponentScan(), sideData.loadHeritagePacksList()]);
  }, [core.isStudioOpen]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (core.isHiddenSpecOpen) void Promise.all([hiddenSpec.loadHiddenSpecs(), sideData.loadFeContract()]);
  }, [core.isHiddenSpecOpen]); // eslint-disable-line react-hooks/exhaustive-deps

  const mutations = useStudioMutations({
    projectId, isValidProject, core, refs, setHeritagePacks: sideData.setHeritagePacks,
    fetchBrandProposals, runComponentScan, onApplyBrandTokens, onSyncToMainChat,
  });

  return {
    isStudioOpen: core.isStudioOpen, openStudio: () => core.setIsStudioOpen(true), closeStudio: () => core.setIsStudioOpen(false),
    isHiddenSpecOpen: core.isHiddenSpecOpen, openHiddenSpec: () => core.setIsHiddenSpecOpen(true), closeHiddenSpec: () => core.setIsHiddenSpecOpen(false),
    requestedModes: core.requestedModes, activeModeId: core.activeModeId,
    referenceAppsText: core.referenceAppsText, setReferenceAppsText: core.setReferenceAppsText,
    proposals: core.proposals, selectedProposalId: core.selectedProposalId, previewBundle: core.previewBundle,
    auditItems: core.auditItems, extractedKinds: core.extractedKinds,
    feContract: sideData.feContract, feContractTs: sideData.feContractTs, feContractDiskPath: sideData.feContractDiskPath,
    openSpecsFolder: sideData.openSpecsFolder, heritagePacks: sideData.heritagePacks,
    entryMode: core.entryMode, setEntryMode: core.setEntryMode,
    isLoadingStudio: core.isLoadingStudio, toastNotice: core.toastNotice, clearToast: () => core.setToastNotice(null),
    syncToMainChat: onSyncToMainChat,
    hiddenSpec, fetchBrandProposals, ...mutations,
  };
}
