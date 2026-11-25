# Epic 14 FMEA Risk Matrix — Cloud Control Plane, Non-Tech Support Desk & SuperAdmin Portal

## 1. FMEA Edge Case Registry

| Risk ID | Failure Mode | Potential Impact & Mitigation | S | O | D | RPN | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| EC-E14-01 | Story 14.1 Client PII Leak & Offline Outbox Loss | User-submitted support tickets or crash events leak local home paths, API keys, or session cookies, or are lost offline. Mitigation: Client-side Zero-PII Scrubber + SQLite WAL outbox with exponential backoff and Air-Gapped opt-out (`0` outbound calls when disabled). | 9 | 5 | 4 | **180** | Enforced in 14.1 AC-01..AC-03 |
| EC-E14-02 | Story 14.2 Self-Asserted OIDC Role Escalation & MFA Bypass | Unauthenticated or lower-privileged callers mint `SUPER_ADMIN` sessions, bypass 6-digit MFA, or spoof `X-Client-Ip` to evade rate limits. Mitigation: Server-side OIDC credential verifier (`HMAC` assertion / shared secret / authenticated BFF), server operator allowlist preventing upward role escalation, mandatory 6-digit TOTP/MFA check (`401 MFA_REQUIRED`), and `X-Client-Ip` restricted to authenticated proxy requests. | 10 | 5 | 4 | **200** | Enforced in 14.2 AC-01, AC-05 |
| EC-E14-03 | Story 14.2 Polyglot Image Upload & Cloud PII Injection | Malicious ticket attachments contain polyglot payloads or symlinks, or ingestion endpoints fail to scrub secrets/paths server-side. Mitigation: Structural PNG/JPEG/WebP magic + parser validation + polyglot trailer rejection, `O_NOFOLLOW`/`0o600` blob persistence, RBAC-guarded `GET /v1/admin/tickets/:id/attachment` (`X-Content-Type-Options: nosniff`, `Cache-Control: no-store`), `8 MiB` BFF body cap, and defense-in-depth server-side PII scrubber (home paths, `Bearer`, `AKIA`, `sk_live_`, `Cookie:`, `x-api-key:`). | 9 | 5 | 4 | **180** | Enforced in 14.2 AC-01, AC-02 |
| EC-E14-04 | Story 14.2 Audit Log Tampering & Funnel/Crash Distortion | Admin audit trail mutated/deleted, future-dated telemetry skews DAU/WAU/MAU or produces `>100%` funnel rates, or resolved crashes fail to reopen on recurrence. Mitigation: `V29` SQLite `BEFORE UPDATE` / `BEFORE DELETE` immutability triggers on `admin_audit_logs`, `occurred_at` clamping `[now-30d, now+5m]`, monotonically non-increasing `day_0` cohort funnel intersections, and exact `json_extract(properties_json, '$.fingerprint')` recurrence reopening. | 8 | 5 | 4 | **160** | Enforced in 14.2 AC-03..AC-05 |

## 2. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-25 | 1.0.0 | Created canonical Epic 14 FMEA Risk Matrix (`EC-E14-01..04`) covering Story 14.1 Client Edge & Story 14.2 Cloud Ingestion API / SuperAdmin Portal Stage 3 hardening | Review Agent (`2d48a0ec`) & Orchestrator |
