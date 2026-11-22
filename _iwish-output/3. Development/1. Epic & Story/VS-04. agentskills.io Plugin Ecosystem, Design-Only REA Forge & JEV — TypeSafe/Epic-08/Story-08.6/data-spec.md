---
type: I-Wish Data & Contract Spec
story_id: "08.6"
title: "Data, Manifest & MCP Tool Contract: Story 08.6 (`design-craft` Standalone Repo & Default Plugin)"
status: draft
data_decision_records:
  - model: PluginPackage
    action: EXTEND
    bounded_context: plugin-host
    justification: "Reuses existing V12 plugin_packages table in crates/spec-registry to mount `design-craft` as default built-in plugin with optionalPeers metadata"
links_to:
  - "../../../../2. Product Planning/2.2. database-spec.md"
  - "../../../../2. Product Planning/2.5. architecture.md"
  - "./story.md"
---

# Manifest & Tool Contract Specification — Story 08.6 (`design-craft`)

## 1. `design-craft/plugin.json` Canonical Manifest

```json
{
  "name": "design-craft",
  "version": "1.0.0",
  "specStandard": "agentskills.io/v1",
  "description": "Cognitive Progressive Design & Presentation Operating Kit for AI Agents (5-Phase Workflow, Business & Craft Skills, Portable Builder & QC/Audit Tools, Multi-Agent Pack & MCP)",
  "defaultForHosts": ["cowok-design"],
  "phaseSkillMap": {
    "phase-0": ["phase-0-intent-framing", "business-requirements-distiller"],
    "phase-1": ["phase-1-structural-sketch", "sketch-monochrome", "flow-state-completer", "ux-microcopy-and-i18n"],
    "phase-2": ["phase-2-brand-visual-studio", "oklch-palette", "typography", "reference-design-contract"],
    "phase-3": ["phase-3-synthesis-polish", "impeccable-polish", "dashboard-ux", "stageshow-slide", "responsive-and-a11y-craft"],
    "phase-4": ["phase-4-verify-handoff", "user-simulation-8d", "self-healing-eval-gate"]
  },
  "optionalPeers": [
    {
      "id": "rea-upstream",
      "kind": "upstream-github-mcp",
      "purpose": "Extract live UI tokens, visual hierarchy & interaction behavior from target apps/websites via Design-Only boundary (reference-design-contract)",
      "fallback": "catalogs/reference-archetypes.json"
    }
  ],
  "capabilities": {
    "builderTools": ["de-phase-router", "de-intake", "de-scan-sketch", "de-tokens-oklch", "de-brand-preview", "de-extract-contract", "de-patch-dom"],
    "qcAuditTools": ["qc-a11y-contrast", "qc-anti-slop-linter", "qc-layout-overflow", "qc-chart-and-data", "qc-spec-parity", "qc-popular-coverage"],
    "hostUpgradeMap": {
      "de-tokens-oklch": "cowok-design::html-forge::oklch",
      "de-patch-dom": "cowok-design::html-forge::reconcile_ff_ids",
      "qc-layout-overflow": "cowok-design::cdp-visual-qc"
    }
  },
  "agents": ["design-orchestrator", "flow-architect", "brand-stylist", "ui-crafter", "design-auditor"],
  "mcpServer": "mcp/server.mjs"
}
```

## 2. `cognitive-state.json` (Portable Standalone State Mirror of SQLite WAL)

```typescript
import { z } from "zod";

export const CognitiveEntryModeSchema = z.enum([
  "GREENFIELD",
  "ADD_FEATURE",
  "REVAMP",
  "MICRO_POLISH",
  "DECK_FROM_APP",
]);

export const CognitiveSessionStateSchema = z.object({
  schemaVersion: z.literal("1.0"),
  projectId: z.string().min(1),
  outputTrack: z.enum(["app-fe-be", "presentation-16-9"]),
  entryMode: CognitiveEntryModeSchema,
  activePhase: z.number().int().min(0).max(4),
  completedPhases: z.array(z.number().int().min(0).max(4)),
  themeModes: z.array(z.string().regex(/^[a-z0-9][a-z0-9-]{0,31}$/)).min(1), // user-driven: light, dark, warm, dim, or custom
  referenceApps: z.array(z.string().max(120)).default([]),
  selectedPopularComponents: z.array(z.string().max(64)).default([]),
  inheritedHeritagePackId: z.string().nullable().default(null),
  pendingHitlGate: z.enum(["NONE", "GATE_1_SKETCH", "GATE_2_BRAND", "GATE_3_APPROVE_V1"]).default("NONE"),
  updatedAt: z.string().datetime(),
});
```
