---
type: I-Wish Data & Contract Spec
story_id: "13.1"
title: "Data, V33 eval_benchmark_runs & Sealed Anti-Goodhart Eval Contract Specification: Story 13.1"
status: ready-for-build
risk_tier: 3
adr_ref:
  - "../../../../2. Product Planning/ADRs/ADR-007-Split-DE-DR-Plugin-Runtime-Tool-Jail-and-Sealed-Eval.md"
  - "../../../../2. Product Planning/ADRs/ADR-009-Zero-Hardcode-AI-Harness-Spine-And-Data-Storytelling-Feedback-Loop.md"
data_decision_records:
  - model: EvalBenchmarkRun
    action: NEW_AGGREGATE
    bounded_context: release-certification
    migration: "V33__story_13_1_eval_benchmark_runs.sql"
    justification: "eval_benchmark_runs does not exist in V1..V32; V33 persists sealed SHA-256 verification, paired ablation (>=12 briefs), design-craft SemVer gate, 3-provider decision calibration, Builder!=Scorer fault injection, and M1..M5 UC-01..UC-25 results via the Story 10.1 MPSC write actor."
links_to:
  - "../../../../2. Product Planning/2.2. database-spec.md"
  - "../../../../2. Product Planning/2.5. architecture.md"
  - "../../../../2. Product Planning/2.12. api-registry.md"
  - "./story.md"
  - "./ui-spec.md"
  - "./impl-plan.md"
---

# Data & Contract Specification — Story 13.1

## 1. Bounded Context & Fixed-Ruler Isolation (`ADR-007`, `ADR-009`)

Story `13.1` owns the immutable **Release Certification & Anti-Goodhart Evaluation** aggregate (`eval_benchmark_runs`) and its append-only audit ledger (`.cowok-design/evals/gate-history.jsonl`).
- **Fixed-Ruler Boundary:** `tests/eval/sealed/` is strictly read-only (`0444` files, zero symlinks) and verified against an **out-of-band pinned root manifest SHA-256** compiled into `harness-daemon` (`PINNED_SEALED_MANIFEST_SHA256`) with optional CI protected env override (`COWOK_SEALED_MANIFEST_SHA256`).
- **Zero Contamination:** Existing guards in `crates/plugin-host/src/tool_jail.rs`, `learn_filter.rs`, and `crates/bot-fleet` already block `LEARN` and `advisor` from touching `tests/eval/sealed/`. Story `13.1` closes the verification half with fail-closed `SEALED_EVAL_TAMPER_ERROR`.
- **Single-Flight Writer Parity:** Concurrent `POST /api/v1/eval/run` calls are rejected immediately with `409 EVAL_ALREADY_RUNNING` (in-process atomic guard + filesystem lock). Committed runs write atomically to `eval_benchmark_runs` via `shared_write_actor_for_pool` (`crates/harness-daemon/src/routes/orchestrator.rs`) and atomic-append `.cowok-design/evals/gate-history.jsonl` (`O_NOFOLLOW | O_CLOEXEC`, `fsync`).

---

## 2. Additive SQLite WAL Migration (`V33`) & ADR-091 Prisma Mirror

> Inspection verified `V1..V32` (highest: `V32__story_14_3_distribution_ops_catalog_ab.sql`, `93` tables) do **not** contain `eval_benchmark_runs`. Story `13.1` introduces **`V33`** (`93 -> 94` tables, `CURRENT_SCHEMA_VERSION = 33`, `DAEMON_MIN_SCHEMA_VERSION = 33`). **NEVER** create `prisma/migrations/` or `dev.db`, and **NEVER** add `eval_benchmark_runs` to `Dockerfile` (guarded by `story_12_3_oci_ci_gate.rs`).

### 2.1 Source-of-Truth DDL: `crates/spec-registry/migrations/V33__story_13_1_eval_benchmark_runs.sql`

```sql
-- @story 13.1
-- @implements AC-01, AC-02, AC-03, AC-04, AC-05 (V33 Additive SQLite WAL DDL for Sealed Anti-Goodhart Eval Benchmark Runs)

CREATE TABLE IF NOT EXISTS eval_benchmark_runs (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    tier TEXT NOT NULL CHECK (tier IN ('pr', 'stage', 'release')),
    offline_enforced INTEGER NOT NULL DEFAULT 0 CHECK (offline_enforced IN (0, 1)),
    sealed_manifest_sha256 TEXT NOT NULL,
    manifest_pin_source TEXT NOT NULL CHECK (manifest_pin_source IN ('compiled_binary', 'ci_env_override')),
    tamper_detected INTEGER NOT NULL DEFAULT 0 CHECK (tamper_detected IN (0, 1)),
    tamper_reason TEXT,
    ablation_briefs_count INTEGER NOT NULL DEFAULT 0,
    ablation_lift_percent REAL,
    ablation_regressions INTEGER NOT NULL DEFAULT 0,
    design_craft_semver_passed INTEGER NOT NULL DEFAULT 1 CHECK (design_craft_semver_passed IN (0, 1)),
    decision_providers_calibrated INTEGER NOT NULL DEFAULT 0,
    decision_float_leaks INTEGER NOT NULL DEFAULT 0,
    fault_injected_count INTEGER NOT NULL DEFAULT 0,
    fault_caught_count INTEGER NOT NULL DEFAULT 0,
    fault_catch_rate REAL,
    sim_cases_passed INTEGER NOT NULL DEFAULT 0,
    sim_cases_blocked INTEGER NOT NULL DEFAULT 0,
    sim_cases_gap INTEGER NOT NULL DEFAULT 0,
    sim_cases_total INTEGER NOT NULL DEFAULT 25,
    security_gates_passed INTEGER NOT NULL CHECK (security_gates_passed IN (0, 1)),
    verdict TEXT NOT NULL CHECK (verdict IN ('PASS', 'FAIL', 'SEALED_EVAL_TAMPER_ERROR')),
    milestones_json TEXT NOT NULL DEFAULT '[]',
    sim_cases_json TEXT NOT NULL DEFAULT '[]',
    ablation_json TEXT NOT NULL DEFAULT '{}',
    calibration_json TEXT NOT NULL DEFAULT '{}',
    fault_injection_json TEXT NOT NULL DEFAULT '{}',
    security_blockers_json TEXT NOT NULL DEFAULT '[]',
    duration_ms BIGINT NOT NULL,
    estimated_cost_usd REAL NOT NULL DEFAULT 0.0,
    triggered_by TEXT NOT NULL DEFAULT 'workbench',
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_eval_benchmark_runs_tenant_created
    ON eval_benchmark_runs (tenant_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_eval_benchmark_runs_tier_created
    ON eval_benchmark_runs (tier, created_at DESC);
```

### 2.2 Read-Only Prisma Mirror (`prisma/schema.prisma`, ADR-091)

```prisma
model EvalBenchmarkRun {
  id                          String  @id
  tenantId                    String  @default("local-default") @map("tenant_id")
  projectId                   String? @map("project_id")
  tier                        String
  offlineEnforced             Int     @default(0) @map("offline_enforced")
  sealedManifestSha256        String  @map("sealed_manifest_sha256")
  manifestPinSource           String  @map("manifest_pin_source")
  tamperDetected              Int     @default(0) @map("tamper_detected")
  tamperReason                String? @map("tamper_reason")
  ablationBriefsCount         Int     @default(0) @map("ablation_briefs_count")
  ablationLiftPercent         Float?  @map("ablation_lift_percent")
  ablationRegressions         Int     @default(0) @map("ablation_regressions")
  designCraftSemverPassed     Int     @default(1) @map("design_craft_semver_passed")
  decisionProvidersCalibrated Int     @default(0) @map("decision_providers_calibrated")
  decisionFloatLeaks          Int     @default(0) @map("decision_float_leaks")
  faultInjectedCount          Int     @default(0) @map("fault_injected_count")
  faultCaughtCount            Int     @default(0) @map("fault_caught_count")
  faultCatchRate              Float?  @map("fault_catch_rate")
  simCasesPassed              Int     @default(0) @map("sim_cases_passed")
  simCasesBlocked             Int     @default(0) @map("sim_cases_blocked")
  simCasesGap                 Int     @default(0) @map("sim_cases_gap")
  simCasesTotal               Int     @default(25) @map("sim_cases_total")
  securityGatesPassed         Int     @map("security_gates_passed")
  verdict                     String
  milestonesJson              String  @default("[]") @map("milestones_json")
  simCasesJson                String  @default("[]") @map("sim_cases_json")
  ablationJson                String  @default("{}") @map("ablation_json")
  calibrationJson             String  @default("{}") @map("calibration_json")
  faultInjectionJson          String  @default("{}") @map("fault_injection_json")
  securityBlockersJson        String  @default("[]") @map("security_blockers_json")
  durationMs                  BigInt  @map("duration_ms")
  estimatedCostUsd            Float   @default(0.0) @map("estimated_cost_usd")
  triggeredBy                 String  @default("workbench") @map("triggered_by")
  createdAt                   BigInt  @map("created_at")

  @@index([tenantId, createdAt], map: "idx_eval_benchmark_runs_tenant_created")
  @@index([tier, createdAt], map: "idx_eval_benchmark_runs_tier_created")
  @@map("eval_benchmark_runs")
}
```

### 2.3 Required Schema Version & Table-Count Updates (Exact Codebase Targets)

| File | Current Value | Updated Value |
| :--- | :--- | :--- |
| `crates/spec-registry/src/pool.rs` | `V32_MIGRATION_*` | Add `V33_MIGRATION_SQL`, `V33_MIGRATION_NAME`, `v33_migration_checksum()` & apply in bootstrap + `run_migrations` |
| `crates/harness-daemon/src/lockfile.rs` | `CURRENT_SCHEMA_VERSION = 32`, `DAEMON_MIN_SCHEMA_VERSION = 32` | Both `= 33` |
| `prisma/schema.contract.test.ts` | `V32`, `toHaveLength(93)` | Load `V33`, assert `"eval_benchmark_runs"`, `toHaveLength(94)` |
| `crates/spec-registry/tests/schema_contract.rs` | `table_count == 93` | `table_count == 94` (`+ 1 V33`) |
| `crates/spec-registry/tests/v12_migration_contract.rs` | `table_count == 93` | `table_count == 94` (`V3..V33`) |
| `crates/harness-daemon/tests/story_02_1_context_assemble.rs:131` | `assert_eq!(CURRENT_SCHEMA_VERSION, 32)` | `33` |
| `crates/harness-daemon/tests/story_05_2_proposals.rs:147` | `assert_eq!(CURRENT_SCHEMA_VERSION, 32)` | `33` |
| `crates/harness-daemon/tests/story_05_3_snapshots_handoff.rs:327` | `assert_eq!(CURRENT_SCHEMA_VERSION, 32)` | `33` |
| `crates/harness-daemon/tests/story_05_4_sketch_polish.rs:111` | `assert_eq!(CURRENT_SCHEMA_VERSION, 32)` | `33` |

---

## 3. Sealed Eval Fixture Manifest & Anti-Tamper Contract (`AC-01`, `AC-05`)

Location: `tests/eval/sealed/`

### 3.1 Directory Layout
```text
tests/eval/sealed/
├── manifest.json                 # Canonical sorted file -> sha256 index + suite metadata
├── briefs.json                   # >=12 paired ablation briefs (incl. >=3 Data Storytelling briefs for data-viz-storyteller v2.0 + dashboard-design)
├── decision-calibration.json     # 3-provider Decision Cascade calibration cases (Local FTS5, gemini-decision-1/TypeSafe JEV, Deterministic MCDA)
├── fault-injections.json         # >=20 Builder!=Scorer fault-injection fixtures (P0 layout, token hex leak, a11y contrast, XSS, float leak, chart-ratio drift)
└── sim-user-25-cases.json        # Canonical UC-01..UC-25 definitions across M1..M5 with mandatory security/integrity gate flags
```

### 3.2 Fail-Closed Integrity Verification Algorithm (`verify_sealed_eval_integrity`)
1. **Out-of-Band Pin Resolution:** Read `COWOK_SEALED_MANIFEST_SHA256` if set (must be exact 64 lowercase hex chars; malformed/empty env -> `SEALED_EVAL_TAMPER_ERROR`), else use Rust compile-time constant `PINNED_SEALED_MANIFEST_SHA256`.
2. **Symlink & Traversal Guard:** Inspect `tests/eval/sealed/` and every nested entry with `fs::symlink_metadata` (and read via `O_NOFOLLOW`); any symlink or `..` -> immediate `SEALED_EVAL_TAMPER_ERROR` (`reason: "SYMLINK_DETECTED"`).
3. **Read-Only Permission Guard (`chmod 444`):** On Unix, every regular file inside `tests/eval/sealed/` MUST satisfy `(metadata.permissions().mode() & 0o222) == 0`. Any writable bit (`0644`, `chmod u+w`, etc.) -> immediate `SEALED_EVAL_TAMPER_ERROR` (`reason: "WRITABLE_SEALED_FILE"`).
   - *Fresh Git Checkout Handling:* Because Git stores normal blobs as `0644`, `cowok-design eval seal` (and optional CLI `--seal-if-clean` / `COWOK_EVAL_SEAL_ON_BOOT=1` for CI/E2E bootstrap) FIRST verifies all hashes + zero symlinks against the out-of-band pin (failing closed on any byte mismatch) and ONLY THEN applies `chmod 0444` to every sealed file. Standard `POST /api/v1/eval/run` and `verify_sealed_eval_integrity` NEVER silently relax or auto-chmod writable files.
4. **Root Manifest & Exhaustive File Hash Check:** Compute SHA-256 of `manifest.json` bytes -> must equal out-of-band pinned root hash (`constant_time_eq`). Parse `manifest.json`, verify every listed file's SHA-256 matches, and reject any unlisted extra file in `tests/eval/sealed/` (`reason: "UNEXPECTED_FILE_IN_SEALED_DIR"`).

---

## 4. Evaluation Subsystems & Pass/Fail Gates (`AC-02`, `AC-03`, `AC-05`)

### 4.1 Tier Execution Matrix
| Tier | Network / Browser Rules | Subsystems Executed | Hard SLA / Gate |
| :--- | :--- | :--- | :--- |
| **`pr`** | Forces `COWOK_PR_TIER_OFFLINE=1`; forbids outbound LLM calls and spawning new Chromium processes | Sealed integrity guard + deterministic offline paired ablation (`>=12` briefs) + 3-provider offline Decision calibration (`0` float leak) + `Builder != Scorer` fault injection (`>=20` faults) + deterministic `UC-01..UC-25` assertions | **`< 15,000 ms`**, **`$0.00`**, `0` tamper, `>=95%` fault catch, `>=22/25` Pass, `0` Blocked, `0` Gap, all mandatory security gates PASS |
| **`stage`** | Allows local daemon / pooled CDP QC & candidate skill dir check | All `pr` checks + full Paired Ablation scoring (`with_skill > baseline`, `0` regressions across all `>=12` briefs incl. `data-viz-storyteller v2.0` + `dashboard-design`) + `design-craft` SemVer `v1.x` PR gate | `ablation_lift_percent > 0`, `ablation_regressions == 0`, `design_craft_semver_passed == true`, `fault_catch_rate >= 95.0` |
| **`release`** | Full release certification | All `stage` checks + full `UC-01..UC-25` `M1..M5` certification check + atomic SQLite `V33` & JSONL ledger sealing | All gates PASS + `>=22/25` Pass + `0` Blocked + `0` Gap + `100%` mandatory security gates PASS |

### 4.2 Mandatory Non-Maskable Security & Integrity Gates (`AC-05`, `EC-E13-02`)
Even when `sim_cases_passed >= 22`, if **any** of the following cases fails, `security_gates_passed = false` and `verdict = "FAIL"`:
- **`M1` (`UC-03` — `M1_SECRET_INTERCEPT`):** 100% API key/secret redaction prior to log/SSE/SQLite persistence.
- **`M2` (`UC-09` — `M2_CSP_XSS_ZERO_SPLIT_BRAIN`):** Strict CSP/XSS sanitization & zero `localStorage`/AI split-brain.
- **`M3` (`UC-14` — `M3_RCE_TOOL_JAIL`):** 100% block on RCE, path traversal, symlink, `HUMAN_ONLY` bypass, and `tests/eval/sealed/` reads.
- **`M4` (`UC-20` — `M4_ZERO_FLOAT_LEAK`):** 3-provider Decision Cascade (`Local FTS5`, `gemini-decision-1`/`TypeSafe JEV`, `Deterministic MCDA`) emits `0` float/logprob leaks + `>=95%` `Builder != Scorer` catch rate.
- **`M5` (`UC-23` — `M5_ZERO_OFFLINE_404` & `UC-25` — `M5_SEALED_TAMPER_GUARD`):** `0` offline 404s/CDN leaks in exported HTML/PDF & `100% SEALED_EVAL_TAMPER_ERROR` guard.

### 4.3 Immutable Ledger Format (`.cowok-design/evals/gate-history.jsonl`)
- One newline-delimited canonical JSON record per run containing: `runId`, `tier`, `verdict`, `sealedManifestSha256`, `manifestPinSource`, `tamperDetected`, `ablationLiftPercent`, `ablationRegressions`, `designCraftSemverPassed`, `faultCatchRate`, `simCasesPassed`, `simCasesBlocked`, `simCasesGap`, `simCasesTotal`, `securityGatesPassed`, `securityBlockers`, `durationMs`, `estimatedCostUsd`, `createdAt`.
- Appended atomically under the same single-flight lock via `O_APPEND | O_CREAT | O_NOFOLLOW | O_CLOEXEC` + `sync_data()`.

---

## 5. HTTP API & Zod Validation Contracts (`POST /api/v1/eval/run`, `GET /api/v1/eval/history`)

Mounted inside `build_protected_legacy_router` (`crates/harness-daemon/src/routes/legacy_router.rs`) with automatic `/api/eval/* -> /api/v1/eval/*` standalone parity and strict verified session / loopback HMAC / superadmin Bearer auth (matching `decision.rs` & `visual_qc.rs`).

### 5.1 Endpoints & Status Codes
| Method & Path | Status Codes | Behavior |
| :--- | :--- | :--- |
| `POST /api/v1/eval/run` | `200 OK` · `400 VALIDATION_ERROR` · `401 UNAUTHORIZED` · `409 EVAL_ALREADY_RUNNING` · `422 SEALED_EVAL_TAMPER_ERROR` | Acquires single-flight lock; runs sealed integrity check (fail-closed `422` + logs tamper run to SQLite `V33` & JSONL); executes requested tier; persists atomically via MPSC actor + JSONL; returns `EvalRunResponse`. |
| `GET /api/v1/eval/history` | `200 OK` · `400 VALIDATION_ERROR` · `401 UNAUTHORIZED` | Query params: `limit` (`1..=100`, default `20`), `tier` (optional `pr|stage|release`), `projectId` (optional). Returns recent runs, latest `M1..M5` milestone summary, latest `UC-01..UC-25` checklist, and live sealed manifest integrity snapshot. |

### 5.2 Canonical Zod Schema (`src/lib/validations/eval-certification.ts`)

```typescript
import { z } from "zod";

export const EvalTierSchema = z.enum(["pr", "stage", "release"]);
export const EvalVerdictSchema = z.enum(["PASS", "FAIL", "SEALED_EVAL_TAMPER_ERROR"]);
export const EvalMilestoneIdSchema = z.enum(["M1", "M2", "M3", "M4", "M5"]);
export const SimCaseStatusSchema = z.enum(["PASS", "FAIL", "BLOCKED", "GAP"]);

export const EvalRunRequestSchema = z.object({
  tier: EvalTierSchema,
  projectId: z.string().trim().min(1).max(128).optional(),
  designCraftCandidateDir: z.string().trim().min(1).max(512).optional(),
  runSimulatedUsers: z.boolean().default(true),
});

export const SimCaseResultSchema = z.object({
  caseId: z.string().regex(/^UC-(0[1-9]|1[0-9]|2[0-5])$/),
  milestone: EvalMilestoneIdSchema,
  titleKey: z.string(),
  mandatorySecurityGate: z.boolean(),
  securityGateKey: z.string().nullable(),
  status: SimCaseStatusSchema,
  durationMs: z.number().int().nonnegative(),
  evidenceSummary: z.string(),
});

export const MilestoneSummarySchema = z.object({
  milestone: EvalMilestoneIdSchema,
  passed: z.number().int().nonnegative(),
  total: z.number().int().positive(),
  blocked: z.number().int().nonnegative(),
  gap: z.number().int().nonnegative(),
  securityGatePassed: z.boolean(),
  status: z.enum(["PASS", "FAIL"]),
});

export const PairedAblationSummarySchema = z.object({
  briefsEvaluated: z.number().int().nonnegative(),
  dataStorytellingBriefsCount: z.number().int().nonnegative(),
  baselineMeanScore: z.number(),
  withSkillMeanScore: z.number(),
  liftPercent: z.number(),
  regressionsCount: z.number().int().nonnegative(),
  designCraftSemverPassed: z.boolean(),
  designCraftVersion: z.string(),
});

export const DecisionCalibrationSummarySchema = z.object({
  providers: z.array(
    z.object({
      providerId: z.enum(["local_fts5", "gemini_decision_jev", "deterministic_mcda"]),
      casesEvaluated: z.number().int().nonnegative(),
      accuracyPercent: z.number().min(0).max(100),
      floatLeaksCount: z.number().int().nonnegative(),
      passed: z.boolean(),
    })
  ),
  totalFloatLeaks: z.number().int().nonnegative(),
  passed: z.boolean(),
});

export const FaultInjectionSummarySchema = z.object({
  injectedCount: z.number().int().nonnegative(),
  caughtCount: z.number().int().nonnegative(),
  catchRatePercent: z.number().min(0).max(100),
  builderScorerIsolated: z.boolean(),
  passed: z.boolean(),
});

export const EvalBenchmarkRunDtoSchema = z.object({
  runId: z.string(),
  projectId: z.string().nullable(),
  tier: EvalTierSchema,
  offlineEnforced: z.boolean(),
  sealedManifestSha256: z.string(),
  manifestPinSource: z.enum(["compiled_binary", "ci_env_override"]),
  tamperDetected: z.boolean(),
  tamperReason: z.string().nullable(),
  verdict: EvalVerdictSchema,
  durationMs: z.number().int().nonnegative(),
  estimatedCostUsd: z.number().nonnegative(),
  simCasesPassed: z.number().int().nonnegative(),
  simCasesBlocked: z.number().int().nonnegative(),
  simCasesGap: z.number().int().nonnegative(),
  simCasesTotal: z.number().int().positive(),
  securityGatesPassed: z.boolean(),
  securityBlockers: z.array(z.string()),
  milestones: z.array(MilestoneSummarySchema),
  simCases: z.array(SimCaseResultSchema),
  ablation: PairedAblationSummarySchema,
  calibration: DecisionCalibrationSummarySchema,
  faultInjection: FaultInjectionSummarySchema,
  ledgerPath: z.string(),
  triggeredBy: z.string(),
  createdAt: z.number().int().nonnegative(),
});

export const EvalRunResponseSchema = z.object({
  ok: z.literal(true),
  data: EvalBenchmarkRunDtoSchema,
  requestId: z.string(),
});

export const EvalHistoryResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    sealedIntegrityOk: z.boolean(),
    sealedManifestSha256: z.string(),
    manifestPinSource: z.enum(["compiled_binary", "ci_env_override"]),
    tamperReason: z.string().nullable(),
    latestRun: EvalBenchmarkRunDtoSchema.nullable(),
    runs: z.array(EvalBenchmarkRunDtoSchema),
    totalRuns: z.number().int().nonnegative(),
  }),
  requestId: z.string(),
});

export const EvalErrorResponseSchema = z.object({
  error: z.object({
    code: z.string(),
    message: z.string(),
    field: z.string().optional(),
    requestId: z.string().optional(),
    details: z.record(z.unknown()).optional(),
  }),
});
```

---

## 6. CLI Contract Parity (`cowok-design eval`)

Canonical binary in `crates/harness-daemon/src/cli/mod.rs` is `cowok-design` (historical `uiux-os` name in Epic 13 docs maps 1:1 to `cowok-design`):
- `cowok-design eval run --tier <pr|stage|release> [--project <id>] [--design-craft-candidate <dir>] [--skip-sim] [--seal-if-clean] [--workspace <dir>] [--json]`
  - Exit codes: `0` = `PASS`, `1` = `FAIL` (gate/regression/security blocker), `2` = CLI parse error, `3` = `SEALED_EVAL_TAMPER_ERROR` (`100%` fail-closed), `4` = `EVAL_ALREADY_RUNNING`.
- `cowok-design eval seal [--workspace <dir>] [--json]`
  - Verifies out-of-band root SHA-256 + every fixture hash + zero symlinks BEFORE applying `chmod 0444`; exits `3` on tamper.
- `cowok-design eval history [--limit <1..100>] [--tier <pr|stage|release>] [--workspace <dir>] [--json]`
  - Emits exact `EvalHistoryResponse` envelope parity with `GET /api/v1/eval/history`.
