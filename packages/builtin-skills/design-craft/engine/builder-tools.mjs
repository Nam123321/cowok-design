import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  PathJailError,
  assertNoNetworkEgress,
  assertNoShellInjection,
  readJailedFile,
  writeJailedFile,
} from "./path-jail.mjs";

const ENGINE_DIR = path.dirname(fileURLToPath(import.meta.url));
const BUNDLE_ROOT = path.resolve(ENGINE_DIR, "..");
const THEME_MODE_RE = /^[a-z0-9][a-z0-9-]{0,31}$/;
const ENTRY_MODES = ["GREENFIELD", "ADD_FEATURE", "REVAMP", "MICRO_POLISH", "DECK_FROM_APP"];

function loadBundleJson(relPath) {
  const fullPath = path.join(BUNDLE_ROOT, relPath);
  return JSON.parse(fs.readFileSync(fullPath, "utf8"));
}

export function resolveHostUpgrade(toolName, env = process.env) {
  const manifest = loadBundleJson("plugin.json");
  const upgradeMap = manifest.capabilities?.hostUpgradeMap ?? {};
  const nativeEnabled = env?.COWOK_DESIGN_NATIVE === "1";
  if (nativeEnabled && upgradeMap[toolName]) {
    return {
      mode: "native",
      host: "cowok-design",
      toolName,
      target: upgradeMap[toolName],
    };
  }
  return {
    mode: "portable",
    host: "standalone",
    toolName,
    target: `design-craft::portable::${toolName}`,
  };
}

function classifyEntryMode(input) {
  if (input.entryMode !== undefined && input.entryMode !== null && String(input.entryMode).trim() !== "") {
    if (typeof input.entryMode !== "string" || !ENTRY_MODES.includes(input.entryMode)) {
      throw new PathJailError(
        "INVALID_ENTRY_MODE",
        `Unknown entryMode '${String(input.entryMode)}'. Allowed: ${ENTRY_MODES.join(", ")}`
      );
    }
    return input.entryMode;
  }
  const prompt = String(input.prompt ?? "").toLowerCase();
  if (/\b(tweak|padding|spacing|typo|nudge|micro|rename|color tweak|align|polish button)\b/.test(prompt)) {
    return "MICRO_POLISH";
  }
  if (input.outputTrack === "presentation-16-9" || /\b(deck|slide|pitch|keynote|16:9)\b/.test(prompt)) {
    return "DECK_FROM_APP";
  }
  if (/\b(revamp|redesign|rebrand|overhaul)\b/.test(prompt)) {
    return "REVAMP";
  }
  if (/\b(add feature|new screen|add tab|extend)\b/.test(prompt)) {
    return "ADD_FEATURE";
  }
  return "GREENFIELD";
}

function routeProfileForMode(entryMode) {
  const profiles = {
    GREENFIELD: { startPhase: 0, phases: [0, 1, 2, 3, 4], spawnSubagents: true, maxAgents: 5, gates: ["GATE_1_SKETCH", "GATE_2_BRAND", "GATE_3_APPROVE_V1"], strategy: "multi-agent-progressive" },
    ADD_FEATURE: { startPhase: 1, phases: [1, 3, 4], spawnSubagents: true, maxAgents: 3, gates: ["GATE_1_SKETCH", "GATE_3_APPROVE_V1"], strategy: "multi-agent-incremental" },
    REVAMP: { startPhase: 2, phases: [2, 3, 4], spawnSubagents: true, maxAgents: 4, gates: ["GATE_2_BRAND", "GATE_3_APPROVE_V1"], strategy: "multi-agent-revamp" },
    MICRO_POLISH: { startPhase: 3, phases: [3], spawnSubagents: false, maxAgents: 1, gates: [], strategy: "single-agent-fast-path" },
    DECK_FROM_APP: { startPhase: 0, phases: [0, 2, 3, 4], spawnSubagents: true, maxAgents: 4, gates: ["GATE_2_BRAND", "GATE_3_APPROVE_V1"], strategy: "multi-agent-deck" },
  };
  return profiles[entryMode];
}

export function dePhaseRouter(input = {}, ctx = {}) {
  const entryMode = classifyEntryMode(input);
  const profile = routeProfileForMode(entryMode);
  const autoPilot = Boolean(input.autoPilot);
  const heritagePackId = input.inheritedHeritagePackId ?? null;
  const autoPilotAllowed = autoPilot && typeof heritagePackId === "string" && heritagePackId.trim().length > 0;
  if (autoPilot && !autoPilotAllowed) {
    throw new PathJailError(
      "AUTO_PILOT_REQUIRES_HERITAGE_PACK",
      "Cannot bypass HITL Phase Gates with --auto-pilot without an explicit inheritedHeritagePackId"
    );
  }
  const pendingHitlGate = autoPilotAllowed || profile.gates.length === 0 ? "NONE" : profile.gates[0];
  return {
    tool: "de-phase-router",
    entryMode,
    activePhase: profile.startPhase,
    phasesToRun: profile.phases,
    executionStrategy: profile.strategy,
    spawnSubagents: profile.spawnSubagents,
    maxAgents: profile.maxAgents,
    estimatedLatencyMs: entryMode === "MICRO_POLISH" ? 0.4 : 12,
    directTool: entryMode === "MICRO_POLISH" ? "de-patch-dom" : null,
    pendingHitlGate,
    hitlGates: profile.gates,
    autoPilotAllowed,
    hostUpgrade: resolveHostUpgrade("de-phase-router", ctx.env),
  };
}

function normalizeThemeModes(rawModes) {
  if (!Array.isArray(rawModes) || rawModes.length === 0) {
    throw new PathJailError(
      "THEME_MODES_REQUIRED",
      "themeModes must be a non-empty array of user-specified mode ids (e.g. ['warm', 'dim'])"
    );
  }
  const seen = new Set();
  const normalized = [];
  for (const mode of rawModes) {
    const clean = String(mode).trim().toLowerCase();
    if (!THEME_MODE_RE.test(clean)) {
      throw new PathJailError("INVALID_THEME_MODE", `Invalid themeMode '${mode}'`);
    }
    if (!seen.has(clean)) {
      seen.add(clean);
      normalized.push(clean);
    }
  }
  return normalized;
}

export function deIntake(input = {}, ctx = {}) {
  const projectId = assertNoShellInjection(String(input.projectId ?? "untitled-project"), "projectId");
  const outputTrack = input.outputTrack === "presentation-16-9" ? "presentation-16-9" : "app-fe-be";
  const hasThemeModes = input.themeModes !== undefined && input.themeModes !== null;
  const themeModes = hasThemeModes ? normalizeThemeModes(input.themeModes) : [];
  const requiresUserThemeChoice = !hasThemeModes;
  const router = dePhaseRouter({ ...input, outputTrack }, ctx);
  const ingestedFiles = [];
  if (Array.isArray(input.files) && input.files.length > 0) {
    if (!ctx.rootDir) {
      throw new PathJailError("ROOT_REQUIRED", "ctx.rootDir is required when reading files");
    }
    for (const relPath of input.files.slice(0, 12)) {
      const content = readJailedFile(ctx.rootDir, relPath);
      ingestedFiles.push({ path: relPath, bytes: Buffer.byteLength(content, "utf8") });
    }
  }
  const blueprints = loadBundleJson("catalogs/domain-blueprints.json").blueprints ?? [];
  const matchedBlueprint =
    blueprints.find((b) => b.id === input.domainBlueprintId) ??
    blueprints.find((b) => b.defaultTrack === outputTrack) ??
    blueprints[0];
  return {
    tool: "de-intake",
    requiresUserThemeChoice,
    cognitiveState: {
      schemaVersion: "1.0",
      projectId,
      outputTrack,
      entryMode: router.entryMode,
      activePhase: router.activePhase,
      completedPhases: [],
      themeModes,
      requiresUserThemeChoice,
      referenceApps: Array.isArray(input.referenceApps) ? input.referenceApps.slice(0, 5) : [],
      selectedPopularComponents: matchedBlueprint?.recommendedComponents ?? [],
      inheritedHeritagePackId: input.inheritedHeritagePackId ?? null,
      pendingHitlGate: router.pendingHitlGate,
      updatedAt: new Date().toISOString(),
    },
    matchedBlueprintId: matchedBlueprint?.id ?? null,
    ingestedFiles,
    hostUpgrade: resolveHostUpgrade("de-intake", ctx.env),
  };
}

export function deScanSketch(input = {}, ctx = {}) {
  let html = typeof input.html === "string" ? input.html : "";
  if (!html && input.filePath) {
    html = readJailedFile(ctx.rootDir, input.filePath);
  }
  const landmarkTags = ["header", "nav", "main", "aside", "footer"];
  const landmarks = landmarkTags.filter((tag) => new RegExp(`<${tag}\\b`, "i").test(html));
  const ffIds = Array.from(html.matchAll(/data-ff-id=["']([^"']+)["']/g), (m) => m[1]);
  const interactiveCount = (html.match(/<(button|a|input|select|textarea)\b/gi) ?? []).length;
  const requiredStates = ["loading", "empty", "error"];
  const detectedStates = requiredStates.filter((s) => new RegExp(`data-state=["']${s}["']`, "i").test(html));
  const missingStates = requiredStates.filter((s) => !detectedStates.includes(s));
  return {
    tool: "de-scan-sketch",
    landmarks,
    missingLandmarks: ["main"].filter((t) => !landmarks.includes(t)),
    ffIds,
    interactiveCount,
    detectedStates,
    missingStates,
    passed: landmarks.includes("main") && ffIds.length > 0,
    hostUpgrade: resolveHostUpgrade("de-scan-sketch", ctx.env),
  };
}

function hashHueOffset(modeName) {
  let hash = 0;
  for (let i = 0; i < modeName.length; i += 1) {
    hash = (hash * 31 + modeName.charCodeAt(i)) % 360;
  }
  return hash;
}

function buildModePalette(modeName, brandHue, accentChroma) {
  const lower = modeName.toLowerCase();
  const isDarkLike = /(dark|dim|night|midnight|noir|slate|obsidian)/.test(lower);
  const isWarmLike = /(warm|sand|sepia|paper|cream|amber|earth)/.test(lower);
  const surfaceHue = isWarmLike ? 72 : brandHue;
  const surfaceChroma = isWarmLike ? 0.022 : 0.012;
  const bgL = lower === "dim" ? 0.24 : isDarkLike ? 0.16 : isWarmLike ? 0.96 : 0.985;
  const cardL = lower === "dim" ? 0.29 : isDarkLike ? 0.21 : isWarmLike ? 0.93 : 0.96;
  const textL = isDarkLike ? 0.94 : 0.20;
  const mutedL = isDarkLike ? 0.74 : 0.44;
  const borderL = isDarkLike ? 0.36 : 0.84;
  const accentHue = (brandHue + (isWarmLike ? -12 : isDarkLike ? 6 : hashHueOffset(modeName) % 15)) % 360;
  const normalizedHue = accentHue < 0 ? accentHue + 360 : accentHue;
  const accentL = isDarkLike ? 0.72 : 0.52;
  const accentTextL = isDarkLike ? 0.16 : 0.98;
  return {
    mode: modeName,
    polarity: isDarkLike ? "low-luminance" : "high-luminance",
    tokens: {
      "--surface-primary": `oklch(${bgL.toFixed(3)} ${surfaceChroma.toFixed(3)} ${surfaceHue})`,
      "--surface-card": `oklch(${cardL.toFixed(3)} ${(surfaceChroma + 0.004).toFixed(3)} ${surfaceHue})`,
      "--text-primary": `oklch(${textL.toFixed(3)} 0.014 ${surfaceHue})`,
      "--text-muted": `oklch(${mutedL.toFixed(3)} 0.018 ${surfaceHue})`,
      "--border-subtle": `oklch(${borderL.toFixed(3)} 0.015 ${surfaceHue})`,
      "--accent-primary": `oklch(${accentL.toFixed(3)} ${accentChroma.toFixed(3)} ${normalizedHue})`,
      "--accent-foreground": `oklch(${accentTextL.toFixed(3)} 0.010 ${normalizedHue})`,
    },
  };
}

export function deTokensOklch(input = {}, ctx = {}) {
  const themeModes = normalizeThemeModes(input.themeModes);
  const brandHue = Number.isFinite(Number(input.brandHue)) ? ((Number(input.brandHue) % 360) + 360) % 360 : 252;
  const accentChroma = Number.isFinite(Number(input.accentChroma))
    ? Math.min(0.24, Math.max(0.06, Number(input.accentChroma)))
    : 0.15;
  const themes = {};
  const cssBlocks = [];
  for (const mode of themeModes) {
    const palette = buildModePalette(mode, brandHue, accentChroma);
    themes[mode] = palette;
    const decls = Object.entries(palette.tokens)
      .map(([k, v]) => `  ${k}: ${v};`)
      .join("\n");
    cssBlocks.push(`[data-theme="${mode}"] {\n${decls}\n}`);
  }
  return {
    tool: "de-tokens-oklch",
    themeModes,
    brandHue,
    themes,
    css: cssBlocks.join("\n\n"),
    hostUpgrade: resolveHostUpgrade("de-tokens-oklch", ctx.env),
  };
}

function escapeHtml(text) {
  return String(text)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function escapeAttrValue(text) {
  return String(text)
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

export function deBrandPreview(input = {}, ctx = {}) {
  const validatedModes = normalizeThemeModes(input.themeModes);
  const tokenResult = deTokensOklch(
    {
      themeModes: validatedModes,
      brandHue: input.brandHue,
      accentChroma: input.accentChroma,
    },
    ctx
  );
  const brandName = assertNoShellInjection(String(input.brandName ?? "Design Craft Studio"), "brandName");
  const safeBrandName = escapeHtml(brandName);
  const sections = tokenResult.themeModes
    .map((mode) => {
      return [
        `<section data-ff-id="preview-${mode}" data-theme="${mode}" style="background:var(--surface-primary);color:var(--text-primary);padding:24px;border:1px solid var(--border-subtle);border-radius:10px;margin-bottom:16px;">`,
        `  <h2 style="margin:0 0 8px 0;">${safeBrandName} — ${mode}</h2>`,
        `  <p style="color:var(--text-muted);margin:0 0 12px 0;">User-selected mode preview using semantic OKLCH variables.</p>`,
        `  <button type="button" style="background:var(--accent-primary);color:var(--accent-foreground);border:none;padding:8px 14px;border-radius:6px;">Primary Action</button>`,
        `</section>`,
      ].join("\n");
    })
    .join("\n");
  const html = [
    "<!doctype html>",
    `<html lang="en" data-theme="${tokenResult.themeModes[0]}">`,
    `<head><meta charset="utf-8"/><title>${safeBrandName} Brand Preview</title>`,
    `<style>${tokenResult.css}</style></head>`,
    `<body><main data-ff-id="brand-preview-root">${sections}</main></body>`,
    "</html>",
  ].join("\n");
  if (input.outputPath && ctx.rootDir) {
    writeJailedFile(ctx.rootDir, input.outputPath, html);
  }
  return {
    tool: "de-brand-preview",
    brandName,
    themeModes: tokenResult.themeModes,
    html,
    hostUpgrade: resolveHostUpgrade("de-brand-preview", ctx.env),
  };
}

export function deExtractContract(input = {}, ctx = {}) {
  if (input.fetchUrl) {
    assertNoNetworkEgress(input.fetchUrl, "fetchUrl");
  }
  const archetypes = loadBundleJson("catalogs/reference-archetypes.json").archetypes ?? [];
  const reaAvailable = Boolean(input.reaEvidence && Array.isArray(input.reaEvidence.observations));
  if (reaAvailable) {
    const evidence = input.reaEvidence.observations.map((obs) => ({
      aspect: String(obs.aspect ?? "layout"),
      value: String(obs.value ?? ""),
      source: ["observed", "provided", "inferred"].includes(obs.source) ? obs.source : "observed",
    }));
    return {
      tool: "de-extract-contract",
      sourceMode: "rea-upstream",
      referenceName: String(input.referenceName ?? "external-reference"),
      evidence,
      matrix: {
        keep: input.reaEvidence.keep ?? ["layout rhythm", "information hierarchy"],
        change: input.reaEvidence.change ?? ["brand color tokens", "typography pair", "domain copy"],
        doNotCopy: input.reaEvidence.doNotCopy ?? ["trademarks", "logos", "proprietary business logic"],
      },
      hostUpgrade: resolveHostUpgrade("de-extract-contract", ctx.env),
    };
  }
  const fallback =
    archetypes.find((a) => a.id === input.archetypeId) ??
    archetypes[0];
  return {
    tool: "de-extract-contract",
    sourceMode: "offline-archetype-fallback",
    fallbackCatalog: "catalogs/reference-archetypes.json",
    archetypeId: fallback.id,
    visualGrammar: fallback.visualGrammar,
    evidence: [
      { aspect: "density", value: fallback.density, source: "provided" },
      { aspect: "borderStyle", value: fallback.visualGrammar.borderStyle, source: "provided" },
      { aspect: "typography", value: fallback.visualGrammar.typographyPair.display, source: "provided" },
    ],
    matrix: {
      keep: fallback.keep,
      change: fallback.change,
      doNotCopy: fallback.doNotCopy,
    },
    hostUpgrade: resolveHostUpgrade("de-extract-contract", ctx.env),
  };
}

function escapeRegex(text) {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function applySinglePatch(html, patch) {
  const targetId = assertNoShellInjection(String(patch.targetId ?? ""), "targetId");
  const escaped = escapeRegex(targetId);
  const tagRegex = new RegExp(`(<([a-zA-Z0-9-]+)[^>]*\\bdata-ff-id=["']${escaped}["'][^>]*>)([\\s\\S]*?)(<\\/\\2>)`);
  const match = html.match(tagRegex);
  if (!match) {
    return { html, applied: false };
  }
  let openTag = match[1];
  let inner = match[3];
  const closeTag = match[4];
  if (patch.op === "setText") {
    inner = escapeHtml(String(patch.value ?? ""));
  } else if (patch.op === "replaceInnerHtml") {
    inner = String(patch.value ?? "");
  } else if (patch.op === "setAttr" && patch.attr) {
    const attrName = assertNoShellInjection(String(patch.attr), "attr");
    if (!/^[a-zA-Z][a-zA-Z0-9:-]*$/.test(attrName)) {
      throw new PathJailError("INVALID_ATTR_NAME", `Invalid attribute name '${attrName}'`);
    }
    const attrRegex = new RegExp(`\\b${escapeRegex(attrName)}=["'][^"']*["']`);
    const safeVal = escapeAttrValue(String(patch.value ?? ""));
    const replacement = `${attrName}="${safeVal}"`;
    openTag = attrRegex.test(openTag)
      ? openTag.replace(attrRegex, () => replacement)
      : openTag.replace(/>$/, () => ` ${replacement}>`);
  } else if (patch.op === "addClass" && patch.value) {
    const safeCls = escapeAttrValue(String(patch.value).trim());
    openTag = /\bclass=["'][^"']*["']/.test(openTag)
      ? openTag.replace(/\bclass=["']([^"']*)["']/, (_, cur) => `class="${`${cur} ${safeCls}`.trim()}"`)
      : openTag.replace(/>$/, () => ` class="${safeCls}">`);
  }
  return {
    html: html.replace(tagRegex, () => `${openTag}${inner}${closeTag}`),
    applied: true,
  };
}

export function dePatchDom(input = {}, ctx = {}) {
  let html = typeof input.html === "string" ? input.html : "";
  if (!html && input.filePath) {
    html = readJailedFile(ctx.rootDir, input.filePath);
  }
  const patches = Array.isArray(input.patches) ? input.patches : [];
  let appliedCount = 0;
  const missingTargets = [];
  for (const patch of patches) {
    const res = applySinglePatch(html, patch);
    html = res.html;
    if (res.applied) {
      appliedCount += 1;
    } else {
      missingTargets.push(String(patch.targetId ?? ""));
    }
  }
  if (input.filePath && input.writeBack && ctx.rootDir) {
    writeJailedFile(ctx.rootDir, input.filePath, html);
  }
  const checksum = crypto.createHash("sha256").update(html).digest("hex").slice(0, 16);
  return {
    tool: "de-patch-dom",
    appliedCount,
    missingTargets,
    checksum,
    patchedHtml: html,
    hostUpgrade: resolveHostUpgrade("de-patch-dom", ctx.env),
  };
}

export const BUILDER_TOOL_REGISTRY = {
  "de-phase-router": dePhaseRouter,
  "de-intake": deIntake,
  "de-scan-sketch": deScanSketch,
  "de-tokens-oklch": deTokensOklch,
  "de-brand-preview": deBrandPreview,
  "de-extract-contract": deExtractContract,
  "de-patch-dom": dePatchDom,
};
