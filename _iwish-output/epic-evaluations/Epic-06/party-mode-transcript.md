# Party Mode Evaluation Transcript — Epic 06 (`VS-03`)

**Epic:** `Epic-06: Dual-Pane 16:9 HTML Presentation & Non-Tech CMS Workbench`  
**Value Stream:** `VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio`  
**Mode:** `deep` (`2` structured cross-examination rounds)  
**Panelists:** `PM` (`pm-agent-persona`), `Architect` & `Review` (`architect-agent-persona`), `UX` (`ux-agent-persona`), backed by `Unknowns Analyst` (`4` quadrants, `26` unknowns)  

---

## Round 1 — Independent Baseline Assessments & Initial Pushbacks

### 1. PM & CEO Scope Lead (`pm-agent-persona`)
- **Baseline Position:** `HOLD_REAUTHOR_BEFORE_MAKE_STORY` (`story_creation_envelope.max_phase: "INVESTIGATE"`).
- **7 Concrete Pushbacks (`PB-PM-01..07`):**
  1. `Story 06.1`, `06.2`, and `06.3` all contain copy-pasted `CS = 4 (Unknowns = 0)` and `9.40/10` scorecards despite unverified backend, schema, and UI scope, plus status drift (`status: refactored` while `Epic 06` is `planned`) and broken table entity references (`PRESENTATIONS` instead of `presentation_decks`).
  2. `Story 06.3` mentions `Presenter Screen Mode` / `Presenter View (màn hình phụ hiển thị Speaker Notes và đồng hồ bấm giờ)` in its title, description, and `AC-01`, yet has **zero dedicated ACs, tasks, or E2E verification** for Speaker Notes display, timer, next-slide preview, or dual-window synchronization.
  3. `Story 06.2` description promises `Ghi chú diễn giả` (Speaker Notes), yet `Story 06.2 AC-03` omits it from the CMS drawer fields and hardcodes a 5-field layout insufficient for 8 heterogeneous templates (`KU-05`).
  4. `FR-17` includes ambiguous `"Chart/3D widget"` phrasing which risks Three.js/CSP-B/bundle bloat unless explicitly bounded out of `Epic 06`.
  5. `useWalkthroughController` (interactive DAG node flow walkthrough) vs `▶ Present` (`vp.slides` 16:9 presentation mode) must be cleanly separated (`AS-05`).
  6. Overlapping `src/features/presentation` vs `src/features/flow` dual-pane components must be reconciled before authoring stories.
  7. `Story 06.1..06.3` dependency contracts on `Epic 03`/`Epic 04` (`patch_by_ff_id`, `KeyedParallelCoordinator`) must be verified at the Rust/TS boundary before implementation planning.

### 2. Architect & Security Reviewer (`architect-agent-persona`)
- **Baseline Position:** `HOLD / REQUIRES CONTRACT & SCHEMA HARDENING BEFORE IMPLEMENTATION`.
- **Key Findings & Pushbacks:**
  1. `SurgicalPatcher` in `crates/constraint-engine` parses via `oxc_parser` (`SourceType::tsx()`) and **cannot parse HTML5 slides**; `crates/presentation-cms` needs an HTML5-native patching pipeline.
  2. `useSandboxedIframeBridge.ts` recomputes `resolvedSha256` on `rawHtml`, rotating `targetCacheKey = ${projectId}::${nodeId}::${resolvedSha256}` on every edit—tearing down the `IframeKeepAliveSlot` and destroying `<16ms` typing performance while causing `128` synchronous `getBoundingClientRect()` layout reads in `applyPatchByFfId`.
  3. Security vulnerabilities in iframe sandboxing (`html-sandbox-wrapper.ts` / `html-sandbox-bridge.ts`): regex comment stripping in CSS creates `</style>` mXSS vectors, `FORBIDDEN_TAGS` excluding `"svg"` without SVG-specific sanitization allows `<svg><style>...</style></svg>` & `<foreignObject>` XSS, and `STRICT_OFFLINE_CSP` `'unsafe-inline'` weakens `CSP-B`.
  4. `V1__initial_16_tables.sql` (`presentation_decks`, `deck_slides`, `wording_slots`) lacks `speaker_notes`, `aspect_ratio`, `charts_json`, `slot_kind`, `revision`, and protocol version metadata — requiring an additive `V12` migration.
  5. Sandboxed `srcDoc` iframes (`0 allow-same-origin`) cannot directly talk to a secondary Presenter window via `BroadcastChannel`; Presenter sync MUST be Host-mediated (`Iframe MessageChannel <-> Host BroadcastChannel <-> Presenter Window`).
  6. Risk tiers for `06.1..06.3` (and split `06.4`) involve CSP-B sanitization boundaries, cross-window auth/channels, and SQLite schema changes — mislabeled at `risk_tier: 2` instead of **`risk_tier: 3`**.

### 3. UX & Design System Lead (`ux-agent-persona`)
- **Baseline Position:** `CONDITIONAL_HOLD_REQUIRES_UX_ALIGNMENT`.
- **5 Concrete UX Pushbacks:**
  1. `DESIGN.md` conflict: `§8 LAY-05` / `CMP-10` specifies Left `220px` sorter + Center `60%` stage + Right `40%` CMS, whereas `§9.1` & `CompanionDualPaneStage.tsx` place `#structureSubPane` (`translateX(-50%)` Slide-Over CMS) on the Left and the 16:9 Stage on the Right.
  2. Off-screen track of `translateX(-50%)` CMS drawer lacks `inert` + `aria-hidden="true"`, allowing `Tab` keyboard focus leakage into hidden DOM nodes.
  3. Fullscreen Deck Player (`data-cowok-deck-protocol="1"`) risks keyboard traps (`Esc` swallowing or double-`Esc` browser fullscreen + React overlay mismatch) and CMS `ArrowLeft`/`ArrowRight` stealing focus inside text inputs.
  4. Workbench container width constraints leave `--slide-scale ~0.26` in default `1440px` splits (`CompanionDualPaneStage.tsx` hardcodes `minWidth: "480px"`), rendering slide text at `~3.6px` physical height.
  5. `tokens.css` `.live-html-frame !important` forces `#FFFFFF` background onto HTML previews (clobbering Dark decks), and Warm/Dim `--human-accent` vs `--agent-accent` collapse to identical blues (`#0057FF` / `#315FB6`).

---

## Round 2 — Cross-Examination, Challenge & Consensus Synthesis

### Socratic Challenge 1: Rust HTML5 Patcher Strategy (`PM` vs `Architect`)
- **Challenge (`PM`):** Can `crates/presentation-cms` avoid building a bespoke HTML5 parser by canonicalizing slide files to strict XHTML or using `tree-sitter-html` / `lol_html`?
- **Response & Resolution (`Architect`):**
  - **Reject** strict XHTML canonicalization (`§9.2` arbitrary HTML5 disk files + void elements would break or alter bytes/SHA-256 `StaleFile` OCC) and **reject** `tree-sitter-html` for security (CST error-recovery diverges from WHATWG HTML5 parsing, enabling parser-differential mXSS).
  - **Lock Hybrid Design:** Use **`lol_html`** for `<1ms` byte-preserving streaming `[data-ff-id]` text/attribute patching, paired with **`html5ever`-backed DOM sanitization + strict SVG element/attribute allowlist** on `APPLY_HTML` / disk-ingest, and a token-level CSS sanitizer banning `</style`, `@import`, `url()`, and `expression()` (replacing regex comment stripping).

### Socratic Challenge 2: `Story 06.3` Split (`06.3` + `06.4`) & `Speaker Notes` Boundary (`PM`, `UX`, `Architect`)
- **Challenge (`UX` on `PM`):** Approves splitting `Story 06.4` (**Presenter Screen Mode, Fullscreen Keyboard Control & Dual-Screen Speaker Notes**), but rejects deferring Speaker Notes editing out of `Story 06.2` (`Ghi chú diễn giả`).
- **Response & Resolution (`PM` & `Architect`):**
  - **Lock Split + Boundary:**
    - `Story 06.2` owns the Slide-Over CMS Drawer form including the `speaker_notes` field and additive `V12` column wiring so CMS scrolling/persistence/OCC are designed once.
    - `Story 06.3` scopes cleanly to **Contextual Slide Copilot Chat (`KeyedParallelCoordinator`) & Offline 2D Live Interactive Charts** (explicitly deferring `3D widget` from `FR-17` out of `Epic 06` to prevent Three.js/CSP-B/bundle bloat).
    - `Story 06.4` owns **Presenter Screen Mode, Host-Mediated `BroadcastChannel` Sync, Stopwatch Timer, Next-Slide Preview & Popup-Blocked Inline Fallback**.
    - All impacted stories (`06.1..06.4`) touching schema/CSP-B/cross-window bridges elevate from `risk_tier: 2` to **`risk_tier: 3`**, requiring `/reconcile-change` prior to `/make-story`.

### Socratic Challenge 3: `<16ms` `MessageChannel` Patching vs Visual Parity, Dark Decks & `--slide-scale` (`UX` vs `Architect`)
- **Challenge (`UX`):** Decoupling `<16ms` `PATCH_BY_FF_ID` from `srcDoc` reload (`targetCacheKey` SHA-256 rotation) fixes typing speed, but risks visual drift if Dark decks get clobbered by `.live-html-frame !important`, `aspect_ratio` stretches `1920x1080`, or `--slide-scale ~0.26` makes copy unreadable.
- **Response & Resolution (`Architect` & `UX`):**
  - **Lock Two-Tier Sync & Fidelity Contract:**
    1. **Fast Path (`<16ms`):** `CMS -> MessageChannel PATCH_BY_FF_ID` mutates live DOM text/chart slots in the existing `IframeKeepAliveSlot` **without** mutating `rawHtml`/`resolvedSha256` or triggering `srcDoc` teardown; IME composition (`compositionstart`..`compositionend`) guards bidirectional echoes; `publishElementRects()` throttles via `rAF`.
    2. **Slow Path (`~150–300ms` debounced + flush):** `CMS -> crates/presentation-cms (lol_html) -> SQLite WAL (V12)` with monotonic `revision` OCC.
    3. **Visual Fidelity (`§13.6`):** Host CSS never injects `!important` inside `<iframe sandbox="allow-scripts allow-forms">` (`srcDoc`); dark decks explicitly emit `html, body` background tokens; slide templates enforce projection type scale (`display >= 56px`, `heading >= 32px`, `body >= 22px`, `micro >= 16px` at `1920x1080`), fluid `min-width: 320px`, auto-collapse chat rail for `--slide-scale >= 0.45`, zoom pill (`Fit | 100% Pan`), and `ResizeObserver` `scrollHeight > clientHeight` overflow warnings (`E3 Gate`).
    4. **Unified Focus & Esc Exit:** `Present` uses single React overlay state synced to `fullscreenchange` (single-`Esc` exit), while `Esc` inside the secondary Presenter Window closes only the popout without blacking out the live audience screen. `iframe` ignores `HostRuntimeContract` arrows unless non-input target; CMS offscreen drawer uses `inert` + `aria-hidden="true"`.

### Socratic Challenge 4: Envelope Verdict (`INVESTIGATE` vs `AUTHOR_ONLY`)
- **Unanimous Consensus across `PM`, `Architect/Review`, `UX`, and `Unknowns Analyst`:**
  - **`story_creation_envelope.max_phase`: `"INVESTIGATE"`** (`epic_readiness: "CONDITIONAL"`, `evaluation_integrity: "VERIFIED"`).
  - **Hard Constraint:** Blocking `"IMPLEMENT"` (and postponing final `Story 06.1..06.4` `/make-story` re-authoring + `/reconcile-change` until the 5 Pre-Implementation Spikes pass) prevents encoding broken contracts (`SurgicalPatcher`, `PRESENTATIONS`, `LAY-05` conflict, CSP-B chart assumptions) into static ACs.

---

## Locked Architectural & Delivery Decisions Summary

| Decision ID | Topic | Locked Resolution |
| :--- | :--- | :--- |
| **DEC-06-01** | Envelope & Readiness Gate | `epic_readiness: "CONDITIONAL"`, `max_phase: "INVESTIGATE"`. 5 technical spikes required before `/reconcile-change` + `/make-story`. |
| **DEC-06-02** | `DESIGN.md` Topology SSOT | Ratify `§9.1` (`CompanionDualPaneStage.tsx`: Left `#structureSubPane` + `translateX(-50%)` CMS Drawer with `inert`; Right 16:9 Stage) and update `§8 LAY-05` / `CMP-10`. |
| **DEC-06-03** | Rust HTML5 Patcher & mXSS | Add `crates/presentation-cms` using `lol_html` (`<1ms` `[data-ff-id]` patch) + `html5ever` SVG allowlist + CSS token sanitizer; keep `SurgicalPatcher` TSX-only. |
| **DEC-06-04** | `<16ms` Non-Reloading Bridge | Decouple `targetCacheKey` from transient DOM edits; pair `<16ms` `MessageChannel` `PATCH_BY_FF_ID` (IME-guarded) with debounced SQLite `revision` OCC. |
| **DEC-06-05** | `V12` Additive DB Schema | Additive `ALTER TABLE` on `presentation_decks`, `deck_slides`, `wording_slots` (keeping `project_id` cascade/default compatibility; replace `PRESENTATIONS` typo in docs). |
| **DEC-06-06** | Scope & `Story 06.4` Split | Scope `06.3` to Contextual Slide Copilot + Offline 2D Charts (defer `3D widget`); split Presenter Screen Mode into `06.4`; keep `speaker_notes` in `06.2`; upgrade `risk_tier` to `3`. |
