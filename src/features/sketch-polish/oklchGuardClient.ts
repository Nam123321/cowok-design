// @story 05.4
// @implements AC-02 (Deterministic Client OKLCH WCAG AA Contrast Guard >= 4.5:1)

export const WCAG_AA_MIN_RATIO = 4.5;
const HEX_RADIX = 16;
const SRGB_THRESHOLD = 0.04045;
const SRGB_SLOPE = 12.92;
const SRGB_OFFSET = 0.055;
const SRGB_DIVISOR = 1.055;
const SRGB_GAMMA = 2.4;
const LUM_R = 0.2126;
const LUM_G = 0.7152;
const LUM_B = 0.0722;
const LUM_OFFSET = 0.05;
const MAX_CHANNEL = 255;
const HEX_SHORT_LEN = 3;
const HEX_FULL_LEN = 6;
const HEX_G_START = 2;
const HEX_B_START = 4;
const RATIO_PRECISION = 100;

export interface ClientContrastCheck {
  ratio: number;
  passesAa: boolean;
  fgHex: string;
  bgHex: string;
}

const RGB_FUNC_RE = /^rgba?\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})(?:\s*,\s*[\d.]+)?\s*\)$/i;

function parseCssColorRgb(raw: string): [number, number, number] | null {
  const trimmed = raw.trim();
  const rgbMatch = RGB_FUNC_RE.exec(trimmed);
  if (rgbMatch) {
    const r = Number(rgbMatch[1]);
    const g = Number(rgbMatch[HEX_G_START]);
    const b = Number(rgbMatch[HEX_SHORT_LEN]);
    if (r <= MAX_CHANNEL && g <= MAX_CHANNEL && b <= MAX_CHANNEL) {
      return [r, g, b];
    }
    return null;
  }
  const clean = trimmed.replace(/^#/, "");
  if (clean.length === HEX_SHORT_LEN && /^[0-9a-fA-F]{3}$/.test(clean)) {
    return [
      parseInt(clean[0]! + clean[0]!, HEX_RADIX),
      parseInt(clean[1]! + clean[1]!, HEX_RADIX),
      parseInt(clean[HEX_G_START]! + clean[HEX_G_START]!, HEX_RADIX),
    ];
  }
  if (clean.length === HEX_FULL_LEN && /^[0-9a-fA-F]{6}$/.test(clean)) {
    return [
      parseInt(clean.slice(0, HEX_G_START), HEX_RADIX),
      parseInt(clean.slice(HEX_G_START, HEX_B_START), HEX_RADIX),
      parseInt(clean.slice(HEX_B_START, HEX_FULL_LEN), HEX_RADIX),
    ];
  }
  return null;
}

function toLinear(channel: number): number {
  const c = channel / MAX_CHANNEL;
  return c <= SRGB_THRESHOLD ? c / SRGB_SLOPE : Math.pow((c + SRGB_OFFSET) / SRGB_DIVISOR, SRGB_GAMMA);
}

function relativeLuminance(rgb: [number, number, number]): number {
  return LUM_R * toLinear(rgb[0]) + LUM_G * toLinear(rgb[1]) + LUM_B * toLinear(rgb[2]);
}

export function evaluateClientContrast(
  fgHex: string,
  bgHex: string,
  minRatio = WCAG_AA_MIN_RATIO
): ClientContrastCheck {
  const fg = parseCssColorRgb(fgHex);
  const bg = parseCssColorRgb(bgHex);
  if (!fg || !bg) {
    return { ratio: 0, passesAa: false, fgHex, bgHex };
  }
  const la = relativeLuminance(fg);
  const lb = relativeLuminance(bg);
  const hi = Math.max(la, lb);
  const lo = Math.min(la, lb);
  const ratio = (hi + LUM_OFFSET) / (lo + LUM_OFFSET);
  return {
    ratio: Math.round(ratio * RATIO_PRECISION) / RATIO_PRECISION,
    passesAa: ratio >= minRatio,
    fgHex,
    bgHex,
  };
}
