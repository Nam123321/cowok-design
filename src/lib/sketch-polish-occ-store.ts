// @story 05.4
// @implements Shared per-(projectId, screenId) OCC Store & Serialized Patch Queue
import {
  applySurgicalPatch,
  getScreenPatchHistory,
  SketchPolishApiError,
} from "../api/sketch-polish";
import type {
  LockedRuleChangeReceipt,
  PatchOpKind,
  ScreenPatchHistoryResponse,
  SurgicalPatchResponse,
} from "./validations/sketch-polish";

export const EMPTY_CONTENT_SHA256 =
  "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

const HTTP_CONFLICT = 409;

export interface ScreenOccState {
  rev: number;
  sha: string;
}

export interface PendingLockedPatchDescriptor {
  projectId: string;
  screenId: string;
  ffId: string;
  opKind: PatchOpKind;
  payloadJson: Record<string, unknown>;
  onApplied?: () => void;
}

export interface LockedRuleReceiptNotice {
  receipt: LockedRuleChangeReceipt;
  pendingPatch: PendingLockedPatchDescriptor;
}

export interface PatchHistoryEntryLike {
  astRevision: number;
  ffId: string;
  opKind: string;
  applyTier?: string;
  payloadJson: Record<string, unknown>;
  lockedRuleReceipt?: { applied: boolean } | null;
}

const occMap = new Map<string, ScreenOccState>();
const queueMap = new Map<string, Promise<unknown>>();
const baseHtmlProviderMap = new Map<string, () => string | undefined>();

function makeScreenKey(projectId: string, screenId: string): string {
  return `${projectId}::${screenId}`;
}

export function registerScreenBaseHtmlProvider(
  projectId: string | undefined,
  screenId: string,
  provider: (() => string | undefined) | null
): void {
  if (!projectId || !screenId) return;
  const key = makeScreenKey(projectId, screenId);
  const fallbackKey = makeScreenKey(projectId, "*");
  if (provider) {
    baseHtmlProviderMap.set(key, provider);
    baseHtmlProviderMap.set(fallbackKey, provider);
  } else {
    baseHtmlProviderMap.delete(key);
    baseHtmlProviderMap.delete(fallbackKey);
  }
}

export function computeEffectivePatchStack<T extends PatchHistoryEntryLike>(
  patchesDesc: ReadonlyArray<T>
): T[] {
  const orderedAsc = [...patchesDesc].reverse();
  const stack: T[] = [];
  for (const patch of orderedAsc) {
    if (patch.lockedRuleReceipt && patch.lockedRuleReceipt.applied === false) {
      continue;
    }
    if (patch.opKind === "UNDO") {
      stack.pop();
    } else {
      stack.push(patch);
    }
  }
  return stack;
}

export function getScreenOccState(projectId: string, screenId: string): ScreenOccState {
  const key = makeScreenKey(projectId, screenId);
  return occMap.get(key) ?? { rev: 0, sha: EMPTY_CONTENT_SHA256 };
}

export function setScreenOccState(projectId: string, screenId: string, state: ScreenOccState): void {
  occMap.set(makeScreenKey(projectId, screenId), state);
}

async function fetchAndStoreScreenOccHistory(
  projectId: string,
  screenId: string
): Promise<ScreenPatchHistoryResponse> {
  const res = await getScreenPatchHistory(projectId, screenId);
  setScreenOccState(projectId, screenId, {
    rev: res.data.astRevision,
    sha: res.data.currentArtifactSha256 || EMPTY_CONTENT_SHA256,
  });
  return res;
}

export function hydrateScreenOccHistory(
  projectId: string,
  screenId: string
): Promise<ScreenPatchHistoryResponse> {
  const key = makeScreenKey(projectId, screenId);
  const prev = queueMap.get(key) ?? Promise.resolve();
  const next = prev.catch(() => undefined).then(() => fetchAndStoreScreenOccHistory(projectId, screenId));
  queueMap.set(key, next);
  return next;
}

async function runSinglePatchWithRetry(opts: {
  projectId: string;
  screenId: string;
  ffId: string;
  opKind: PatchOpKind;
  payloadJson: Record<string, unknown>;
  baseHtml?: string;
  confirmLockedRuleOverride?: boolean;
}): Promise<SurgicalPatchResponse> {
  const key = makeScreenKey(opts.projectId, opts.screenId);
  const submit = async () => {
    const occ = getScreenOccState(opts.projectId, opts.screenId);
    const provider = baseHtmlProviderMap.get(key) ?? baseHtmlProviderMap.get(makeScreenKey(opts.projectId, "*"));
    const seedHtml = occ.rev === 0 ? (opts.baseHtml ?? provider?.()) : opts.baseHtml;
    const res = await applySurgicalPatch({
      projectId: opts.projectId,
      screenId: opts.screenId,
      baseArtifactSha256: occ.sha || EMPTY_CONTENT_SHA256,
      ffId: opts.ffId,
      opKind: opts.opKind,
      payloadJson: opts.payloadJson,
      baseHtml: seedHtml && seedHtml.trim().length > 0 ? seedHtml : undefined,
      expectedAstRevision: occ.rev,
      confirmLockedRuleOverride: opts.confirmLockedRuleOverride,
    });
    setScreenOccState(opts.projectId, opts.screenId, {
      rev: res.data.astRevision,
      sha: res.data.resultArtifactSha256,
    });
    return res;
  };

  try {
    return await submit();
  } catch (err) {
    if (err instanceof SketchPolishApiError && err.status === HTTP_CONFLICT && err.code === "STALE_MAPPING") {
      await fetchAndStoreScreenOccHistory(opts.projectId, opts.screenId);
      return submit();
    }
    throw err;
  }
}

export function enqueueSerializedScreenPatch(opts: {
  projectId: string;
  screenId: string;
  ffId: string;
  opKind: PatchOpKind;
  payloadJson: Record<string, unknown>;
  baseHtml?: string;
  confirmLockedRuleOverride?: boolean;
}): Promise<SurgicalPatchResponse> {
  const key = makeScreenKey(opts.projectId, opts.screenId);
  const prev = queueMap.get(key) ?? Promise.resolve();
  const next = prev.catch(() => undefined).then(() => runSinglePatchWithRetry(opts));
  queueMap.set(key, next);
  return next;
}
