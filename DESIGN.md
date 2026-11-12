# cowok-design — Canonical Brand & Product Design System (`DESIGN.md` v4.3-OS)

> **Single Source of Truth (SSOT) for UI/UX, Brand Assets, Tokens, Component Registry & Workbench Layouts**
> - **Product Name**: `cowok-design` (AI-Native Flow-First Product Design, Live Runtime Preview, HTML Presentation Studio & Multi-Agent Execution OS)
> - **Parent Brand System**: `cowok.ai` Brand & Product Design System **v4.3** (`Brand-guideline/cowok-brand-product-system-v4.3/`)
> - **Architecture Mandate**: **100% Pure Vanilla CSS Custom Properties (`tokens.css` + `components.css`) — ZERO Tailwind Utility Classes in Workbench & Flagship Preset**
> - **Core Visual Mantra**: *"Warm in light. Calm in dark. Quiet by default. Color only when meaningful."*

---

## 1. Brand Essence, Positioning & Non-Negotiable Principles

### 1.1 Brand Essence
`cowok-design` is a **Human + Agent Co-Working Product Design & Execution Operating System** where humans direct product intent, flow architecture, and high-stakes approvals while specialized AI agents (`cowok`, `advisor`, `REA Forge`, `Presentation Studio`) synthesize, verify, and execute within explicit deterministic guardrails (`E1..E4` gates and `R0..R3` risk tiers).

### 1.2 Seven Non-Negotiable Product Design Principles
1. **Shared Clarity Over AI Theater**: Every screen must make 6 questions effortless to answer within 2 seconds: *Who owns this? What state is it in? Is it a draft, running, blocked, or approved? What changed? What requires human judgment? What happens next?*
2. **Human Authority stays Explicit**: `Human Blue` (`--human-accent: #4B7FFF`) represents **Human ownership, decision, and approval gates**. `Agent Orange` (`--agent-accent: #E89A35`) represents **Agent ownership, autonomous execution, and review-required proposals**. Never confuse the two, and never use `Human Blue` or `Agent Orange` as generic decorative fills.
3. **Calm Surfaces for 8–12 Hour Sessions**: Never use stark `#FFFFFF` as the primary light workspace canvas. Default to **Warm Light (`#F7F5F0`)**, **Focus Dim (`#ECEAE4`)**, or **Night Workspace (`#0B1220`)** with hairline `1px` borders (`--border-subtle`) before drop shadows.
4. **Zero-Tailwind Workbench Isolation**: The `cowok-design` Workbench (`src/pages/workbench/**`, `src/features/**`, `src/components/ui/**`) and Flagship Preset (`cowok-brand-product-system-v4.3`) MUST use semantic CSS classes (`.ff-*`, `.cowok-*`) backed by `Brand-guideline/cowok-brand-product-system-v4.3/foundations/css/tokens.css` and `components/css/components.css`. Raw Tailwind utility classes (`bg-amber-500`, `text-slate-900`, etc.) are strictly forbidden in the Workbench shell.
5. **Draft $\neq$ Approved $\neq$ Executed**: AI-generated flows, UI screen patches, design tokens, and slide decks must never look committed while still in `Draft` or `Pending Approval` state.
6. **Inspect $\neq$ Interact Separation**: The Center Canvas strictly separates **Inspect Mode** (selecting `data-ff-id` nodes, viewing token bindings, anchoring comments, dragging XYFlow edges) from **Interact Mode** (executing live state transitions, typing in forms, testing hover/focus/modal states inside the sandboxed runtime).
7. **Multi-Brand Preset Isolation**: While the `cowok-design` Workbench itself is locked to `cowok-brand-product-system-v4.3`, user projects designed inside `cowok-design` can switch between 4 isolated Brand Presets without leaking styles into the host Workbench.

---

## 2. Logo, Wordmark & Canonical Brand Asset Registry

### 2.1 Approved Brand Mark & Wordmark
- **Approved Symbol**: **Option 3 Refined — Human + Agent**
  - Left Node (`Human Blue #4B7FFF`): Human intent, governance, and direction.
  - Right Node (`Agent Orange #E89A35`): Agent synthesis, execution, and automation.
  - Asymmetric Dynamic Bridge (`#4B7FFF` $\rightarrow$ `#E89A35`): Active handoff and co-working loop.
  - Shared Center Node: Synchronized state, approval, and shared workspace.
- **Approved Wordmark**: **Customized Geist Option C** (`cowok` in weight `650`, tracking `-0.032em`, paired with `.ai` or `-design` badge in `Geist Mono`).
- **Minimum Size & Clear Space**:
  - Clear space around symbol/lockup $\ge 0.5\times$ symbol height (`0.5H`).
  - Minimum symbol size: `16px` (`favicon` / compact status indicator), `24px` (`Workbench TopBar`), `32px` (`Splash / Deck Cover`).
  - Never recolor both nodes to a single accent color unless rendering in strict `Monochrome Ink (#0F172A)` or `Monochrome White (#F8FAFC)`.

### 2.2 Physical Brand Asset Paths (Zero-Placeholder Mandate)
All UI surfaces must load official brand assets from `Brand-guideline/cowok-brand-product-system-v4.3/`:

| Asset Category | Canonical File Path | Usage in `cowok-design` |
| :--- | :--- | :--- |
| **Horizontal Lockup — Warm Light** | `Brand-guideline/cowok-brand-product-system-v4.3/logo/assets/01-horizontal/cowok-logo-horizontal-light.png` (and `cowok-logo-horizontal-light-no-tagline.png`) | TopNav brand header (`warm`/`dim`), Light mode export watermark, Presentation cover |
| **Horizontal Lockup — Night Mode** | `Brand-guideline/cowok-brand-product-system-v4.3/logo/assets/01-horizontal/cowok-logo-horizontal-dark.png` (and `cowok-logo-horizontal-dark-no-tagline.png`) | TopNav brand header (`night`), Dark presentation cover |
| **Stacked Lockup (Light / Dark)** | `Brand-guideline/cowok-brand-product-system-v4.3/logo/assets/02-stacked/cowok-logo-stacked-light.png` / `cowok-logo-stacked-dark.png` | Empty-state hero, Onboarding modal, About dialog |
| **Symbol Only — Primary Color** | `Brand-guideline/cowok-brand-product-system-v4.3/logo/assets/03-symbol/cowok-symbol-color-transparent-4096.png` (and `cowok-symbol-color-2048.png`) | `cowok` Orchestrator avatar, Compact rail identity, Favicon source |
| **Wordmark Only (Navy / White)** | `Brand-guideline/cowok-brand-product-system-v4.3/logo/assets/04-wordmark/cowok-wordmark-navy-4096.png` / `cowok-wordmark-white-4096.png` | Co-branded header lockup with `-design` mono pill |
| **App Icon — Warm Paper / Dark Slate** | `Brand-guideline/cowok-brand-product-system-v4.3/logo/assets/05-app-icons/cowok-app-icon-light-1024.png` / `cowok-app-icon-dark-1024.png` | Desktop/PWA launcher icon, Project switcher avatar |
| **Symbol — Monochrome Black / White** | `Brand-guideline/cowok-brand-product-system-v4.3/logo/assets/06-mono/cowok-symbol-mono-black-transparent.png` / `cowok-symbol-mono-white-transparent.png` | Print / PDF export / low-chroma focus mode |
| **CSS Design Tokens (SSOT)** | `Brand-guideline/cowok-brand-product-system-v4.3/foundations/css/tokens.css` | Global CSS Custom Properties imported in `src/main.tsx` |
| **Component Stylesheet (SSOT)** | `Brand-guideline/cowok-brand-product-system-v4.3/components/css/components.css` | Base `.cowok-*` and `.ff-*` component classes |
| **JSON Token Registry** | `Brand-guideline/cowok-brand-product-system-v4.3/foundations/tokens/tokens.json` | AST Constraint Engine & Token Inspector validation |
| **Canonical React Icon Library (143/145 Icons)** | `Brand-guideline/cowok-brand-product-system-v4.3/iconography/react/icons.tsx` | All Workbench icons (`24x24` viewBox, `1.8px` stroke, `currentColor`) |
| **Icon Manifest** | `Brand-guideline/cowok-brand-product-system-v4.3/iconography/icon-manifest.json` | Searchable icon metadata across 11 categories |

### 2.3 Canonical Iconography Rules (`143/145` SVG System)
- **Geometry**: `24x24` viewBox, `2px` live area padding (`20x20` live box), `1.8px` stroke width (`strokeLinecap="round"`, `strokeLinejoin="round"`, `fill="none"`, `stroke="currentColor"`).
- **Protected Exact-Source Brand Icons (NEVER redraw or substitute)**:
  1. `approval` (`ApprovalIcon` — Human + Agent approval seal)
  2. `connector` (`ConnectorIcon` — Co-work bridge connector)
  3. `shared-cowok` (`SharedCowokIcon` — Shared Human+Agent state node)
- **Forbidden**: Never hardcode ad-hoc `<svg>` paths in feature components when an icon exists in `Brand-guideline/cowok-brand-product-system-v4.3/iconography/react/icons.tsx`. `<BrandIcon />` (`src/components/ui/BrandIcon.tsx`) MUST wrap and re-export the official icons from `iconography/react/icons.tsx`.

---

## 3. Foundations: Theme Modes, Color Tokens, Typography, Spacing & Motion

### 3.1 Four Long-Session Theme Modes (`3 + 1 Focus Mode`)
Theme switching is controlled via `document.documentElement.dataset.theme` (`"warm" | "dim" | "night"`) and optional `document.documentElement.dataset.focus` (`"low"`):

| Theme ID | Selector | Primary Canvas (`--bg-canvas`) | Elevated Surface (`--bg-surface`) | Elevated Modal (`--bg-elevated`) | Subtle Fill (`--bg-subtle`) | Primary Text (`--text-primary`) | Secondary Text (`--text-secondary`) | Muted Text (`--text-muted`) | Subtle Border (`--border-subtle`) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **1. Warm Light (Default)** | `:root`, `[data-theme="warm"]` | `#F7F5F0` | `#FFFFFF` | `#FCFBF8` | `#EFECE6` | `#12161F` | `#4B5565` | `#7B8494` | `#E5E0D8` |
| **2. Focus Light (Dim)** | `[data-theme="dim"]` | `#ECEAE4` | `#F6F4EF` | `#FAF8F4` | `#E2DFD8` | `#141923` | `#465062` | `#707989` | `#D7D2C8` |
| **3. Night Workspace** | `[data-theme="night"]` | `#0B1220` | `#111A2E` | `#16213A` | `#1B2946` | `#F1F5F9` | `#9FB0C7` | `#6E819C` | `#1E2C49` |
| **4. Low-Chroma Focus** | `[data-theme="night"][data-focus="low"]` | `#0B1220` | `#111A2E` | `#16213A` | `#1B2946` | `#F1F5F9` | `#9FB0C7` | `#6E819C` | `#1E2C49` (Softened accents: `--human-accent: #5C83E6`, `--agent-accent: #D29445`) |

### 3.2 Semantic Ownership, Action & Status Tokens
| Token Name | Warm / Dim Value | Night Value | Strict Semantic Rule |
| :--- | :--- | :--- | :--- |
| `--action-primary` | `#081E3A` (Text: `#F8FAFC`) | `#F1F5F9` (Text: `#0B1220`) | **Default Primary CTA** (Save, Create, Publish, Export). High-contrast neutral ink/alabaster. |
| `--action-primary-hover` | `#0D2B52` | `#E2E8F0` | Hover state for primary CTA buttons. |
| `--human-accent` | `#4B7FFF` (`--human-soft: #EAF1FF`, `--human-border: #B8CEFF`) | `#6A96FF` (`--human-soft: rgba(75,127,255,0.16)`, `--human-border: #3358B8`) | **Human Ownership / Decision / Approval Gate ONLY**. Used for human-assigned nodes, `@human` review gates, active selection focus ring (`--focus-ring`), and Approve CTA when explicitly resolving an agent gate. |
| `--agent-accent` | `#E89A35` (`--agent-soft: #FFF4E5`, `--agent-border: #F5C98B`) | `#F2AC4E` (`--agent-soft: rgba(232,154,53,0.16)`, `--agent-border: #8B5B1E`) | **Agent Ownership / Autonomous Run / Proposal Review ONLY**. Used as `3px` left border, agent badge, `ChangeProposal` indicator, or live agent execution pulse. **NEVER** use as a full-card background fill or generic warning! |
| `--state-success` | `#169B62` (`--state-success-soft: #E9F9F1`) | `#2AD189` (`rgba(42,209,137,0.14)`) | Verified `E1..E4` gate pass, approved proposal, synced live spec. |
| `--state-warning` | `#D9822B` (`--state-warning-soft: #FFF5E8`) | `#F3A447` (`rgba(243,164,71,0.15)`) | Drift warning, stale snapshot, high latency (`>16ms` / `>50ms`). |
| `--state-danger` | `#D9485F` (`--state-danger-soft: #FDECEF`) | `#F06A7E` (`rgba(240,106,126,0.16)`) | Blocked gate (`E1..E4` fail), R3 destructive action, broken edge, runtime crash. |
| `--state-info` | `#2E7CF6` (`--state-info-soft: #EBF3FF`) | `#5CA0FF` (`rgba(92,160,255,0.15)`) | Informational system hint or read-only inspection metadata. |

### 3.3 Typography System (`Geist Sans` + `Geist Mono`)
- **UI & Editorial Display (`--font-sans`)**: `'Geist', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif`
- **Technical & Telemetry (`--font-mono`)**: `'Geist Mono', 'SFMono-Regular', Consolas, monospace` with mandatory `font-variant-numeric: tabular-nums` on all metrics, timers, token values, latencies (`<16ms`, `<50ms`), node IDs (`SCR-01`, `CMP-04`), and diff line numbers.

| Scale Token | Size / Line-Height | Weight | Letter Spacing | Canonical Application |
| :--- | :--- | :--- | :--- | :--- |
| `display-xl` | `40px / 46px` (`2.5rem`) | `650` | `-0.03em` | HTML Presentation Studio Cover Title (`16:9` Slide Deck) |
| `display-lg` | `32px / 38px` (`2.0rem`) | `650` | `-0.025em` | Section Slide Header / Project Brief Hero Title |
| `heading-lg` | `24px / 30px` (`1.5rem`) | `600` | `-0.02em` | Modal / Drawer / Tournament Arena Header |
| `heading-md` | `18px / 24px` (`1.125rem`) | `600` | `-0.015em` | Zone Header, Active Screen Title, Proposal Summary |
| `heading-sm` | `15px / 20px` (`0.9375rem`) | `600` | `-0.01em` | Card Title, Inspector Section Header, Node Header |
| `body-md` | `14px / 20px` (`0.875rem`) | `400–500` | `0` | Primary Workbench UI copy, Chat messages, Spec editor |
| `body-sm` | `13px / 18px` (`0.8125rem`) | `400–500` | `0` | Dense Inspector properties, Tree items, Table cells |
| `caption` | `12px / 16px` (`0.75rem`) | `500` | `+0.01em` | Badges, Keyboard shortcuts (`⌘K`), Edge labels |
| `micro-mono` | `11px / 14px` (`0.6875rem`) | `500` | `+0.02em` | `data-ff-id` tags, Risk Tier pills (`R0..R3`), Gate chips (`E1..E4`), Latency telemetry |

### 3.4 Spacing, Radii, Borders & Motion Tokens
- **4px Spatial Grid**: `--space-1: 4px`, `--space-2: 8px`, `--space-3: 12px`, `--space-4: 16px`, `--space-5: 20px`, `--space-6: 24px`, `--space-8: 32px`, `--space-10: 40px`, `--space-12: 48px`.
- **Corner Radii**: `--radius-xs: 6px` (badges/chips/code pills), `--radius-sm: 8px` (buttons/inputs/tree rows), `--radius-md: 12px` (cards/XYFlow nodes/inspector blocks), `--radius-lg: 16px` (modals/drawers/command palette), `--radius-xl: 20px` (presentation stage frames), `--radius-pill: 999px` (ownership & status pills).
- **Border-First Hierarchy**: All zone dividers and cards use `1px solid var(--border-subtle)`. Active human selection uses `1.5px solid var(--human-accent)` with `0 0 0 3px var(--focus-ring)`. Agent-authored draft proposals use a `3px solid var(--agent-accent)` left accent bar + `1px dashed var(--agent-border)` outline until approved.
- **Quiet Motion**: `--duration-fast: 120ms`, `--duration-normal: 180ms`, `--duration-slow: 280ms` with `--ease-standard: cubic-bezier(0.2, 0.8, 0.2, 1)`. Strictly honor `@media (prefers-reduced-motion: reduce)`.

---

## 4. Multi-Brand Preset Engine Architecture

`cowok-design` separates **Host Workbench Styling** from **Target Project Preset Styling**:

| Preset ID | Preset Name | Target Use Case | Surface & Token Profile |
| :--- | :--- | :--- | :--- |
| **`preset-cowok-v4.3` (Default Flagship)** | `cowok.ai v4.3 Warm Editorial` | Default for `cowok-design` Workbench & AI-native SaaS products | `#F7F5F0` Warm Paper / `#0B1220` Night, `#081E3A` Ink CTA, `#4B7FFF` Human Blue, `#E89A35` Agent Orange, Geist Sans/Mono |
| **`preset-enterprise-neutral`** | `Enterprise Slate Neutral` | B2B ERP, Fintech, Dense Operations Portals | `#F8FAFC` Cool Alabaster / `#090D16` Dark Slate, `#0F172A` Primary CTA, `6px` compact radii, high-density `32px` table rows |
| **`preset-editorial-craft`** | `High-Craft Editorial Studio` | Consumer apps, Creative tools, Pitch Decks | `#FAF7F2` Cream / `#121316` Obsidian, high-contrast display headers, generous `24px` card padding |
| **`preset-custom-project`** | `Project-Scoped Custom Brand` | Imported via `05_DESIGN_SYSTEM.md` or `REA Forge` extraction | Dynamically compiled from `.cowok-design/specs/05_DESIGN_SYSTEM.md` into isolated iframe CSS variables |

---

## 5. UI Component Layouts (Mandatory UX Pattern & Component Registry)

All stories and `ui-spec.md` specifications in `cowok-design` MUST leverage the registered patterns below rather than inventing ad-hoc layouts, tables, drawers, or cards.

### 5.1 Core Platform Layout Patterns (`[5.10]..[5.25]`)

| Pattern ID | Canonical Name | DOM / CSS Hook | Required Structure & Behavior in `cowok-design` |
| :--- | :--- | :--- | :--- |
| **`[5.10]`** | **Metric Line (`MetricLine`)** | `.ff-metric-line` / `.cowok-metric-strip` | Compact horizontal telemetry bar (`height: 36px` or `48px` card row) displaying 3–6 key-value indicators in `Geist Mono` (`tabular-nums`): e.g., **SDI Score (`0.18`)**, **Patch Latency (`4.2ms / <16ms`)**, **Gate Status (`E1..E4 PASS`)**, **Active Agents (`2 Running`)**, and **Pending Approvals (`1 R2 Gate`)**. Never use oversized multi-row KPI cards inside the Workbench header. |
| **`[5.12]`** | **Toast & Action Receipt (`Toast`)** | `.ff-toast` / `.cowok-toast` | Non-blocking bottom-right notification stack (`z-index: 90`, `max-width: 380px`, `--bg-elevated`, `1px solid var(--border-default)`). Displays deterministic receipts (e.g., `Patch applied to SCR-02 in 3.8ms`, `Snapshot SNAP-04 sealed`, `Proposal CP-19 awaiting @human approval`) with an inline `Undo` or `Inspect Diff` button. |
| **`[5.21]`** | **DataTableView (`DataTableView`)** | `.ff-data-table` / `.cowok-table` | High-density structured table (`36px` compact / `44px` standard row height) for **9-Spec Registry Matrices**, **Token Bindings**, **FMEA Risk Ledgers**, **Run History**, and **Slide Outline Tables**. Enforces sticky headers, `Geist Mono` IDs, inline ownership badges (`Human` vs `Agent`), and keyboard row navigation (`↑/↓`, `Enter`). |
| **`[5.22]`** | **AdvancedFilterBar (`AdvancedFilterBar`)** | `.ff-filter-bar` / `.cowok-filter-bar` | Unified filter & search strip (`height: 40px`) sitting above `DataTableView`, `Flow Canvas`, `Approval Inbox`, or `Skills Hub`. Combines instant `⌘F` text filter, **Owner Chip Filter (`All | @human | @agent | Shared`)**, **Lifecycle Filter (`Draft | Needs Approval | Approved | Executed | Blocked`)**, **Risk Tier Filter (`R0..R3`)**, and **View Mode Toggle**. |
| **`[5.25]`** | **RightDrawer (`RightDrawer`)** | `.ff-right-drawer` / `.cowok-inspector-drawer` | Contextual dockable / slide-over right panel (`width: 360px` default, resizable `320px–520px`, `border-left: 1px solid var(--border-subtle)`) with 4 tabbed lenses: **(1) `cowok` Orchestrator & Clarification Cards**, **(2) Element & Token Inspector (`data-ff-id`)**, **(3) ChangeProposal Diff & Approval Gate (`E1..E4`)**, and **(4) Spec Drift & Trace Log**. |

### 5.2 `cowok.ai` v4.3 Application Patterns (`PAT-01..PAT-08`)

| Pattern ID | Pattern Name | Source Reference (`PRODUCT_PATTERNS.md`) | Role in `cowok-design` |
| :--- | :--- | :--- | :--- |
| **`PAT-01`** | **Three-Zone Workspace Shell** | `Workspace Shell` | `Left Rail/Navigator (260px)` + `Center Canvas/Stage (1fr)` + `Right Inspector/Orchestrator (360px)` + `Top Control Bar (48px)` + `Bottom Status/PTY Bar (28px collapsed / 220px expanded)`. |
| **`PAT-02`** | **Human + Agent Task & Flow Board** | `Task Board` | Visualizes screens, userflows, or fleet tasks grouped by state (`Draft`, `In Review`, `Approved`, `Executed`) with explicit Human/Agent owner pills. |
| **`PAT-03`** | **Segmented Run Timeline** | `Run Timeline` | Deterministic step-by-step execution trace (`Request → Interpret → Draft → Review → Execute → Report`) showing duration (`ms`), tool calls, and gate verdicts. |
| **`PAT-04`** | **Approval Center (`R0..R3` Gate)** | `Approval Center` | Dedicated review queue for high-impact AI changes (`R2` schema/flow mutations, `R3` destructive actions) with impact summary, before/after diff, and `Approve` / `Request Changes` / `Reject` actions. |
| **`PAT-05`** | **Inspector / Trace Drawer** | `Inspector / Trace Drawer` | Deep inspection of selected `ScreenStateNode`, DOM element (`data-ff-id`), CSS token bindings, and `E1..E4` validation receipts. |
| **`PAT-06`** | **Command Palette (`⌘K`)** | `Command Palette` | Keyboard-first modal launcher (`max-width: 640px`) separating **Immediate Navigation/Commands** from **Agent Generation/Mutation Actions** (with visible Risk Tier badge `R0..R3`). |
| **`PAT-07`** | **Artifact & Diff Review Surface** | `Artifact Review Surface` | Side-by-side or unified semantic diff for `.cowok-design/specs/*.md`, UI DOM patches, and 16:9 HTML slides with hunk-level accept/reject. |
| **`PAT-08`** | **Connector & Skill Setup Card** | `Connector Setup` | Standardized card for `agentskills.io` plugins, MCP servers, and `REA Forge` extractors showing permissions, scope, health status, and sandbox boundary. |

### 5.3 Canonical `cowok-design` Workbench Components (`CMP-01..CMP-16`)

| Component ID | Component Name | Target File Path | Mandatory Visual & Behavioral Contract |
| :--- | :--- | :--- | :--- |
| **`CMP-01`** | `WorkbenchShell` | `src/pages/workbench/WorkbenchShell.tsx` | Master 3-Zone CSS Grid layout (`TopNav 48px` + `Left Spec/Flow Navigator 260px` + `Center Multi-Mode Stage 1fr` + `Right Contextual Drawer 360px` + `Bottom Telemetry & PTY Strip`). Zero Tailwind classes; uses `.ff-workbench-shell` and CSS variables. |
| **`CMP-02`** | `WorkbenchTopNav` | `src/pages/workbench/components/WorkbenchTopNav.tsx` | Displays official `cowok-design` logo lockup, Project Breadcrumb, **4-Mode Segmented Switcher (`Flow & Preview | Presentation 16:9 | Fleet & PTY | Skills & REA`)**, `MetricLine [5.10]` (SDI score + Gate pill), Theme Switcher (`Warm | Dim | Night | Low-Focus`), Brand Preset selector, and `⌘K` trigger. |
| **`CMP-03`** | `SpecHierarchyTree` | `src/features/spec-tree/components/SpecHierarchyTree.tsx` | Left Zone navigator rendering the 9 canonical specs (`01_PROJECT_BRIEF` .. `09_EVAL_SPEC`), Userflow graphs (`FLW-*`), and Screen Registry (`SCR-*`) with live sync status dots and drift badges. |
| **`CMP-04`** | `ScreenStateNode` | `src/features/flow/components/ScreenStateNode.tsx` | Custom XYFlow node representing a screen and its state variants (`default`, `loading`, `empty`, `error`, `offline`, `permission-denied`). Features header with `SCR-*` mono ID, ownership badge (`@human` `#4B7FFF` vs `@agent` `#E89A35`), mini wireframe/live thumbnail, state pill selector, and typed edge handles. |
| **`CMP-05`** | `FlowValidationToolbar` | `src/features/flow/components/FlowValidationToolbar.tsx` | Floating canvas HUD (`top-left` / `bottom-center`) providing **Canvas Sub-Mode Toggle (`Flow Graph | Split Stage | Live Preview`)**, **Pointer Mode (`Inspect data-ff-id` vs `Interact Runtime`)**, Viewport Breakpoint (`Desktop 1440 | Tablet 768 | Mobile 390`), Zoom controls, and `E1..E4` Graph Completeness Check. |
| **`CMP-06`** | `ClarificationCardWidget` | `src/features/orchestrator/components/ClarificationCardWidget.tsx` | Structured Socratic Question Card inside the `cowok` Orchestrator stream when user intent is ambiguous. Renders single/multi-choice visual option cards, trade-off badges, recommended default, and custom rationale input (replaces wall-of-text AI chat). |
| **`CMP-07`** | `DesignTournamentArena` | `src/features/tournament/components/DesignTournamentArena.tsx` | Side-by-side (`2-up` or `3-up`) comparative arena for evaluating competing AI-generated layout variants (`Variant A`, `Variant B`, `Variant C`) with synchronized viewport scrolling, rubric scorecards (Brand Fit, Accessibility, Density), and **"Promote Winner"** or **"Synthesize Hybrid"** actions. |
| **`CMP-08`** | `ChangeProposalDiffModal` | `src/features/proposals/components/ChangeProposalDiffModal.tsx` | Implements `PAT-04` (`Approval Center`) + `PAT-07` (`Artifact Review Surface`). Shows `Draft` vs `Approved` status, Risk Tier (`R0..R3`), `E1..E4` gate results, visual before/after preview, AST/Spec diff hunks, and explicit `@human` approval signature button (`--human-accent`). |
| **`CMP-09`** | `CommentPinOverlay` | `src/features/comments/components/CommentPinOverlay.tsx` | Spatial pin overlay anchored deterministically to `[data-ff-id]` DOM nodes on both UI screens and 16:9 HTML slides. Supports `@cowok` / `@advisor` / `@human` thread resolution and converts comments directly into scoped `ChangeProposal` tasks. |
| **`CMP-10`** | `PresentationDualPaneView` | `src/features/presentation/components/PresentationDualPaneView.tsx` | **Mode 2 Studio**: Synchronized split-screen combining a **16:9 HTML Slide Stage** (left/center) with a **Structured Markdown/CMS Outline & Non-Tech Property Editor** (right). Edits to text, metrics, or charts patch `[data-ff-id]` DOM nodes in `<16ms` without full iframe reloads. |
| **`CMP-11`** | `OPCFleetControlConsole` | `src/features/fleet/components/OPCFleetControlConsole.tsx` | **Mode 3 Hub**: Multi-agent orchestration console monitoring `cowok` (Worker) and `advisor` (Reviewer) agents, active worktree leases, `.cowok-design/coordination/` mailbox queues, token/cost budgets, and `Run Timeline [PAT-03]`. |
| **`CMP-12`** | `EmbeddedTerminalPanel` | `src/features/terminal/components/EmbeddedTerminalPanel.tsx` | Dockable bottom/split PTY terminal (`xterm.js` style surface in Night theme `#0B1220`) streaming live CLI agent runs (`claude`, `codex`, `gemini`) and `<50ms` `$UIUX_LIVE_SPEC` IPC file-watch events. |
| **`CMP-13`** | `SkillLifecycleManagerView` | `src/features/skills/components/SkillLifecycleManagerView.tsx` | **Mode 4 Hub**: Browse, inspect, test, and toggle `agentskills.io` design/UX skills, MCP connectors, and `Design-Only REA Forge` (Reverse-Engineering Reference URL/Image into isolated Design DNA tokens without polluting project specs). |
| **`CMP-14`** | `SpecDriftIndicator` | `src/features/drift/components/SpecDriftIndicator.tsx` | Real-time Spec Drift Index (`SDI`) gauge & breakdown drawer (`0.00` = In Sync, `>0.25` = Warning, `>0.50` = Drift Block). Clicking opens the bi-directional reconciliation wizard (`Sync Code → Spec` or `Patch Code ← Spec`). |
| **`CMP-15`** | `TokenInspectorPanel` | `src/features/inspector/components/TokenInspectorPanel.tsx` | Live token & style inspector inside `RightDrawer [5.25]`. Displays the selected `[data-ff-id]` element's computed CSS variables (`--bg-*`, `--text-*`, `--space-*`), checks zero-hardcoded-hex compliance (`E1 Gate`), and performs `<16ms` surgical AST patches. |
| **`CMP-16`** | `SnapshotTimelineDrawer` | `src/features/snapshots/components/SnapshotTimelineDrawer.tsx` | Immutable Git-backed visual time-travel timeline (`SNAP-*`) allowing instant preview, visual diffing, and 1-click rollback of any screen, flow, or slide deck. |

---

## 6. AI UX Lifecycle, Risk Tiers (`R0..R3`) & Governance Gates (`E1..E4`)

### 6.1 Six-Stage Co-Working Lifecycle
Every AI action in `cowok-design` transitions visibly through 6 deterministic stages:
1. **`1. Request`** (`Human Blue` border): User prompts via `⌘K`, Right Drawer, Comment Pin `@cowok`, or Clarification Card.
2. **`2. Interpret`** (`Agent Orange` subtle pulse): Agent parses affected `SCR-*` nodes, `01..09` specs, and calculates Risk Tier (`R0..R3`). If ambiguity score is high, emits `ClarificationCardWidget [CMP-06]`.
3. **`3. Draft`** (`Agent Orange` `3px` left bar + dashed border, badge: `DRAFT PROPOSAL`): Agent generates preview branch / AST patch / layout tournament variants without mutating the locked SSOT.
4. **`4. Review`** (`E1..E4` Automated Gates + `@human` Approval Gate):
   - **`E1 (Token & Brand Gate)`**: Zero hardcoded hex/Tailwind violations; 100% `tokens.css` compliance.
   - **`E2 (Flow Completeness Gate)`**: Every `SCR-*` node covers required states (`default`, `loading`, `empty`, `error`) and valid transitions.
   - **`E3 (Accessibility & Contrast Gate)`**: WCAG AA contrast across `Warm`, `Dim`, and `Night` themes; keyboard focus order verified.
   - **`E4 (Spec-to-DOM Sync Gate)`**: `SDI <= 0.15`; all interactive nodes carry deterministic `data-ff-id` attributes.
5. **`5. Execute`** (`State Success` green pulse): Surgical AST patch (`<16ms`) or spec commit applied; `$UIUX_LIVE_SPEC` broadcasts in `<50ms`.
6. **`6. Report`** (`Toast [5.12]` + `SnapshotTimelineDrawer [CMP-16]` receipt): Immutable audit trail recorded with rollback link.

### 6.2 Authority & Risk Matrix (`R0..R3`)
| Risk Tier | Scope of Mutation | Agent Autonomy | Required UI Gate |
| :--- | :--- | :--- | :--- |
| **`R0 — Read / Inspect`** | Querying specs, inspecting `data-ff-id`, running `E1..E4` audits, calculating SDI | **Auto-Execute** | Silent telemetry in `MetricLine [5.10]` |
| **`R1 — Low-Risk Draft / Local Micro-Patch`** | Generating draft preview, tweaking token preview in sandbox, copywriting suggestion | **Optimistic Draft Preview** | Shown with `DRAFT` badge + `Undo` in `Toast [5.12]` |
| **`R2 — Structural Spec / Flow / Multi-Screen Mutation`** | Adding/deleting `SCR-*` nodes, modifying `01..09` specs, promoting Tournament winner, applying `REA Forge` tokens | **Blocked until `@human` Approval** | `ChangeProposalDiffModal [CMP-08]` with `Human Blue` Approve CTA |
| **`R3 — Destructive / External Publish / Fleet Override`** | Deleting specs, force-resetting snapshots, publishing external deck, killing fleet worktrees | **Strict Human Confirmation + Reason** | High-contrast Danger Confirmation Modal (`--state-danger`) |

---

## 7. Workbench Spatial Architecture (3 Coordinated Zones & 4 Integrated Modes)

```
+--------------------------------------------------------------------------------------------------+
| CMP-02 WorkbenchTopNav (48px): [Logo] [Project/Branch] | [Mode 1: Flow & UI] [Mode 2: Deck 16:9] |
| [Mode 3: Fleet & PTY] [Mode 4: Skills & REA] | [MetricLine 5.10: SDI 0.04 | E1-E4 PASS] [Theme]  |
+----------------------+----------------------------------------------------+----------------------+
| LEFT ZONE (260px)    | CENTER STAGE (1fr — Multi-Mode Workspace)          | RIGHT ZONE (360px)   |
| CMP-03 Spec & Flow   | CMP-05 FlowValidationToolbar:                      | RightDrawer [5.25]:  |
| Navigator            | [Graph | Split | Preview] [Inspect | Interact]     | Tabs:                |
|                      |----------------------------------------------------| 1. cowok Orchestrator|
| - 01_PROJECT_BRIEF   |                                                    |    & CMP-06 Cards    |
| - 02_PRD             |   Active Mode Surface:                             | 2. CMP-15 Token &    |
| - 03_USER_FLOWS      |   - Mode 1: XYFlow DAG (CMP-04) <-> Live Iframe    |    DOM Inspector     |
| - 04_SCREEN_SPECS    |   - Mode 2: 16:9 HTML Deck <-> CMS Outline (CMP-10)| 3. CMP-08 Proposal   |
| - 05_DESIGN_SYSTEM   |   - Mode 3: OPC Fleet Console (CMP-11)             |    Diff & E1-E4 Gate |
| - 06_COMPONENT_REG   |   - Mode 4: Skills & REA Forge (CMP-13)            | 4. CMP-14 SDI Drift  |
| - 07_COPY_DECK       |                                                    |    & CMP-16 History  |
| - 08_TECH_HANDOFF    |                                                    |                      |
| - 09_EVAL_SPEC       |                                                    |                      |
+----------------------+----------------------------------------------------+----------------------+
| CMP-12 EmbeddedTerminalPanel & Live Telemetry Strip (28px collapsed / 220px expanded):           |
| [$UIUX_LIVE_SPEC: SYNCED 4.1ms] [Active Agents: cowok-1, advisor-1] [PTY: claude / codex stream] |
+--------------------------------------------------------------------------------------------------+
```

---

## 8. Ten Canonical Layout Blueprints (`LAY-01` .. `LAY-10`) for `cowok-design`

To cover every product capability in the `cowok-design` PRD (`flowforge-design-spec/product/01-PRD.md`, `04-USER-FLOWS.md`, `2.1. product-brief-or-prd.md`), the Workbench supports **10 Canonical Layout Blueprints**:

### Layout 01 (`LAY-01`): Master Three-Zone Flow-First Studio (Default Mode 1)
- **Target Persona / Use Case**: Product Managers, UX Architects, and Engineers mapping multi-screen state graphs (`SCR-*`) and inspecting live screen states.
- **Zone Allocation**:
  - **TopBar (`48px`)**: `CMP-02 WorkbenchTopNav` (`Logo Lockup` + `Mode Switcher` + `MetricLine [5.10]` + `Theme Switcher` + `⌘K`).
  - **Left Zone (`260px`)**: `CMP-03 SpecHierarchyTree` (9 Foundational Specs `01..09` + Multi-Portal Flow Tree + Orphan Node filter).
  - **Center Stage (`1fr`)**: `@xyflow/react` v12 DAG Canvas rendering `CMP-04 ScreenStateNode` cards (`default`, `loading`, `empty`, `error`, `confirmation-modal`) with `CMP-05 FlowValidationToolbar` floating HUD.
  - **Right Zone (`360px`)**: `RightDrawer [5.25]` with `cowok` Orchestrator chat, `CMP-06 ClarificationCardWidget`, and `CMP-15 TokenInspectorPanel`.
- **Registered UX Patterns**: `PAT-01 (Workspace Shell)`, `MetricLine [5.10]`, `RightDrawer [5.25]`, `CMP-01..CMP-06`.

### Layout 02 (`LAY-02`): Synchronized Dual-Canvas Split-Stage (`XYFlow Graph ↔ State-Hydrated Live Iframe`)
- **Target Persona / Use Case**: Designers and Developers verifying how a flow transition (`Edge`) behaves inside the live state-hydrated runtime (`route + fixtureRef`) simultaneously.
- **Zone Allocation**:
  - **Left Zone (`240px` compact)**: `CMP-03 SpecHierarchyTree` collapsed to active portal screens.
  - **Center Left Sub-Stage (`45%`)**: Interactive XYFlow DAG focused on the active user journey (`FLW-*`). Clicking any `ScreenStateNode` or transition edge immediately hydrates the right sub-stage.
  - **Center Right Sub-Stage (`55%`)**: Sandboxed Live Preview `iframe` (`1440x900` / `768x1024` / `390x844`) with 6-state runtime banner (`ready | loading | unresolved | sandbox_failed | fixture_invalid | stale_mapping`) and `CMP-09 CommentPinOverlay` (`[data-ff-id]`).
  - **Right Zone (`340px`)**: `CMP-15 TokenInspectorPanel` + `CMP-09` Comment Thread.
- **Registered UX Patterns**: `PAT-01`, `PAT-05 (Inspector Drawer)`, `CMP-04`, `CMP-05`, `CMP-09`, `CMP-15`.

### Layout 03 (`LAY-03`): Focus-First Full-Bleed Live Runtime with Floating Mini-DAG HUD
- **Target Persona / Use Case**: Deep UI/UX craft review, responsive breakpoint testing (`Desktop / Tablet / Mobile`), and spatial comment pinning (`[data-ff-id]`) during 8–12 hour sessions (`data-theme="dim"` or `"night"`).
- **Zone Allocation**:
  - **Left Zone**: Auto-collapsed to a `48px` icon rail.
  - **Center Stage (`100%` width)**: Full-bleed State-Hydrated Live Preview canvas with top `CMP-05` pointer toggle (`Inspect [data-ff-id]` vs `Interact Live State`) and a draggable bottom-left **Mini-DAG PIP (Picture-in-Picture) Radar** (`240x160px`) for 1-click state jumping without leaving full-screen preview.
  - **Right Zone (`360px` slide-over)**: Opens on `[data-ff-id]` click to expose `CMP-15 TokenInspectorPanel` (`<16ms` surgical AST patcher) or anchored comment thread.
- **Registered UX Patterns**: `RightDrawer [5.25]`, `Toast [5.12]`, `CMP-05`, `CMP-09`, `CMP-15`.

### Layout 04 (`LAY-04`): Visual Clarification & Side-by-Side Design Tournament Arena (`2-Up / 3-Up`)
- **Target Persona / Use Case**: Resolving ambiguous prompts (`ambiguityScore >= 0.45`) or running multi-agent Design Tournaments (`Variant A` vs `Variant B` vs `Variant C`).
- **Zone Allocation**:
  - **Top Arena Header (`56px`)**: Displays the structured `DesignIntentV1 (SDI)` prompt, Risk Tier (`R2`), and synchronized viewport controls (lock scroll, lock state `default/loading/error`, toggle theme across all variants simultaneously).
  - **Center Stage (`2-Column` or `3-Column` Equal Grid)**: Renders `CMP-07 DesignTournamentArena` cards side-by-side. Each column contains a live sandboxed preview, Agent Author badge (`--agent-accent`), Automated `E1..E4` Scorecard (`Brand Fit`, `Zero-Tailwind`, `WCAG Contrast`, `DOM Density`), and human action bar (`Promote Winner` in `--human-accent`, `Fork & Combine`, `Reject`).
  - **Bottom / Right Drawer**: `CMP-06 ClarificationCardWidget` capturing explicit human rationale so the decision is persisted to `05_DESIGN_SYSTEM.md` / `04_SCREEN_SPECS.md`.
- **Registered UX Patterns**: `PAT-04 (Approval Center)`, `MetricLine [5.10]`, `CMP-06`, `CMP-07`.

### Layout 05 (`LAY-05`): Dual-Pane 16:9 HTML Presentation & Non-Tech CMS Studio (Mode 2)
- **Target Persona / Use Case**: Founders, PMs, and Design Leads creating executive pitch decks, architecture review decks, and sprint acceptance reports (`flowforge-design-spec/product/07-HTML-PRESENTATION-CMS-SPEC.md`).
- **Zone Allocation**:
  - **Left Rail (`220px`)**: Thumbnail Slide Sorter (`SLD-01..SLD-N`) with narrative arc badges (`Hook`, `Problem`, `Flow Demo`, `Architecture`, `Metrics`, `Roadmap`) and drag-and-drop reordering.
  - **Center Stage (`60%`)**: Fixed-aspect `16:9` (`1920x1080` scale-fitted) Interactive HTML Slide Canvas with live embedded `cowok-design` components, Mermaid diagrams, and `ResizeObserver` overflow guard (`E3 Gate`).
  - **Right CMS Pane (`40%`, `CMP-10 PresentationDualPaneView`)**: Zero-code Structured Markdown & Property Form Editor (`Headline`, `Key Takeaway`, `Metric Strip [5.10]`, `Chart DataTableView [5.21]`, `Brand Preset`). Keystrokes patch `[data-ff-id]` in `<16ms` via `MessageChannel` without reloading the slide stage.
- **Registered UX Patterns**: `CMP-10`, `DataTableView [5.21]`, `MetricLine [5.10]`, `PAT-07`.

### Layout 06 (`LAY-06`): ChangeProposal Review Inbox & `E1..E4` Governance Diff Studio
- **Target Persona / Use Case**: Human Tech Lead / Design Lead reviewing pending AI-generated `ChangeProposals` (`DRAFT` $\rightarrow$ `APPROVED` $\rightarrow$ `EXECUTED`).
- **Zone Allocation**:
  - **Top Filter Strip**: `AdvancedFilterBar [5.22]` filtering proposals by Risk Tier (`R0..R3`), Actor (`@cowok`, `@advisor`, `claude-code`, `codex`), Gate Verdict (`E1..E4 PASS/FAIL`), and Portal.
  - **Left Proposal Queue (`300px`)**: `PAT-04 Approval Center` list showing pending proposals with `3px solid var(--agent-accent)` draft indicators, affected `SCR-*` / `01..09` files, and `StaleFile` OCC lock status.
  - **Center Diff & Visual Stage (`1fr`, `CMP-08 ChangeProposalDiffModal`)**: Split view with **Top Visual Before/After Slider (or Side-by-Side DOM Diff)** and **Bottom AST / Markdown Unified Hunk Diff (`PAT-07`)**, paired with the `E1..E4` Verification Ladder receipt and explicit `Approve & Execute` button (`--human-accent`).
- **Registered UX Patterns**: `PAT-04`, `PAT-07`, `AdvancedFilterBar [5.22]`, `CMP-08`.

### Layout 07 (`LAY-07`): `cowok` & `advisor` Autonomous OPC Fleet Hub + Embedded `$UIUX_LIVE_SPEC` PTY (Mode 3)
- **Target Persona / Use Case**: Orchestrating parallel CLI coding/design agents (`Claude Code`, `Codex`, `Gemini`) with real-time `<50ms` `$UIUX_LIVE_SPEC` visual feedback and `3-Strike Circuit Breaker` governance.
- **Zone Allocation**:
  - **Top Telemetry Strip**: `MetricLine [5.10]` showing Active Worktree Leases, `.cowok-design/coordination/` Mailbox Queue Depth, Token/Cost Burn, and Circuit Breaker status (`0/3 Strikes`).
  - **Upper Center Grid (`55% height`, `CMP-11 OPCFleetControlConsole`)**: `PAT-02 Task Board` + `PAT-03 Run Timeline` cards for each active agent slot (`cowok-orchestrator`, `advisor-auditor`, `worker-slot-1`, `worker-slot-2`) alongside a live Mini-Preview of the target screen being mutated via `$UIUX_LIVE_SPEC`.
  - **Lower Dock (`45% height`, `CMP-12 EmbeddedTerminalPanel`)**: Multi-tab native PTY console (`[data-theme="night"]`) streaming live agent stdout/stderr and `Jev Noul <1ms` AST guard intercepts.
- **Registered UX Patterns**: `PAT-02`, `PAT-03`, `MetricLine [5.10]`, `CMP-11`, `CMP-12`.

### Layout 08 (`LAY-08`): 9-Spec Registry Matrix, Constraint Configurator & Spec Drift Reconciler
- **Target Persona / Use Case**: Managing the 9 Foundational Specs (`01_PROJECT_BRIEF` .. `09_EVAL_SPEC`), locking `PROJECT_CONSTRAINTS` (`Zero-Tailwind`, allowed icon manifest, target viewports), and reconciling Spec Drift (`SDI`).
- **Zone Allocation**:
  - **Left Zone (`260px`)**: `CMP-03 SpecHierarchyTree` with version hashes and lock icons.
  - **Center Top**: `CMP-14 SpecDriftIndicator` breakdown strip + `AdvancedFilterBar [5.22]`.
  - **Center Split Stage**: Left sub-pane renders the structured `DataTableView [5.21]` of Spec Requirements (`FR-*`, `SCR-*`, `CMP-*`, `Token Constraints`) with live linkage status; Right sub-pane renders the Markdown/AST editor and Bi-Directional Drift Reconciler (`Sync Code → Spec` vs `Generate ChangeProposal Spec → Code`).
- **Registered UX Patterns**: `DataTableView [5.21]`, `AdvancedFilterBar [5.22]`, `CMP-03`, `CMP-14`.

### Layout 09 (`LAY-09`): `agentskills.io` Lifecycle Hub & `Design-Only REA Forge` Studio (Mode 4)
- **Target Persona / Use Case**: Installing/auditing `agentskills.io` skills and extracting Design DNA (color tokens, typography scales, spacing rhythm, component anatomy) from reference websites/screenshots via `Design-Only REA Forge`.
- **Zone Allocation**:
  - **Left Catalog (`280px`)**: `AdvancedFilterBar [5.22]` + `PAT-08 Connector & Skill Setup Cards` (`Installed Skills`, `MCP Servers`, `REA Extractors`) with sandbox permission badges.
  - **Center Stage (`CMP-13 SkillLifecycleManagerView`)**:
    - *Tab A (Skill Workbench)*: Skill `SKILL.md` inspector, input/output schema validator, and dry-run sandbox tester.
    - *Tab B (Design-Only REA Forge)*: Reference URL/Screenshot dropzone on the left $\rightarrow$ Extracted Token & Component DNA Preview (`tokens.css` diff + typography scale + component specimen preview) on the right, isolated inside a `DRAFT` sandbox until promoted via `CMP-08`.
- **Registered UX Patterns**: `PAT-08`, `AdvancedFilterBar [5.22]`, `DataTableView [5.21]`, `CMP-13`.

### Layout 10 (`LAY-10`): Time-Travel Snapshot Ledger & Executive Acceptance Sign-Off Companion
- **Target Persona / Use Case**: Sprint acceptance review, visual regression time-travel across immutable snapshots (`SNAP-*`), and exporting signed acceptance bundles (`HTML Deck` / `GitHub PR` / `Spec Pack`).
- **Zone Allocation**:
  - **Left Timeline (`280px`, `CMP-16 SnapshotTimelineDrawer`)**: Chronological ledger of sealed snapshots (`SNAP-01..SNAP-N`) with author (`@human` vs `@agent`), `E1..E4` gate seal, and 1-click `Compare` or `Rollback`.
  - **Center Comparison Stage (`1fr`)**: Onion-skin / Curtain slider / Side-by-Side visual diff between `SNAP-(N-1)` and `SNAP-N` across all `SCR-*` states.
  - **Right Sign-Off Drawer (`360px`)**: Acceptance checklist (`DataTableView [5.21]`), FMEA risk sign-off, and final `@human` cryptographic gate approval (`--human-accent`).
- **Registered UX Patterns**: `PAT-03`, `PAT-04`, `DataTableView [5.21]`, `CMP-16`.

---

## 9. Eight Canonical End-to-End Userflows (`UF-01` .. `UF-08`) Aligned with PRD

### Userflow 01 (`UF-01`): Ambiguous Prompt $\rightarrow$ Socratic Visual Clarification $\rightarrow$ 9-Spec & Flow DAG Generation
1. User enters a high-level product request in `cowok` Orchestrator or `⌘K` (`PAT-06`).
2. `cowok` computes `DesignIntentV1 (SDI)`. If `ambiguityScore >= 0.45` (`R2`), `cowok` does **not** guess silently; it renders `CMP-06 ClarificationCardWidget` + `LAY-04` (2–3 visual layout/flow options).
3. Human selects the preferred direction (`--human-accent`).
4. `cowok` generates/updates `.cowok-design/specs/01..09_*.md` and populates the XYFlow DAG (`LAY-01`, `CMP-04 ScreenStateNodes` with `default`, `loading`, `empty`, `error` states).

### Userflow 02 (`UF-02`): Flow Node Selection $\rightarrow$ State-Hydrated Live Preview $\rightarrow$ Spatial `[data-ff-id]` Comment $\rightarrow$ `<16ms` Surgical Patch
1. In `LAY-01` or `LAY-02`, user clicks a `ScreenStateNode` (`SCR-04: Checkout — Error State`).
2. Center Stage hydrates the sandboxed `iframe` at `route + fixtureRef="checkout-card-declined"`.
3. User switches `CMP-05` to `Inspect Mode`, clicks the primary CTA button (`[data-ff-id="checkout-retry-btn"]`), and drops a `CMP-09 CommentPinOverlay`: *"@cowok change spacing to `--space-4` and label to 'Retry Payment Method'"* (`R0/R1`).
4. `oxc` AST Patcher (`crates/ast-guardian`) + `Jev Noul` (`<1ms`) validates token compliance (`E1 Gate`), patches the AST, and updates `[data-ff-id="checkout-retry-btn"]` in `<16ms` via `MessageChannel` with an instant `Undo` receipt in `Toast [5.12]`.

### Userflow 03 (`UF-03`): Multi-Agent Design Tournament (`3-Up Arena`) $\rightarrow$ Rubric Scoring $\rightarrow$ Winner Promotion
1. For a high-craft screen (`SCR-01: Workspace Dashboard`), user triggers **"Run Design Tournament (3 Variants)"** from `⌘K` (`PAT-06`).
2. Workbench switches to `LAY-04 (`CMP-07 DesignTournamentArena`)`. Three isolated agent workers generate `Variant A (Dense Editorial)`, `Variant B (Split Canvas)`, and `Variant C (Focus Card)` using `cowok-brand-product-system-v4.3` tokens.
3. Automated `E1..E4` scorer grades each variant on Zero-Tailwind compliance, WCAG AA contrast across `Warm/Dim/Night`, and layout rhythm.
4. Human compares variants with synchronized scroll/theme toggles and clicks **`Promote Variant B`** (`--human-accent`), sealing a snapshot in `CMP-16`.

### Userflow 04 (`UF-04`): Dual-Pane 16:9 HTML Presentation Studio $\rightarrow$ `<16ms` Non-Tech CMS Editing $\rightarrow$ Export
1. User switches `CMP-02 WorkbenchTopNav` to **Mode 2: Presentation 16:9 (`LAY-05`)**.
2. Slide Copilot synthesizes a 16:9 HTML slide deck directly from `01_PROJECT_BRIEF`, `03_USER_FLOWS`, and live `SCR-*` components.
3. Non-tech stakeholder edits slide headlines, KPI numbers (`MetricLine [5.10]`), or table rows (`DataTableView [5.21]`) in the Right CMS Editor (`CMP-10`).
4. Every keystroke updates the left `1920x1080` HTML slide DOM node (`[data-ff-id]`) in `<16ms` while `ResizeObserver` enforces zero 16:9 overflow (`E3 Gate`).

### Userflow 05 (`UF-05`): Autonomous OPC Fleet Execution (`$UIUX_LIVE_SPEC` `<50ms`) & `3-Strike Circuit Breaker` Escalation
1. User switches to **Mode 3: Fleet & PTY (`LAY-07`, `CMP-11` + `CMP-12`)** and dispatches an implementation wave to `cowok` worker agents.
2. As CLI agents write to `$UIUX_LIVE_SPEC`, the Rust watcher runs `Jev Noul (<1ms)` + `StaleFile` OCC check and streams live DOM morphs (`<50ms`) to the stage preview.
3. If a worker violates `Zero-Tailwind` or fails `E1..E3` three times consecutively, the **3-Strike Monotonic Circuit Breaker** trips, halts the worker's process group, and routes the failure trace to `advisor` for a 6-Step Hardening Loop proposal in `CMP-08`.

### Userflow 06 (`UF-06`): Spec Drift Detection (`SDI`) $\rightarrow$ Bi-Directional Reconciliation Gate
1. External code edits in the repository cause implementation DOM/routes to diverge from `03_USER_FLOWS.md` and `04_SCREEN_SPECS.md`.
2. `CMP-14 SpecDriftIndicator` in `WorkbenchTopNav` shifts from `SDI 0.02 (Ready)` to `SDI 0.34 (Drift Warning — --state-warning)`.
3. User clicks the SDI badge to open `LAY-08`, inspects the exact drifted `[data-ff-id]` nodes and routes, and chooses either **"Accept Code as New Spec Truth (`R2`)"** or **"Generate Repair Proposal to Restore Code (`R1`)"**.

### Userflow 07 (`UF-07`): `Design-Only REA Forge` Reference Extraction $\rightarrow$ Brand Preset Sandbox $\rightarrow$ Promotion
1. User switches to **Mode 4: Skills & REA (`LAY-09`, `CMP-13`)** and provides a reference URL or UI screenshot to `Design-Only REA Forge`.
2. `REA Forge` extracts color palette, typography scale, border radii, and spacing rhythm into an isolated `preset-custom-project` draft (`3px solid var(--agent-accent)`).
3. User previews how existing `SCR-*` screens look under the extracted preset without mutating the locked workspace, then approves promotion via `CMP-08`.

### Userflow 08 (`UF-08`): High-Risk (`R2/R3`) `ChangeProposal` Review $\rightarrow$ `E1..E4` Verification $\rightarrow$ Time-Travel Snapshot
1. Any structural flow change, multi-screen refactor, or 9-Spec mutation lands in `LAY-06 (`CMP-08 ChangeProposalDiffModal`)` as `DRAFT`.
2. Independent verifier (`Builder != Scorer`) executes the `E1..E4` verification ladder and attaches the signed receipt.
3. Human reviews the visual before/after diff and AST hunks, clicks **`Approve & Apply Proposal`** (`--human-accent`), transitioning state `DRAFT → APPROVED → EXECUTED` and recording a reversible entry in `CMP-16 SnapshotTimelineDrawer`.


---

## 9. Kiến Trúc Hợp Nhất `DualMainPanel` & `CompanionCanvasTab` (v5.0 — Đồng bộ `cowok.ai` Epic 36 & Epic 10)

### 9.1. Bố Cục Vĩ Mô (`Macro Shell`)
1. **Main Sidebar (`Left`, `248px`, có thể thu gọn)**:
   - Tổ chức theo cây **Projects (Thư mục dự án)** $\rightarrow$ **Chat Sessions (Các phiên hội thoại thuộc dự án)** + `Pinned Conversations` + `+ New Conversation` (đồng bộ chuẩn Antigravity 2.0 & `cowok.ai`).
   - Nút **`⚙️ Settings`** cố định ở đáy Main Sidebar, khi click mở **Popup Modal Cài đặt** (chuyển đổi 4 Theme `Warm` / `Dim` / `Night` / `Low-Focus`, cấu hình Boundary dự án, Model & Agent).
2. **Single Main Panel (`DualMainPanel [5.29]`)**:
   - Chỉ duy trì **1 Main Panel duy nhất**, chia làm 2 không gian có thanh kéo `drag-splitter` (`col-resize`) tùy chỉnh kích thước:
     - **Bên trái (`UniversalChatConsole` + `FloatingCapsuleComposer [5.40]`)**: Khung hội thoại với Agent, hiển thị thẻ Tournament trực tiếp trong luồng chat và **Khay Đính kèm Nhận xét Vùng (`Chat Attachment Tray`)** nằm ngay phía trên ô nhập liệu (chuẩn `cowok.ai` Epic 10).
     - **Bên phải (`CompanionCanvasTab [5.31]`)**: Khung Canvas đa tab (`Multi-Tab Bar` cao `38px`) có nút **`＋ Mở Tab / File`** cho phép:
       1. **📱 Tạo Apps (`kind: app-flow`)**: Mở giao diện **DAG Userflow (Trái) $\leftrightarrow$ Live HTML (Phải)** (gộp `LAY-01`, `LAY-02`, `LAY-03`, `LAY-04`).
       2. **📑 Tạo Slide / Mở File HTML (`kind: slide-deck | html-doc`)**: Mở giao diện **Cấu trúc Slide/Phase + Slide-Over CMS (Trái) $\leftrightarrow$ Live HTML (Phải)** (`LAY-05`).
       3. **📂 Mở các file `.html` theo từng Dự án**: Tự động định tuyến sang chế độ hiển thị phù hợp.

### 9.2. Cơ Chế Định Tuyến 4 Tầng (`4-Tier HTML Routing Cascade` — Giải Quyết Khoảng Trống 6.1)
Vì đầu ra vật lý đều là `.html`, hệ thống định tuyến khung bên trái của Canvas (`DAG Userflow` vs `Cấu trúc Slide/Phase CMS`) theo thứ tự ưu tiên:
1. **Tầng 1 — Sidecar Manifest (`<filename>.artifact.json`) & Thẻ `<meta name="cowok:canvas-mode">`**: Ghi nhận ngay lúc khởi tạo từ nút `＋` hoặc công cụ Agent (`kind: "app-flow" | "deck" | "html-doc"`, `renderer: "flow-html" | "deck-html"` — tương tự `chunk-RAKLGIRL.mjs` của OpenDesign).
2. **Tầng 2 — Đồ thị Liên kết Dự án (`DESIGN-MANIFEST.json` / `flow-graph.json`)**: Nếu file `.html` tham gia vào mạng lưới chuyển trạng thái đa màn hình (`screens[]`), tự động mở **DAG Userflow**.
3. **Tầng 3 — Dấu Vân Tay Cấu Trúc DOM/AST (`<5ms` Fast Parser khi mở file `.html` bất kỳ từ ổ đĩa)**:
   - Có `[data-slide]`, `aspect-ratio: 16/9`, hoặc danh sách `<section data-phase>` tuần tự không có bộ định tuyến trạng thái $\rightarrow$ mở **Cấu trúc Slide/Phase + Slide-Over CMS**.
   - Có nhiều `[data-screen]`, `<form>`, modal/drawer hoặc `role="application"` $\rightarrow$ mở **DAG Userflow**.
4. **Tầng 4 — Nút Gạt Cưỡng Chế 1-Click trên Sub-Toolbar Canvas (`[🔀 DAG Userflow | 📑 Cấu trúc Slide/CMS]`)**: Cho phép người dùng chuyển góc nhìn cấu trúc bên trái bất cứ lúc nào mà không làm thay đổi mã nguồn HTML.

### 9.3. Ranh Giới Dự Án Lai & Đa Phiên Hội Thoại (`Hybrid Project Boundary & Multi-Session` — Giải Quyết Khoảng Trống 6.2)
- **Quan hệ `1 Project : N Chat Sessions`**: Một Project chứa nhiều phiên chat độc lập về ngữ cảnh hội thoại và bộ tab đang mở, nhưng chia sẻ chung `DESIGN.md`, `tokens.css` và cây thư mục file.
- **Boundary 2 Chế độ (Dual-Provenance giống OpenDesign `server-WTGXM3LB.mjs`)**:
  1. **`Folder-Backed Project` (`writeback: "in-place"`)**: Gắn trực tiếp với thư mục Git/Local trên máy người dùng (xác thực qua HMAC Token).
  2. **`Managed Scratch Project` (`od-owned`)**: Khởi tạo tức thì trong `~/.cowok-design/projects/<projectId>` khi user muốn phác thảo nhanh không cần chọn thư mục trước, hỗ trợ liên kết (`Link to Local Folder`) về sau.


---

## 10. Tinh Chỉnh UX Thực Tế v5.1 (Phê Duyệt Chính Thức)

1. **Đa Định Dạng Tệp Trên `CompanionCanvasTab` (`.html`, `.md`, Hình ảnh `.svg/.png/.webp`, `.css/.json`)**:
   - Nút **`＋ Mở Tab / Tệp Dự Án`** trên thanh Tab của Canvas cho phép mở trực tiếp các tài liệu `.md` (`DESIGN.md`, `PRD.md`), hình ảnh/tài nguyên thương hiệu (`.svg`, `.png`, `.webp`) và bảng token (`.css`, `.json`) bên cạnh file `.html` để người dùng tham chiếu và khoanh vùng comment ngay trong quá trình thiết kế.
2. **Cơ Chế Xác Nhận Thực Thi Tournament Trong Khung Chat (`Enter` / Nút `Xác nhận`)**:
   - Khi người dùng click vào `Option A / B / C` trên khung Chat hoặc trên Userflow DAG, giao diện HTML bên phải sẽ hiển thị bản xem thử (`Preview`) ngay lập tức.
   - Để Agent chính thức nhận phản hồi và thực thi cập nhật mã nguồn, người dùng ấn phím **`Enter`** hoặc bấm nút **`✓ Xác nhận thực thi Option X`** ngay trong thẻ Tournament ở khung Chat.
3. **Thanh Công Cụ Riêng Trên Khung HTML & 2 UX Pattern Chọn Vùng Comment**:
   - Thanh công cụ của khung HTML có bộ chuyển đổi trực tiếp giữa **`🖱️ Tương tác HTML`** và **2 chế độ Chọn vùng Comment**:
     - **Pattern 1 (`🎯 Click Component`)**: Click trực tiếp vào từng khối/component DOM trên HTML.
     - **Pattern 2 (`⬚ Kéo Khung Chọn` — Marquee Drag Box giống Antigravity 2.0 / Codex)**: Nhấn giữ chuột và kéo khung hình chữ nhật tọa độ (`width × height px`) trên bất kỳ vùng giao diện/hình ảnh nào để mở hộp ghi chú và đẩy thẳng vào khay Attachment của khung Chat.
4. **Thu Gọn Khung Chat (`⇤ Thu gọn Chat` / `Shorten Chat Rail`)**:
   - Loại bỏ hai nút `Full Chat` và `Full Canvas` rời rạc; thay bằng nút icon **`⇤ Thu gọn Chat`** đặt ngay trên thanh Header của khung Chat. Khi bấm, khung Chat thu gọn thành một thanh dọc mảnh (`42px`) bên mép trái để nhường không gian tối đa cho khung Canvas, và có thể bấm `⇥ Mở rộng Khung Chat` để khôi phục ngay.
5. **Nút Chuyển Đổi Góc Nhìn Cấu Trúc Đặt Ngay Trên Header Phần Cấu Trúc**:
   - Nút gạt **`[🔀 DAG Userflow | 📑 Cấu trúc Slide / CMS]`** nằm trực tiếp trên thanh Header của khung Cấu trúc bên trái (trong Canvas).
6. **Bộ Chọn Responsive & Khung Hình Present Trên Thanh Công Cụ HTML**:
   - Hỗ trợ chuyển đổi kích thước Responsive (`🖥️ Desktop`, `💻 Tablet 768px`, `📱 Mobile 375px`), chọn tỉ lệ khung hình trình chiếu (`Auto Fit`, `16:9 Presentation Deck`, `16:10 MacBook Studio`) và nút **`▶ Present`** toàn màn hình.
7. **Chốt Kiến Trúc Ranh Giới Dự Án Lai (`Approved Hybrid Project Boundary`)**:
   - Chính thức áp dụng mô hình **Hybrid Dual-Provenance (`Folder-Backed In-Place Git` + `Managed Scratch`)** với quan hệ `1 Project : N Chat Sessions`.


---

## 11. Chuẩn Tối Giản `cowok.ai` (Minimalist Desktop Ergonomics), 9 Ngôn Ngữ & Quản Trị Tương Tác Userflow (v5.2)

### 11.1. Mở & Chỉnh Sửa Trực Tiếp Các Tệp Code Cho Advanced Users (`Live Source Code Editor`)
- Bên cạnh `.html`, `.md`, hình ảnh (`.svg`, `.png`, `.webp`), `CompanionCanvasTab` hỗ trợ mở trực tiếp các tệp mã nguồn (`.tsx`, `.ts`, `.css`, `.json`, `.html` raw source) hoặc bật nút icon **`</> Code Editor`** ngay trên thanh công cụ HTML để người dùng nâng cao (Advanced Users / Developers) chỉnh sửa trực tiếp mã nguồn và bấm **`✓ Áp dụng Code (⌘S)`** để đồng bộ tức thì sang giao diện Live HTML và cây AST.

### 11.2. Hệ Thống Nút Icon-Only Kèm Tooltip & Giữ Lại Key Buttons (`Minimalist Action Hierarchy`)
- **Icon-Only Buttons (`28×28px`, SVG stroke `1.8px`, kèm `[data-tooltip]` tức thì)**: Toàn bộ nút chức năng phụ trợ trên Sidebar, Header Khung Chat (`Thu gọn Chat`), Header Cấu trúc (`Thêm Node`, `Ẩn/Hiện khung`), Thanh công cụ HTML (`Tương tác`, `Click Component`, `Kéo Khung Chọn`, `Code Editor`, `Desktop / Tablet / Mobile`), và thanh dưới `FloatingCapsuleComposer` (`Đính kèm`, `Chọn vùng`) đều chuyển về dạng Icon tinh gọn kèm Tooltip.
- **Key Buttons (Giữ cả Icon + Chữ)**: Chỉ giữ lại trên 4 nút hành động chính yếu:
  1. `+ New Conversation` (Đỉnh Main Sidebar)
  2. `[🔀 Userflow | 📑 Slide / CMS]` (Chuyển góc nhìn trên Header Cấu trúc)
  3. `▶ Present` (Trình chiếu trên Thanh công cụ HTML)
  4. `✓ Xác nhận Option X (Enter ↵)` (Chốt phương án Tournament trong Khung Chat)

### 11.3. Hỗ Trợ 9 Ngôn Ngữ (`i18n` Kế Thừa Trực Tiếp Từ `cowok.ai`)
- Đồng bộ đầy đủ 9 ngôn ngữ chuẩn của `cowok.ai` (`LocaleForm.tsx` & `src/i18n/locales/`):
  - `vi-VN` (🇻🇳 Tiếng Việt), `en-US` (🇺🇸 English), `zh-CN` (🇨🇳 中文), `ja-JP` (🇯🇵 日本語), `ko-KR` (🇰🇷 한국어), `fr-FR` (🇫🇷 Français), `es-ES` (🇪🇸 Español), `pt-PT` (🇵🇹 Português), `it-IT` (🇮🇹 Italiano).

### 11.4. Kế Thừa `FloatingCapsuleComposer` & `ChatStreamLayout` Tối Giản Từ `cowok.ai`
- Áp dụng triết lý **Minimalist Desktop Ergonomics** của `cowok.ai` để giảm tải nhận thức khi làm việc 8–12 giờ liên tục:
  - Lược bỏ các nhãn trạng thái rườm rà và đường viền đứt nét nặng nề; sử dụng đường kẻ mảnh `1px solid var(--border-subtle)`.
  - Khung hội thoại kế thừa `ChatStreamLayout` (Avatar tròn `26px`, dòng meta tinh gọn) và `FloatingCapsuleComposer` (Khay Attachment Chip ở tầng trên; nút Icon đính kèm + pill `✦ Cowok Swarm · High` + nút gửi tròn `MorphingSendStopButton` ở tầng dưới).

### 11.5. Cơ Chế Tương Tác Trên Đồ Thị `DAG Userflow` (Guardrail-First Hybrid Model)
- **Cho phép Comment trực tiếp trên từng Node/Edge của Userflow**: Mỗi Node trên DAG có nút icon `💬 Comment` để đính kèm chip `[Node SCR-02] "..."` thẳng vào khay Attachment của khung Chat.
- **Phân tách rõ các thao tác trực tiếp được phép (Safe Visual/Navigational Actions) và các thao tác kiến trúc phải đi qua Chat/Agent (Semantic & Structural Mutations)** để bảo toàn tính toàn vẹn của mã nguồn HTML và `flow-graph.json`.


---

## 12. Đặc tả Nâng cấp v5.3: Agent-Centric Open-Prose Chat, Text Selection `@[Quote]`, Draft-Aware DAG Userflow Studio & Ma trận Mapping Epics/Stories (`Anti-Drift`)

### 12.1. Triết lý Khung Chat Làm việc `Human ↔ Agent` (Chuẩn Antigravity 2.0 / Codex / Gemini)
1. **Loại bỏ hoàn toàn Avatar (`Zero Avatar`)**: Không hiển thị avatar tròn cho cả User và Agent vì đây là không gian làm việc 1-1 trực tiếp với Agent.
2. **Bong bóng Khung chỉ dành cho Human (`.chat-human-bubble`)**: Chỉ đóng khung gọn gàng ở phía phải cho lời nhắc (prompt) của người dùng.
3. **Văn bản Mở toàn chiều rộng cho Agent (`.chat-agent-prose`)**: Phản hồi của Agent hiển thị dạng văn bản mở (Open Prose) trải rộng toàn bộ chiều ngang khung Chat, giúp đọc tài liệu dài, bảng biểu, mã nguồn và thẻ Tournament thoải mái, không bị bó hẹp trong bong bóng.
4. **Tinh giản Công cụ Chat Human-to-Human**: Loại bỏ hoàn toàn bộ chọn Emoji, thanh định dạng chữ (In đậm/Nghiêng/Gạch chân/Cỡ chữ cục bộ) trên thanh nhập liệu. Phần cấu hình **Cỡ chữ toàn cục (`Global Font Size: 12px Compact / 13px Default / 14px Comfort / 15px Large`)** được đưa vào **`⚙️ Settings Modal`** (`--app-font-size`).

### 12.2. Tính năng Bôi đen Trích dẫn Nhanh `❝ Quote (@[Quote])` trong Chat & Tài liệu Doc
- Khi người dùng bôi đen bất kỳ đoạn văn bản nào (`> 3 ký tự`) trong phản hồi của Agent (`.chat-agent-prose[data-quotable="true"]`) hoặc trong tài liệu Markdown (`DESIGN.md` / `.md` trên `CompanionCanvasTab`), một nút nổi **`❝ Quote vào Chat`** (`#floatingQuotePill`) lập tức xuất hiện ngay trên vùng chọn.
- Khi bấm **`❝ Quote vào Chat`**, đoạn trích dẫn được gắn thành chip **`@[Quote] "..."`** nằm trong khay đính kèm của ô nhập liệu (`#chatAttachmentTray`), đồng thời tự động focus vào ô nhập để người dùng gõ phản hồi nhanh (giống hệt cơ chế Quote của Antigravity 2.0).

### 12.3. Cơ chế Interactive Draft-Aware DAG Userflow Studio (`Dirty-State Separation & JSON Diff`)
1. **Phân tách Thao tác Bố cục (Non-Structural) và Thao tác Cấu trúc (Structural Draft)**:
   - **Thao tác Không tạo Draft (`● Saved` — Giống Excel/Docs khi chỉ cuộn/thu phóng)**: Kéo thả di chuyển vị trí hiển thị của Node trên mặt phẳng, đóng/mở thu gọn cụm khung (`Collapse/Expand Cluster`), phóng to/thu nhỏ (`Zoom - / +`), hoặc bật/tắt `🗺️ Minimap` sẽ lưu bố cục tức thì mà **không** kích hoạt chế độ Draft.
   - **Thao tác Cấu trúc kích hoạt Chế độ `DRAFT` (`● Unsaved Draft`)**: Thêm Node mới (`+ Node`), kéo nối/đổi dây dẫn, **đảo chiều mũi tên (`⇄`)**, đổi loại luồng (**Luồng chính `Main ───▶`**, **Luồng rẽ nhánh `IF / ELSE - - ▶`**, **Luồng dự phòng `IF NOT / Fallback ···▶`**), nhập/sửa điều kiện (`Condition Input`), hoặc sửa `📝 Note` trên Node sẽ gắn nhãn **`DRAFT`** theo chuẩn tối giản Cowok (**tuyệt đối cấm dùng màu hổ phách/amber tràn nền**; thanh `#dagDraftBar` dùng nền liền mạch `var(--bg-subtle)` cùng nút CTA chính `var(--action-primary)`) và hiển thị thanh **`Triển khai Draft (Gửi JSON → Agent)`** (`#dagDraftBar`).
2. **Đóng khung Cụm Node theo Luồng (`Cluster Group Frames`) & Bộ công cụ Đầy đủ**:
   - Đóng khung phân cụm rõ ràng: **`📦 Cụm 1: Luồng Thanh toán Chính (Main Flow)`** và **`🛡️ Cụm 2: Luồng Rẽ nhánh IF/ELSE & Dự phòng (IF NOT / Fallback)`**.
   - Thanh công cụ DAG hỗ trợ: `↶ Undo`, `↷ Redo`, `Zoom - / +`, `🗺️ Minimap`, `📝 Note` trên từng Node, và nút **`💬` (Comment trên Node)** đẩy trực tiếp chip `[Node SCR-XX]` vào khay đính kèm Chat.
3. **Triển khai Draft qua Gói `flow-changes-draft.json` & Agent Kiểm tra Xung đột (`Conflict Confirmation`)**:
   - Khi người dùng hài lòng với bản nháp trên DAG và bấm **`🚀 Triển khai (Gửi JSON → Agent)`**, hệ thống tự động đóng gói các thay đổi cấu trúc thành tệp JSON chuẩn (`schema: "cowok-design.flow-diff.v1"`) gửi sang khung Chat.
   - Agent phân tích đồ thị AST; nếu phát hiện mâu thuẫn/xung đột (ví dụ: đảo chiều mũi tên tạo vòng lặp về màn hình đầu `SCR-01` hoặc nhánh `IF` chưa định nghĩa nhánh `IF NOT` dự phòng), Agent sẽ **hỏi lại và hiển thị nút xác nhận** để người dùng chốt phương án trước khi thực thi sinh mã HTML.


### 12.4. Chuẩn hóa Đồ thị Có hướng Không chu trình 2D Thực thụ (`True 2D Node-Graph DAG Canvas v5.4`) & Thanh Công cụ Dọc Lề Phải + Cử chỉ Bàn phím / Touchpad / Chuột
1. **True 2D Directed Acyclic Graph (`Branch & Merge 2D Topology`)**:
   - Tuyệt đối không trình bày DAG dưới dạng danh sách thẻ xếp chồng dọc (stacked list).
   - Mỗi Node là một đỉnh đồ thị có tọa độ 2D `(x, y)` độc lập trên mặt phẳng vô cực (`.dag-zoom-stage`), trang bị các **Cổng kết nối (`● Connection Ports`)**: Cổng vào bên trái (`.port-in`), Cổng ra nhánh chính/điều kiện đúng bên phải (`.port-out-main`), và Cổng ra nhánh dự phòng bên dưới (`.port-out-branch`).
   - Các liên kết có hướng được vẽ bằng **Đường cong Bezier bậc 3 SVG thực thụ (`<svg class="dag-svg-edges"> <path d="M x1 y1 C cx1 cy1, cx2 cy2, x2 y2">` kèm `<marker>` mũi tên định hướng)** nối trực tiếp từ cổng xuất của Node nguồn sang cổng nhập của Node đích, hỗ trợ cả **Rẽ nhánh (`Divergence`: `SCR-02` $\nearrow$ `SCR-03` và $\searrow$ `SCR-02B`)** lẫn **Hội tụ (`Convergence / Merge`: `SCR-02B` $\nearrow$ `SCR-03`)**.
   - Người dùng có thể **kéo thả tự do từng Node trong không gian 2D `(x, y)`** (đường cong Bezier tự động tính toán lại theo thời gian thực ở `60fps` mà **không** bật Draft), hoặc **kéo trực tiếp dây dẫn từ chấm cổng `●`** sang Node khác để nối dây mới (bật chế độ `DRAFT`).
2. **Thanh Công cụ Nổi Dọc Lề Bên Phải của Khung DAG (`.dag-right-vertical-dock`)**:
   - Đưa toàn bộ cụm công cụ thao tác đồ thị vào thanh dock dọc nằm sát lề phải của khung DAG (với tooltip mở sang bên trái để không bao giờ bị khuất), bao gồm:
     - `+` (`Thêm Node mới`)
     - `↶` (`Hoàn tác — ⌘Z`) & `↷` (`Làm lại — ⇧⌘Z`)
     - `🔍+` (`Phóng to — ⌘+`), nút hiển thị **`% Zoom` (`Click để Reset 100% — ⌘0`)**, `🔍-` (`Thu nhỏ — ⌘-`), và `⊡` (`Fit View căn vừa toàn đồ thị — Phím F`)
     - `🗺️` (`Bật / Tắt 2D DAG Radar Minimap — Phím M`)
3. **UX Pattern Phóng to / Thu nhỏ & Di chuyển Kết hợp Bàn phím + Touchpad + Chuột**:
   - **Touchpad Pinch-to-Zoom & `Ctrl / Cmd + Con lăn Chuột`**: Phóng to / thu nhỏ mượt mà lấy tâm ngay tại vị trí con trỏ chuột (`zoomDag(step, e.clientX, e.clientY)`).
   - **Touchpad Vuốt 2 ngón (`2-Finger Scroll`) & Kéo chuột trên nền Canvas / `Space + Drag`**: Trượt (`Pan X, Y`) tự do trên mặt phẳng 2D.
   - **Phím tắt Bàn phím (`Keyboard Shortcuts`)**: `⌘/Ctrl + '+'` (Zoom In), `⌘/Ctrl + '-'` (Zoom Out), `⌘/Ctrl + '0'` (Reset 100%), `F` (Fit View), `M` (Toggle Minimap), `⌘/Ctrl + Z` (Undo), `⇧ + ⌘/Ctrl + Z` (Redo).

---

## 13. Enterprise Host Shell, Model/BYOS Gateway & MCP Integration Registry v5.6

### 13.1. Cấu trúc Điều hướng Vĩ mô (Macro Navigation Architecture)
1. **Phân cấp Tri-Group Đồng đẳng trên MainSidebar**:
   - **Group 1: Pinned Conversations**: Ghim các phiên làm việc quan trọng, truy cập nhanh với timestamp gần nhất.
   - **Group 2: Projects (Folder-Backed / Managed)**: Mỗi dự án tương ứng một thư mục độc lập trên ổ đĩa cục bộ hoặc không gian đám mây. Click mở dự án sẽ liệt kê các phiên chat (`Sessions`) liên quan.
   - **Group 3: Conversations (Unassigned / General)**: Chứa các cuộc hội thoại chung chưa gán vào dự án cụ thể (chuẩn Antigravity 2.0).
2. **Nút Thu gọn / Mở rộng Đồng bộ (Synchronized Button Geometry)**:
   - Cả nút thu gọn MainSidebar lẫn nút thu gọn Chat Panel đều sử dụng kích thước chuẩn `28x28px`, bo góc nhẹ, viền `1px solid var(--border-subtle)`, màu nền `transparent / hover: var(--bg-hover)`, sử dụng icon `panel-left-close` và `panel-left-open` chuẩn mực.

### 13.2. Đa ngôn ngữ Phản ứng (Reactive i18n Architecture)
1. **Kiến trúc Reactive Store**: `useTranslation.ts` chuyển đổi từ chuỗi tĩnh sang mô hình Reactive Store với `setGlobalLocale`, `getGlobalLocale`, listener subscription, và `localStorage` persistence.
2. **Cập nhật Thời gian thực Không Tải lại Trang**: Khi người dùng thay đổi ngôn ngữ (tại Footer Sidebar hoặc trong Settings Modal), toàn bộ giao diện (Sidebar, Chat, Canvas, Modal) lập tức cập nhật ngôn ngữ đích mà không cần reload trang.
3. **Chuẩn hóa Iconology & Typology**:
   - Loại bỏ hoàn toàn cờ emoji trong danh sách chọn ngôn ngữ (chỉ hiển thị tên bản địa chuẩn: `Tiếng Việt (vi-VN)`, `English (en-US)`, `Français (fr-FR)`, v.v.).
   - Loại bỏ emoji thừa trên các nút bấm chức năng (ví dụ: `Chọn thư mục` thay vì `📁 Chọn thư mục`).

### 13.3. Chuẩn hóa 4 Giao diện Chuẩn (Strict 4-Theme Canonical Model)
Hệ thống khóa cố định 4 theme chủ đạo theo triết lý công thái học:
1. **Warm Paper (Mặc định)**: Nền giấy ấm, dịu mắt, độ tương phản cao cho đọc tài liệu và thiết kế ban ngày.
2. **Focus Dim (Chống lóa)**: Tone xám trung tính, giảm thiểu mỏi mắt khi làm việc kéo dài.
3. **Night Workspace (Ban đêm)**: Tối sâu, tương phản tinh tế, hỗ trợ code và đồ thị DAG ban đêm.
4. **Monochrome Minimal (Đơn sắc)**: Tối giản tối đa, triệt tiêu phân tâm màu sắc.

### 13.4. Trung tâm Cấu hình Host Shell 4 Tab (`SettingsModal.tsx`)
1. **Tab 1: Chung (General)**: Chọn Theme (4 giao diện), Ngôn ngữ (9 locales), Cỡ chữ toàn cục (`12px..15px`).
2. **Tab 2: Mô hình & BYOS (Models & BYOS)**:
   - Cung cấp ô nhập API Key cho Google Gemini, Anthropic Claude, OpenAI, DeepSeek.
   - Chế độ BYOS (Bring Your Own Subscription): Cho phép cấu hình Custom Base URL & Bearer Token cho các endpoint nội bộ doanh nghiệp.
3. **Tab 3: MCP & Plugin (MCP & Plugins)**:
   - Thông số Host MCP Server của Cowok-Design (`http://127.0.0.1:3100/mcp`) với nút Sao chép URL & Tạo lại API Key.
   - Bảng kết nối Client MCP Servers (`cowok-falkordb`, `local-filesystem`, `browser-devtools`, `github-mcp-server`) với badge trạng thái Kết nối/Lỗi.
   - Nút hành động nhanh: `+ Thêm MCP Server` và `Nhập Plugin`.
4. **Tab 4: Tự động hóa (Automations)**:
   - Quản lý các tiến trình nền daemon, cron trigger theo dõi biến động file, và nhật ký đồng bộ tự động.

### 13.5. Ranh giới Trách nhiệm & Điều hướng Liên Story (Navigation-Only Contract)
- **Story 04.1 đóng vai trò Host Shell Container**: Cung cấp bộ khung giao diện, nút kích hoạt, routing hooks và modal chứa.
- **Phân tách thực thi**:
  - Logic gọi LLM model sâu, sinh variant SDI thuộc về **Epic 05**.
  - Logic thực thi công cụ MCP, proxy và quản lý marketplace thuộc về **Epic 09** & **Epic 11**.
  - Logic điều phối daemon tự động nền thuộc về **Epic 10**.

### 13.6. Chuẩn hóa Công thái học Mềm mại, Đồng bộ Nút Thu gọn/Mở rộng Góc Phải & Cô lập Phạm vi Nút Chức năng Tab (v5.7)
1. **Công thái học Mềm mại & Độ Đàn hồi Xúc giác (Ergonomic Softness & Tactile Comfort)**:
   - Tăng cường bán kính bo góc công thái học giúp không gian làm việc dài hạn (`8–12h`) mềm mại, giảm căng thẳng thị giác nhưng vẫn giữ trọn vẹn 4-theme token chuẩn của `cowok-design`:
     - `--radius-xs: 6px` (Buttons, Badges, Code Pills)
     - `--radius-sm: 8px` (Inputs, Interactive Controls, Toolbar Docks)
     - `--radius-md: 12px` (Semantic Node Cards, Inspector Cards, Sub-panels)
     - `--radius-lg: 16px` (Drawers, Dialogs, Popovers)
     - `--radius-xl: 20px` (Modals, Cluster Bounding Frames)
   - Đường cong chuyển động đàn hồi tự nhiên: `--ease-standard: cubic-bezier(0.22, 1, 0.36, 1)` kết hợp phản hồi lực nhấn vi mô (`:active { transform: translateY(1px) scale(0.985); }`).
2. **Đồng bộ Icon Thu gọn / Mở rộng & Chuẩn hóa Vị trí Góc Phải trên Mỗi Tab/Pane**:
   - Mọi nút thu gọn/mở rộng (`Shorten / Expand / Maximize / Restore`) trên từng phân vùng đều có kích thước chuẩn `28x28px`, đặt thống nhất ở **góc trên bên phải** của thanh tiêu đề phân vùng đó và tự động lật trạng thái icon SVG (`panel-left-close` $\leftrightarrow$ `panel-left-open` hoặc `maximize` $\leftrightarrow$ `minimize`):
     - **MainSidebar (`#mainSidebar`)**: Nút `#btnToggleMainSidebar` đặt ở góc phải `.sidebar-brand-row`; khi thu gọn chuyển sang thanh Rail `44px` và đổi icon sang `panel-left-open`.
     - **AgentChatPanel (`#chatSubpanel`)**: Nút `#btnShortenChat` đặt ở góc phải `.chat-subpanel-header`; khi thu gọn chuyển sang thanh Rail tối giản `40px` (`PD-02 = A`) và đổi icon từ `panel-left-close` sang `panel-left-open`. Ô nhập Chat tự giãn tối đa 6 dòng (`max-height: 144px`) sau đó cuộn dọc (`overflow-y: auto`), không vẽ viền chia cứng quanh composer để liền mạch với luồng chat, kèm nút mũi tên nổi về cuối trang khi tin nhắn dài.
     - **Cô lập HTML của Dự án Người dùng (`Sandboxed Project HTML Isolation`)**: Thanh công cụ phía trên khung Live HTML tuân theo Theme của Workspace, nhưng nội dung HTML bên dưới của dự án người dùng bắt buộc chạy trong `<iframe sandbox="allow-scripts allow-forms">` (`srcDoc`) để giữ nguyên 100% theme/màu sắc gốc của bản thiết kế HTML người dùng.
     - **Chuẩn Tối giản Cowok Minimalist CTA vs. Tab Selection**: Trên Light Mode chỉ nút CTA quan trọng dùng nền xanh (`--action-primary: #0057FF`), còn tab chọn dùng chữ đậm kèm vạch nhấn xanh mảnh (`--tab-indicator-active: #4B7FFF`); trên Dark Mode chỉ nút CTA quan trọng dùng nền cam chuẩn Cowok (`--action-primary: #FF9A2F`, chữ tối `#081E3A`), còn tab chọn dùng chữ sáng đậm kèm vạch nhấn cam mảnh (`--tab-indicator-active: #FF9A2F`). Cấm hoàn toàn màu hổ phách/amber tràn nền.
     - **Cột Cấu trúc Trái (`#canvasLeftPane`)**: Nút `#btnToggleLeftPaneFull` đặt ở góc phải `.structure-pane-header`, đồng bộ icon `maximize` $\leftrightarrow$ `minimize`.
     - **Cột Live HTML Preview Phải (`#canvasRightPane`)**: Nút `#btnToggleRightPaneFull` bắt buộc đặt ở **góc phải** của `.html-pane-toolbar` (ngay cạnh nút `▶ Present`, tuyệt đối không đặt ở góc trái), đồng bộ icon `maximize` $\leftrightarrow$ `minimize`.
3. **Cô lập Nghiêm ngặt Phạm vi Nút Chức năng theo Tab (Strict Tab-Scoped Functional Controls)**:
   - Các nút chức năng thuộc riêng về tab **DAG Userflow / Slide Deck - HTML** (bao gồm nút ẩn/hiện cột cấu trúc trái, ẩn/hiện cột Live HTML Preview phải, huy hiệu `● DRAFT`, nút `Triển khai Draft (Gửi JSON → Agent)` và `Hủy Draft`) **bắt buộc nằm gọn bên trong phạm vi không gian của tab đó** (`#dagDraftBar`, `.structure-pane-header`, `.html-pane-toolbar`).
   - **Tuyệt đối cấm** đưa các nút chức năng nội bộ của tab DAG/Slide/HTML lên thanh điều hướng đỉnh của toàn Workbench (`WorkbenchTopNav.tsx`) hoặc thanh chứa danh sách tab chung (`.canvas-tab-bar` của `CompanionCanvasTab.tsx`). Thanh `.canvas-tab-bar` chỉ chứa các thẻ tab đang mở và nút `+` mở/tạo tab mới.


