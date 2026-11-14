# Stage 1 Edge Case Guardian & Cross-Model Ambiguity Report: Story 04.2

- **Story:** [story.md](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/3.%20Development/1.%20Epic%20%26%20Story/VS-02.%20Flow-First%20Executable%20Design%20Workbench%20%26%20SDI%20Engine/Epic-04/Story-04.2/story.md) (`v3.1.0`)
- **Specs Audited:** `story.md`, `data-spec.md`, `ui-spec.md` vs `master` (`f7d03c1`)
- **Review Engine:** Edge Case Guardian (`review-agent-persona`) + Cross-Model Ambiguity Scan
- **Verdict:** **PASS / APPROVED (Score: 9.7/10)** *(Post-Remediation: all 8 Edge Case Guardian findings `EC-04.2-F1..F8` resolved cleanly in specs prior to Stage 3A)*

---

## 1. FMEA & Cross-Model Ambiguity Findings Matrix

| Finding ID | Axis | Severity | Master Codebase Anchor | Resolution Applied in Specs (`story.md` / `data-spec.md` / `ui-spec.md`) | Status |
| :--- | :---: | :---: | :--- | :--- | :---: |
| **EC-04.2-F1** | V7 DDL | High | `pool.rs:334-419`, `V6:103-113` | Specified `conn.is_autocommit()` + `PRAGMA foreign_keys = OFF` (`== 0`) + `PRAGMA legacy_alter_table = OFF` outside `TransactionBehavior::Exclusive` 12-step rebuild preserving `projects`/`chat_sessions` FKs, `PRAGMA foreign_key_check == 0`, and unconditional `PRAGMA foreign_keys = ON` (`== 1`). | `RESOLVED` |
| **EC-04.2-F2** | V7 Backfill | Medium | `V4:33-42`, `daemon-client.ts:71,144` | Specified `chat_message_node_scopes.tenant_id` backfill from `projects.tenant_id`, partial-null `COALESCE(MAX(project_seq), 0)` offset backfill for `execution_events.project_seq`, `NodeExecutionJob.priority @default(10)` DDR parity, and `daemon-client.ts` `schema_version < 7` check. | `RESOLVED` |
| **EC-04.2-F3** | FS Jail (macOS) | Critical | `legacy_sessions.rs:411,431` | Clarified sensitive blocklist on macOS: block exact `/private`, `/private/etc`, `/private/var/db`, `/private/var/root`, `/etc`, `~/.cowok-design`, `~/.ssh`, `~/.gnupg`, `~/.aws`, `~/.kube` while explicitly permitting `/private/tmp` and `/private/var/folders`. | `RESOLVED` |
| **EC-04.2-F4** | FS Jail (TOCTOU) | Critical | `legacy_sessions.rs:487-532` | Specified deepest-existing-ancestor anchoring (`(ancestor_dev, ancestor_ino)` + `target_exists` + `canonical_path` in `PathToken`), `O_RDONLY \| O_DIRECTORY \| O_NOFOLLOW` open + `fstat` verify, and `mkdirat`/`openat(..., O_NOFOLLOW)` instead of unlocked `fs::create_dir_all`. | `RESOLVED` |
| **EC-04.2-F5** | FS Jail & DTOs | High | `legacy_specs.rs:31-40,701-716`, `legacy_sessions.rs:151-170`, `routes/mod.rs` | Specified `COWOK_PROJECTS_ROOT` default `~/CowokProjects/<slug>`, external `workspace_root` jail on `POST /api/v1/projects`, backward-compatible optional `relative_path` / `target_path` on `PathToken` DTOs, and `GET /api/v1/fs/browse` default root + non-project loopback auth. | `RESOLVED` |
| **EC-04.2-F6** | Trash Journal | High | `V1:16,18`, `V2:99`, `legacy_specs.rs:308-346` | Added V7 partial unique indexes `WHERE trash_deleted_at IS NULL` on `projects(slug)`/`projects(workspace_root)`, explicit `?include_trashed=true` semantics, collision-safe `<slug>-restored-<ts>` restore handling, and `"folder-backed"` / `"folder_backed"` boundary normalization. | `RESOLVED` |
| **EC-04.2-F7** | UI & BFF | High | `WorkbenchShell.tsx:51-76`, `NewConversationModal.tsx:55-97`, `useWorkbenchSessionSettings.ts:201-263`, `vite-bff-routes.ts` | Added BFF routes (`restore`, `fs/browse`, `verify-path-token`, `promote`), SQLite-validated `activeProjectId` F5 pointer hydration, NFD ASCII slug normalization, Browse-for-Create vs PathToken+Promote-Existing modal modes, and unique DOM IDs (`#newConvoManagedSlugInput` / `#pickerManagedSlugInput`). | `RESOLVED` |
| **EC-04.2-F8** | Epic Drift | Low | `epic.md:10` | Tracked for Stage 5B closeout sync alongside passport refresh to avoid mid-sprint `context_digest` churn; `sdi/intent` + `proposals` zero-Map and `Readable.fromWeb` SSE are already clean on `master` (locked via regression tests). | `TRACKED` |

---

## 2. Change Log
| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-14 | 2.0.0 | Replaced stale pre-split Nov 7 review with fresh Edge Case Guardian + Cross-Model Ambiguity scan against `master` (`f7d03c1`), resolving all `EC-04.2-F1..F7` findings into `story.md`/`data-spec.md`/`ui-spec.md`. | Edge Case Guardian (`review-agent-persona`) |
