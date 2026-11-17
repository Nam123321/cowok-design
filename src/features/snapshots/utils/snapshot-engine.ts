// @story 05.3
import {
  type CreateSnapshotInput,
  type RevertResultResponse,
  type SnapshotFileItem,
  type VersionSnapshotResponse,
} from "../../../lib/validations/version-snapshot";
import {
  SHA256_K,
  SHA256_H_INIT,
  BITS_PER_BYTE,
  BLOCK_SIZE,
  WORDS_IN_BLOCK,
  SCHEDULE_WORDS,
  WORD_BYTES,
  HEX_PAD_LEN,
  HEX_BASE,
  PADDING_MAGIC,
  PADDING_OFFSET,
  BITS_IN_WORD,
  SHIFT_2,
  SHIFT_3,
  SHIFT_6,
  SHIFT_7,
  SHIFT_10,
  SHIFT_11,
  SHIFT_13,
  SHIFT_15,
  SHIFT_16,
  SHIFT_17,
  SHIFT_18,
  SHIFT_19,
  SHIFT_22,
  SHIFT_25,
} from "./sha256-constants";

function rotr(n: number, b: number): number {
  return (n >>> b) | (n << (BITS_IN_WORD - b));
}

function buildScheduleWord(w: Uint32Array, t: number): number {
  const s0 =
    rotr(w[t - SHIFT_15], SHIFT_7) ^
    rotr(w[t - SHIFT_15], SHIFT_18) ^
    (w[t - SHIFT_15] >>> SHIFT_3);
  const s1 =
    rotr(w[t - SHIFT_2], SHIFT_17) ^
    rotr(w[t - SHIFT_2], SHIFT_19) ^
    (w[t - SHIFT_2] >>> SHIFT_10);
  return (w[t - SHIFT_16] + s0 + w[t - SHIFT_7] + s1) >>> 0;
}

function processSha256Block(view: DataView, offset: number, h: number[]): void {
  const w = new Uint32Array(SCHEDULE_WORDS);
  for (let t = 0; t < WORDS_IN_BLOCK; t++) {
    w[t] = view.getUint32(offset + t * WORD_BYTES, false);
  }
  for (let t = WORDS_IN_BLOCK; t < SCHEDULE_WORDS; t++) {
    w[t] = buildScheduleWord(w, t);
  }

  let [a, b, c, d, e, f, g, valH] = h;
  for (let t = 0; t < SCHEDULE_WORDS; t++) {
    const s1 = rotr(e, SHIFT_6) ^ rotr(e, SHIFT_11) ^ rotr(e, SHIFT_25);
    const ch = (e & f) ^ (~e & g);
    const temp1 = (valH + s1 + ch + SHA256_K[t] + w[t]) >>> 0;
    const s0 = rotr(a, SHIFT_2) ^ rotr(a, SHIFT_13) ^ rotr(a, SHIFT_22);
    const maj = (a & b) ^ (a & c) ^ (b & c);
    const temp2 = (s0 + maj) >>> 0;

    valH = g;
    g = f;
    f = e;
    e = (d + temp1) >>> 0;
    d = c;
    c = b;
    b = a;
    a = (temp1 + temp2) >>> 0;
  }

  const state = [a, b, c, d, e, f, g, valH];
  for (let i = 0; i < state.length; i++) {
    h[i] = (h[i] + state[i]) >>> 0;
  }
}

export function computeSha256Hex(ascii: string): string {
  const encoder = new TextEncoder();
  const msg = encoder.encode(ascii);
  const l = msg.length;
  const bitLen = l * BITS_PER_BYTE;
  const newLen = ((l + PADDING_OFFSET) >> SHIFT_6 << SHIFT_6) + BLOCK_SIZE;
  const padded = new Uint8Array(newLen);
  padded.set(msg);
  padded[l] = PADDING_MAGIC;
  const view = new DataView(padded.buffer);
  view.setUint32(newLen - WORD_BYTES, bitLen, false);

  const h = [...SHA256_H_INIT];

  for (let offset = 0; offset < newLen; offset += BLOCK_SIZE) {
    processSha256Block(view, offset, h);
  }

  return h.map((v) => v.toString(HEX_BASE).padStart(HEX_PAD_LEN, "0")).join("");
}

export function computeTreeSha256(files: SnapshotFileItem[]): string {
  const sorted = [...files].sort((a, b) => a.path.localeCompare(b.path));
  return computeSha256Hex(JSON.stringify(sorted));
}

export function createSnapshotFromInput(
  projectId: string,
  tenantId: string,
  input: CreateSnapshotInput,
  existingSnapshotsCount: number = 0
): VersionSnapshotResponse {
  const now = new Date().toISOString();
  const nextVerNum = existingSnapshotsCount + 1;
  const versionTag = input.versionTag || `v1.${nextVerNum}.0`;
  const treeSha256 = computeTreeSha256(input.files);
  const id = input.id || crypto.randomUUID();
  const description = input.description || "";
  const trigger = input.trigger || "manual";

  return {
    id,
    tenantId,
    projectId,
    versionTag,
    label: input.label,
    description,
    trigger,
    treeSha256,
    proposalId: input.proposalId ?? null,
    parentSnapshotId: input.parentSnapshotId ?? null,
    chatArtifactSnapshotId: input.chatArtifactSnapshotId ?? null,
    files: input.files,
    metadata: input.metadata || {},
    createdAt: now,
  };
}

export function isHistoricalPreviewReadOnly(
  activeTurnSnapshotId: string | null | undefined
): boolean {
  return typeof activeTurnSnapshotId === "string" && activeTurnSnapshotId.trim().length > 0;
}

export function parseTurnManifestFiles(manifestJson: string): SnapshotFileItem[] {
  try {
    const parsed: unknown = JSON.parse(manifestJson);
    let rawList: unknown[] = [];
    if (Array.isArray(parsed)) {
      rawList = parsed;
    } else if (parsed && typeof parsed === "object") {
      const filesProp: unknown = Reflect.get(parsed, "files");
      if (Array.isArray(filesProp)) {
        rawList = filesProp;
      }
    }
    return rawList.filter((item): item is SnapshotFileItem => {
      if (!item || typeof item !== "object") return false;
      const candidatePath = Reflect.get(item, "path");
      const candidateContent = Reflect.get(item, "content");
      const candidateSha = Reflect.get(item, "sha256");
      const candidateSize = Reflect.get(item, "sizeBytes");
      return (
        typeof candidatePath === "string" &&
        candidatePath.length > 0 &&
        typeof candidateContent === "string" &&
        (candidateSha === undefined || typeof candidateSha === "string") &&
        (candidateSize === undefined || typeof candidateSize === "number")
      );
    });
  } catch {
    return [];
  }
}

export function buildPureRevertPreview(
  currentFiles: SnapshotFileItem[],
  targetSnapshot: VersionSnapshotResponse,
  projectId: string,
  tenantId: string
): {
  safetySnapshot: VersionSnapshotResponse;
  restoredFiles: SnapshotFileItem[];
  result: RevertResultResponse;
} {
  const safetyId = crypto.randomUUID();
  const now = new Date().toISOString();

  const safetySnapshot = createSnapshotFromInput(projectId, tenantId, {
    id: safetyId,
    label: `Pre-Revert Safety Backup (${targetSnapshot.versionTag})`,
    description: `Auto-generated safety snapshot prior to reverting to ${targetSnapshot.id}`,
    trigger: "pre_revert_safety",
    files: currentFiles,
  });

  const restoredFiles = targetSnapshot.files.map((f) => ({ ...f }));
  const result: RevertResultResponse = {
    success: true,
    revertedToSnapshotId: targetSnapshot.id,
    safetySnapshotId: safetySnapshot.id,
    restoredFilesCount: restoredFiles.length,
    timestamp: now,
  };

  return {
    safetySnapshot,
    restoredFiles,
    result,
  };
}

export interface StageMutationCallbacks<TConflict = unknown> {
  isDraftDirty?: boolean;
  conflictState?: TConflict | null;
  onDeployDraft?: () => void;
  onDiscardDraft?: () => void;
  onResolveConflict?: (choice: "keep_mine" | "take_agent" | "inspect_collisions") => void;
}

/** Strips live draft/conflict mutators whenever a read-only historical snapshot is previewed. */
export function resolveHistoricalReadOnlyStageBindings<TConflict = unknown>(
  raw: StageMutationCallbacks<TConflict>,
  isHistoricalReadOnly: boolean
): StageMutationCallbacks<TConflict> {
  if (!isHistoricalReadOnly) return raw;
  return {
    isDraftDirty: false,
    conflictState: null,
    onDeployDraft: undefined,
    onDiscardDraft: undefined,
    onResolveConflict: undefined,
  };
}

/** Guards inline stage mutation callbacks against mutating live state while Historical Preview is active. */
export function guardReadOnlyAction<TArgs extends unknown[]>(
  isReadOnly: boolean,
  fn: ((...args: TArgs) => void) | undefined
): (...args: TArgs) => void {
  return (...args: TArgs) => {
    if (!isReadOnly && fn) fn(...args);
  };
}
