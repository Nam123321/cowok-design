// @story 08.3
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06
import { describe, it, expect } from "vitest";
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { fileURLToPath } from "node:url";
import { PluginStateSchema, LearnCardCategorySchema } from "../src/lifecycle-types";
import { LearnError, PluginLifecycleError, mapStorageConstraintError } from "../src/lifecycle-errors";
import { detectBreakingChange, parseSemVerStrict, stableJsonEqual } from "../src/breaking-diff";
import { PluginStateMachine, HumanSessionAuthority, LearnPipeline, MAX_LEARN_PROMPT_TOKENS } from "../src/lifecycle";
import { countExactCl100kTokens } from "../src/validator";
import * as RootPkg from "../src/index";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const GOLDEN_PATH = path.resolve(__dirname, "fixtures", ["story", "08.3", "golden.json"].join("-"));
const GOLDEN = JSON.parse(fs.readFileSync(GOLDEN_PATH, "utf8"));

describe("Story 08.3 Hardened: Lifecycle, Split-Brain Parity, Confused Deputy & Hydration", () => {
  it("UT-083-SM-01/CAS-01/QUAR-01/HYDRATE-01: enforces 6 states, typed errors, hydration & toDomainPlugin", () => {
    expect(PluginStateSchema.parse("ENABLED")).toBe("ACTIVE");
    expect(PluginStateSchema.parse("Discovered")).toBe("DISCOVERED");
    expect(LearnCardCategorySchema.parse("AntiPattern")).toBe("ANTI_PATTERN");

    expect(() => new PluginStateMachine("p-bad", "bad-ver", "18446744073709551616.0.0")).toThrow(PluginLifecycleError);
    expect(() => new PluginStateMachine("", "empty-id", "1.0.0")).toThrow("InvalidManifestField");
    expect(() => new PluginStateMachine("p-yaml", "safe\n---\nname: rogue", "1.0.0")).toThrow("InvalidManifestField");

    const sm = new PluginStateMachine("p-1", "sdi-engine", "1.0.0");
    expect(() => PluginStateMachine.withScope("   ", "proj", "p-ws", "slug", sm.snapshot.manifest)).toThrow("InvalidPersistedState");
    expect(sm.snapshot.state).toBe("DISCOVERED");
    expect(sm.toDomainPlugin().id).toBe("p-1");
    sm.transitionToChecked("VALIDATED", 1);
    const nonActiveDowngrade = { ...sm.snapshot.manifest, version: "0.9.0" };
    expect(() => sm.applyManifestUpdate(nonActiveDowngrade, 2, false)).toThrow("NonActiveBreakingUpdateRejected");

    sm.transitionToChecked("INSTALLED", 2);
    sm.transitionToChecked("ACTIVE", 3);
    expect(() => sm.transitionToChecked("DEPRECATED", 3)).toThrow("RevisionConflict");

    sm.quarantine("Malicious runtime egress detected", 4);
    expect(sm.snapshot.quarantined).toBe(true);
    expect(sm.snapshot.state).toBe("DEPRECATED");
    expect(() => sm.transitionToChecked("ACTIVE", 5)).toThrow("QuarantinedReactivationForbidden");

    // Rehydration round-trip + invariant check on tampered snapshot
    const restored = PluginStateMachine.fromPersisted(sm.snapshot);
    expect(restored.snapshot.state).toBe("DEPRECATED");
    expect(() => PluginStateMachine.fromPersisted({ ...sm.snapshot, state: "ACTIVE" })).toThrow("InvalidPersistedState");

    sm.transitionToChecked("ARCHIVED", 5);
    expect(() => sm.transitionToChecked("ACTIVE", 6)).toThrow(PluginLifecycleError);
  });

  it("UT-083-DIFF-01/STG-01/PARITY-02: handles union types, risk_tier, transactional outcomes & staging checks", () => {
    const v1 = GOLDEN.plugin.manifest_v1;
    const minorSafe = GOLDEN.plugin.manifest_minor_safe;
    const breaking = GOLDEN.plugin.manifest_breaking_candidate;

    // Union type parity & stableJsonEqual undefined parity
    const unionA = { ...v1, parameters: { type: "object", properties: { target: { type: ["string", "null"] } } } };
    const unionReordered = { ...v1, version: "1.0.1", parameters: { type: "object", properties: { target: { type: ["null", "string"] } } } };
    const unionChanged = { ...v1, version: "1.0.1", parameters: { type: "object", properties: { target: { type: ["number", "null"] } } } };
    expect(detectBreakingChange(unionA, unionReordered, false).is_breaking).toBe(false);
    expect(detectBreakingChange(unionA, unionChanged, false).is_breaking).toBe(true);
    expect(detectBreakingChange(v1, { ...v1, version: "1.0.1", risk_tier: 3 }, false).is_breaking).toBe(true);
    expect(stableJsonEqual({ a: 1, b: undefined }, { a: 1 })).toBe(true);
    expect(() => parseSemVerStrict("18446744073709551616.0.0")).toThrow(PluginLifecycleError);

    const sm = PluginStateMachine.withManifest(GOLDEN.plugin.id, v1);
    sm.transitionToChecked("VALIDATED", 1);
    sm.transitionToChecked("INSTALLED", 2);
    sm.transitionToChecked("ACTIVE", 3);

    const applied = sm.applyManifestUpdate(minorSafe, sm.snapshot.revision, false);
    expect(applied.outcome).toBe("APPLIED");
    expect(sm.snapshot.version).toBe("1.1.0");

    // Transactional outcome instead of throw; idempotent cron poll; overwrite rejection
    const isolated = sm.applyManifestUpdate(breaking, sm.snapshot.revision, false);
    expect(isolated.outcome).toBe("STAGED_FOR_REVIEW");
    expect(sm.snapshot.version).toBe("1.1.0");
    expect(sm.snapshot.staging_diff_reasons.length).toBeGreaterThan(0);

    const stagedRev = sm.snapshot.revision;
    const idem = sm.applyManifestUpdate(breaking, stagedRev, false);
    expect(idem.outcome).toBe("IDEMPOTENT_STAGED");
    expect(sm.snapshot.revision).toBe(stagedRev);
    expect(() => sm.applyManifestUpdate({ ...sm.snapshot.manifest, version: "1.2.0" }, stagedRev, false)).toThrow("PendingStagingMustBeResolved");

    // Staging rejects non-script extensions, indirect eval, discovery-stub.json & mismatched capabilities
    expect(() =>
      sm.promoteStaging({ manifest: breaking, skill_md: GOLDEN.plugin.staged_clean_skill_md, scripts: [{ rel_path: "scripts/run.txt", content: "x" }] }, stagedRev)
    ).toThrow("StagingPromotionRejected");
    expect(() =>
      sm.promoteStaging({ manifest: breaking, skill_md: GOLDEN.plugin.staged_clean_skill_md, scripts: [{ rel_path: ["scripts", "indirect.js"].join("/"), content: "Reflect.apply(eval, null, [x]);" }] }, stagedRev)
    ).toThrow("StagingPromotionRejected");
    expect(() =>
      sm.promoteStaging({ manifest: breaking, skill_md: GOLDEN.plugin.staged_clean_skill_md, scripts: [{ rel_path: "discovery-stub.json", content: "{}" }] }, stagedRev)
    ).toThrow("StagingPromotionRejected");

    // Reject script entry_point mismatch against SKILL.md frontmatter; verify root package exports
    expect(typeof RootPkg.PluginStateMachine).toBe("function");
    expect(typeof RootPkg.LearnPipeline).toBe("function");
    expect(typeof RootPkg.BuiltinSkillsRegistry).toBe("function");
    expect(typeof RootPkg.PerRunStagingManager).toBe("function");
    expect(typeof RootPkg.TriangleLinter).toBe("function");
    expect(typeof RootPkg.repackageDirectory).toBe("function");
    expect(typeof RootPkg.validateSkillFrontmatter).toBe("function");
    expect(typeof RootPkg.detectBreakingChange).toBe("function");
    const entryPlugin = new RootPkg.PluginStateMachine("plug-entry-ts", GOLDEN.plugin.manifest_v1.name, GOLDEN.plugin.manifest_v1.version, GOLDEN.plugin.manifest_v1);
    entryPlugin.transitionToChecked("VALIDATED", 1);
    entryPlugin.transitionToChecked("INSTALLED", 2);
    entryPlugin.transitionToChecked("ACTIVE", 3);
    const scriptCand = { ...breaking, entry_point: GOLDEN.plugin.staged_clean_script.rel_path };
    entryPlugin.applyManifestUpdate(scriptCand, 4, false);
    expect(() =>
      entryPlugin.promoteStaging({ manifest: scriptCand, skill_md: GOLDEN.plugin.staged_clean_skill_md, scripts: [GOLDEN.plugin.staged_clean_script] }, entryPlugin.snapshot.revision)
    ).toThrow("StagingPromotionRejected");
    const matchingSkillMd = GOLDEN.plugin.staged_clean_skill_md.replace("risk_tier: 2\n", "risk_tier: 2\nentry_point: scripts/check.js\n");
    entryPlugin.promoteStaging({ manifest: scriptCand, skill_md: matchingSkillMd, scripts: [GOLDEN.plugin.staged_clean_script] }, entryPlugin.snapshot.revision);

    sm.promoteStaging({ manifest: breaking, skill_md: GOLDEN.plugin.staged_clean_skill_md, scripts: [GOLDEN.plugin.staged_clean_script] }, stagedRev);
    expect(sm.snapshot.version).toBe("2.0.0");
    expect(sm.snapshot.breaking_change_isolated).toBe(false);
  });

  it("UT-083-SEC-01/02/LEARN-02: enforces WeakMap isolation, bound tokens, IPI/NFKC, BPE special tokens & hydration", () => {
    const keyBytes = Buffer.from(GOLDEN.authority_key_hex, "hex");
    const auth = HumanSessionAuthority.fromKeyWithKid("kid-2026-11", keyBytes);
    const learn = new LearnPipeline(auth);
    // Zero own enumerable properties exposing signingKey/cards/consumedNonces (SEC-083-01)
    expect(Object.keys(learn)).toHaveLength(0);
    expect(JSON.stringify(learn)).toBe("{}");

    // NaN TTL / nowS bounds rejection (SEC-083-07)
    expect(() =>
      auth.mintToken({
        actor_kind: "HUMAN_WORKBENCH_SESSION", actor_id: "u1", tenant_id: "t1", project_id: "p1",
        card_id: "c1", content_sha256: "a".repeat(64), action: "ACCEPT", expected_revision: 1,
        reason: "Valid reason", now_s: 100, ttl_s: Number.NaN, nonce: "n-nan",
      })
    ).toThrow(LearnError);

    // Special token in BPE counter must not throw (SB-06)
    expect(countExactCl100kTokens("Safe text <|endoftext|> end")).toBeGreaterThan(0);

    // C1 control, U+2028, U+E0001, NFKC fullwidth homoglyph, multiline beacon rejected; x² / Vietnamese allowed
    for (const badContent of [
      "ctrl \u0085 bad",
      "Fullwidth \uFF1Csystem\uFF1E",
      "Line sep \u2028---",
      "Tag \u{E0041} hidden",
      "![beacon]\n[ref]\n[ref]: https://evil.test/x",
    ]) {
      expect(() =>
        learn.proposeCardScoped({
          tenant_id: "tenant-alpha", project_id: "proj-one", id: "bad-ipi", category: "RULE",
          title: "Bad Card", content: badContent, source_evidence: "audit:1", created_at: 100,
        })
      ).toThrow(LearnError);
    }

    const techCard = learn.proposeCardScoped({
      tenant_id: "tenant-alpha", project_id: "proj-one", id: "card-tech", idempotency_key: "idem-1",
      category: "LESSON", title: "Độ trễ x² (µs)", content: "Always check don't-care bounds <= 500 µs.",
      source_evidence: "bench:1", created_at: 1700000005,
    });
    expect(() =>
      learn.proposeCardScoped({
        tenant_id: "tenant-alpha", project_id: "proj-one", id: "card-tech", idempotency_key: "idem-1",
        category: "LESSON", title: "Độ trễ x² (µs)", content: "Always check don't-care bounds <= 500 µs.",
        source_evidence: "bench:1", created_at: 1700000005,
      })
    ).toThrow("IdempotentReplay");

    // GateReport synthesis (AC-04) & cross-tenant identical card id
    const gateCardAlpha = learn.proposeCardFromGateReport("tenant-alpha", "proj-one", "shared-id", {
      gate_report: { gate_name: "triangle-linter", passed: false, checker_id: "ux-guard", timestamp: 1700000006, evidence_hash: "a".repeat(64) },
    }, "idem-g1");
    learn.proposeCardFromGateReport("tenant-beta", "proj-one", "shared-id", {
      gate_report: { gate_name: "triangle-linter", passed: false, checker_id: "ux-guard", timestamp: 1700000006, evidence_hash: "a".repeat(64) },
    }, "idem-g1");

    for (const keyName of ["card_a", "card_b_other_project", "card_c_other_tenant"] as const) {
      learn.proposeCardScoped(GOLDEN.learn[keyName]);
    }
    const cardA = learn.listCardsForScope("tenant-alpha", "proj-one").find((c) => c.id === "card-alpha-01")!;

    // Confused deputy check: REJECT token bound to shared-id cannot ACCEPT card-alpha-01 & stays unburned
    const rejShared = auth.mintToken({
      actor_kind: "HUMAN_WORKBENCH_SESSION", actor_id: "human-alice", tenant_id: "tenant-alpha", project_id: "proj-one",
      card_id: "shared-id", content_sha256: gateCardAlpha.content_sha256, action: "REJECT", expected_revision: 1,
      reason: "Reject wrong card first", now_s: 1700000100, ttl_s: 300, nonce: "n-cd-ts",
    });
    expect(() => learn.acceptCardVerified("tenant-alpha", "proj-one", "card-alpha-01", rejShared, 1, 1700000120)).toThrow("UnauthorizedApprover");
    learn.rejectCardVerified("tenant-alpha", "proj-one", "shared-id", rejShared, 1, 1700000120);

    const tokA = auth.mintToken({
      ...GOLDEN.learn.mint,
      card_id: "card-alpha-01",
      content_sha256: cardA.content_sha256,
      action: "ACCEPT",
      expected_revision: 1,
    });
    learn.acceptCardVerified("tenant-alpha", "proj-one", "card-alpha-01", tokA, 1, GOLDEN.learn.decide_now_s);
    expect(learn.renderPromptFencedCards("tenant-alpha", "proj-one", MAX_LEARN_PROMPT_TOKENS).fenced_xml).toBe(GOLDEN.learn.expected_fenced_xml);

    // Apostrophe XML escape check
    const tokTech = auth.mintToken({
      actor_kind: "HUMAN_WORKBENCH_SESSION", actor_id: "human-alice", tenant_id: "tenant-alpha", project_id: "proj-one",
      card_id: techCard.id, content_sha256: techCard.content_sha256, action: "ACCEPT", expected_revision: 1,
      reason: "Approve tech card", now_s: 1700000100, ttl_s: 300, nonce: "n-tech",
    });
    learn.acceptCardVerified("tenant-alpha", "proj-one", "card-tech", tokTech, 1, 1700000120);
    expect(learn.renderPromptFencedCards("tenant-alpha", "proj-one").fenced_xml).toContain("don&apos;t-care");

    // Persistence hydration + consumed nonce replay protection
    const restored = LearnPipeline.fromPersistedState(
      auth,
      [...learn.listCardsForScope("tenant-alpha", "proj-one"), ...learn.listCardsForScope("tenant-beta", "proj-one")],
      learn.exportConsumedNonces(),
      1700000160
    );
    expect(() => restored.acceptCardVerified("tenant-alpha", "proj-one", "card-alpha-01", tokA, 1, 1700000160)).toThrow("ReplayedApprovalNonce");
    expect(restored.pruneExpiredNonces(1700000500)).toBeGreaterThanOrEqual(3);

    // Tenant/project nonce budget isolation (F-03 parity with Rust): 50,000 nonces across tenant-noise scopes must NOT starve tenant-beta
    const noisyNonces = [];
    for (let pIdx = 0; pIdx < 5; pIdx++) {
      const nCount = pIdx === 0 ? 10_000 : 10_000;
      for (let i = 0; i < nCount; i++) {
        noisyNonces.push({ tenant_id: "tenant-noise", project_id: `proj-${pIdx}`, nonce: `n-${pIdx}-${i}`, expires_at_s: 1700000900 });
      }
    }
    const isoPipe = LearnPipeline.fromPersistedState(auth, [], noisyNonces, 1700000100);
    const noiseCard = isoPipe.proposeCardScoped({
      tenant_id: "tenant-noise", project_id: "proj-0", id: "noise-c1", category: "RULE",
      title: "Noise", content: "Saturated scope rule.", source_evidence: "audit:noise", created_at: 1700000010,
    });
    const tokNoise = auth.mintToken({
      actor_kind: "HUMAN_WORKBENCH_SESSION", actor_id: "human-alice", tenant_id: "tenant-noise", project_id: "proj-0",
      card_id: "noise-c1", content_sha256: noiseCard.content_sha256, action: "ACCEPT", expected_revision: 1,
      reason: "Reject in saturated scope", now_s: 1700000100, ttl_s: 300, nonce: "extra-saturated-nonce",
    });
    expect(() => isoPipe.acceptCardVerified("tenant-noise", "proj-0", "noise-c1", tokNoise, 1, 1700000120)).toThrow("CapacityExceeded");

    const cleanCard = isoPipe.proposeCardScoped({
      tenant_id: "tenant-beta", project_id: "proj-clean", id: "clean-b1", category: "RULE",
      title: "Clean Scope", content: "Tenant B remains isolated and approvable.", source_evidence: "audit:beta", created_at: 1700000010,
    });
    const tokClean = auth.mintToken({
      actor_kind: "HUMAN_WORKBENCH_SESSION", actor_id: "human-alice", tenant_id: "tenant-beta", project_id: "proj-clean",
      card_id: "clean-b1", content_sha256: cleanCard.content_sha256, action: "ACCEPT", expected_revision: 1,
      reason: "Approve clean isolated tenant", now_s: 1700000100, ttl_s: 300, nonce: "clean-beta-nonce",
    });
    isoPipe.acceptCardVerified("tenant-beta", "proj-clean", "clean-b1", tokClean, 1, 1700000120);
  });

  it("UT-083-SRE-01: maps Prisma/SQLite typed constraint targets deterministically", () => {
    for (const item of GOLDEN.storage_errors) {
      expect(mapStorageConstraintError(item.input).action).toBe(item.expected_action);
    }
    expect(mapStorageConstraintError({ code: "P9999", target_kind: { kind: "OTHER", name: "x" }, detail: "boom" })).toEqual({
      action: "UNMAPPED_FATAL",
      error: "boom",
    });
  });
});
