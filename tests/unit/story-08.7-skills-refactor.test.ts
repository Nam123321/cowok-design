// @story 08.7
// @implements AC-01, AC-04, AC-05 (3-Bucket Skill Separation, Zero-Hardcode 27-Skill Refactor, data-viz-storyteller v2.1 & Bridge 1 Zero-PII Client)
import fs from "node:fs";
import path from "node:path";
import { describe, expect, it, vi } from "vitest";
import {
  buildStudioCorrectionPayload,
  buildZeroPiiCorrectionSignature,
  detectStudioOverrideCorrection,
  emitStudioCorrectionNonBlocking,
} from "../../src/features/flow/components/dashboard-studio/studioCorrectionClient";
import { createInitialSnapshot } from "../../src/features/flow/components/dashboard-studio/types";
import { RecordStudioCorrectionRequestSchema } from "../../src/lib/validations/ai-harness";

const REPO_ROOT = path.resolve(__dirname, "../..");
const DESIGN_CRAFT_ROOT = path.join(REPO_ROOT, "packages/design-craft");
const BUILTIN_DC_ROOT = path.join(REPO_ROOT, "packages/builtin-skills/design-craft");
const HOST_SKILLS_ROOT = path.join(REPO_ROOT, "packages/host-skills");
const UPSTREAM_DASHBOARD_ROOT = path.join(
  REPO_ROOT,
  "packages/builtin-skills/upstream-skills/dashboard-design"
);

function estimateTokenCount(text: string): number {
  const trimmed = text.trim();
  if (!trimmed) return 0;
  const charEst = Math.ceil(trimmed.length / 4);
  const wordEst = Math.ceil(trimmed.split(/\s+/).length * 1.33);
  return Math.max(charEst, wordEst);
}

function collectSkillMdFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...collectSkillMdFiles(full));
    } else if (entry.isFile() && entry.name === "SKILL.md") {
      out.push(full);
    }
  }
  return out.sort();
}

function extractFrontmatter(raw: string): string {
  const m = raw.replace(/^\uFEFF/, "").match(/^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/);
  if (!m) throw new Error("Missing YAML frontmatter");
  return m[1].trim();
}

describe("Story 08.7: 3-Bucket Skill Separation, Zero-Hardcode Refactor & Bridge 1 Client", () => {
  it("AC-01: enforces canonical 3-bucket separation with 0 broken phaseSkillMap links and <= 100 tok headers", () => {
    // 1. Canonical packages/host-skills strictly owns the 3 host-runtime skills
    const hostDirs = fs
      .readdirSync(HOST_SKILLS_ROOT, { withFileTypes: true })
      .filter((d) => d.isDirectory())
      .map((d) => d.name)
      .sort();
    expect(hostDirs).toEqual([
      "cowok-host-spine",
      "project-spec-synthesizer",
      "research-decision-room",
    ]);

    // 2. Upstream verbatim dashboard-design remains intact
    expect(fs.existsSync(path.join(UPSTREAM_DASHBOARD_ROOT, "LICENSE"))).toBe(true);
    expect(fs.existsSync(path.join(UPSTREAM_DASHBOARD_ROOT, "SKILL.md"))).toBe(true);

    // 3. packages/design-craft absorbs the 3 formerly cross-bucket skills and resolves 100% of phaseSkillMap internally
    const dcSkills = collectSkillMdFiles(path.join(DESIGN_CRAFT_ROOT, "skills"));
    const dcSkillNames = new Set<string>();
    for (const file of dcSkills) {
      const fm = extractFrontmatter(fs.readFileSync(file, "utf8"));
      expect(estimateTokenCount(fm)).toBeLessThanOrEqual(100);
      const nameMatch = fm.match(/^name:\s*["']?([^"'\r\n]+)["']?/m);
      expect(nameMatch).not.toBeNull();
      dcSkillNames.add(nameMatch![1].trim());
    }
    expect(dcSkillNames.has("flow-state-completer")).toBe(true);
    expect(dcSkillNames.has("reference-design-contract")).toBe(true);
    expect(dcSkillNames.has("self-healing-eval-gate")).toBe(true);

    const manifest = JSON.parse(
      fs.readFileSync(path.join(DESIGN_CRAFT_ROOT, "plugin.json"), "utf8")
    ) as { phaseSkillMap: Record<string, string[]> };
    for (const [phase, skills] of Object.entries(manifest.phaseSkillMap)) {
      for (const skill of skills) {
        expect(
          dcSkillNames.has(skill),
          `phaseSkillMap '${phase}' references unmounted skill '${skill}'`
        ).toBe(true);
      }
    }

    // 4. Every host skill also satisfies <= 100 tok frontmatter
    for (const file of collectSkillMdFiles(HOST_SKILLS_ROOT)) {
      const fm = extractFrontmatter(fs.readFileSync(file, "utf8"));
      expect(estimateTokenCount(fm)).toBeLessThanOrEqual(100);
    }
  });

  it("AC-01: data-viz-storyteller v2.1 has 0 hardcoded demo anchors, Semantic Role Contract, 5 Core + 8 Extensible Patterns", () => {
    for (const root of [DESIGN_CRAFT_ROOT, BUILTIN_DC_ROOT]) {
      const base = path.join(root, "skills/domain-business/data-viz-storyteller");
      const skillMd = fs.readFileSync(path.join(base, "SKILL.md"), "utf8");
      const arcMd = fs.readFileSync(path.join(base, "references/storytelling-arc.md"), "utf8");
      const canvasMd = fs.readFileSync(
        path.join(base, "references/html-canvas-contract.md"),
        "utf8"
      );
      const matrixMd = fs.readFileSync(
        path.join(base, "references/chart-selection-matrix.md"),
        "utf8"
      );

      // Version 2.1.0 & <= 100 tok frontmatter
      const fm = extractFrontmatter(skillMd);
      expect(fm).toContain("version: 2.1.0");
      expect(estimateTokenCount(fm)).toBeLessThanOrEqual(100);

      // Zero hardcoded demo anchors / Q4 Revenue demo copy
      for (const doc of [skillMd, arcMd, canvasMd, matrixMd]) {
        expect(doc).not.toContain("kpi-revenue-card");
        expect(doc).not.toContain("kpi-growth-card");
        expect(doc).not.toContain("kpi-target-card");
        expect(doc).not.toContain("Q4 Revenue");
      }

      // All 5 Core Interactive UX Patterns + 8 Open-Ended Extensible Patterns + Minimum Floor guidance
      expect(arcMd).toContain("Minimum Floor, Never Maximum Ceiling");
      expect(arcMd).toContain("AI Lens Strip");
      expect(arcMd).toContain("Contextual Point Actions");
      expect(arcMd).toContain("5-Rule Honest & Wordless");
      expect(arcMd).toContain("What-If Scenario Drag");
      expect(arcMd).toContain("CEO 3s");
      for (const extensible of [
        "Small Multiples",
        "Event-Marker",
        "Cross-Filtering",
        "Scrollytelling",
        "Waterfall",
        "Uncertainty",
        "Slopegraph",
        "Sensitivity Sliders",
      ]) {
        expect(arcMd).toContain(extensible);
      }

      // Semantic Role Contract + 5-Rule Auto-Fix table + Undo + sourceRows invariant
      for (const attr of [
        "data-ff-role",
        "data-row-id",
        "data-whatif-handle",
        "data-ff-callout-for",
        "data-ff-compare-pair",
        "data-audience-mode",
        "sourceRows",
        "HONEST_ACTION_TITLE",
        "HONEST_ZERO_BASELINE",
        "HONEST_MISSING_NULL",
        "HONEST_PROPORTION_SAMPLE",
        "WORDLESS_SPOTLIGHT_CLUTTER",
      ]) {
        expect(canvasMd).toContain(attr);
      }

      // Graceful high-cardinality escalation (> 6 categories)
      expect(matrixMd).toContain("> 6");
      expect(matrixMd).toContain("Small Multiples");
    }
  });

  it("AC-01: all 7 Craft skills upgraded with substantive SKILL.md + Tier-3 references/*.md and zero token mismatch", () => {
    const craftSlugs = [
      "archify-diagram",
      "dashboard-ux",
      "impeccable-polish",
      "oklch-palette",
      "sketch-monochrome",
      "stageshow-slide",
      "typography",
    ];

    for (const root of [DESIGN_CRAFT_ROOT, BUILTIN_DC_ROOT]) {
      for (const slug of craftSlugs) {
        const dir = path.join(root, "skills/craft", slug);
        const skillText = fs.readFileSync(path.join(dir, "SKILL.md"), "utf8");
        expect(skillText.split("\n").length).toBeGreaterThanOrEqual(15);
        expect(estimateTokenCount(extractFrontmatter(skillText))).toBeLessThanOrEqual(100);
        expect(skillText).toContain("Minimum Floor");

        const refsDir = path.join(dir, "references");
        expect(fs.existsSync(refsDir), `Missing references/ for ${slug}`).toBe(true);
        const refFiles = fs.readdirSync(refsDir).filter((f) => f.endsWith(".md"));
        expect(refFiles.length).toBeGreaterThanOrEqual(1);
      }

      // phase-3-synthesis-polish uses dynamic tokens.css (--bg-surface), zero --surface-primary mismatch
      const p3 = fs.readFileSync(
        path.join(root, "skills/phase-3-synthesis-polish/SKILL.md"),
        "utf8"
      );
      expect(p3).toContain("--bg-surface");
      expect(p3).not.toContain("--surface-primary");

      // ux-microcopy-and-i18n removes rigid 18-word ceiling and supports RTL/CJK + >=35% floor
      const micro = fs.readFileSync(
        path.join(root, "skills/domain-business/ux-microcopy-and-i18n/SKILL.md"),
        "utf8"
      );
      expect(micro).not.toContain("under 18 words");
      expect(micro).toContain("RTL");
      expect(micro).toContain("CJK");
      expect(micro).toContain("35%");

      // stageshow-slide supports adaptive 16:9 | 16:10 | Auto without hardcoded 1280x720
      const slide = fs.readFileSync(
        path.join(root, "skills/craft/stageshow-slide/SKILL.md"),
        "utf8"
      );
      expect(slide).toContain("16:9");
      expect(slide).toContain("16:10");
      expect(slide).toContain("Auto");
      expect(slide).not.toContain("1280x720");
    }
  });

  it("AC-04: Bridge 1 studioCorrectionClient enforces strict Zero-PII signatures and non-blocking fire-and-forget", async () => {
    const sig = buildZeroPiiCorrectionSignature("chart_type_override", "bar", "tick-bar");
    expect(sig).toBe("chart_type_override:bar:tick-bar");

    // Raw CSV / PII values, phone numbers, paths, and unknown fields rejected by strict boundary Zod schema
    expect(
      RecordStudioCorrectionRequestSchema.safeParse({
        projectId: "proj-1",
        correctionKind: "chart_type_override",
        signatureKey: "alice@example.com,Revenue,99999",
      }).success
    ).toBe(false);
    expect(
      RecordStudioCorrectionRequestSchema.safeParse({
        projectId: "proj-1",
        correctionKind: "chart_type_override",
        signatureKey: "chart_type_override:bar:line",
        fromValue: "+15551234567",
      }).success
    ).toBe(false);
    expect(
      RecordStudioCorrectionRequestSchema.safeParse({
        projectId: "proj-1",
        correctionKind: "chart_type_override",
        signatureKey: "chart_type_override:bar:line",
        toValue: "john.doe/acct",
      }).success
    ).toBe(false);
    expect(
      RecordStudioCorrectionRequestSchema.safeParse({
        projectId: "proj-1",
        correctionKind: "chart_type_override",
        signatureKey: "chart_type_override:bar:line",
        extraSpoofField: "nope",
      }).success
    ).toBe(false);

    // Structural override detector catches chart, palette, and audience mode overrides cleanly
    const base = createInitialSnapshot();
    const chartOverride = detectStudioOverrideCorrection("proj-1", base, {
      ...base,
      chartType: "tick-bar",
    });
    expect(chartOverride?.correctionKind).toBe("chart_type_override");
    expect(chartOverride?.signatureKey).toBe(`chart_type_override:${base.chartType}:tick-bar`);

    const densityOverride = detectStudioOverrideCorrection("proj-1", base, {
      ...base,
      audienceMode: base.audienceMode === "ceo" ? "analyst" : "ceo",
    });
    expect(densityOverride?.correctionKind).toBe("density_mode_override");

    // Payload sanitizer strips unsafe chars so no PII leaks
    const sanitized = buildStudioCorrectionPayload({
      projectId: "proj-1",
      correctionKind: "qa_autofix_click",
      fromValue: "bar-issues-2",
      toValue: "autofixed",
    });
    expect(sanitized).not.toBeNull();
    expect(sanitized?.signatureKey).toMatch(/^[a-z0-9:_-]+$/);

    // Non-blocking emitter never throws even when network fails
    const failingSender = vi.fn().mockRejectedValue(new Error("offline"));
    const dispatched = emitStudioCorrectionNonBlocking(
      {
        projectId: "proj-1",
        correctionKind: "qa_autofix_click",
        fromValue: "bar",
        toValue: "autofixed",
      },
      failingSender
    );
    expect(dispatched).toBe(true);
    await new Promise((r) => setTimeout(r, 10));
    expect(failingSender).toHaveBeenCalledTimes(1);
  });
});
