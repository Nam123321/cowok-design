// @story 05.6
// @implements [5.26] Brand Studio Scoped Sub-Chat Command Interpreter & Main-Chat Sync Builder
import type { BrandProposalV1 } from "../../lib/validations/cognitive-studio";

export interface BrandSubChatCommandResult {
  reply: string;
  syncSummary: string;
  nextModeId?: string;
  nextProposalId?: string;
  nextReferenceAppsText?: string;
  radiusScale?: BrandProposalV1["radiusScale"];
  density?: BrandProposalV1["density"];
}

const KNOWN_MODES = ["warm", "light", "dim", "dark", "night"] as const;

function detectRequestedMode(lower: string): string | undefined {
  return KNOWN_MODES.find((m) => lower.includes(m));
}

function detectRadiusScale(lower: string): BrandProposalV1["radiusScale"] | undefined {
  if (lower.includes("sharp") || lower.includes("vuông") || lower.includes("sắc")) return "sharp";
  if (lower.includes("soft") || lower.includes("mềm") || lower.includes("bo tròn")) return "soft";
  if (lower.includes("balanced") || lower.includes("cân bằng")) return "balanced";
  return undefined;
}

function detectDensity(lower: string): BrandProposalV1["density"] | undefined {
  if (lower.includes("compact") || lower.includes("gọn") || lower.includes("dày")) return "compact";
  if (lower.includes("airy") || lower.includes("thoáng") || lower.includes("rộng")) return "airy";
  if (lower.includes("comfortable") || lower.includes("thoải mái")) return "comfortable";
  return undefined;
}

function detectProposalMatch(lower: string, proposals: readonly BrandProposalV1[]): BrandProposalV1 | undefined {
  return proposals.find((p) => lower.includes(p.brandName.toLowerCase()) || lower.includes(p.id.toLowerCase()));
}

function detectReferenceApps(raw: string): string | undefined {
  const match = raw.match(/(?:tham\s*chiếu|reference|giống|like)\s*[:\-]?\s*([A-Za-z0-9,.\s-]{2,64})/i);
  return match?.[1]?.trim() || undefined;
}

export interface BrandSubChatCommandInput {
  rawPrompt: string;
  proposals: readonly BrandProposalV1[];
  activeProposal: BrandProposalV1 | null;
  activeModeId: string;
  selectedFfId: string | null;
}

export function interpretBrandSubChatCommand(input: BrandSubChatCommandInput): BrandSubChatCommandResult {
  const { rawPrompt, proposals, activeProposal, activeModeId, selectedFfId } = input;
  const lower = rawPrompt.trim().toLowerCase();
  const nextModeId = detectRequestedMode(lower);
  const matchedProposal = detectProposalMatch(lower, proposals);
  const radiusScale = detectRadiusScale(lower);
  const density = detectDensity(lower);
  const nextReferenceAppsText = detectReferenceApps(rawPrompt);

  const appliedNotes: string[] = [];
  if (matchedProposal) appliedNotes.push(`Brand: ${matchedProposal.brandName}`);
  if (nextModeId) appliedNotes.push(`Mode: ${nextModeId}`);
  if (radiusScale) appliedNotes.push(`Radius: ${radiusScale}`);
  if (density) appliedNotes.push(`Density: ${density}`);
  if (nextReferenceAppsText) appliedNotes.push(`Refs: ${nextReferenceAppsText}`);

  const scopeNote = selectedFfId ? ` (${selectedFfId})` : "";
  const brandLabel = matchedProposal?.brandName ?? activeProposal?.brandName ?? "Brand Studio";
  const reply = appliedNotes.length > 0
    ? `Đã cập nhật ${brandLabel}${scopeNote}: ${appliedNotes.join(" · ")}.`
    : `Đã ghi nhận yêu cầu cho ${brandLabel}${scopeNote} (${activeModeId}): "${rawPrompt.trim()}".`;

  const syncSummary = `[Brand Studio] ${brandLabel} (${nextModeId ?? activeModeId})${scopeNote}: ${appliedNotes.length > 0 ? appliedNotes.join(", ") : rawPrompt.trim()}`;

  return {
    reply,
    syncSummary,
    nextModeId,
    nextProposalId: matchedProposal?.id,
    nextReferenceAppsText,
    radiusScale,
    density,
  };
}
