# Epic 10 FMEA Risk Matrix — cowok Master Orchestrator, advisor Meta-Agent, OPC Execution Fleet & Design Tournament

## 1. FMEA Edge Case Registry

| Risk ID | Failure Mode | Potential Impact & Mitigation | S | O | D | RPN | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| EC-E10-01 | Story 10.1 Invalid `$UIUX_LIVE_SPEC` | Nếu Execution Agent ghi file JSON không hợp lệ vào `$UIUX_LIVE_SPEC` / `doc-patch.jsonl`, tự động chặn áp dụng lên Canvas và giữ nguyên trạng thái ổn định trước đó kèm 1-step `Cmd+Z`. | 8 | 5 | 3 | **120** | Enforced in 10.1 AC-03 & Task 10.1.3 |
| EC-E10-02 | Story 10.2 `Builder != Scorer` Spoofing | Chặn tuyệt đối mọi nỗ lực của Builder Agent tự ghi kết quả `PASS` vào hộp thư kiểm định của Verifier Agent (`>=95%` violation catch). | 9 | 4 | 4 | **144** | Enforced in 10.2 AC-02, AC-03 |
| EC-E10-03 | Story 10.3 `advisor` Regression Gate | Mọi bản vá `SKILL.md` do `advisor` sinh ra phải vượt qua bài kiểm thử hồi quy và bước qua `08.4 Staging + HUMAN_ONLY` trước khi kích hoạt. | 9 | 5 | 4 | **180** | Enforced in 10.3 AC-03, AC-04 |
| EC-E10-04 | Story 10.1 MPSC Actor Deadlock & `doc-patch.jsonl` Bounds | Malformed `$UIUX_LIVE_SPEC`, symlink attack, MPSC queue OOM under 50+ concurrent writers, or blocking I/O inside SQLite actor deadlocking callers in `06.2`/`08.4`. Mitigation: Schema validation + `O_NOFOLLOW` `<=512KB` cap, bounded MPSC returning `429 Retry-After`, zero blocking disk/HTTP inside SQLite actor (`busy_timeout=5000`, `0 SQLITE_BUSY`). | 9 | 5 | 5 | **225** | Enforced in 10.1 AC-02, AC-03 |
| EC-E10-05 | Story 10.2 Disjoint Prefix Lease & Expired Worker Write | Lexical `owned_paths[]` allows parent/child or symlink overlap, or slow worker mutates disk after TTL expiry; missing `"08.5"` dep. Mitigation: Canonicalized ancestor/prefix overlap check in `leases/acquire`, `-PGID` kill + scratch staging on lease expiry, monotonic `fencing_token` CAS, `429` clamp `3->1`, `"08.5"` dep, `V17b` slot. | 9 | 6 | 4 | **216** | Enforced in 10.2 AC-01, AC-03, §6 |
| EC-E10-06 | Story 10.3 `advisor` `sealed/` Tamper & Borrow Style Loss | `advisor` `Patch AST/Eval` mutates `tests/eval/sealed/` or auto-activates `SKILL.md` bypassing `08.4 HUMAN_ONLY`; `Cross-Variant Borrow` drops CSS styles; partial `<=15s` timeout kills tournament. Mitigation: Kernel deny-guard (`SEALED_EVAL_TAMPER_ERROR`) on `tests/eval/sealed/`, mandatory `08.4 Staging + HUMAN_ONLY`, extract dependent CSS/tokens with `reconcile_ff_ids`, `min_variants>=2` fallback, `generation fencing`, `V18b` slot. | 10 | 5 | 5 | **250** | Enforced in 10.3 AC-01..AC-04 |

## 2. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Initial FMEA Risk Matrix for Epic 10 (cowok Master Orchestrator, advisor Meta-Agent, OPC Execution Fleet & Design Tournament) | Review Agent & Advisor |
| 2026-11-20 | 2.0.0 | Migrated to 1-10 FMEA scale (`EC-E10-01..06`), enforced MPSC non-blocking actor, canonical `V17b` leases, `-PGID` expiry kill, and kernel `advisor` isolation from `tests/eval/sealed/` | Review Agent (`573b2485`) |
