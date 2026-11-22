# Epic 13 FMEA Risk Matrix — End-to-End Sealed Anti-Goodhart Benchmark & 25-Case `/simulate-user` Certification Suite

## 1. FMEA Edge Case Registry

| Risk ID | Target Story | Failure Mode & Mitigation | S | O | D | RPN | Status |
| :--- | :---: | :--- | :-: | :-: | :-: | :-: | :--- |
| `EC-E13-01` | `13.1` | Agent bypasses DAC `chmod 444` (`chmod u+w`) or Git checkout resets permissions to `0644` and modifies both `tests/eval/sealed/` fixtures and an in-repo SHA-256 manifest (`[B-60..B-62]`). Mitigation: Pin expected root manifest SHA-256 **out-of-band** (compiled constant in Rust binary / CI protected env `COWOK_SEALED_MANIFEST_SHA256` / Watchmen receipt) and lock `0444` + non-symlink verification fail-closed (`100% SEALED_EVAL_TAMPER_ERROR`). | 10 | 6 | 6 | **360** | Enforced in `13.1 AC-01, AC-05` |
| `EC-E13-02` | `13.1` | Aggregate `>=22/25` threshold masks failures in mandatory security/integrity gates (`M1` secret intercept, `M3` RCE Tool Jail, `M2` CSP/XSS & localStorage parity, `M4` `0` float leak, `M5` `0` offline 404). Mitigation: Mark all security/integrity gates across `M1..M5` as non-maskable hard blockers overriding aggregate counts. | 10 | 5 | 5 | **250** | Enforced in `13.1 AC-03, AC-05` |
| `EC-E13-03` | `13.1` | `--tier pr` leaks live LLM API calls (`>$0`/`>15s`) via ambient keys or concurrent `/api/v1/eval/run` calls interleave `.cowok-design/evals/gate-history.jsonl` vs `V18c eval_benchmark_runs`. Mitigation: Enforce `COWOK_PR_TIER_OFFLINE=1` + `<15s` cap on PR tier, single-flight lock (`409 EVAL_ALREADY_RUNNING`), MPSC SQLite actor (`10.1`) + atomic JSONL append. | 8 | 5 | 5 | **200** | Enforced in `13.1 AC-02, AC-04` |

## 2. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-20 | 1.0.0 | Initial Canonical 1-10 FMEA Risk Matrix for `Epic 13` (`Story 13.1` Sealed Anti-Goodhart Benchmark & 25-Case `/simulate-user` Certification Suite) | Review Agent (`573b2485`) & Edge Case Guardian |
