import { describe, it, expect, beforeEach } from "vitest";
import * as path from "path";
import * as fs from "fs";
import {
  BuiltinSkillsRegistry,
  PerRunStagingManager,
  TriangleLinter,
  validateSkillFrontmatter,
  estimateTokenCount,
} from "../src/index";

describe("Story 08.1: Built-in Harness Skills & Multi-Axis Registry", () => {
  const packageRoot = path.resolve(__dirname, "..");

  describe("AC-03 & Boundary Guard: Frontmatter <= 100 Tokens", () => {
    it("validates that all 9 packaged skills have valid frontmatter <= 100 tokens", () => {
      const skillsDir = path.join(packageRoot, "skills");
      const skillFolders = fs.readdirSync(skillsDir, { withFileTypes: true })
        .filter((d) => d.isDirectory())
        .map((d) => d.name);

      expect(skillFolders.length).toBeGreaterThanOrEqual(9);

      for (const folder of skillFolders) {
        const skillMdPath = path.join(skillsDir, folder, "SKILL.md");
        expect(fs.existsSync(skillMdPath)).toBe(true);

        const content = fs.readFileSync(skillMdPath, "utf8");
        const res = validateSkillFrontmatter(content);

        expect(res.valid).toBe(true);
        expect(res.tokens).toBeLessThanOrEqual(100);
        expect(res.fields?.name).toBeDefined();
        expect(res.fields?.description).toBeDefined();
      }
    });

    it("rejects bloated frontmatter exceeding 100 tokens", () => {
      const bloated = `---
name: bloated-skill
version: 1.0.0
description: ${"extremely detailed description ".repeat(30)}
---
# Content`;
      const res = validateSkillFrontmatter(bloated);
      expect(res.valid).toBe(false);
      expect(res.tokens).toBeGreaterThan(100);
      expect(res.error).toContain("exceeds 100-token budget");
    });
  });

  describe("AC-01: Multi-Axis Capability Discovery & Registry", () => {
    let registry: BuiltinSkillsRegistry;

    beforeEach(() => {
      registry = new BuiltinSkillsRegistry(packageRoot);
      registry.discover();
    });

    it("indexes skills, craft rules, templates, design systems, scenarios, atoms, and prompts", () => {
      const skills = registry.listSkills();
      expect(skills.length).toBeGreaterThanOrEqual(9);

      // Verify specific core seed skills
      expect(registry.getSkill("intent-clarifier-sdi")).toBeDefined();
      expect(registry.getSkill("project-spec-synthesizer")).toBeDefined();
      expect(registry.getSkill("flow-state-completer")).toBeDefined();
      expect(registry.getSkill("design-taste-and-ux-guard")).toBeDefined();
      expect(registry.getSkill("html-presentation-storyteller")).toBeDefined();
      expect(registry.getSkill("self-healing-eval-gate")).toBeDefined();
      expect(registry.getSkill("reference-design-contract")).toBeDefined();
      expect(registry.getSkill("impeccable-design-polish")).toBeDefined();
      expect(registry.getSkill("research-decision-room")).toBeDefined();

      // Verify Craft Rules
      const craftRules = registry.listCraftRules();
      expect(craftRules.length).toBeGreaterThanOrEqual(5);
      expect(registry.getCraftRule("typography")).toBeDefined();
      expect(registry.getCraftRule("spacing-and-grids")).toBeDefined();

      // Verify Design Templates
      const templates = registry.listDesignTemplates();
      expect(templates.length).toBeGreaterThanOrEqual(5);
      expect(registry.getDesignTemplate("template-saas-dashboard")).toBeDefined();

      // Verify Design System v1
      const ds = registry.getDesignSystem("v1");
      expect(ds).toBeDefined();
      expect(ds?.version).toBe("v1");

      // Verify Scenarios
      const scenarios = registry.listScenarios();
      expect(scenarios.length).toBeGreaterThanOrEqual(3);
      expect(registry.getScenario("scenario-sdi-to-prototype")).toBeDefined();

      // Verify Atoms
      const atoms = registry.listAtoms();
      expect(atoms.length).toBeGreaterThanOrEqual(4);
      expect(registry.getAtom("atom-buttons")).toBeDefined();

      // Verify Prompt Templates
      expect(registry.getPromptTemplate("cowok-designer")).toBeDefined();
      expect(registry.getPromptTemplate("advisor-critic")).toBeDefined();
    });
  });

  describe("AC-01 & Per-Run Staging: Stable Prompt Hash Preservation", () => {
    it("computes deterministic order-independent stable prompt hash for KV cache", () => {
      const s1 = {
        name: "intent-clarifier-sdi",
        version: "1.0.0",
        description: "Clarifies design intent",
        entry_point: "SKILL.md",
        capabilities_required: ["sdi"],
        risk_tier: 1 as const,
        tags: [],
      };
      const s2 = {
        name: "design-taste-and-ux-guard",
        version: "1.0.0",
        description: "Enforces 50 OKLCH palettes",
        entry_point: "SKILL.md",
        capabilities_required: ["taste"],
        risk_tier: 1 as const,
        tags: [],
      };

      const hash1 = PerRunStagingManager.computeStablePromptHash([s1, s2]);
      const hash2 = PerRunStagingManager.computeStablePromptHash([s2, s1]);

      expect(hash1).toBe(hash2);
      expect(hash1.length).toBe(64);
    });

    it("stages skills into isolated sandbox directory and generates run receipt", () => {
      const tmpBase = path.join(packageRoot, ".test-cowok-skills");
      const stager = new PerRunStagingManager(tmpBase);

      const skill = {
        name: "test-run-skill",
        version: "1.0.0",
        description: "Test description",
        entry_point: "SKILL.md",
        capabilities_required: [],
        risk_tier: 1 as const,
        tags: [],
      };

      const receipt = stager.stageRun("run-999", [skill]);
      expect(receipt.run_id).toBe("run-999");
      expect(receipt.staged_skills).toContain("test-run-skill");
      expect(fs.existsSync(path.join(tmpBase, "run-999", "test-run-skill", "plugin.json"))).toBe(true);
      expect(fs.existsSync(path.join(tmpBase, "run-999", "run-receipt.json"))).toBe(true);

      // Cleanup
      fs.rmSync(tmpBase, { recursive: true, force: true });
    });
  });

  describe("AC-04: Triangle Linter & Single-Pass Auto-Fix Engine", () => {
    it("detects anti-AI-slop tells and accessibility violations", () => {
      const sample = `<head></head>
<body style="line-height: 1;">
  <div style="background: #6366f1; color: #999">
    <button class="btn">✨ Deploy “Instantly”</button>
    <p>10x faster build times with lorem ipsum text</p>
    <input type="text" tabindex="1" />
    <svg class="icon"><path d="" /></svg>
  </div>
</body>`;

      const report = TriangleLinter.lint(sample);
      expect(report.passed).toBe(false);
      expect(report.total_issues).toBeGreaterThanOrEqual(5);

      const ruleIds = report.issues.map((i) => i.rule_id);
      expect(ruleIds).toContain("ANTI_SLOP_01_SMART_QUOTES");
      expect(ruleIds).toContain("ANTI_SLOP_02_PURPLE_INDIGO_GRADIENT");
      expect(ruleIds).toContain("ANTI_SLOP_03_EMOJI_ICON_SUBSTITUTE");
      expect(ruleIds).toContain("ANTI_SLOP_04_LOREM_IPSUM_PLACEHOLDER");
      expect(ruleIds).toContain("ANTI_SLOP_05_FABRICATED_METRIC_CLAIM");
      expect(ruleIds).toContain("RULE_GEO_23_TAB_ORDER_INTEGRITY");
    });

    it("auto-fixes quotes, gradients, tabindex, and line-height in a single pass", () => {
      const buggy = `<head></head>
<body style="line-height: 1;">
  <div style="background: #6366f1">
    <button class="btn">Click “Here”</button>
    <input tabindex="1" />
  </div>
</body>`;

      const { content: fixed, report } = TriangleLinter.lintAndFix(buggy);
      expect(report.fixed_issues).toBeGreaterThanOrEqual(3);
      expect(fixed).not.toContain("“");
      expect(fixed).toContain('"Here"');
      expect(fixed).toContain("var(--action-primary");
      expect(fixed).toContain('tabindex="0"');
      expect(fixed).toContain("line-height: 1.5;");
      expect(fixed).toContain('name="viewport"');
    });
  });

  describe("AC-05: Reference Design Contract & Deep-Research Seed Suite", () => {
    it("verifies intent-clarifier-sdi contains 67 styles, 25 chart types, and 84 brand presets", () => {
      const sdiDataDir = path.join(packageRoot, "skills", "intent-clarifier-sdi", "data");
      const styles = JSON.parse(fs.readFileSync(path.join(sdiDataDir, "styles.json"), "utf8"));
      const charts = JSON.parse(fs.readFileSync(path.join(sdiDataDir, "charts.json"), "utf8"));
      const brands = JSON.parse(fs.readFileSync(path.join(sdiDataDir, "brand-dna-presets.json"), "utf8"));

      expect(styles.total).toBe(67);
      expect(charts.total).toBe(25);
      expect(brands.total).toBe(84);
    });

    it("verifies reference-design-contract and research-decision-room specs", () => {
      const refContract = fs.readFileSync(
        path.join(packageRoot, "skills", "reference-design-contract", "SKILL.md"),
        "utf8"
      );
      expect(refContract).toContain("observed | provided | inferred");
      expect(refContract).toContain("Keep | Change | Do-Not-Copy");

      const researchRoom = fs.readFileSync(
        path.join(packageRoot, "skills", "research-decision-room", "SKILL.md"),
        "utf8"
      );
      expect(researchRoom).toContain("anti-fabrication");
      expect(researchRoom).toContain("ADR");
    });
  });
});
