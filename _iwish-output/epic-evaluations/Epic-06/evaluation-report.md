# Epic 06 Evaluation & Actionable Implementation Plan (`VS-03`)

**Epic:** `Epic-06: Dual-Pane 16:9 HTML Presentation & Non-Tech CMS Workbench`  
**Value Stream:** `VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio`  
**Overall Verdict:** `CONDITIONAL` (`HOLD_REAUTHOR_BEFORE_MAKE_STORY`)  
**Authorized Envelope (`story_creation_envelope.max_phase`):** `"INVESTIGATE"` (Technical Spikes & Spec Reconciliation authorized; **Implementation blocked** until Spike Exit Criteria pass and Stories `06.1`–`06.4` are re-authored via `/reconcile-change` + `/make-story`)

---

## 1. Executive Summary & Synthesis

Deep-mode evaluation across **PM**, **Architect/Security**, **UX**, and **Unknowns Analyst** (2 rounds, 26 classified unknowns: `6 CRITICAL`, `13 HIGH`, `7 MEDIUM`) plus **Edge Case Guardian** Phase 1 FMEA audit established that `Epic-06` **cannot move directly to implementation** on top of the current story drafts:
1. Existing `Story-06.1`, `Story-06.2`, and `Story-06.3` contain copy-pasted `CS = 4 (Unknowns = 0)` / `9.40/10` scorecards, status drift (`refactored` vs `Epic-06` `planned`), non-existent `PRESENTATIONS` table references (`V1` defines `presentation_decks`, `deck_slides`, `wording_slots`), and **zero Presenter View ACs/Tasks** in `Story-06.3` despite `Presenter Screen Mode` being in its title.
2. `crates/constraint-engine/src/surgical_patcher.rs` uses `oxc_parser` (`SourceType::tsx()`) and cannot parse HTML5 slides; `useSandboxedIframeBridge.ts` hashes `rawHtml` into `targetCacheKey` (`resolvedSha256`), tearing down the `IframeKeepAliveSlot` and preventing `<16ms` typing sync; CSS comment-strip mXSS and SVG `<style>`/`<foreignObject>` mXSS gaps exist under `CSP-B`.
3. `DESIGN.md` contains an internal topology conflict between `§8 LAY-05` / `CMP-10` (right CMS column) and `§9.1` / `src/features/flow/components/CompanionDualPaneStage.tsx` (left `#structureSubPane` + `translateX(-50%)` Slide-Over CMS Drawer + right 16:9 Stage).

---

## 2. Plugin Decomposition Strategy

Extracted and locked from `plugin-boundary-review.md` (`DEC-EPIC-06-PRESENTATION-CMS`, verdict `NOT_APPLICABLE` / `delivery: "core"`):

- **Classification (`CORE_ONLY`):** `Epic-06` owns core workbench transactions, SQLite WAL state (`presentation_decks`, `deck_slides`, `wording_slots`), `fs_jail` AST/HTML writes, and `CSP-B` `<iframe sandbox="allow-scripts allow-forms">` (`0 allow-same-origin`) enforcement. No capability is safe or beneficial to extract into third-party `SKILL.md` / plugin runtimes.
- **Layering Boundary (`KERNEL` vs `DOMAIN` vs `EXTENSION`):**
  1. **`KERNEL` (Platform Primitives — Shared Core):**
     - `src/features/flow/hooks/useSandboxedIframeBridge.ts`: LRU keep-alive iframe slot pooling (`POOL_MAX_SLOTS`), CSPRNG nonce/epoch `MessageChannel` handshake, focus-aware `HostRuntimeContract` (`data-cowok-deck-protocol="1"`), and `BroadcastChannel` Presenter relay.
     - `KeyedParallelCoordinator` (`Epic 04` reuse) and `src/styles/tokens.css` (isolated host vs deck token rules, Warm/Dim `--agent-accent` separation).
     - **Explicitly Excluded (`OUT_OF_CORE`):** No third-party CDN chart scripts (`jsdelivr`/`unpkg`), no dynamic `EXTENSION` runtime injection inside sandboxed `srcDoc` slides.
  2. **`DOMAIN` (VS-03 Core Workbench Modules):**
     - `crates/presentation-cms`: HTML5-native `lol_html` `<1ms` surgical slot patcher, `html5ever` SVG/DOM structural sanitizer, CSS token sanitizer, 8 branded 16:9 template generators, chart JSON validator, and `V12` SQLite WAL repository.
     - Presentation workbench UI domain module integrated cleanly into `src/features/flow/components/CompanionCanvasTab.tsx` and `src/features/flow/components/CompanionDualPaneStage.tsx`: Slide-Over CMS Drawer (`inert`-guarded), E3 `ResizeObserver` overflow telemetry, 2D chart SVG/`<canvas>` bridge, and Presenter Window Controller.

---

## 3. Actionable Implementation Plan (Pre-Implementation Spikes & Spec Reconciliation)

No product source code may be edited under this evaluation gate. Execute Phases A–C strictly inside `INVESTIGATE` before unlocking `AUTHOR_ONLY` / `IMPLEMENT`:

### Phase A — Five Non-Negotiable Technical Spikes (Unlock `AUTHOR_ONLY`)

1. **Spike S1 — Rust HTML5 Patcher, mXSS Defense, Cross-Boundary Atomicity & Upstream Contracts (`crates/presentation-cms`):**
   - Keep `crates/constraint-engine/src/surgical_patcher.rs` exclusively for TSX/JSX (`SourceType::tsx()`).
   - Design `crates/presentation-cms` hybrid pipeline: `lol_html` for `<1ms` byte-preserving `[data-ff-id]` text/metric/chart replacement, paired with `html5ever` DOM tree sanitization + strict SVG element/attribute allowlist on `APPLY_HTML` / disk ingest.
   - **mXSS & CSP-B Hardening (`EC-06-05`):**
     - Ban `<svg><style>`, `<foreignObject>`, `<script>`, `<animate>`, `<animateMotion>`, `<animateTransform>`, `<set>`, `<use>`, `on*`, `xlink:href`, `<meta>`, `<base>`, nested `srcdoc`, `formaction`, and entity-obfuscated `javascript:`/`data:text/html`/`data:image/svg+xml` URIs.
     - Route both `<style>` blocks AND inline `style=""` attributes through a CSS token sanitizer rejecting `</style`, `@import`, `url()`, and `expression()` (replacing regex comment stripping).
     - Define deterministic CSP-B import policy: inline `<style>` blocks in valid imported HTML are CSS-token-sanitized and nonce-rewritten (`style-src 'nonce-...'`) via `lol_html`; remove unsafe `'unsafe-inline'` offline CSP fallbacks and fail closed with an explicit user error banner + `securitypolicyviolation` telemetry if sanitization/nonce checks fail.
   - **Structural Protocol Verification (`EC-06-13`):** Validate `.slide` / `.slide.active` presence and unique `[data-ff-id]` per slide at ingest/generation; reject or repair malformed structures before DOM mount, and bind unmapped `[data-ff-id]` slots to read-only inspector chips without crashing the 8 template schemas.
   - **Cross-Boundary Disk + SQLite WAL Atomicity (`EC-06-02`):** Specify write-staging protocol under `fs_jail` (`O_NOFOLLOW` + canonicalized containment): (1) write & `fsync` to `.tmp` in target dir -> (2) open SQLite WAL tx and update `content_sha256` + monotonic `revision` -> (3) atomic `rename()` -> on `rename()` failure, roll back SQLite tx & unlink `.tmp`; on startup/read, reconcile `content_sha256` mismatch and sweep orphaned `.tmp` files. Enforce `MAX_SLIDE_HTML_BYTES = 524288` (`512 KB`) and `MAX_SLOT_CHARS = 8000` (`E_PAYLOAD_TOO_LARGE`).
   - **Cross-Epic Contract Audit (`EC-06-11`):** Explicitly audit and plan extensions for `Story 03.3` `patch_by_ff_id` / `postPatchByFfId` (adding `chart_json`, structured bullet arrays, and `revision` receipts beyond `textContent` & `safeStyles`) and `Story 04.3` `KeyedParallelCoordinator` per-slide cancellation/receipts.

2. **Spike S2 — `<16ms` Non-Reloading Bridge, `POOL_MAX_SLOTS` Pinning, Crash Recovery & Unified Undo (`useSandboxedIframeBridge.ts`):**
   - **Zero-Reload Fast Path & `targetKey` Isolation:** Decouple `targetCacheKey` in `src/features/flow/hooks/useSandboxedIframeBridge.ts` from live `PATCH_BY_FF_ID` text/chart edits so `resolvedSha256` does **not** rotate or tear down `IframeKeepAliveSlot` during typing (`srcDoc` reloads only on template change, `APPLY_HTML`, or structural slide change). Fix `resolveTargetNodeId` and `targetKey` for `structureLens === "slide-cms"` so preserving `selectedNodeId` across lens switching neither merges slides nor fragments slot keys, keying each slide by immutable ULID `SLD-*`.
   - **Pinned Pool & `one-page` Scoping (`EC-06-01`):** Pin active Audience slide, Presenter active slide, and Presenter next-slide preview slots inside `IframeKeepAlivePool` (raise or reserve pinned capacity inside `POOL_MAX_SLOTS = 5` / configurable floor so 15–25 slide navigation never evicts active Presenter/Audience frames). For `one-page` decks (`deck_mode = 'one-page'`), isolate each slide inside `<section data-slide-uid="SLD-*">` sharing the parent `html_path` and namespace `[data-ff-id]` slots to `slide_uid` under the shared file's `content_sha256` OCC.
   - **Queue Policy & Stale Closure Fix (`EC-06-12`):** Replace `channel.port1.onmessage` load-time captures (`activeSubState`, `nodeLabel`) with latest React `ref` accessors; cap pre-`PORT_ACK` queue in `postToPort` at `32` frames, coalescing repeated `PATCH_BY_FF_ID` by `(slide_uid, ff_id)`, dropping stale rect probes, and triggering handshake retry (max `3`, `500ms` timeout). Throttle `publishElementRects()` via `requestAnimationFrame`.
   - **Debounced Persistence, Dirty-Draft Guard & Crash Recovery (`EC-06-03`):** Enforce two-tier sync (`<16ms` `MessageChannel` DOM patch; `150–300ms` debounced + immediate flush on blur/slide-switch/`▶ Present`/export to Rust + SQLite `revision` OCC). Add: (a) `beforeunload` & route/tab-switch dirty-draft confirmation when flush is pending; (b) `sessionStorage`/`IndexedDB` crash-recovery draft journal keyed by `(slide_uid, revision)` to restore unflushed keystrokes on crash/reload; (c) explicit retry/rollback notification (`Toast [5.12]`) if async flush hits `SQLITE_BUSY`, OCC stale hash, or `ENOSPC`.
   - **Split-Brain Resolution (`EC-06-06`), IME Safety & Unified Undo/Redo (`EC-06-10`):**
     - **Single SSOT Rule on `APPLY_HTML`:** Executing `APPLY_HTML` invokes the Rust HTML5 parser to re-extract matching `[data-ff-id]` slots back into `wording_slots` (`vp.slides`); if manual HTML deletes/breaks required slot bindings, transition that slide explicitly to `custom_html_override` mode and disable/badge conflicting CMS inputs until re-synced or reset to template.
     - **IME Guard:** Skip normalization/echo updates while `isComposing === true` (`compositionstart`..`compositionend`) for `vi-VN` Telex/VNI and CJK locales.
     - **Unified Reverse-Patch Undo (`⌘Z`/`⇧⌘Z`):** Maintain per-slide inverse-command stack `(slot_uid, before, after, source)` surviving `translateX(-50%)` drawer transitions; synchronize DOM + CMS + debounced AST/SQLite state cleanly, treating Copilot batch updates (`Story 06.3`) and `APPLY_HTML` as atomic single-step undo frames.

3. **Spike S3 — Offline 2D Live Charts, Bounded Inputs & Typed Validator (`CSP-B`):**
   - Scope `Story-06.3` strictly to **2D interactive charts** (`pie`, `donut`, `100_stacked`, `bar`, `line`, `area`, `funnel`) and explicitly **defer `3D widget` out of `Epic-06`**.
   - Render charts zero-CDN under `default-src 'none'; connect-src 'none'; script-src 'nonce-...'` via allowlisted SVG primitives or Host-bridged `<canvas>` + positioned `[data-ff-id]` overlay + accessible `aria-label` / tabular fallback. On malformed Copilot `charts_json`, keep last-valid rendered chart DOM intact and surface inline retry feedback (`E2`).
   - **Numeric DoS Caps & Chart-Type Validation (`EC-06-08`):** Enforce explicit Rust/Zod bounds (`deny_unknown_fields`): `MAX_SERIES = 12`, `MAX_POINTS_PER_SERIES = 200`, `MAX_SVG_PRIMITIVES = 2000`, `MAX_LABEL_CHARS = 120`, finite coordinate magnitude `<= 1e6` (blocking `1e308` SVG hangs, `NaN`, `Infinity`), with deterministic error codes. Apply `|sum - 100.0| <= 0.05` and non-negative enforcement (`>= 0`) strictly to proportional/funnel charts (`pie`, `donut`, `100_stacked`, `funnel`), while permitting signed values on cartesian charts (`bar`, `line`, `area`).

4. **Spike S4 — Additive `V12` SQLite WAL Migration, Backfill & Copilot Lock Lease (`V1` Compatibility):**
   - Extend `crates/spec-registry` `V1` tables (`presentation_decks`, `deck_slides`, `wording_slots`) via backwards-compatible additive columns and SQLite-safe unique indexing inside a single transaction (`EC-06-07`):
     - `presentation_decks`: `aspect_ratio TEXT NOT NULL DEFAULT '16:9'`, `deck_protocol_version INTEGER NOT NULL DEFAULT 1`, `deck_mode TEXT NOT NULL DEFAULT 'multi-page'`, `revision INTEGER NOT NULL DEFAULT 0`.
     - `deck_slides`: add nullable `slide_uid TEXT`, backfill deterministic ULID (`SLD-*`) across all existing `V1` rows, then `CREATE UNIQUE INDEX IF NOT EXISTS deck_slides_slide_uid_key ON deck_slides(slide_uid)`; add `speaker_notes TEXT NOT NULL DEFAULT ''`, `narrative_arc TEXT NOT NULL DEFAULT ''`, `eyebrow_tag TEXT NOT NULL DEFAULT ''`, `revision INTEGER NOT NULL DEFAULT 0`. Handle slide reorder swaps atomically and regenerate new `slide_uid`/`ff_id` on slide duplication.
     - `wording_slots`: `slot_kind TEXT NOT NULL DEFAULT 'text'`, `charts_json TEXT NOT NULL DEFAULT '[]'`, `safe_styles_json TEXT NOT NULL DEFAULT '{}'`, `updated_by TEXT NOT NULL DEFAULT 'human'`, `ast_revision INTEGER NOT NULL DEFAULT 0`.
   - Audit and replace all positional `SELECT *` / `row.get(idx)` queries in `crates/spec-registry` prior to `V12`.
   - **Copilot Concurrency & Snapshot Guard (`EC-06-09`):** Require synchronous dirty-CMS flush immediately *before* Copilot reads/streams on a slide (preventing TOCTOU against unflushed human typing); lock that slide's CMS inputs with a visual lease badge & **`60s` TTL auto-release** (releasing deterministically on worker crash, timeout, or SSE cancel); enforce `guardReadOnlyAction` across both Host CMS and Presenter View when previewing historical `SNAP-*` snapshots.

5. **Spike S5 — `HostRuntimeContract`, Dual-Screen Presenter Sync Security & Focus Routing:**
   - Finalize Zod schemas for `IframeToHostMessageSchema` / `HostToIframeMessage` (`SLIDE_CHANGE`, `DECK_KEY_NAV`, `DECK_PROTOCOL_INIT`, `SLIDE_OVERFLOW_WARN`, `PATCH_ACK`).
   - **Hardened Presenter Channel (`EC-06-04`):** Relay `Iframe MessageChannel <-> Host BroadcastChannel <-> Presenter Window`. Provision `sessionSecret` out-of-band via `window.opener` / `MessageChannel` handshake (**strictly forbid URL query params `?secret=`**), validate Host `origin`, constant-time `sessionSecret`, `deckId`, and Zod payload on both Host and Presenter listeners, support `SYNC_FULL_STATE` rehydration if Presenter refreshes, provide Tauri multi-webview IPC parity, call `channel.close()` on unload, and trigger `window.open` synchronously in `onClick` with a mandatory **Inline Split Presenter fallback** + `Toast [5.12]` when blocked by popup blockers.
   - **Focus & Keyboard Traps (`EC-06-14`):** Enforce `inert` + `aria-hidden="true"` on the inactive half of `#structureSubPane` (`translateX(-50%)`). Suppress deck arrow navigation inside both Host and `HostRuntimeContract` script whenever focus is inside interactive inputs (`input`, `textarea`, `select`, `[contenteditable="true"]`), `pointerMode === "interact"`, or `event.isComposing || event.keyCode === 229` (IME active). Forward Presenter keys (`B`/`W` blackout, `F` fullscreen) via `HostRuntimeContract`; bind React `Present` state to `fullscreenchange` for **single-`Esc` exit** on stage, while `Esc` inside the Presenter Window closes only the popout.
   - **Observability Targets (P10):** Instrument p95 `PATCH_BY_FF_ID` frame latency (`<16ms` budget), `E3` overflow counts, CSP `securitypolicyviolation` reports, sanitizer rejections, and async flush failure metrics.
   - Separate Deck `▶ Present` state controller from `useWalkthroughController` (`WorkbenchNode` DAG walkthrough).

### Phase B — `DESIGN.md` & `tokens.css` Alignment

- **Topology SSOT & Legacy Deprecation (`EC-06-14`):** Ratify `DESIGN.md §9.1` + `src/features/flow/components/CompanionDualPaneStage.tsx` (Left `#structureSubPane` toggle `[Userflow | Slide / CMS]` + `translateX(-50%)` Slide-Over CMS Drawer ↔ Right 16:9 Isolated Stage) as canonical; update `DESIGN.md §8 LAY-05` / `CMP-10` and explicitly deprecate the superseded `PresentationDualPaneView` right-CMS concept so no duplicate dual-pane UI is built.
- **Stage Scale & Overflow (`E3 Gate`):** Remove inline `minWidth: "480px"` overflow risk on `1280px`/`1440px` screens (use fluid `min-width: 320px`), auto-collapse chat rail (`40px`) in Slide Studio preset for `--slide-scale >= 0.45`, enforce `1920x1080` projection type scale (`display >= 56px`, `heading >= 32px`, `body >= 22px`, `micro >= 16px`), expose scale zoom pill (`Fit | 100% Pan`), and run unscaled `scrollHeight > clientHeight` overflow warnings inside the slide container (`16:9`, `16:10`, `Auto` via adaptive `--slide-w` / `--slide-h` variables).
- **Theme Isolation:** Guarantee `.live-html-frame` `!important` white overrides in `src/styles/tokens.css` never bleed into dark-mode decks (decks own `html, body` background tokens in isolated `srcDoc`), and differentiate Warm/Dim `--agent-accent` (`#E89A35` / `#C97B1D`) from Human `--human-accent`.

### Phase C — `/reconcile-change` & Clean Story Re-Authoring (`06.1`–`06.4`)

Once Spikes `S1`–`S5` succeed and user approval advances the envelope:
1. Run `/reconcile-change` across `2.4. epics-and-stories.md`, `2.6. feature-hierarchy.md`, `Epic-06/epic.md`, `2.2. database-spec.md` (`V12` table names replacing `PRESENTATIONS`), and `DESIGN.md`.
2. Re-author clean stories via `/make-story` (with genuine `CS`, `risk_tier: 3` for schema/CSP-B/cross-window security paths, `status: planned`, and verified 7-axis scorecards):
   - **`Story 06.1` (`risk_tier: 3`):** 16:9 (`16:10`/`Auto`) Isolated HTML Slide Engine, 8 Project-Standard `layout_kind` Templates, `V12` Schema Foundation & `HostRuntimeContract` (`data-cowok-deck-protocol="1"`).
   - **`Story 06.2` (`risk_tier: 3`):** Dual-Pane Slide-Over Human CMS Workbench (`inert`-guarded), Dynamic Schema-Driven Fields + `speaker_notes`, `<16ms` Non-Reloading `MessageChannel` Sync, IME-Safe Cursor & Unified Undo/Crash-Recovery, `E3` Overflow Guard, and `lol_html` + `html5ever` mXSS Sanitizer.
   - **`Story 06.3` (`risk_tier: 3`):** Contextual Slide Copilot (`KeyedParallelCoordinator` pre-flushed per-slide lock lease) & Offline CSP-B 2D Interactive Charts (`SVG`/`<canvas>` + chart-type-aware numeric-bounded JSON Validator; `3D widget` deferred).
   - **`Story 06.4` (`risk_tier: 3`, split from `06.3`):** Dual-Screen Presenter Mode (out-of-band `sessionSecret` `BroadcastChannel` Host relay + Tauri parity, Speaker Notes viewer, `tabular-nums` pace timer, next-slide preview, popup-blocked inline fallback, and single-`Esc` keyboard safety).

---

## 4. Risk Matrix & Spike Gate Summary

| Blocker Group | Unknown / FMEA IDs | Mitigation Anchor | Exit Requirement to Unlock `AUTHOR_ONLY` |
| :--- | :--- | :--- | :--- |
| HTML5 Patcher, mXSS & Atomicity | `AS-02`, `AS-03`, `AS-06`, `EC-06-02`, `EC-06-05`, `EC-06-11`, `EC-06-13` | Spike `S1` (`lol_html` + `html5ever` + CSS tokens + `.tmp` rename/WAL) | Adversarial mXSS + `style=""`/SVG tests, `<1ms` patch benchmark, crash/rollback & `03.3`/`04.3` contract audit pass |
| `<16ms` Bridge, Pool, Recovery & Undo | `KU-04`, `UU-01..05`, `BS-02..04`, `BS-06`, `EC-06-01`, `EC-06-03`, `EC-06-06`, `EC-06-10`, `EC-06-12` | Spike `S2` (decoupled `targetCacheKey`, pool pinning, `one-page` scope, crash draft, `APPLY_HTML` SSOT, reverse-patch undo) | Zero `srcDoc` reloads on typing; pinned multi-slide pool, IME, crash recovery & split-brain tests pass |
| Offline 2D Charts & DoS Caps | `KU-03`, `AS-04`, `EC-06-08` | Spike `S3` (CSP-B zero-CDN SVG/Canvas + bounded typed Zod) | `pie`/`bar` render offline under `default-src 'none'`; numeric DoS caps & signed/non-negative rules verified |
| DB `V1` $\rightarrow$ `V12` & Copilot Lease | `KU-01`, `BS-07`, `EC-06-07`, `EC-06-09` | Spike `S4` (nullable+ULID+UNIQUE index `V12`, pre-flush + `60s` TTL lease) | Migration & duplicate/reorder index swaps succeed on seeded `V1` DB; `SELECT *` & lease expiry verified |
| Presenter Security, UX SSOT & Telemetry | `KU-02`, `KU-05..08`, `AS-01`, `AS-05`, `BS-01`, `BS-05`, `EC-06-04`, `EC-06-14` | Spike `S5` + Phase B + Phase C | Out-of-band `BroadcastChannel` security, `DESIGN.md §9.1` ratification, `06.4` split & P10 telemetry verified |
