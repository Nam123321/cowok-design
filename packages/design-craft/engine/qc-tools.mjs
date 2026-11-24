import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PathJailError, readJailedFile } from "./path-jail.mjs";
import { resolveHostUpgrade } from "./builder-tools.mjs";

const ENGINE_DIR = path.dirname(fileURLToPath(import.meta.url));
const BUNDLE_ROOT = path.resolve(ENGINE_DIR, "..");

function loadCatalog(relPath) {
  return JSON.parse(fs.readFileSync(path.join(BUNDLE_ROOT, relPath), "utf8"));
}

function clamp01(v) {
  return Math.min(1, Math.max(0, v));
}

function srgbChannelToLinear(c) {
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

function parseHexToLinearRgb(hex) {
  const clean = hex.trim().replace(/^#/, "");
  const expanded =
    clean.length === 3
      ? clean.split("").map((ch) => ch + ch).join("")
      : clean;
  if (!/^[0-9a-fA-F]{6}$/.test(expanded)) {
    throw new Error(`Invalid hex color '${hex}'`);
  }
  const r = parseInt(expanded.slice(0, 2), 16) / 255;
  const g = parseInt(expanded.slice(2, 4), 16) / 255;
  const b = parseInt(expanded.slice(4, 6), 16) / 255;
  return [srgbChannelToLinear(r), srgbChannelToLinear(g), srgbChannelToLinear(b)];
}

function parseOklchToLinearRgb(str) {
  const match = str.trim().match(/^oklch\(\s*([0-9.]+%?)\s+([0-9.]+)\s+([0-9.]+)(?:deg)?\s*\)$/i);
  if (!match) {
    throw new Error(`Invalid OKLCH color '${str}'`);
  }
  const rawL = match[1];
  const L = rawL.endsWith("%") ? parseFloat(rawL) / 100 : parseFloat(rawL);
  const C = parseFloat(match[2]);
  const hRad = (parseFloat(match[3]) * Math.PI) / 180;
  const a = C * Math.cos(hRad);
  const b = C * Math.sin(hRad);
  const l_ = L + 0.3963377774 * a + 0.2158037573 * b;
  const m_ = L - 0.1055613458 * a - 0.0638541728 * b;
  const s_ = L - 0.0894841775 * a - 1.2914855480 * b;
  const l = l_ * l_ * l_;
  const m = m_ * m_ * m_;
  const s = s_ * s_ * s_;
  const rLin = clamp01(+4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s);
  const gLin = clamp01(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s);
  const bLin = clamp01(-0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s);
  return [rLin, gLin, bLin];
}

function colorToLuminance(color) {
  const [r, g, b] = color.trim().startsWith("#")
    ? parseHexToLinearRgb(color)
    : parseOklchToLinearRgb(color);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrastRatio(fg, bg) {
  const l1 = colorToLuminance(fg);
  const l2 = colorToLuminance(bg);
  const lighter = Math.max(l1, l2);
  const darker = Math.min(l1, l2);
  return Number(((lighter + 0.05) / (darker + 0.05)).toFixed(2));
}

export function qcA11yContrast(input = {}, ctx = {}) {
  const pairs = Array.isArray(input.pairs) ? input.pairs : [];
  const results = [];
  const violations = [];
  for (const pair of pairs) {
    const ratio = contrastRatio(String(pair.fg), String(pair.bg));
    const minRequired = pair.largeText ? 3.0 : 4.5;
    const passed = ratio >= minRequired;
    const entry = {
      name: String(pair.name ?? "unnamed-pair"),
      fg: pair.fg,
      bg: pair.bg,
      ratio,
      minRequired,
      aaPass: passed,
      aaaPass: ratio >= (pair.largeText ? 4.5 : 7.0),
    };
    results.push(entry);
    if (!passed) {
      violations.push(entry);
    }
  }
  const passRatio = results.length === 0 ? 1 : (results.length - violations.length) / results.length;
  return {
    tool: "qc-a11y-contrast",
    passed: violations.length === 0,
    checkedPairs: results.length,
    pairs: results,
    violations,
    score: Number((passRatio * 10).toFixed(2)),
    hostUpgrade: resolveHostUpgrade("qc-a11y-contrast", ctx.env),
  };
}

const JARGON_PATTERNS = [
  /\bBYOS\b/g,
  /\bCRDT\b/g,
  /\bSQLite\s+WAL\b/gi,
  /\bJSON-RPC\b/gi,
  /\bSSOT\b/g,
  /\bMCP\s+stdio\b/gi,
  /\bFMEA\b/g,
  /\bRBAC\/PBAC\b/g,
];

const PLACEHOLDER_PATTERNS = [
  /\blorem\s+ipsum\b/gi,
  /\bTODO\b/g,
  /\bFIXME\b/g,
  /\bTBD\b/g,
  /\bcoming\s+soon\b/gi,
  /\bjohn\s+doe\b/gi,
];

const CLICHE_PATTERNS = [
  /\bsupercharge\s+your\s+workflow\b/gi,
  /\bseamless\s+synergy\b/gi,
  /\brevolutionize\s+your\b/gi,
];

function collectPatternMatches(content, patterns, ruleId, messagePrefix) {
  const found = [];
  for (const regex of patterns) {
    const matches = content.match(regex);
    if (matches) {
      for (const m of matches) {
        found.push({ ruleId, severity: "error", match: m, message: `${messagePrefix}: '${m}'` });
      }
    }
  }
  return found;
}

export function qcAntiSlopLinter(input = {}, ctx = {}) {
  let content = typeof input.content === "string" ? input.content : "";
  if (!content && input.filePath) {
    content = readJailedFile(ctx.rootDir, input.filePath);
  }
  const violations = [
    ...collectPatternMatches(content, JARGON_PATTERNS, "ANTI_SLOP_JARGON", "Internal engineering jargon exposed in UI"),
    ...collectPatternMatches(content, PLACEHOLDER_PATTERNS, "ANTI_SLOP_PLACEHOLDER", "Placeholder copy detected"),
    ...collectPatternMatches(content, CLICHE_PATTERNS, "ANTI_SLOP_CLICHE", "Generic AI marketing cliché detected"),
  ];
  const darkClassMatches = content.match(/\bdark:[a-z0-9-:/[\]]+/g) ?? [];
  for (const m of darkClassMatches) {
    violations.push({
      ruleId: "FORBIDDEN_TAILWIND_DARK",
      severity: "error",
      match: m,
      message: `Forbidden Tailwind dark: override '${m}' (use semantic CSS variables instead)`,
    });
  }
  if (!input.allowRawHex) {
    const rawHexMatches = content.match(/#[0-9a-fA-F]{6}\b/g) ?? [];
    for (const m of rawHexMatches) {
      violations.push({
        ruleId: "RAW_HEX_COLOR",
        severity: "warning",
        match: m,
        message: `Raw hex color '${m}' detected outside token sheet`,
      });
    }
  }
  const hardErrors = violations.filter((v) => v.severity === "error");
  const score = Math.max(0, Number((10 - hardErrors.length * 2.5 - (violations.length - hardErrors.length) * 0.5).toFixed(2)));
  return {
    tool: "qc-anti-slop-linter",
    passed: hardErrors.length === 0,
    violationCount: violations.length,
    errorCount: hardErrors.length,
    violations,
    score,
    hostUpgrade: resolveHostUpgrade("qc-anti-slop-linter", ctx.env),
  };
}

function boxesOverlap(a, b) {
  return a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y;
}

export function qcLayoutOverflow(input = {}, ctx = {}) {
  const preset = input.preset ?? "desktop";
  const defaults = {
    desktop: { width: 1440, height: 900, checkVertical: false },
    mobile: { width: 390, height: 844, checkVertical: false },
    "slide-16-9": { width: 1280, height: 720, checkVertical: true },
  };
  const base = typeof preset === "string" && Object.hasOwn(defaults, preset) ? defaults[preset] : defaults.desktop;
  const width = Number(input.viewport?.width ?? base.width);
  const height = Number(input.viewport?.height ?? base.height);
  if (!Number.isFinite(width) || width <= 0 || !Number.isFinite(height) || height <= 0) {
    throw new PathJailError(
      "INVALID_VIEWPORT",
      `viewport width (${String(input.viewport?.width)}) and height (${String(input.viewport?.height)}) must be finite positive numbers`
    );
  }
  const viewport = {
    width,
    height,
    checkVertical: input.viewport?.checkVertical ?? base.checkVertical,
  };
  const boxes = Array.isArray(input.boxes) ? input.boxes : [];
  const overflows = [];
  const overlaps = [];
  for (const box of boxes) {
    if (box.x < 0 || box.x + box.width > viewport.width) {
      overflows.push({ id: box.id, axis: "x", extent: box.x + box.width, limit: viewport.width });
    }
    if (viewport.checkVertical && (box.y < 0 || box.y + box.height > viewport.height)) {
      overflows.push({ id: box.id, axis: "y", extent: box.y + box.height, limit: viewport.height });
    }
  }
  for (let i = 0; i < boxes.length; i += 1) {
    for (let j = i + 1; j < boxes.length; j += 1) {
      if (boxes[i].layer === boxes[j].layer && boxesOverlap(boxes[i], boxes[j])) {
        overlaps.push({ a: boxes[i].id, b: boxes[j].id });
      }
    }
  }
  if (typeof input.html === "string") {
    for (const match of input.html.matchAll(/width\s*:\s*(\d+)px/gi)) {
      const px = Number(match[1]);
      if (px > viewport.width) {
        overflows.push({ id: "inline-css", axis: "x", extent: px, limit: viewport.width });
      }
    }
  }
  const issuesCount = overflows.length + overlaps.length;
  return {
    tool: "qc-layout-overflow",
    passed: issuesCount === 0,
    preset,
    viewport,
    overflows,
    overlaps,
    score: Math.max(0, Number((10 - issuesCount * 2.5).toFixed(2))),
    hostUpgrade: resolveHostUpgrade("qc-layout-overflow", ctx.env),
  };
}

export function qcChartAndData(input = {}, ctx = {}) {
  const charts = Array.isArray(input.charts) ? input.charts : [];
  const issues = [];
  for (const chart of charts) {
    const id = String(chart.id ?? "unnamed-chart");
    if (!chart.title || !String(chart.title).trim()) {
      issues.push({ chartId: id, ruleId: "CHART_MISSING_TITLE", message: "Chart requires an action-oriented title" });
    }
    if (!chart.ariaLabel && !chart.title) {
      issues.push({ chartId: id, ruleId: "CHART_MISSING_A11Y", message: "Chart requires ariaLabel or accessible title" });
    }
    const series = Array.isArray(chart.series) ? chart.series : [];
    if (series.length === 0) {
      issues.push({ chartId: id, ruleId: "CHART_EMPTY_SERIES", message: "Chart has no data series" });
    }
    if ((chart.type === "pie" || chart.type === "donut") && series.length > 6) {
      issues.push({ chartId: id, ruleId: "PIE_TOO_MANY_SLICES", message: `Pie/donut has ${series.length} slices (max 6)` });
    }
    for (const pt of series) {
      const val = typeof pt === "number" ? pt : pt?.value;
      if (typeof val !== "number" || !Number.isFinite(val)) {
        issues.push({ chartId: id, ruleId: "CHART_NON_FINITE_VALUE", message: "Chart contains non-finite data point" });
        break;
      }
    }
    if (!chart.unit && chart.type !== "pie" && chart.type !== "donut") {
      issues.push({ chartId: id, ruleId: "CHART_MISSING_UNIT", message: "Axis/metric unit must be explicitly declared" });
    }
  }
  return {
    tool: "qc-chart-and-data",
    passed: issues.length === 0,
    checkedCharts: charts.length,
    issues,
    score: Math.max(0, Number((10 - issues.length * 2).toFixed(2))),
    hostUpgrade: resolveHostUpgrade("qc-chart-and-data", ctx.env),
  };
}

export function qcSpecParity(input = {}, ctx = {}) {
  const required = {
    screens: input.requiredScreens ?? [],
    states: input.requiredStates ?? [],
    themeModes: input.requiredThemeModes ?? [],
    components: input.requiredComponents ?? [],
  };
  const actual = {
    screens: new Set(input.actualScreens ?? []),
    states: new Set(input.actualStates ?? []),
    themeModes: new Set(input.actualThemeModes ?? []),
    components: new Set(input.actualComponents ?? []),
  };
  const missing = {};
  let totalReq = 0;
  let totalCovered = 0;
  for (const key of Object.keys(required)) {
    const items = required[key];
    totalReq += items.length;
    const miss = items.filter((item) => !actual[key].has(item));
    totalCovered += items.length - miss.length;
    missing[key] = miss;
  }
  const parityRatio = totalReq === 0 ? 1 : Number((totalCovered / totalReq).toFixed(3));
  return {
    tool: "qc-spec-parity",
    passed: parityRatio >= 0.9,
    parityRatio,
    totalRequired: totalReq,
    totalCovered,
    missing,
    score: Number((parityRatio * 10).toFixed(2)),
    hostUpgrade: resolveHostUpgrade("qc-spec-parity", ctx.env),
  };
}

export function qcPopularCoverage(input = {}, ctx = {}) {
  const track = input.outputTrack === "presentation-16-9" ? "presentation-16-9" : "app-fe-be";
  const catalogPath =
    track === "presentation-16-9"
      ? "catalogs/popular-components.deck.json"
      : "catalogs/popular-components.fe.json";
  const catalog = loadCatalog(catalogPath);
  const used = new Set((input.usedComponents ?? []).map((c) => String(c).toLowerCase()));
  const byId = new Map(catalog.components.map((c) => [c.id, c]));
  const aliasToId = new Map();
  for (const comp of catalog.components) {
    for (const alias of comp.aliases ?? []) {
      aliasToId.set(alias.toLowerCase(), comp.id);
    }
  }
  const matched = [];
  const customReinvented = [];
  for (const item of used) {
    if (byId.has(item)) {
      matched.push(item);
    } else if (aliasToId.has(item)) {
      customReinvented.push({ used: item, canonicalId: aliasToId.get(item) });
    }
  }
  const recommended = catalog.components.filter((c) => c.recommended).map((c) => c.id);
  const missingRecommended = recommended.filter((id) => !matched.includes(id));
  const coverageRatio = recommended.length === 0 ? 1 : Number(((recommended.length - missingRecommended.length) / recommended.length).toFixed(3));
  return {
    tool: "qc-popular-coverage",
    track,
    passed: coverageRatio >= 0.6 && customReinvented.length === 0,
    coverageRatio,
    matched,
    missingRecommended,
    customReinvented,
    score: Number((Math.max(0, coverageRatio * 10 - customReinvented.length * 1.5)).toFixed(2)),
    hostUpgrade: resolveHostUpgrade("qc-popular-coverage", ctx.env),
  };
}

export const QC_TOOL_REGISTRY = {
  "qc-a11y-contrast": qcA11yContrast,
  "qc-anti-slop-linter": qcAntiSlopLinter,
  "qc-layout-overflow": qcLayoutOverflow,
  "qc-chart-and-data": qcChartAndData,
  "qc-spec-parity": qcSpecParity,
  "qc-popular-coverage": qcPopularCoverage,
};
