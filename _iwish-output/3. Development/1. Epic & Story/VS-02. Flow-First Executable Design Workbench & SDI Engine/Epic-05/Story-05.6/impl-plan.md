---
type: I-Wish Implementation Plan
story_id: "05.6"
risk_tier: 3
status: pending_approval
target_files:
  - crates/html-forge/src/brand_studio.rs
  - crates/html-forge/src/sketch_scanner.rs
  - crates/html-forge/src/fe_contract.rs
  - crates/html-forge/src/lib.rs
  - crates/html-forge/tests/story_05_6_brand_studio.rs
  - crates/spec-registry/migrations/V25__story_05_6_brand_proposals_and_heritage_packs.sql
  - crates/harness-daemon/src/routes/cognitive_studio.rs
  - crates/harness-daemon/tests/story_05_6_cognitive_studio.rs
  - src/lib/validations/cognitive-studio.ts
  - src/api/cognitive-studio.ts
  - src/app/api/v1/cognitive/brand-proposals/route.ts
  - src/app/api/v1/cognitive/scan-components/route.ts
  - src/app/api/v1/cognitive/fe-data-contract/route.ts
  - src/app/api/v1/cognitive/heritage-packs/route.ts
  - src/app/api/v1/cognitive/hidden-specs/route.ts
  - src/features/brand-studio/BrandVisualStudioModal.tsx
  - src/features/brand-studio/SharedComponentGallery.tsx
  - src/features/brand-studio/PopularComponentAuditTable.tsx
  - src/features/brand-studio/HiddenSpecDrawer.tsx
  - src/features/brand-studio/HeritagePackPicker.tsx
  - src/features/brand-studio/useBrandStudioController.ts
  - src/features/brand-studio/index.ts
  - src/features/sketch-polish/StageSkinToggleBar.tsx
  - src/pages/workbench/components/WorkbenchCanvasTabArea.tsx
  - src/features/brand-studio/__tests__/brand-visual-studio.test.tsx
  - tests/e2e/story-05.6.spec.ts
coordinator_glue_files:
  - crates/spec-registry/src/lib.rs
  - crates/harness-daemon/src/routes/mod.rs
  - crates/harness-daemon/src/lib.rs
  - prisma/schema.prisma
  - src/lib/vite-bff-routes.ts
  - src/i18n/locales/*.json
---

# Implementation Plan — Story 05.6: Phase 2 Brand & Component Visual Studio, User-Driven `themeModes`, Hidden Spec Drawer & FE Data Contract

## 1. Architecture & Execution Breakdown

### Phase A — Rust Engine (`crates/html-forge`, `Task 05.6.1` & `05.6.2`)
1. **`crates/html-forge/src/brand_studio.rs`:**
   - Implement `BrandProposalV1`, `BrandModeTokens`, and `render_tokenized_brand_preview(sketch_pages, proposal, active_mode)`:
     - Enforces **User-Driven `themeModes`** (never injects unrequested Light/Dark modes; fails closed if `themeModes` is empty or contains invalid mode IDs).
     - Runs every CSS token through `validate_and_scrub_css_tokens` (blocking `url(`, `expression(`, `@import`, `</style`) + `evaluate_oklch_contrast_guard` (`>= 4.5:1` WCAG AA) in `<5ms`.
     - Caps rendered representative Sketch pages to `<= 3` (`Hero`, `List/Detail`, `Action Form`) when project has `> 20` screens (`AC-06`).
2. **`crates/html-forge/src/sketch_scanner.rs`:**
   - AST/DOM scanner (`html5ever`) extracting `data-component-kind` / semantic component primitives from Sketch Phase 1 HTML and cross-referencing `popular-components.{fe,deck}.json` into 3 contextual buckets (`detected_in_sketch`, `suggested_contextual`, `not_needed`).
3. **`crates/html-forge/src/fe_contract.rs`:**
   - Deterministic extractor `extract_fe_data_contract(project_id, screen_id, html_doc)` emitting `FeDataContractDraft` (`FE_REQUIRED_VIEWMODEL_AND_ACTIONS_ONLY`: `viewModelFields`, `actionInputs`, `subStateFixtures` for `DEFAULT/EMPTY/LOADING/ERROR`) and serializing clean `fe-data-contract.draft.ts` TypeScript code.

### Phase B — SQLite `V25` & Daemon APIs (`crates/harness-daemon/src/routes/cognitive_studio.rs`)
1. **Migration `V25__story_05_6_brand_proposals_and_heritage_packs.sql`:**
   - Additive `brand_proposal_snapshots` & `heritage_design_packs` tables with composite `(tenant_id, project_id)` indexing + ADR-091 Prisma mirror.
2. **Routes (`cognitive_studio.rs`) & Vite BFF (`src/app/api/v1/cognitive/*`):**
   - `POST /api/v1/cognitive/brand-proposals`: generates/persists 2–3 `BrandProposalV1` variants strictly honoring requested `themeModes`, returns `<5ms` tokenized previews for `<= 3` representative Sketch pages.
   - `POST /api/v1/cognitive/scan-components`: runs `scan_sketch_components`, merges with popular catalog, persists user selections to `07_COMPONENT_REGISTRY` / `component_patterns`.
   - `GET /api/v1/cognitive/fe-data-contract`: returns `FeDataContractDraft` + generated `fe-data-contract.draft.ts` source.
   - `GET / PUT /api/v1/cognitive/hidden-specs`: bidirectional `01..09` spec read/update with strict OCC `expectedRevision` → returns `409 STALE_MAPPING` on concurrent Visual vs Drawer edit (`AC-06`).
   - `GET / POST /api/v1/cognitive/heritage-packs`: persists/lists Heritage Packs in SQLite `V25` + path-jailed `~/.cowok-design/heritage-packs/<pack_id>.json` (`O_NOFOLLOW`, `0o600`, reusable across App FE & 16:9 Presentation with 5 Adaptive Entry Modes).

### Phase C — Frontend Brand Studio, Gallery & Hidden Spec Drawer (`src/features/brand-studio/*`, `Task 05.6.3` & `05.6.4`)
1. **`BrandVisualStudioModal.tsx`:** Modular modal with left Brand Proposal list + Heritage Pack Picker, dynamic `themeModes` pill bar (auto-hidden when `themeModes.length === 1`), and 3-tab right stage (`Trang thật từ Sketch`, `SharedComponentGallery.tsx`, `PopularComponentAuditTable.tsx`).
2. **`HiddenSpecDrawer.tsx` (`RightDrawer [5.25]`):** Bidirectional `01..09` Spec editor with OCC conflict banner + `FE Data Contract` tab (`DataTableView [5.21]` + copyable `fe-data-contract.draft.ts`).
3. **Workbench Integration:** Compact trigger buttons in `StageSkinToggleBar.tsx` / `WorkbenchCanvasTabArea.tsx` (`"Brand Studio"` in Phase 2 + `"Đặc tả ẩn"` drawer button) preserving the single-row `34px` toolbar height from Story 05.5.
4. **9-Locale i18n Parity:** Add all new keys across all 9 locales (`en, es, fr, it, ja, ko, pt, vi, zh`).

## 2. Four-Pillar Risk & Edge-Case Verification (`/plan-proven-safe`)
- **Single-Mode Guard (`AC-02`, `AC-06`):** When `themeModes = ["warm"]`, zero Light/Dark switcher rendered and zero unused CSS variables emitted.
- **3-Iframe RAM Cap (`AC-06`):** `> 20` screens deterministically select max 3 representative pages (`Hero`, `List/Detail`, `Action Form`).
- **CSS Injection & OKLCH AA Fail-Closed (`AC-06`):** `url(`, `expression(`, `</style`, and contrast `< 4.5:1` rejected or clamped deterministically before rendering.
- **Concurrent Visual + Drawer OCC (`AC-06`):** Outdated `expectedRevision` returns HTTP `409 STALE_MAPPING` with non-destructive reload prompt.
- **Path Jail for Heritage Packs (`AC-05`):** Strict slug regex + `O_NOFOLLOW` + canonical parent check inside `~/.cowok-design/heritage-packs/`.

## 3. Verification Plan
- **Rust Tests:** `cargo test -p html-forge --test story_05_6_brand_studio` + `cargo test -p harness-daemon --test story_05_6_cognitive_studio`
- **Frontend Unit Tests:** `npx vitest run src/features/brand-studio/__tests__/brand-visual-studio.test.tsx src/i18n/locales.test.ts`
- **Live Browser E2E (Stage 4):** `tests/e2e/story-05.6.spec.ts` against isolated Rust daemon (Desktop Warm Light, Night Dark, Mobile `390px`, single-mode `["warm"]` vs multi-mode `["warm","dim"]`, Shared Component Gallery, Popular Audit, Hidden Spec Drawer OCC `409`, FE Data Contract export, Heritage Pack save/load — 0 console/page/5xx errors).
