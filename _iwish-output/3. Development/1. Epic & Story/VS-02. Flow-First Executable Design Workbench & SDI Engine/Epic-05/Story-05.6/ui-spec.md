---
type: I-Wish UI Specification
story_id: "05.6"
title: "UI Specification: Story 05.6 (Phase 2 Brand & Component Visual Studio, Dynamic Theme Modes, Bidirectional Hidden Spec Drawer & Heritage Pack Picker)"
status: pending_approval
links_to:
  - "../../../../2. Product Planning/2.3. ui-ux-spec.md"
  - "../../../../../DESIGN.md"
  - "./story.md"
  - "./data-spec.md"
---

# UI Specification — Story 05.6: Phase 2 Brand & Component Visual Studio

## 1. Visual Architecture & Userflow
Extends the existing Phase 1–4 Cognitive Progressive bar (`StageSkinToggleBar.tsx` + `WorkbenchCanvasTabArea.tsx`) with 4 focused, minimalist surfaces using 100% semantic CSS variables (`var(--bg-surface)`, `var(--bg-elevated)`, `var(--border-default)`, `var(--text-primary)`, `var(--action-primary)`, zero Tailwind `dark:` utilities, `<BrandIcon>` 1.85px monoline SVG sprite, full 9-locale i18n parity):

1. **Brand & Component Visual Studio Modal (`BrandVisualStudioModal.tsx`, trigger from Phase 2 `"2. Diện mạo"` / `"Chọn diện mạo"`):**
   - **Header (`MetricLine [5.10]`):** Title, user-driven `themeModes` segmented switcher (renders **only** the exact modes requested by user, e.g., `["warm"]` hides any Light/Dark toggle; `["warm", "dim", "night"]` shows 3 pills with WCAG AA `>= 4.5:1` badge), and `"Áp dụng Brand ID"` primary CTA.
   - **Left Column (`280px`):** 2–3 Brand Proposal cards (`BrandProposalV1`: brandName, rationale, inspirationRefs chips, OKLCH swatches, radius/density metadata) + **Heritage Pack Picker** toggle (`Kế thừa Heritage Pack`).
   - **Right Stage (`1fr`, Tabs: `Trang thật từ Sketch (<=3)` | `Thư viện Component dùng chung` | `Popular Audit`):**
     - *Tab 1 — Real Sketch Pages (`<= 3` sandboxed iframes cap):* Renders up to 3 representative Sketch Phase 1 pages (`Hero`, `List/Detail`, `Action Form`) tokenized live (`<5ms`, `$0`) with selected Brand ID & active user `themeMode`.
     - *Tab 2 — Extracted Shared Component Gallery:* Visual live preview of every extracted `data-component-kind` + user-selected Popular Components styled in the active Brand ID & `themeMode`.
     - *Tab 3 — Contextual Popular Component Audit (`DataTableView [5.21]` + `AdvancedFilterBar [5.22]`):* 3-group checklist (`Đã có` / `Gợi ý theo ngữ cảnh` / `Không cần cho dự án này`) synced 1:1 to `07_COMPONENT_REGISTRY`.
2. **Bidirectional Hidden Spec Drawer (`HiddenSpecDrawer.tsx` — `RightDrawer [5.25]`):**
   - Accessible from the compact top strip (`"Đặc tả ẩn 01..09"`) so non-tech users stay visual-first while power users/engineers can inspect & edit `01..09` specs + `fe-data-contract.draft.ts` with OCC `ast_revision` conflict detection (`409 STALE_MAPPING` banner + reload/merge CTA).
3. **FE-Required Data Contract Preview (`FeDataContractTab` inside `HiddenSpecDrawer`):**
   - Renders explicit boundary banner (`FE_REQUIRED_VIEWMODEL_AND_ACTIONS_ONLY`), ViewModel fields table (`DataTableView [5.21]`), Action Inputs, 4-State Matrix (`DEFAULT | EMPTY | LOADING | ERROR`), and 1-click Copy/Export `fe-data-contract.draft.ts`.
4. **Save Heritage Pack Action (Phase 4 `Approve v1` + `Toast [5.12]` receipt):**
   - Saves Brand ID + Component Registry + Presentation Theme + `themeModes` to `~/.cowok-design/heritage-packs/` and SQLite `V25` for reuse across both App FE and 16:9 Presentation Decks (`GREENFIELD`, `ADD_FEATURE`, `REVAMP`, `MICRO_POLISH`, `DECK_FROM_APP`).

## 2. Registered UX Patterns & Components Alignment (`DESIGN.md §5`)

| Story 05.6 UI Widget | Canonical Pattern / Component (`DESIGN.md §5`) | Reuse Strategy |
| :--- | :--- | :--- |
| Studio Top Telemetry & Dynamic `themeModes` Bar | **`MetricLine [5.10]`** | Compact horizontal bar (`tabular-nums`, WCAG AA ratio badge, render latency `<5ms`), zero multi-row bloat. |
| Bidirectional Hidden Spec & FE Data Contract Panel | **`RightDrawer [5.25]`** + **`PAT-05 Inspector Drawer`** | Resizable right slide-over drawer with `01..09 Specs` and `FE Data Contract` tabs + OCC conflict alert. |
| Popular Component Audit & ViewModel Fields | **`DataTableView [5.21]`** + **`AdvancedFilterBar [5.22]`** | Compact `36px` rows with status filter chips (`Đã có | Gợi ý | Không cần`) and keyboard navigation. |
| Brand Proposal & Heritage Pack Selector Cards | **`PAT-08 Connector / Pack Card`** | Consistent bordered surface (`--bg-surface`, `--border-default`), active accent border (`--action-primary`), rationale snippet. |
| Brand Apply / Heritage Save / OCC Conflict Notices | **`Toast [5.12]`** | Deterministic bottom-right receipt with inline `Undo` or `Tải lại bản mới` action. |
| Icons & Theme Polarity | **`<BrandIcon>`** + **`tokens.css`** | `1.85px` monoline SVG sprite, semantic CSS variables only (verified in Warm Light, Night Dark, and Mobile `390px`). |

## 3. Platform AI Consultation & Debate Report
- Request for consultation was sent to internal UX architecture alignment (no external Stitch/Figma mockup generation required for this incremental studio modal extending `VisualClarificationModal`). Fallback to internal UX evaluation against `DESIGN.md §5`:
  - **Accepted:** Cap simultaneous Sketch iframes to `<= 3` (`Hero`, `List/Detail`, `Action Form`) to prevent browser RAM/CPU spikes; hide `themeModes` switcher completely when `themeModes.length === 1` (`AC-06`).
  - **Accepted:** Place `FE Data Contract` as a dedicated tab inside `HiddenSpecDrawer [5.25]` rather than adding another top-level modal, keeping workbench chrome uncluttered.
  - **Rejected:** Auto-generating Light + Dark pairs when user only asks for `"warm"` — strictly forbidden by `AC-02` & `AC-06`.
