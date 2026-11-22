# Epic 09 FMEA Risk Matrix — Design-Only REA & Awesome-RE Forge & JEV — TypeSafe 6-Primitive Acceleration Engine

## 1. FMEA Edge Case Registry

| Risk ID | Failure Mode | Potential Impact & Mitigation | S | O | D | RPN | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| EC-E09-01 | Story 09.1 Design-Only DNA Extraction | Tuyệt đối loại bỏ mọi chuỗi kết nối DB, API keys, logic nghiệp vụ độc quyền và văn bản có bản quyền khi trích xuất Design DNA qua `crates/rea-bridge`. | 9 | 5 | 3 | **135** | Enforced in 09.1 AC-02, AC-03 & Task 09.1.2 |
| EC-E09-02 | Story 09.2 Offline FTS5 & Egress Guard | Khi mất mạng hoặc chưa nhập BYOK key, tự động lùi mượt về `Local FTS5 BM25` (`<5ms`); mọi dữ liệu gửi Decision API bị băm cắt `4KB/64KB`. | 8 | 5 | 4 | **160** | Enforced in 09.2 AC-01..AC-03 |
| EC-E09-03 | Story 09.2 `MAX_ATTEMPTS = 3` Breaker (`09.3` absorbed) | Khi 3-Strike Circuit Breaker hoặc dao động quyết định kích hoạt `MAX_ATTEMPTS = 3`, lập tức ngắt mạch và chuyển log sang `advisor`/human (`09.3` marked `cancelled`). | 8 | 5 | 4 | **160** | Enforced in 09.2 AC-03 & Task 09.2.3 |
| EC-E09-04 | Story 09.1 Two-Store SSRF, Zip-Bomb & Context Bloat | URL webcapture / repo ingest triggers SSRF redirect, zip-bomb decompression on `.zip`/`.fig`, proprietary ToS breach, or `Tier-1A` `>250`-tok context bloat. Mitigation: Reuse `04.3` `SocketAddr` SSRF + `Policy::none()`, decompression quota, `License/ToS Firewall` `REJECT`, deterministic `<=250`-tok truncator. | 9 | 5 | 4 | **180** | Enforced in 09.1 AC-01..AC-04 |
| EC-E09-05 | Story 09.2 `ConfidenceKind` DB Mismatch & Prompt Fence Breakout | `ConfidenceKind` enum mismatch breaks `V17 decision_audit_log` `CHECK`, `</untrusted_artifact_bytes>` escapes prompt fence, or `RubricShield` oscillates on swap tie. Mitigation: Align SQLite/Rust ENUM (`ExactMatch\|ModelVerdict\|DeterministicFallback`), nonce-tagged `<untrusted_artifact_bytes nonce="...">`, `VerdictShield` `0%` float leak, deterministic tie-break, `V17a` slot. | 9 | 7 | 5 | **315** | Enforced in 09.2 AC-02..AC-04 |

## 2. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Initial FMEA Risk Matrix for Epic 09 (Design-Only REA & Awesome-RE Forge & JEV — TypeSafe 6-Primitive Acceleration Engine) | Review Agent & Advisor |
| 2026-11-20 | 2.0.0 | Migrated to 1-10 FMEA scale (`EC-E09-01..05`), retired `crates/jev-engine` in favor of `DecisionCascadeClient`, aligned `ConfidenceKind` `ExactMatch\|ModelVerdict\|DeterministicFallback` with `2.2. database-spec.md`, `09.3` cancelled | Review Agent (`573b2485`) |
