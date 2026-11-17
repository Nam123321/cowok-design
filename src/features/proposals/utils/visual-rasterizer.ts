// @story 05.2
import type { ChangeProposalResponse, ComputeVisualDiffInput } from "../../../lib/validations/change-proposal";

const FRAME_WIDTH = 320;
const FRAME_HEIGHT = 200;
const DIFF_THRESHOLD = 0.1;
const CHUNK_SIZE = 0x8000;
const TITLE_CHAR_LIMIT = 28;
const BADGE_CHAR_LIMIT = 18;
const INSET_PX = 16;
const PAD_X_PX = 28;
const TITLE_Y_PX = 44;
const BAR1_Y_PX = 60;
const BAR1_W_PX = 210;
const BAR1_H_PX = 12;
const BAR2_Y_PX = 80;
const BAR2_W_PX = 160;
const BADGE_Y_PX = 102;
const BADGE_W_PX = 120;
const BADGE_TEXT_X_PX = 34;
const BADGE_TEXT_Y_PX = 113;
const CTA_Y_PX = 134;
const CTA_W_PX = 136;
const CTA_H_PX = 32;
const CTA_TEXT_X_PX = 44;
const CTA_TEXT_Y_PX = 154;

export interface RasterizedVisualFrames {
  requestPayload: ComputeVisualDiffInput;
  beforeImageUrl?: string;
  afterImageUrl?: string;
}

function bytesToBase64(bytes: Uint8ClampedArray): string {
  let binary = "";
  for (let i = 0; i < bytes.length; i += CHUNK_SIZE) {
    const chunk = bytes.subarray(i, Math.min(i + CHUNK_SIZE, bytes.length));
    binary += String.fromCharCode(...chunk);
  }
  return btoa(binary);
}

function readTokenColor(tokenName: string, fallbackRgb: string): string {
  if (typeof window === "undefined" || typeof document === "undefined") return fallbackRgb;
  const val = window.getComputedStyle(document.documentElement).getPropertyValue(tokenName).trim();
  return val || fallbackRgb;
}

function drawWireframeCard(
  ctx: CanvasRenderingContext2D,
  opts: {
    bg: string;
    cardBg: string;
    borderColor: string;
    inkColor: string;
    subtleBarColor: string;
    successColor: string;
    ctaBg: string;
    ctaFg: string;
    title: string;
    badgeText: string;
    highlightBar: boolean;
  }
): void {
  const boxW = FRAME_WIDTH - INSET_PX - INSET_PX;
  const boxH = FRAME_HEIGHT - INSET_PX - INSET_PX;
  ctx.fillStyle = opts.bg;
  ctx.fillRect(0, 0, FRAME_WIDTH, FRAME_HEIGHT);
  ctx.fillStyle = opts.cardBg;
  ctx.fillRect(INSET_PX, INSET_PX, boxW, boxH);
  ctx.strokeStyle = opts.borderColor;
  ctx.lineWidth = 1;
  ctx.strokeRect(INSET_PX, INSET_PX, boxW, boxH);

  ctx.fillStyle = opts.inkColor;
  ctx.font = "bold 13px sans-serif";
  ctx.fillText(opts.title.slice(0, TITLE_CHAR_LIMIT), PAD_X_PX, TITLE_Y_PX);

  ctx.fillStyle = opts.subtleBarColor;
  ctx.fillRect(PAD_X_PX, BAR1_Y_PX, BAR1_W_PX, BAR1_H_PX);
  ctx.fillRect(PAD_X_PX, BAR2_Y_PX, BAR2_W_PX, BAR1_H_PX);

  if (opts.highlightBar) {
    ctx.fillStyle = opts.successColor;
    ctx.fillRect(PAD_X_PX, BADGE_Y_PX, BADGE_W_PX, INSET_PX);
    ctx.fillStyle = opts.cardBg;
    ctx.font = "10px monospace";
    ctx.fillText(opts.badgeText.slice(0, BADGE_CHAR_LIMIT), BADGE_TEXT_X_PX, BADGE_TEXT_Y_PX);
  }

  ctx.fillStyle = opts.ctaBg;
  ctx.fillRect(PAD_X_PX, CTA_Y_PX, CTA_W_PX, CTA_H_PX);
  ctx.fillStyle = opts.ctaFg;
  ctx.font = "bold 12px sans-serif";
  ctx.fillText(opts.highlightBar ? "Updated CTA" : "Primary CTA", CTA_TEXT_X_PX, CTA_TEXT_Y_PX);
}

/** Rasterizes Before and After HTML proposal previews via HTMLCanvasElement 2D for pixelmatch + SSIM. */
export function rasterizeProposalVisualFrames(proposal: ChangeProposalResponse): RasterizedVisualFrames {
  if (typeof document === "undefined") return { requestPayload: {} };
  const canvas = document.createElement("canvas");
  canvas.width = FRAME_WIDTH;
  canvas.height = FRAME_HEIGHT;
  const ctx = canvas.getContext("2d");
  if (!ctx) return { requestPayload: {} };

  const bg = readTokenColor("--bg-canvas", "Canvas");
  const cardBg = readTokenColor("--bg-surface", "Window");
  const borderColor = readTokenColor("--border-default", "GrayText");
  const inkColor = readTokenColor("--text-primary", "CanvasText");
  const subtleBarColor = readTokenColor("--bg-subtle", "ButtonFace");
  const mutedCtaBg = readTokenColor("--text-muted", "GrayText");
  const successColor = readTokenColor("--state-success", "Highlight");
  const primaryCtaBg = readTokenColor("--action-primary", "ButtonText");
  const primaryCtaFg = readTokenColor("--action-primary-fg", "ButtonFace");

  drawWireframeCard(ctx, {
    bg, cardBg, borderColor, inkColor, subtleBarColor, successColor,
    ctaBg: mutedCtaBg, ctaFg: inkColor,
    title: `Before: ${proposal.title}`, badgeText: "baseline", highlightBar: false,
  });
  const beforeData = ctx.getImageData(0, 0, FRAME_WIDTH, FRAME_HEIGHT).data;
  const beforeImageUrl = canvas.toDataURL("image/png");

  const totalAdds = proposal.files.reduce((sum, f) => sum + f.additions, 0);
  drawWireframeCard(ctx, {
    bg, cardBg, borderColor, inkColor, subtleBarColor, successColor,
    ctaBg: primaryCtaBg, ctaFg: primaryCtaFg,
    title: `After: ${proposal.title}`, badgeText: `+${totalAdds} lines`, highlightBar: true,
  });
  const afterData = ctx.getImageData(0, 0, FRAME_WIDTH, FRAME_HEIGHT).data;
  const afterImageUrl = canvas.toDataURL("image/png");

  return {
    requestPayload: {
      beforeRgbaBase64: bytesToBase64(beforeData),
      afterRgbaBase64: bytesToBase64(afterData),
      width: FRAME_WIDTH,
      height: FRAME_HEIGHT,
      threshold: DIFF_THRESHOLD,
    },
    beforeImageUrl,
    afterImageUrl,
  };
}
