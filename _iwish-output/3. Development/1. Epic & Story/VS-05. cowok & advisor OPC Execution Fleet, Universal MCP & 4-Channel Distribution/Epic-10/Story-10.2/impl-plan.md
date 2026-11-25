---
type: I-Wish Implementation Plan
story_id: "10.2"
epic_id: "10"
risk_tier: 3
migration_slot: "V27__story_10_2_worker_file_leases.sql"
parallel_wave: "Wave-C"
status: pending_approval
target_files:
  - crates/spec-registry/migrations/V27__story_10_2_worker_file_leases.sql
  - crates/bot-fleet/src/swarm_leases.rs
  - crates/bot-fleet/src/design_jury.rs
  - crates/bot-fleet/tests/story_10_2_swarm_leases.rs
  - crates/harness-daemon/src/routes/fleet_swarm.rs
  - crates/harness-daemon/tests/story_10_2_fleet_swarm.rs
  - src/lib/validations/fleet-swarm.ts
  - src/api/fleet-swarm.ts
  - src/app/api/v1/fleet/dispatch/route.ts
  - src/app/api/v1/fleet/leases/acquire/route.ts
  - src/app/api/v1/fleet/jury/evaluate/route.ts
coordinator_shared_wiring:
  - crates/bot-fleet/src/lib.rs
  - crates/spec-registry/src/pool.rs
  - crates/harness-daemon/src/routes/mod.rs
  - crates/harness-daemon/src/routes/legacy_router.rs
  - prisma/schema.prisma
  - prisma/schema.contract.test.ts
  - src/lib/vite-bff-routes.ts
---

# Implementation Plan — Story 10.2 (Parallel Swarm Dispatch, Disjoint `worker_file_leases`, `429` Clamp & Independent `Builder != Scorer` Design Jury)

## 1. Architectural Strategy
1. **Additive SQLite WAL Migration (`V27`):** Create `worker_file_leases` (with `worker_pgid`, monotonic `fencing_token`, canonicalized `owned_paths_json`, `expires_at`, `released`) and `fleet_jury_verdicts` (with DB-enforced `CHECK (builder_worker_id <> scorer_worker_id)`). Mirror both in `prisma/schema.prisma`.
2. **Disjoint Lease & Coordination Kernel (`crates/bot-fleet/src/swarm_leases.rs`):**
   - Canonical ancestor/prefix overlap detector (`paths_overlap_canonical`) blocking nested collisions (`src/pages/` vs `src/pages/Home.tsx`), `..` traversal, and symlink escape (`O_NOFOLLOW`).
   - `.cowok-design/coordination/` `ONE_JOB + NEVER` task contract builder & staging commit verifier requiring an active matching `fencing_token`.
   - Expired lease reaper triggering POSIX `kill(-pgid, SIGKILL)` (bounded to owned worker PGIDs only, never `pgid <= 1`) prior to releasing the lease.
   - `RateLimitSemaphore` starting at concurrency `3`, deterministically clamping `3 -> 1` upon HTTP `429` with jittered exponential backoff and guarded recovery.
3. **Independent `Builder != Scorer` & Tier-3 Design Jury (`crates/bot-fleet/src/design_jury.rs`):**
   - Hard-blocks Builder self-PASS spoofing (`403 BUILDER_SELF_PASS_FORBIDDEN`) at both domain & SQLite constraint layers.
   - `E1..E4` independent QC verifier scoring against `QcRunContract` (reads `V26 qc_gate_runs` / invokes `07.1` evaluator) targeting `>= 95%` catch rate on canonical defect benchmarks.
   - Tier-3 `Single-Session Design Jury` (`Visual`, `UX Flow`, `Copy`, `Brand Identity`) operating strictly on frozen `review_freeze` bundle with `generation_fence` CAS (rejecting stale generations with `409 STALE_GENERATION_FENCE`).
   - Structured `QcViolationReport` feedback to `cowok` orchestrator and automatic escalation of recurring violations (`>= 2` repeats) into `V18a gate_failure_ledger` (`HUMAN_ONLY`).
4. **Daemon & BFF Routes (`crates/harness-daemon/src/routes/fleet_swarm.rs` + `src/app/api/v1/fleet/*`):**
   - `POST /api/v1/fleet/leases/acquire` (returns `409 LEASE_PATH_CONFLICT` on overlap).
   - `POST /api/v1/fleet/dispatch` (validates disjoint write-sets, enforces `429` clamp `3->1`, dispatches `ONE_JOB + NEVER` tasks).
   - `POST /api/v1/fleet/jury/evaluate` (enforces `Builder != Scorer`, verifies `generation_fence`, persists `fleet_jury_verdicts` + optional `V18a` ledger handoff).

## 2. Verification & TDD Gates
- Rust: `cargo test -p bot-fleet --test story_10_2_swarm_leases` & `cargo test -p harness-daemon --test story_10_2_fleet_swarm`
- Boundary assertions: ancestor/prefix overlap rejection, stale `fencing_token` rejection, Builder self-PASS spoof block (`403`), `429` semaphore clamp `3 -> 1`, `E1..E4 >= 95%` violation catch benchmark, `V18a` ledger handoff.
- Stage 3: Dual-Engine Plan-Blind Adversarial Review (`risk_tier: 3`)
