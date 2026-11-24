// @story 08.7
// @implements AC-04 (Bridge 1 Non-Blocking Zero-PII Studio Correction Client -> POST /api/v1/learn/studio-corrections)
import { recordStudioCorrection } from "../../../../api/ai-harness";
import {
  type RecordStudioCorrectionRequest,
  RecordStudioCorrectionRequestSchema,
  type StudioCorrectionKind,
} from "../../../../lib/validations/ai-harness";
import type { StudioSnapshot } from "./types";

const MAX_SEGMENT_LEN = 32;
const UNSAFE_SEGMENT_CHARS = /[^a-z0-9_-]+/g;
const DATA_VIZ_SKILL_ID = "data-viz-storyteller";

function sanitizeStructuralSegment(raw: string): string {
  const cleaned = raw
    .trim()
    .toLowerCase()
    .replace(UNSAFE_SEGMENT_CHARS, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, MAX_SEGMENT_LEN);
  return cleaned || "unknown";
}

/**
 * Builds a strictly Zero-PII structural signature key (`^[a-z0-9:_-]+$`).
 * Never includes user CSV content, row labels, titles, or free-text values.
 */
export function buildZeroPiiCorrectionSignature(
  correctionKind: StudioCorrectionKind,
  fromToken?: string,
  toToken?: string
): string {
  const parts: string[] = [correctionKind];
  if (fromToken) parts.push(sanitizeStructuralSegment(fromToken));
  if (toToken) parts.push(sanitizeStructuralSegment(toToken));
  return parts.join(":");
}

export interface BuildCorrectionInput {
  projectId: string;
  correctionKind: StudioCorrectionKind;
  fromValue?: string;
  toValue?: string;
  skillId?: string;
}

/**
 * Validates and returns a strictly Zero-PII Bridge 1 payload, or `null` if invalid.
 */
export function buildStudioCorrectionPayload(
  input: BuildCorrectionInput
): RecordStudioCorrectionRequest | null {
  const projectId = input.projectId.trim();
  if (!projectId) return null;
  const safeFrom = input.fromValue ? sanitizeStructuralSegment(input.fromValue) : undefined;
  const safeTo = input.toValue ? sanitizeStructuralSegment(input.toValue) : undefined;
  const signatureKey = buildZeroPiiCorrectionSignature(input.correctionKind, safeFrom, safeTo);
  const candidate = {
    projectId,
    correctionKind: input.correctionKind,
    signatureKey,
    ...(safeFrom ? { fromValue: safeFrom } : {}),
    ...(safeTo ? { toValue: safeTo } : {}),
    skillId: input.skillId ? sanitizeStructuralSegment(input.skillId) : DATA_VIZ_SKILL_ID,
  };
  const parsed = RecordStudioCorrectionRequestSchema.safeParse(candidate);
  return parsed.success ? parsed.data : null;
}

/**
 * Detects structural manual overrides between two consecutive `StudioSnapshot` states
 * (excluding raw user data/titles) for Bridge 1 learning signals.
 */
export function detectStudioOverrideCorrection(
  projectId: string,
  prev: StudioSnapshot,
  next: StudioSnapshot
): RecordStudioCorrectionRequest | null {
  if (prev.chartType !== next.chartType) {
    return buildStudioCorrectionPayload({
      projectId,
      correctionKind: "chart_type_override",
      fromValue: prev.chartType,
      toValue: next.chartType,
    });
  }
  if (prev.paletteId !== next.paletteId) {
    return buildStudioCorrectionPayload({
      projectId,
      correctionKind: "palette_contrast_fix",
      fromValue: prev.paletteId,
      toValue: next.paletteId,
    });
  }
  if (prev.audienceMode !== next.audienceMode) {
    return buildStudioCorrectionPayload({
      projectId,
      correctionKind: "density_mode_override",
      fromValue: prev.audienceMode,
      toValue: next.audienceMode,
    });
  }
  return null;
}

/**
 * Non-blocking, fire-and-forget Bridge 1 emitter. Never throws or blocks Studio UI interactions.
 */
export function emitStudioCorrectionNonBlocking(
  input: BuildCorrectionInput,
  sender: (payload: RecordStudioCorrectionRequest) => Promise<unknown> = recordStudioCorrection
): boolean {
  const payload = buildStudioCorrectionPayload(input);
  if (!payload) return false;
  void Promise.resolve()
    .then(() => sender(payload))
    .catch(() => undefined);
  return true;
}
