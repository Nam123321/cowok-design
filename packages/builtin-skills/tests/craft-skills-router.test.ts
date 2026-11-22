// @story 02.4
// @implements AC-03, AC-05
import { describe, expect, it } from "vitest";
import {
  COWOK_DESIGN_OPT_SPINE,
  COWOK_DESIGN_OPT_SPINE_MAX_TOKENS,
  CRAFT_SKILLS_3TIER_REGISTRY,
  CRAFT_SKILL_SLUGS,
  computeLengthPrefixedPrefixSha256,
  countCl100kBpeTokens,
  routeCraftSkillsDeterministic,
  verifySpineBpeBudget,
} from "../src/craft-skills";

describe("Story 02.4: cowok-design-opt & 3-Zone Craft Skill Router (BENCH-01 & BENCH-02)", () => {
  it("BENCH-01: keeps cowok-design-opt spine <= 250 cl100k_base BPE tokens, validates bounds, and exports all 7 3-tier craft skills", () => {
    const budget = verifySpineBpeBudget();
    expect(budget.tokens).toBeGreaterThan(20);
    expect(budget.tokens).toBeLessThanOrEqual(COWOK_DESIGN_OPT_SPINE_MAX_TOKENS);
    expect(budget.withinBudget).toBe(true);
    expect(COWOK_DESIGN_OPT_SPINE).toContain("--bg-surface");
    expect(COWOK_DESIGN_OPT_SPINE).toContain("--action-primary");

    expect(() =>
      routeCraftSkillsDeterministic({
        projectId: "proj-1",
        portalId: "DeveloperPortal",
        intentSummary: "   ",
      })
    ).toThrow(/INVALID_REQUEST/);
    expect(() =>
      routeCraftSkillsDeterministic({
        projectId: "proj-1",
        portalId: "DeveloperPortal",
        intentSummary: "Build KPI card",
        maxSpineTokens: 999,
      })
    ).toThrow(/SPINE_TOKEN_BUDGET_EXCEEDED/);
    expect(() =>
      routeCraftSkillsDeterministic({
        projectId: "proj-1",
        portalId: "UnknownPortal",
        intentSummary: "Build KPI card",
      })
    ).toThrow(/INVALID_REQUEST/);

    expect(CRAFT_SKILL_SLUGS).toHaveLength(7);
    for (const slug of CRAFT_SKILL_SLUGS) {
      const def = CRAFT_SKILLS_3TIER_REGISTRY[slug];
      expect(def.slug).toBe(slug);
      expect(def.tier1Rule.length).toBeGreaterThan(10);
      expect(def.tier2Guide.length).toBeGreaterThan(20);
      expect(def.tier3Examples.length).toBeGreaterThan(10);
    }
  });

  it("BENCH-02: sorts skills lexicographically, keeps prefixSha256 invariant across intents, and completes <15ms p95", () => {
    const r1 = routeCraftSkillsDeterministic({
      projectId: "proj-1",
      portalId: "DeveloperPortal",
      intentSummary: "Render billing summary bento cards",
      requestedSlugs: ["typography", "oklch-palette", "dashboard-ux", "typography"],
    });

    const r2 = routeCraftSkillsDeterministic({
      projectId: "proj-1",
      portalId: "DeveloperPortal",
      intentSummary: "Generate onboarding modal with contrast checks",
      requestedSlugs: ["dashboard-ux", "typography", "oklch-palette"],
    });

    expect(r1.selectedSlugs).toEqual(["dashboard-ux", "oklch-palette", "typography"]);
    expect(r2.selectedSlugs).toEqual(r1.selectedSlugs);
    expect(r1.prefixSha256).toHaveLength(64);
    expect(r2.prefixSha256).toBe(r1.prefixSha256);

    const r3 = routeCraftSkillsDeterministic({
      projectId: "proj-1",
      portalId: "DeveloperPortal",
      intentSummary: "Render billing summary bento cards",
      requestedSlugs: ["dashboard-ux", "typography"],
    });
    expect(r3.prefixSha256).not.toBe(r1.prefixSha256);

    // Length-prefixed SHA256 prevents boundary ambiguity between Zone 1 and Zone 2
    expect(computeLengthPrefixedPrefixSha256("ab", "cd")).not.toBe(
      computeLengthPrefixedPrefixSha256("abc", "d")
    );

    const latencies: number[] = [];
    for (let i = 0; i < 40; i += 1) {
      const out = routeCraftSkillsDeterministic({
        projectId: "proj-1",
        portalId: "SalesApp",
        intentSummary: `Render card ${i}`,
        requestedSlugs: ["impeccable-polish", "typography"],
      });
      latencies.push(out.latencyMs);
    }
    latencies.sort((a, b) => a - b);
    const p95 = latencies[Math.floor(latencies.length * 0.95)];
    expect(p95).toBeLessThan(15);
  });
});
