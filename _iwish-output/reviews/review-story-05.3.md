# Edge Case Guardian & Adversarial Review Report — Story 05.3

- **Story:** Story 05.3 — Immutable Turn Snapshots, Conversation Forking, Real GitHub PR, cowok-handoff-manifest.json & 1-Click Open in IDE
- **Epic:** Epic 05 (VS-02 Flow-First Executable Design Workbench & SDI Engine)
- **Reviewer:** Review Agent (Edge Case Guardian Skill)
- **Verdict:** APPROVED (PASS)

## 1. FMEA & Boundary Evaluation
- **EC-EXPORT-001 (ZIP Secret Exclusion & Completeness):** Export collector strips `.env*`, `.npmrc`, `.netrc`, `.git-credentials`, `*.pem`, `*.key` via `fs_jail` and always guarantees `tokens.css`, `icon-manifest.json`, `package.json`, fixtures, and `cowok-handoff-manifest.json` with `u32::MAX` overflow checks.
- **EC-EXPORT-002 (Tailwind Purge in Snapshots Modals):** Zero Tailwind utility classes permitted anywhere under `src/features/snapshots/**`; all layout/surfaces use semantic `.snap-*` classes in `src/styles/tokens.css`.
- **EC-PR-001 (Real GitHub PR vs Mock Deletion & Git Ref Injection):** `prSequenceCounter` completely removed; git ref names strictly regex-validated against flag injection; non-interactive `git -c core.hooksPath=/dev/null` and `gh` CLI fail closed with `501 GH_CLI_MISSING`/`GH_UNAUTHENTICATED`/`GIT_REMOTE_MISSING` when offline.
- **EC-REVERT-001 (DB-First Revert & Deleted-Node Turn Guard):** Atomic SQLite transaction writes `pre_revert_safety` before touching jailed disk files and enforces `409 NODE_DELETED_REQUIRES_GRAPH_RESTORE` on deleted nodes.
- **Complexity Score (CS):** `5.0/10` (< 7 threshold — No story split required).

## 2. Incremental Stage 4 Plan-Blind OCR Review (Redesign & Integrations)
Audited 16 incremental source/style files (`github_ide_integrations.rs`, `doctor_vault.rs`, `github-ide-integration.ts`, `version-snapshot.ts`, `src/api/version-snapshot.ts`, `VersionPublishMenuButton.tsx`, `SnapshotDetailInspector.tsx`, `GitHubAccountSection.tsx`, `HandoffManifestPanel.tsx`, `ExportBundleModal.tsx`, `DrawerSidebar.tsx`, `VersionHistoryDrawer.tsx`, `useSnapshotWorkspaceController.ts`, `AgentChatPanel.tsx`, `CompanionCanvasTab.tsx`, `src/styles/tokens.css` lines `793-810` & `1624-2050`+) against SSRF/Vault/path-jail/command-injection, Zod contract parity, and DESIGN.md Pure CSS rules:

1. **SSRF, 0o600 Vault Custody, Path-Jail & Command-Injection (`github_ide_integrations.rs`, `doctor_vault.rs`) — PASS (0 Blockers):**
   - `verify_github_token_live` pins DNS via `resolve_and_pin_ssrf_addr("https://api.github.com/user")` (`validate_ssrf_safe_url`, `check_ssrf_socket_addr` on all resolved IPs, non-loopback `https://` enforcement, `reqwest` `.resolve()` + `Policy::none()` no-redirect + 4000ms timeout).
   - GitHub tokens (`github:<login>`, `github`, `github_accounts_json`) are sealed in AES-256-GCM + Argon2id `.cowok-design/vault.enc` (`O_NOFOLLOW | O_CREAT | O_EXCL` `0o600` atomic `renameat`), with master key stored separately under `~/.config/cowok-design/keys/` and only `maskedHint` returned to UI.
   - `verify_anti_idor_with_caller` enforces caller + tenant/project ownership across all 5 `/api/v1/projects/:id/integrations/*` endpoints; `git`/`gh` run via `run_bounded_cmd_with_pgid` (no shell, `-c core.hooksPath=/dev/null`, bounded PGID); `validate_remote_url_safe` rejects leading `-`, whitespace/control characters, and restricts remotes to `https://` or `git@`; `handle_set_default_ide` allowlists `antigravity | cursor | vscode | claude-code`.
   - *Non-blocking hardening notes (Low):* `write_ide_pref`/`read_ide_pref` (`github_ide_integrations.rs:119-140`) operate on daemon `ws_root/.cowok-design/ide-preferences.json` without `symlink_metadata` / read-allowlist validation; `validate_remote_url_safe` (`277-297`) can additionally disallow `git@-...` (leading hyphen after `git@`) as defense-in-depth.

2. **Zod & Rust DTO Contract Parity (`github-ide-integration.ts`, `version-snapshot.ts`, `src/api/version-snapshot.ts`, `vite-bff-routes.ts`) — PASS (0 Blockers):**
   - All `github_ide_integrations.rs` camelCase DTOs and request payloads match `github-ide-integration.ts` schemas, Vite BFF `proxyRoute` entries (`vite-bff-routes.ts:134-138`, `strict-loopback` + `safeEncodeSegment`), and `VersionSnapshotClient` SDK methods.
   - `VersionSnapshotDtoSchema` aligns with Rust `VersionSnapshotDto` (`SnapshotTriggerSchema` includes `"pre_apply_safety"` matching `legacy_proposals.rs:2797` and `DrawerSidebar.tsx:34,42,298`; `metadata: z.record(z.unknown()).nullable().optional()` matches Rust `Option<HashMap<String, String>>`).

3. **Pure CSS DESIGN.md Compliance, Portal Cleanup & Slide Deck Guard — PASS (0 Blockers):**
   - Zero Tailwind utility classes in `src/features/snapshots/**` and zero hardcoded `#0b0e14`, `#6366f1`, or `#ff9a2f` hex values in `src/styles/tokens.css` or `src/features/snapshots/**`; `--cp-*` (`tokens.css:793-810`) and `--snap-*` (`tokens.css:1627-2050+`) map strictly to DESIGN.md semantic tokens.
   - `VersionPublishMenuButton.tsx` mounts its `document.body` portal strictly when `open && coords`, unmounts cleanly on dismiss/action/unmount, and cleans up `document` `mousedown`/`keydown` listeners in `useDropdownDismiss`.
   - `CompanionCanvasTab.tsx:522` (`effectiveInspectorFormat`) + `SnapshotDetailInspector.tsx:69-77,102-115,171-174` hide the `Userflow` tab and grid when `artifactFormat === "slide-cms"`, coerce any stale `"userflow"` mode to `"html"`, and label the tab `Slide 16:9`.
   - `AgentChatPanel.tsx` cleanly surfaces `Feedback (T)` and `Proposals (N)` controls across both collapsed rail and expanded header.

## 3. Incremental Delta Review (Reviewer `7fc0afed-30e9-461c-bd2c-433faf5be819`)
Audited post-refactor delta (`CompanionDualPaneStage.tsx`, `CompanionCanvasTab.tsx`, `CompanionLiveHtmlRightPane.tsx`, `usePreviewPaneActions.ts`, `CanvasProposalControls.tsx`, `snapshot-engine.ts`) and verified all 5 historical read-only / type-safety findings resolved (`0` remaining blockers):
- `[HIGH-1]`: `isReadOnly={isHistoricalReadOnly}` propagated into `CompanionLiveHtmlRightPane` and `usePreviewPaneActions` (`handleAddComment`, `handleResolveComment`, `handleReanchorComment`, `handleSelectFixtureVariant`, `handlePersistViewport` short-circuit when `isReadOnly === true`).
- `[HIGH-2]`: `CanvasProposalDrawerHost` and `CompanionOverlaysHost` enforce `isHistoricalReadOnly` across `onOpenInlineEditor`, `onCreateQuickProposal`, `onApply`, `onReject`, `onRebaseAndValidate`, `onComputeVisualDiff`, `onLaunchPath`, and `onApplyFixture`.
- `[MEDIUM-1]`: `useDualPanePreviewModel` isolates `customRawHtml` in historical read-only mode so live `customHtmlByTarget` edits cannot leak when a turn manifest omits `.html`.
- `[LOW-1]/[LOW-2]`: Tightened `parseTurnManifestFiles` field validation (`path.length > 0`, optional `sha256`/`sizeBytes`), hoisted `EMPTY_ELEMENT_RECTS`, and removed dead `onGuardedNodeSelect`.

## 4. Final Verdict
**VERDICT: PASS / APPROVED** (`0` critical/high findings; verified by `38652a3c` + `7fc0afed` + Vitest `140/140` + Rust `cargo test --workspace --locked`)
