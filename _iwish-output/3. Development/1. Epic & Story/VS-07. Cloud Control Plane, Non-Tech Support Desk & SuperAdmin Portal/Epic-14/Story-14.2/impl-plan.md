---
type: I-Wish Implementation Plan
story_id: "14.2"
epic_id: "14"
risk_tier: 3
migration_slot: "V29__story_14_2_cloud_control_plane.sql"
parallel_wave: "Wave-C"
status: pending_approval
target_files:
  - crates/spec-registry/migrations/V29__story_14_2_cloud_control_plane.sql
  - crates/harness-daemon/src/routes/cloud_admin_auth.rs
  - crates/harness-daemon/src/routes/cloud_admin.rs
  - crates/harness-daemon/tests/story_14_2_cloud_admin.rs
  - src/lib/validations/cloud-admin.ts
  - src/api/cloud-admin.ts
  - src/app/api/v1/cloud-admin/ingest/tickets/route.ts
  - src/app/api/v1/cloud-admin/ingest/telemetry/route.ts
  - src/app/api/v1/cloud-admin/auth/session/route.ts
  - src/app/api/v1/cloud-admin/tickets/route.ts
  - src/app/api/v1/cloud-admin/tickets/[id]/reply/route.ts
  - src/app/api/v1/cloud-admin/clusters/route.ts
  - src/app/api/v1/cloud-admin/analytics/funnel/route.ts
  - src/app/api/v1/cloud-admin/crashes/route.ts
  - src/features/superadmin/SuperAdminPortalPage.tsx
  - src/features/superadmin/SuperAdminKpiStrip.tsx
  - src/features/superadmin/HelpdeskInboxTab.tsx
  - src/features/superadmin/TicketDetailDrawer.tsx
  - src/features/superadmin/FeatureClustersTab.tsx
  - src/features/superadmin/ActivationFunnelTab.tsx
  - src/features/superadmin/CrashCenterTab.tsx
  - src/features/superadmin/useSuperAdminController.ts
  - src/features/superadmin/superadmin.test.tsx
  - tests/e2e/story-14.2.spec.ts
coordinator_shared_wiring:
  - crates/spec-registry/src/pool.rs
  - crates/harness-daemon/src/routes/mod.rs
  - crates/harness-daemon/src/routes/legacy_router.rs
  - prisma/schema.prisma
  - prisma/schema.contract.test.ts
  - src/lib/vite-bff-routes.ts
  - src/App.jsx
  - src/i18n/locales/*.json
---

# Implementation Plan — Story 14.2 (Cloud Ingestion API & SuperAdmin Portal MVP)

## 1. Architectural Strategy
1. **Additive Migration (`V29`):** Create `cloud_support_tickets`, `cloud_ticket_replies`, `cloud_feature_clusters`, `cloud_telemetry_events`, `cloud_crash_groups`, and `admin_audit_logs` (PostgreSQL-compatible DDL mirrored in `prisma/schema.prisma`).
2. **Zero-Trust Cloud Ingestion & Anti-Spam (`crates/harness-daemon/src/routes/cloud_admin.rs`):**
   - Separate isolated module mounted under `/v1/ingest/*` and `/v1/admin/*` (keeps Cloud Control Plane strictly decoupled from local design workbench routes).
   - Fixed-window/token-bucket rate limiter keyed by `(install_id, client_ip)` returning `429 RATE_LIMIT_EXCEEDED` without impacting other install IDs.
   - Server-side Defense-in-Depth PII redaction (API keys `sk-*`, `ghp_*`, `AIza*`, JWTs, emails, bearer tokens, home paths) applied BEFORE DB persistence or log output.
   - Strict attachment validation (`<= 5 MB`), magic-byte MIME verification (`PNG/JPEG/WEBP`), polyglot/SVG/HTML payload rejection (`422 INVALID_ATTACHMENT_PAYLOAD`), and clean image re-encoding via `image` crate (or strict lossless PNG/JPEG structural re-pack) stripping EXIF/trailing polyglot bytes.
3. **OIDC + MFA + 4-Role RBAC & Immutable Audit Log (`crates/harness-daemon/src/routes/cloud_admin_auth.rs`):**
   - Signed HMAC/JWT session verifier enforcing `mfa_verified == true` and 4 canonical roles (`SUPER_ADMIN`, `SUPPORT_AGENT`, `SKILL_CURATOR`, `ANALYST_READONLY`).
   - Every `/v1/admin/*` mutation atomically appends a row to `admin_audit_logs` inside `TransactionBehavior::Immediate` (no `UPDATE`/`DELETE` endpoint exists on `admin_audit_logs`).
   - 2-way ticket sync: replies written to `cloud_ticket_replies` also propagate cleanly to client ticket status (`replied`) for `GET /api/v1/support/tickets` polling (`Story 14.1`).
4. **SuperAdmin Portal UI (`src/features/superadmin/` mounted at `/admin`):**
   - Implements `DESIGN.md` patterns `[5.10] Metric Line`, `[5.21] DataTableView`, `[5.22] AdvancedFilterBar`, `[5.25] RightDrawer`, `[5.12] Toast` with zero Tailwind, semantic CSS tokens, `<BrandIcon>`, and 9 locales.

## 2. Verification & TDD Gates
- Rust: `cargo test -p harness-daemon --test story_14_2_cloud_admin` (RBAC escalation `403`, MFA missing `401`, flood `429`, polyglot image `422`, server-side PII scrub, 2-way reply & audit immutability).
- Frontend: `npx vitest run src/features/superadmin/superadmin.test.tsx`
- Stage 3: Dual-Engine Plan-Blind Adversarial Review (`risk_tier: 3`)
- Stage 4: Live Browser Playwright E2E `tests/e2e/story-14.2.spec.ts` against `http://127.0.0.1:<PORT>/admin`
