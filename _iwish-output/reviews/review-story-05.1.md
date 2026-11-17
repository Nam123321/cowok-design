# Stage 4 Edge Case Guardian & Adversarial Review Report: Story 05.1

- **Story:** [Story 05.1: Project Launchpad (4-Path), Option 5 Action-Linked 2D Storyboard <-> Entry-First 4-SubState HTML, Shared AppShell, Sample Data Studio & Gap-Driven Discovery](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/3.%20Development/1.%20Epic%20&%20Story/VS-02.%20Flow-First%20Executable%20Design%20Workbench%20&%20SDI%20Engine/Epic-05/Story-05.1/story.md)
- **Review Engine:** Edge Case Guardian (13-Pillar FMEA Scan & Adversarial Boundary Verification)
- VERDICT: APPROVED (Score: 9.6/10)
- **Status:** `PROVEN_SAFE`

---

## 1. 13-Pillar FMEA Risk Analysis Table

| Failure Mode ID | Potential Failure / Edge Case Mode | Severity (S) | Occurrence (O) | Detection (D) | RPN | Mitigation & Remediation Action |
| :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| **FMEA-05.1-01** | Ambiguous prompt bypassing clarification threshold or using 8-keyword stub (`legacy_sdi.rs` / `ambiguity-calculator.ts`) | 4 | 2 | 2 | 16 | Replace hardcoded keyword/mock generator with SQLite `V9` entity grounding (`flow_nodes`, `flow_edges`, `project_specs`, `constraint_profiles`) and Gap-Driven Discovery (1–3 visual questions). |
| **FMEA-05.1-02** | Automated code generation or draft execution triggering while `ambiguityScore >= 0.45` `[AC-03]` or unconfirmed DAG conflicts `[AC-04]` | 5 | 1 | 1 | 5 | Hard backend + UI boundary guard: `status === 'clarifying'` and unconfirmed conflicts keep mutation CTAs locked (`409`) until explicit user resolution. |
| **FMEA-05.1-03** | DOMParser XSS escape via `<script>` / `<svg>` / CSS `url()` or `'unsafe-inline'` CSP & 32-bit FNV hash collisions (`html-sandbox-wrapper.ts`) | 5 | 2 | 1 | 10 | Extended DOMParser sanitization blocklist (`script, iframe, object, embed, foreignobject, svg, template, noscript`), per-render CSPRNG CSP `'nonce-...'`, and async Web Crypto SHA-256 precomputation. |
| **FMEA-05.1-04** | Global `useBridgeEpochRotation` tearing down `MessageChannel` on node switch, `display: none` collapsing `elementRects`, or LRU memory leak | 4 | 2 | 1 | 8 | Refactor to `IframeKeepAlivePool` (max 5 slots, key `${projectId}::${nodeId}::${sha256}`, `visibility: hidden` for inactive slots, explicit `port1.close()` + `srcDoc = ''` eviction). |
| **FMEA-05.1-05** | `V3` CHECK constraint rejecting `'loopback'` edge or unlimited infinite cycle in `flow-graph` | 4 | 2 | 1 | 8 | Additive `V9` rebuild allowing `'loopback'` only with `max_loop_iterations` `1..10` and explicit confirmation while blocking non-loopback cycles. |
| **FMEA-05.1-06** | Dirty inline `</>` buffer overwritten silently on substate/node switch (`[EC-STORYBOARD-001]`) or malicious Git hook execution in Path 4 | 5 | 1 | 1 | 5 | Dirty-buffer prompt before frame switch + Path 4 jailed `git -c core.hooksPath=/dev/null -c core.fsmonitor=false -c protocol.file.allow=never`. |

---

## 2. Adversarial Boundary Verification
- **RPN Threshold:** All RPN scores `<= 16` (far below critical `100` threshold).
- **Static Analysis & Styling Parity:** `AstLinter::walk_jsx_element` extended for `*ClassName`/`cn(...)` and zero Tailwind utility classes across `src/features/sdi/components/**`.
- **Verdict:** Remediation actions validated for Story `05.1` vertical slice.

---

## 3. Audit-360 Post-Hardening Delta Review (Reviewer `ff0ec11e-a43c-4101-93bc-20394cb68b20`)
- **Round 4 Verdict:** `VERDICT: APPROVED` — Score: **9.3 / 10**
- **Verified Fixes:**
  - `src/features/flow/utils/html-sandbox-bridge.ts`, `html-sandbox-wrapper.ts`, `useSandboxedIframeBridge.ts`: `safeInlineJson` escaping (`<`, `>`, `&`, `\u2028`, `\u2029`) + `NONCE_SAFE_RE`, named hashing constants, localized `subStateLabels`.
  - `src/features/flow/components/CompanionLiveHtmlRightPane.tsx` & `companion-canvas-actions.ts`: `useLiveHtmlPaneController` + subcomponent decomposition, localized `fallbackScreenData`, node-filtered `useSampleFixtureSyncEffect`, `submitLaunchpadProject` (`"cowok:project-create-error"`), and `applyStudioSampleFixture` (`"cowok:sample-fixture-error"` + `UUID_RE`).
  - `src/pages/workbench/hooks/useWorkbenchSessionSettings.ts`: all helpers/hooks decomposed (`<= 46` lines), `ProjectDtoSchema.safeParse`, session rollback, and hydration merge guard.
