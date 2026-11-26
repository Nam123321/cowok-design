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
