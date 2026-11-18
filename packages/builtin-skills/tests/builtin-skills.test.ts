// @story 08.1
// @cover AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08
import { describe, it, expect, beforeEach } from "vitest";
import * as path from "path";
import * as fs from "fs";
import * as os from "os";
import { BuiltinSkillsRegistry } from "../src/registry";
import { PerRunStagingManager } from "../src/staging";
import { TriangleLinter } from "../src/triangle-linter";
import { validateSkillFrontmatter, estimateTokenCount } from "../src/validator";

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
      const tmpBase = fs.mkdtempSync(path.join(os.tmpdir(), "cowok-skills-stage-"));
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

      try {
        const receipt = stager.stageRun("run-999", [skill]);
        expect(receipt.run_id).toBe("run-999");
        expect(receipt.staged_skills).toContain("test-run-skill");
        expect(fs.existsSync(path.join(tmpBase, "run-999", "test-run-skill", "plugin.json"))).toBe(true);
        expect(fs.existsSync(path.join(tmpBase, "run-999", "run-receipt.json"))).toBe(true);
      } finally {
        stager.cleanupRun("run-999");
        fs.rmSync(tmpBase, { recursive: true, force: true });
      }
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

      const multiTab = `<button tabindex="5" title="Say “hi”"><svg></svg><svg></svg></button>`;
      const fixedMulti = TriangleLinter.lintAndFix(multiTab).content;
      expect(fixedMulti).toContain('tabindex="0"');
      expect(fixedMulti).toContain('title="Say &quot;hi&quot;"');
      expect((fixedMulti.match(/aria-hidden="true"/g) || []).length).toBe(2);
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

  describe("Golden Parity & AC-06..AC-08 Hardening", () => {
    const fixturesDir = path.join(packageRoot, "tests", "fixtures");
    const goldenFile = fs.readdirSync(fixturesDir).find((f) => f.endsWith("-golden.json"));
    if (!goldenFile) {
      throw new Error(`Missing golden parity fixture in ${fixturesDir}`);
    }
    const goldenPath = path.join(fixturesDir, goldenFile);
    const golden = JSON.parse(fs.readFileSync(goldenPath, "utf8"));

    it("enforces strict line-bounded YAML delimiters and 1,024B ceiling (AC-06)", () => {
      const shared = golden.shared_vectors;
      const validRes = validateSkillFrontmatter(shared.yaml_valid_with_triple_dash_scalar);
      expect(validRes.valid).toBe(true);
      expect(validRes.fields?.name).toBe("dash-in-scalar");
      expect(validRes.fields?.description).toBe("Handle --- delimiter inside quoted scalar safely");

      expect(validateSkillFrontmatter(shared.yaml_invalid_no_opening_delimiter).valid).toBe(false);
      expect(validateSkillFrontmatter(shared.yaml_invalid_over_1024_bytes).valid).toBe(false);
      // F-01 & W2-02 regression: unterminated/unclosed YAML scalars & adjacent quotes fail closed; quoted keys + inline comments pass
      expect(
        validateSkillFrontmatter("---\nname: ok\ndescription: \"unterminated\n---\n").valid,
      ).toBe(false);
      expect(
        validateSkillFrontmatter("---\nname: ok\ndescription: {unclosed\n---\n").valid,
      ).toBe(false);
      expect(
        validateSkillFrontmatter("---\nname: \"a\" \"b\"\ndescription: desc\n---\n").valid,
      ).toBe(false);
      expect(
        validateSkillFrontmatter("---\nname: demo\ndescription: 12\n---\n").valid,
      ).toBe(false);
      const quotedAndHashRes = validateSkillFrontmatter(
        "---\n# top comment\n\"name\": quoted-key\nversion: 1.0.0 # inline comment\ndescription: C# guide # note\n---\n",
      );
      expect(quotedAndHashRes.valid).toBe(true);
      expect(quotedAndHashRes.fields?.name).toBe("quoted-key");
      expect(quotedAndHashRes.fields?.description).toBe("C# guide");
      // F-02 parity: TS now runs real yaml + cl100k_base BPE matching Rust D2 SSOT across all SSOT edge vectors
      expect(
        validateSkillFrontmatter(golden.rust_ssot_bpe_vectors.yaml_vietnamese_multilingual_over_100_bpe).valid,
      ).toBe(false);
      expect(
        validateSkillFrontmatter(golden.rust_ssot_bpe_vectors.yaml_invalid_anchor_alias).valid,
      ).toBe(false);
      expect(
        validateSkillFrontmatter(golden.rust_ssot_bpe_vectors.yaml_invalid_custom_tag).valid,
      ).toBe(false);
      expect(
        validateSkillFrontmatter(golden.rust_ssot_bpe_vectors.yaml_invalid_duplicate_key).valid,
      ).toBe(false);
    });

    it("matches Rust canonical KV-cache hash and rejects unsafe staging / prompts (AC-07)", () => {
      const kv = golden.shared_vectors.kv_hash_parity;
      const toEntries = (arr: Array<{ manifest: any; skill_md_sha256: string }>) =>
        arr.map((e) => ({ manifest: e.manifest, skillMdSha256: e.skill_md_sha256 }));

      const hashA = PerRunStagingManager.computeStablePromptHashWithSources(toEntries(kv.entries_variant_a));
      const hashB = PerRunStagingManager.computeStablePromptHashWithSources(toEntries(kv.entries_variant_b));
      expect(hashA).toBe(kv.expected_stable_prompt_hash);
      expect(hashB).toBe(kv.expected_stable_prompt_hash);

      const tmpBase = fs.mkdtempSync(path.join(os.tmpdir(), "cowok-skills-hardening-"));
      const stager = new PerRunStagingManager(tmpBase);
      try {
        for (const badVec of golden.shared_vectors.prompt_injection_rejections) {
          const badSkill = {
            name: "bad-prompt",
            version: "1.0.0",
            description: "desc",
            entry_point: "SKILL.md",
            capabilities_required: [],
            risk_tier: 1 as const,
            tags: [],
            prompt_overview: badVec.content,
          };
          expect(() => stager.stageRun(`run-${badVec.id}`, [badSkill])).toThrow();
          expect(fs.existsSync(path.join(tmpBase, `run-${badVec.id}`))).toBe(false);
        }
      } finally {
        fs.rmSync(tmpBase, { recursive: true, force: true });
      }
    });

    it("detects script, dangerouslySetInnerHTML, javascript: URI, and beacon tells (AC-08)", () => {
      for (const item of golden.shared_vectors.linter_security_hygiene) {
        const report = TriangleLinter.lint(item.content);
        expect(report.passed).toBe(false);
        expect(report.issues.map((i) => i.rule_id)).toContain(item.expected_rule);
      }
      // F-03 regression: multi-line <img> and Markdown beacons
      const multilineImg = '<img\n  src="https://tracker.invalid/p.gif"\n  alt="beacon"\n/>';
      expect(TriangleLinter.lint(multilineImg).issues.map((i) => i.rule_id)).toContain(
        "ANTI_SLOP_07_EXTERNAL_BEACON",
      );
      const multilineMd = "![tracker](\nhttps://tracker.invalid/p.gif\n)";
      expect(TriangleLinter.lint(multilineMd).issues.map((i) => i.rule_id)).toContain(
        "ANTI_SLOP_07_EXTERNAL_BEACON",
      );
    });

    it("enforces >= 25 distinct geometric and WCAG rules in TriangleLinter (AC-04 / F-04)", () => {
      const sample = `<html lang="en"><style>
@font-face { font-family: Custom; }
@keyframes spin { from { transform: rotate(0deg); } }
</style>
<h1>Title</h1><h3>Skipped H2</h3>
<div class="inner" style="border-radius: 12px; padding: 16px;"></div>
<div style="background-color: #fff; color: #999; outline: none;"></div>
<p style="width: 120px; line-height: 1; font-size: 9px;">Clipped text</p>
<div style="padding: 13px; min-width: 1200px; z-index: 99999; backdrop-filter: blur(8px); transition: width 0.2s; position: sticky; top: 0;"></div>
<button style="height: 24px">Click</button>
<img src="local.png" />
<span class="badge bg-red-500 status-dot"></span>
<span class="truncate">Long text</span>
<svg class="icon"><path d="" /></svg>
<video src="demo.mp4"></video>
<input type="text" />
<a tabindex="1">First</a>
</html>`;
      const report = TriangleLinter.lint(sample);
      const distinctGeo = new Set(
        report.issues.map((i) => i.rule_id).filter((id) => id.startsWith("RULE_GEO_")),
      );
      expect(distinctGeo.size).toBeGreaterThanOrEqual(25);
    });

    it("rejects symlinks/hardlinks across craft, prompt-templates & JSON axes and aligns 2px hairline spacing", () => {
      const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), "cowok-reg-sec-"));
      try {
        for (const dir of ["craft", "prompt-templates", "design-templates"]) {
          fs.mkdirSync(path.join(tmpRoot, dir), { recursive: true });
        }
        const outside = path.join(tmpRoot, "outside.txt");
        fs.writeFileSync(outside, '{"id":"bad-hardlink"}', "utf8");
        fs.linkSync(outside, path.join(tmpRoot, "craft", "hardlink.md"));
        fs.linkSync(outside, path.join(tmpRoot, "prompt-templates", "hardlink.md"));
        fs.linkSync(outside, path.join(tmpRoot, "design-templates", "hardlink.json"));
        fs.writeFileSync(path.join(tmpRoot, "craft", "ok.md"), "Clean rule", "utf8");

        const reg = new BuiltinSkillsRegistry(tmpRoot);
        reg.discover();
        expect(reg.getCraftRule("ok")).toBeDefined();
        expect(reg.getCraftRule("hardlink")).toBeUndefined();
        expect(reg.getPromptTemplate("hardlink")).toBeUndefined();
        expect(reg.getDesignTemplate("bad-hardlink")).toBeUndefined();
      } finally {
        fs.rmSync(tmpRoot, { recursive: true, force: true });
      }

      const hairlineReport = TriangleLinter.lint('<div style="padding: 1px; margin: 2px;"></div>');
      expect(hairlineReport.issues.map((i) => i.rule_id)).not.toContain("RULE_GEO_06_SPACING_GRID_SCALE");
      const offGridReport = TriangleLinter.lint('<div style="padding: 5px;"></div>');
      expect(offGridReport.issues.map((i) => i.rule_id)).toContain("RULE_GEO_06_SPACING_GRID_SCALE");
    });
  });
});
