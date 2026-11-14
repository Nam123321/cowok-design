# Edge Case Guardian & Adversarial Review Report — Story 01.2

- **Story:** Story 01.2 — Xây dựng crates/spec-registry với rusqlite SQLite WAL (16 Bảng) & 2-Way File Sync
- **Epic:** Epic 01 (Cargo Workspace, Domain Contracts (ts-rs) & 16-Table SQLite Registry)
- **Reviewer:** Review Agent (Edge Case Guardian Skill)
- **Verdict:** APPROVED (PASS)

## 1. FMEA & Boundary Evaluation
- **Identified Edge Cases:**
  1. `EC-E01-01` / `UU-01.1`: Ngăn chặn vòng lặp vô tận (Infinite Watcher Echo Loop) giữa `notify` watcher (FSEvents trên macOS) và SQLite `PROJECT_SPECS` bằng cơ chế **Hash-Gated Watcher Suppression** (`disk_sha256 == db_sha256` $\rightarrow$ `NO-OP`) và ghi file nguyên tử qua `.tmp.<uuid>` + `rename()`.
  2. `EC-E01-02` / `EC-E01-05`: Ngăn chặn xung đột ghi đè mất dữ liệu (`Lost Update`) giữa Human và Execution Bot bằng kiểm tra bắt buộc `expected_sha256` (`StaleFile` OCC) kết hợp `PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;`.
  3. `EC-E01-04` / `UU-01.2` / `AS-01.2`: Chống xóa trắng nội dung do trình soạn thảo bên ngoài truncate file về `0 bytes` tạm thời khi Save bằng debounce `50ms`, đồng thời chặn thoát thư mục bằng `canonicalize()` và `O_NOFOLLOW`.
  4. `EC-E01-11`, `EC-E01-12`, `EC-E01-13`: Spawn `SpecWatcher` chạy nền thật trong `harness-daemon`; siết atomic SQL-before-disk và post-tombstone OCC cho `upsert_spec`/`delete_spec`; xác thực SHA-256 disk-vs-DB và báo cáo trung thực `atomic_complete` trong `ContextSlicer`; đóng lỗ hổng symlink trong `SpecLockGuard` và non-transactional `V4` migration / conflicting `project_workspace_files` DDL.
- **Mitigation Verified:** Embedded into `[EDGE-CASE]` Acceptance Criteria `AC-01`, `AC-03`, `AC-04`, `AC-05`, and `AC-06`, mapped to `Task 01.2.1`..`Task 01.2.5`.
- **Complexity Score (CS):** `5.5/10` (< 7 threshold — No story split required).

## 2. Final Verdict
**VERDICT: PASS / APPROVED** (Reconciled 2026-11-13 via `/audit-360` + `/party-mode`)
