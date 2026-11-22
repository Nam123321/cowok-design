// @story 02.4
// @implements AC-03, AC-05
import { getEncoding } from "js-tiktoken";

export const COWOK_DESIGN_OPT_SPINE = `# cowok-design-opt (Orchestrator Spine)
1. SSOT Law: Load project tokens, portal locks, and registered UI patterns before generating code or layouts.
2. AppShell Lock: Respect active AppShell lease boundaries; never rewrite locked chrome without matching fencingToken.
3. Minimum Floor Discovery: Inspect Tier 1 (rule) -> Tier 2 (implementation guide) -> Tier 3 (examples on demand); enrich deeper edge cases rather than truncating.
4. Token Discipline: Use CSS semantic variables (--bg-surface, --text-primary, --action-primary); zero raw hex or dark: overrides.
5. Plain UX Copy: Surface concise user language without internal engine jargon.`;

export const COWOK_DESIGN_OPT_SPINE_MAX_TOKENS = 250;

export const CRAFT_SKILL_SLUGS = [
  "archify-diagram",
  "dashboard-ux",
  "impeccable-polish",
  "oklch-palette",
  "sketch-monochrome",
  "stageshow-slide",
  "typography",
] as const;

export type CraftSkillSlug = (typeof CRAFT_SKILL_SLUGS)[number];

export interface CraftSkillDefinition {
  slug: CraftSkillSlug;
  tier1Rule: string;
  tier2Guide: string;
  tier3Examples: string;
}

export const CRAFT_SKILLS_3TIER_REGISTRY: Readonly<Record<CraftSkillSlug, CraftSkillDefinition>> = {
  "archify-diagram": {
    slug: "archify-diagram",
    tier1Rule:
      "Rule[archify-diagram]: Enforce directional node hierarchy, >=24px lane spacing, and single-purpose edge labels.",
    tier2Guide:
      "Guide[archify-diagram]: Group nodes by bounded subsystem, keep bezier crossings minimal, bind node fill/stroke to CSS theme variables, and annotate latency/ownership badges cleanly.",
    tier3Examples:
      "Example[archify-diagram]: <svg data-diagram='flow'><g class='lane-core'><rect rx='8' stroke='var(--border-subtle)' fill='var(--bg-surface)'/></g></svg>",
  },
  "dashboard-ux": {
    slug: "dashboard-ux",
    tier1Rule:
      "Rule[dashboard-ux]: Enforce 12-column Bento grid rhythm, KPI scan priority in the top fold, and zero decorative clutter.",
    tier2Guide:
      "Guide[dashboard-ux]: Pair Metric Line summary KPIs with sortable DataTableView and RightDrawer inspection; ensure responsive collapse at <=768px with unambiguous state pills.",
    tier3Examples:
      "Example[dashboard-ux]: <div className='bento-grid-12'><MetricLine label='Active Portals' value='6' /><DataTableView stickyHeader /></div>",
  },
  "impeccable-polish": {
    slug: "impeccable-polish",
    tier1Rule:
      "Rule[impeccable-polish]: Enforce optical alignment, consistent 4px/8px spatial scale, and explicit loading/empty/error states.",
    tier2Guide:
      "Guide[impeccable-polish]: Verify >=4.5:1 WCAG contrast on interactive surfaces, smooth 150ms cubic-bezier transitions, 1.85px monoline SVG iconography, and non-shifting skeletons.",
    tier3Examples:
      "Example[impeccable-polish]: <button aria-busy={isSaving} data-ff-id='primary-action'><BrandIcon size={16} strokeWidth={1.85} />Save</button>",
  },
  "oklch-palette": {
    slug: "oklch-palette",
    tier1Rule:
      "Rule[oklch-palette]: Enforce perceptually uniform OKLCH lightness steps paired into semantic Light/Dark CSS variables.",
    tier2Guide:
      "Guide[oklch-palette]: Anchor surface hierarchy by delta-L steps (L=0.98..0.92 light, L=0.14..0.22 dark), constrain chroma for readable body copy, and emit var(--*) tokens.",
    tier3Examples:
      "Example[oklch-palette]: :root { --bg-surface: oklch(0.985 0.004 250); --action-primary: oklch(0.58 0.19 258); }",
  },
  "sketch-monochrome": {
    slug: "sketch-monochrome",
    tier1Rule:
      "Rule[sketch-monochrome]: Enforce high-contrast monochrome wireframe restraint with hairline 1px borders and single accent focus.",
    tier2Guide:
      "Guide[sketch-monochrome]: Prioritize structural hierarchy, typography weight, and negative space before color; limit chromatic ink strictly to primary CTA or critical status.",
    tier3Examples:
      "Example[sketch-monochrome]: <section style={{ border: '1px solid var(--border-subtle)', background: 'var(--bg-surface)' }}>Wireframe Frame</section>",
  },
  "stageshow-slide": {
    slug: "stageshow-slide",
    tier1Rule:
      "Rule[stageshow-slide]: Enforce 16:9 viewport framing, max 1 primary claim per slide, and >=24px outer safe margins.",
    tier2Guide:
      "Guide[stageshow-slide]: Use asymmetric 60/40 visual-to-copy split, crisp headline under 9 words, tabular metrics for numeric proof, and keyboard Left/Right navigation support.",
    tier3Examples:
      "Example[stageshow-slide]: <article aspect='16/9' padding='32px'><h1>Unified Design Lock</h1><p>Six portals synchronized cleanly.</p></article>",
  },
  typography: {
    slug: "typography",
    tier1Rule:
      "Rule[typography]: Enforce modular type scale (1.250 major third), tabular numerals on metrics, and 55-72ch measure.",
    tier2Guide:
      "Guide[typography]: Separate display sans from monospace IDs/hashes (`font-variant-numeric: tabular-nums`), keep body line-height 1.5, and avoid weights thinner than 400 on dark surfaces.",
    tier3Examples:
      "Example[typography]: .metric-num { font-variant-numeric: tabular-nums; letter-spacing: -0.02em; line-height: 1.15; }",
  },
};

let cachedEncoder: ReturnType<typeof getEncoding> | null = null;

function getCl100k(): ReturnType<typeof getEncoding> {
  if (!cachedEncoder) {
    cachedEncoder = getEncoding("cl100k_base");
  }
  return cachedEncoder;
}

export function countCl100kBpeTokens(text: string): number {
  if (!text) return 0;
  return getCl100k().encode(text).length;
}

export function verifySpineBpeBudget(
  maxTokens: number = COWOK_DESIGN_OPT_SPINE_MAX_TOKENS
): { tokens: number; withinBudget: boolean } {
  const tokens = countCl100kBpeTokens(COWOK_DESIGN_OPT_SPINE);
  return { tokens, withinBudget: tokens > 0 && tokens <= maxTokens };
}

/** Pure-JS SHA-256 helper (no Node `Buffer` or `node:crypto` needed; browser-safe). */
function sha256Bytes(bytes: Uint8Array): string {
  const K = new Uint32Array([
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
  ]);
  const H = new Uint32Array([
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
  ]);
  const bitLen = bytes.length * 8;
  const paddedLen = Math.ceil((bytes.length + 9) / 64) * 64;
  const padded = new Uint8Array(paddedLen);
  padded.set(bytes);
  padded[bytes.length] = 0x80;
  const view = new DataView(padded.buffer);
  view.setUint32(paddedLen - 8, Math.floor(bitLen / 0x100000000), false);
  view.setUint32(paddedLen - 4, bitLen >>> 0, false);

  const w = new Uint32Array(64);
  for (let offset = 0; offset < paddedLen; offset += 64) {
    for (let i = 0; i < 16; i += 1) {
      w[i] = view.getUint32(offset + i * 4, false);
    }
    for (let i = 16; i < 64; i += 1) {
      const s0 = ((w[i - 15] >>> 7) | (w[i - 15] << 25)) ^ ((w[i - 15] >>> 18) | (w[i - 15] << 14)) ^ (w[i - 15] >>> 3);
      const s1 = ((w[i - 2] >>> 17) | (w[i - 2] << 15)) ^ ((w[i - 2] >>> 19) | (w[i - 2] << 13)) ^ (w[i - 2] >>> 10);
      w[i] = (w[i - 16] + s0 + w[i - 7] + s1) >>> 0;
    }
    let [a, b, c, d, e, f, g, h] = H;
    for (let i = 0; i < 64; i += 1) {
      const S1 = ((e >>> 6) | (e << 26)) ^ ((e >>> 11) | (e << 21)) ^ ((e >>> 25) | (e << 7));
      const ch = (e & f) ^ (~e & g);
      const temp1 = (h + S1 + ch + K[i] + w[i]) >>> 0;
      const S0 = ((a >>> 2) | (a << 30)) ^ ((a >>> 13) | (a << 19)) ^ ((a >>> 22) | (a << 10));
      const maj = (a & b) ^ (a & c) ^ (b & c);
      const temp2 = (S0 + maj) >>> 0;
      h = g;
      g = f;
      f = e;
      e = (d + temp1) >>> 0;
      d = c;
      c = b;
      b = a;
      a = (temp1 + temp2) >>> 0;
    }
    H[0] = (H[0] + a) >>> 0;
    H[1] = (H[1] + b) >>> 0;
    H[2] = (H[2] + c) >>> 0;
    H[3] = (H[3] + d) >>> 0;
    H[4] = (H[4] + e) >>> 0;
    H[5] = (H[5] + f) >>> 0;
    H[6] = (H[6] + g) >>> 0;
    H[7] = (H[7] + h) >>> 0;
  }
  return Array.from(H, (word) => word.toString(16).padStart(8, "0")).join("");
}

export function computeLengthPrefixedPrefixSha256(zone1Spine: string, zone2CraftPrefix: string): string {
  const enc = new TextEncoder();
  const z1 = enc.encode(zone1Spine);
  const z2 = enc.encode(zone2CraftPrefix);
  const frame = new Uint8Array(8 + z1.length + 8 + z2.length);
  const view = new DataView(frame.buffer);
  view.setUint32(0, Math.floor(z1.length / 0x100000000), false);
  view.setUint32(4, z1.length >>> 0, false);
  frame.set(z1, 8);
  const z2LenOffset = 8 + z1.length;
  view.setUint32(z2LenOffset, Math.floor(z2.length / 0x100000000), false);
  view.setUint32(z2LenOffset + 4, z2.length >>> 0, false);
  frame.set(z2, z2LenOffset + 8);
  return sha256Bytes(frame);
}

export interface LocalSkillRouteResult {
  portalId: string;
  spineTokens: number;
  selectedSlugs: CraftSkillSlug[];
  prefixSha256: string;
  zone1Spine: string;
  zone2CraftPrefix: string;
  zone3IntentTail: string;
  latencyMs: number;
}

function inferSlugsFromIntent(portalId: string, intent: string): CraftSkillSlug[] {
  const lower = intent.toLowerCase();
  const inferred = new Set<CraftSkillSlug>();
  if (/chart|kpi|dashboard|bento/.test(lower)) inferred.add("dashboard-ux");
  if (/color|palette|theme|oklch/.test(lower)) inferred.add("oklch-palette");
  if (/font|type|heading|typography/.test(lower)) inferred.add("typography");
  if (/diagram|graph|architecture/.test(lower)) inferred.add("archify-diagram");
  if (/slide|deck/.test(lower) || portalId === "Presentation") inferred.add("stageshow-slide");
  if (/wireframe|sketch|monochrome/.test(lower)) inferred.add("sketch-monochrome");
  if (inferred.size === 0 || /polish|contrast|a11y/.test(lower)) inferred.add("impeccable-polish");
  return Array.from(inferred).sort() as CraftSkillSlug[];
}

const MAX_CRAFT_SKILLS_COUNT = 7;
const MAX_PROJECT_ID_LEN = 128;
const MAX_INTENT_SUMMARY_LEN = 1000;
const ALLOWED_SSOT_PORTALS = new Set([
  "SuperAdmin",
  "TenantAdmin",
  "SalesApp",
  "MarketingWeb",
  "DeveloperPortal",
  "Presentation",
]);

function validateSkillRouteInput(input: {
  projectId: string;
  portalId: string;
  intentSummary: string;
  requestedSlugs?: CraftSkillSlug[];
  maxSpineTokens?: number;
}): { maxSpine: number; trimmedIntent: string } {
  if (!input.projectId.trim() || input.projectId.length > MAX_PROJECT_ID_LEN) {
    throw new Error(`INVALID_REQUEST: projectId must be 1..=${MAX_PROJECT_ID_LEN} chars`);
  }
  if (!ALLOWED_SSOT_PORTALS.has(input.portalId)) {
    throw new Error(`INVALID_REQUEST: Unsupported portalId: ${input.portalId}`);
  }
  const trimmedIntent = input.intentSummary.trim();
  if (!trimmedIntent || input.intentSummary.length > MAX_INTENT_SUMMARY_LEN) {
    throw new Error(`INVALID_REQUEST: intentSummary must be 1..=${MAX_INTENT_SUMMARY_LEN} characters`);
  }
  const maxSpine = input.maxSpineTokens ?? COWOK_DESIGN_OPT_SPINE_MAX_TOKENS;
  if (!Number.isInteger(maxSpine) || maxSpine < 1 || maxSpine > COWOK_DESIGN_OPT_SPINE_MAX_TOKENS) {
    throw new Error(
      `SPINE_TOKEN_BUDGET_EXCEEDED: maxSpineTokens must be between 1 and ${COWOK_DESIGN_OPT_SPINE_MAX_TOKENS}`
    );
  }
  if (input.requestedSlugs && input.requestedSlugs.length > MAX_CRAFT_SKILLS_COUNT) {
    throw new Error(`INVALID_REQUEST: requestedSlugs exceeds maximum of ${MAX_CRAFT_SKILLS_COUNT}`);
  }
  return { maxSpine, trimmedIntent };
}

export function routeCraftSkillsDeterministic(input: {
  projectId: string;
  portalId: string;
  intentSummary: string;
  requestedSlugs?: CraftSkillSlug[];
  maxSpineTokens?: number;
}): LocalSkillRouteResult {
  const start = performance.now();
  const { maxSpine, trimmedIntent } = validateSkillRouteInput(input);
  const spineTokens = countCl100kBpeTokens(COWOK_DESIGN_OPT_SPINE);
  if (spineTokens > maxSpine || spineTokens > COWOK_DESIGN_OPT_SPINE_MAX_TOKENS) {
    throw new Error(`SPINE_TOKEN_BUDGET_EXCEEDED: ${spineTokens} > ${maxSpine}`);
  }
  const candidateSlugs = input.requestedSlugs ?? inferSlugsFromIntent(input.portalId, trimmedIntent);
  for (const slug of candidateSlugs) {
    if (!Object.prototype.hasOwnProperty.call(CRAFT_SKILLS_3TIER_REGISTRY, slug)) {
      throw new Error(`INVALID_REQUEST: Unknown craft skill slug: ${slug}`);
    }
  }
  const uniqueSorted = Array.from(new Set(candidateSlugs)).sort((a, b) =>
    a < b ? -1 : a > b ? 1 : 0
  ) as CraftSkillSlug[];
  const zone2CraftPrefix = uniqueSorted
    .map((slug) => `${CRAFT_SKILLS_3TIER_REGISTRY[slug].tier1Rule}\n${CRAFT_SKILLS_3TIER_REGISTRY[slug].tier2Guide}`)
    .join("\n---\n");
  const prefixSha256 = computeLengthPrefixedPrefixSha256(COWOK_DESIGN_OPT_SPINE, zone2CraftPrefix);
  const zone3IntentTail = `portal=${input.portalId} | projectId=${input.projectId} | intent=${trimmedIntent}`;
  return {
    portalId: input.portalId,
    spineTokens,
    selectedSlugs: uniqueSorted,
    prefixSha256,
    zone1Spine: COWOK_DESIGN_OPT_SPINE,
    zone2CraftPrefix,
    zone3IntentTail,
    latencyMs: Math.max(0, performance.now() - start),
  };
}
