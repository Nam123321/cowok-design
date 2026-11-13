// @story 01.3
// @implements AC-02, AC-03, AC-05, AC-07
use std::fs;
use std::path::{Path, PathBuf};

use domain::{CanonicalSpecKey, HarnessError, ProvenanceStatus};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spec_registry::{
    map_sqlite_error, read_jailed_spec_bytes, SpecRecord, SpecRegistry, SqlitePool,
    SqliteWalConfig, UpsertSpecInput,
};
use uuid::Uuid;

use crate::lockfile::{reject_if_symlink, validate_and_canonicalize_workspace_root};

pub const SPARSE_BRIEF_MIN_CHARS: usize = 40;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SynthesizeSpecsRequest {
    pub product_brief: Option<String>,
    pub seed_repo_path: Option<String>,
    pub inherit_brand_from_project_id: Option<Uuid>,
    pub actor_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SynthesizedSpecItemDto {
    pub spec_key: CanonicalSpecKey,
    pub status: ProvenanceStatus,
    pub version: u32,
    pub content_sha256: String,
    pub confirmed_count: u32,
    pub inferred_count: u32,
    pub missing_count: u32,
    pub source_project_id: Option<Uuid>,
    pub source_revision_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SynthesizeSpecsResponse {
    pub project_id: Uuid,
    pub sparse_brief_detected: bool,
    pub total_confirmed_count: u32,
    pub total_inferred_count: u32,
    pub total_missing_count: u32,
    pub open_questions: Vec<String>,
    pub specs: Vec<SynthesizedSpecItemDto>,
}

/// Resolves and canonicalizes the isolated filesystem workspace root for `project_id`,
/// returning a `SpecRegistry` scoped to that project's `workspace_root` while sharing
/// the daemon's embedded SQLite WAL database.
pub fn resolve_project_scoped_registry(
    base_registry: &SpecRegistry,
    project_id: Uuid,
) -> Result<(SpecRegistry, PathBuf), HarnessError> {
    let conn = base_registry.pool().connection()?;
    let row: Option<String> = conn
        .query_row(
            "SELECT workspace_root FROM projects WHERE id = ?1",
            params![project_id.to_string()],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "resolve_project_scoped_registry"))?;

    let db_root_str = row.ok_or_else(|| HarnessError::ProjectNotFound {
        project_id: project_id.to_string(),
    })?;

    let daemon_canon_root =
        validate_and_canonicalize_workspace_root(base_registry.pool().workspace_root())?;

    let raw_proj_path = if db_root_str == "." || db_root_str.trim().is_empty() {
        daemon_canon_root.clone()
    } else {
        PathBuf::from(&db_root_str)
    };

    if !raw_proj_path.exists() {
        fs::create_dir_all(&raw_proj_path).map_err(|e| HarnessError::ValidationFailed {
            reason: format!(
                "PROJECT_WORKSPACE_CREATE_FAILED [{}]: {e}",
                raw_proj_path.display()
            ),
        })?;
    }

    let proj_canon_root = validate_and_canonicalize_workspace_root(&raw_proj_path)?;

    // AC-07 / F-01: Check boundary_type in project_boundaries.
    // If project is folder_backed (promoted from scratch or linked to an external folder repo),
    // allow external workspace root outside daemon_canon_root.
    let boundary_type: Option<String> = conn
        .query_row(
            "SELECT boundary_type FROM project_boundaries WHERE project_id = ?1",
            params![project_id.to_string()],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "resolve_project_scoped_registry_boundary_check"))?;

    let is_folder_backed = boundary_type.as_deref() == Some("folder_backed");

    if !is_folder_backed && !proj_canon_root.starts_with(&daemon_canon_root) {
        return Err(HarnessError::ValidationFailed {
            reason: format!(
                "PROJECT_SCOPE_VIOLATION: project workspace '{}' escapes daemon root '{}'",
                proj_canon_root.display(),
                daemon_canon_root.display()
            ),
        });
    }

    ensure_project_token(&proj_canon_root)?;

    let mut cfg: SqliteWalConfig = base_registry.pool().config().clone();
    cfg.workspace_root = proj_canon_root.clone();
    let scoped_pool = SqlitePool::open_wal(cfg)?;
    Ok((SpecRegistry::new(scoped_pool), proj_canon_root))
}

/// Ensures that `.cowok-design/project.token` exists inside `proj_canon_root` with POSIX `0600` permissions.
/// Returns the secret token string.
pub fn ensure_project_token(proj_canon_root: &Path) -> Result<String, HarnessError> {
    let token_dir = proj_canon_root.join(".cowok-design");
    if !token_dir.exists() {
        fs::create_dir_all(&token_dir).map_err(|e| HarnessError::ValidationFailed {
            reason: format!(
                "Failed to create token directory '{}': {e}",
                token_dir.display()
            ),
        })?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&token_dir, fs::Permissions::from_mode(0o700));
    }

    let token_file = token_dir.join("project.token");
    if let Ok(meta) = fs::symlink_metadata(&token_dir) {
        if meta.file_type().is_symlink() {
            return Err(HarnessError::PathTraversalDenied {
                path: token_dir.display().to_string(),
            });
        }
    }
    if let Ok(meta) = fs::symlink_metadata(&token_file) {
        if meta.file_type().is_symlink() {
            return Err(HarnessError::PathTraversalDenied {
                path: token_file.display().to_string(),
            });
        }
    }
    if !token_file.exists() {
        let secret = format!("tok_{}", Uuid::new_v4().simple());
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut opts = fs::OpenOptions::new();
            opts.write(true)
                .create_new(true)
                .custom_flags(libc::O_NOFOLLOW)
                .mode(0o600);
            let mut f = opts
                .open(&token_file)
                .map_err(|e| HarnessError::ValidationFailed {
                    reason: format!(
                        "Failed to create token file '{}': {e}",
                        token_file.display()
                    ),
                })?;
            f.write_all(secret.as_bytes())
                .map_err(|e| HarnessError::ValidationFailed {
                    reason: format!("Failed to write token file '{}': {e}", token_file.display()),
                })?;
            f.sync_all().map_err(|e| HarnessError::ValidationFailed {
                reason: format!("Failed to sync token file '{}': {e}", token_file.display()),
            })?;
        }
        #[cfg(not(unix))]
        {
            let mut opts = fs::OpenOptions::new();
            opts.write(true).create_new(true);
            use std::io::Write;
            let mut f = opts
                .open(&token_file)
                .map_err(|e| HarnessError::ValidationFailed {
                    reason: format!(
                        "Failed to create token file '{}': {e}",
                        token_file.display()
                    ),
                })?;
            f.write_all(secret.as_bytes())
                .map_err(|e| HarnessError::ValidationFailed {
                    reason: format!("Failed to write token file '{}': {e}", token_file.display()),
                })?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&token_file, fs::Permissions::from_mode(0o600)).map_err(|e| {
                HarnessError::ValidationFailed {
                    reason: format!("Failed to set 0600 permissions on project.token: {e}"),
                }
            })?;
        }
        Ok(secret)
    } else {
        #[cfg(unix)]
        {
            use std::io::Read;
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            fs::set_permissions(&token_file, fs::Permissions::from_mode(0o600)).map_err(|e| {
                HarnessError::ValidationFailed {
                    reason: format!("Failed to enforce 0600 permissions on project.token: {e}"),
                }
            })?;
            let mut f = fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&token_file)
                .map_err(|e| HarnessError::PathTraversalDenied {
                    path: format!("{} ({e})", token_file.display()),
                })?;
            let mut s = String::new();
            f.read_to_string(&mut s)
                .map_err(|e| HarnessError::ValidationFailed {
                    reason: format!("Failed to read project.token: {e}"),
                })?;
            return Ok(s.trim().to_string());
        }
        #[cfg(not(unix))]
        {
            fs::read_to_string(&token_file)
                .map(|s| s.trim().to_string())
                .map_err(|e| HarnessError::ValidationFailed {
                    reason: format!("Failed to read project.token: {e}"),
                })
        }
    }
}

/// Validates an optional `seed_repo_path`, rejecting traversal (`..`) and symlinks (`EC-E01-03`),
/// and returns its canonicalized path inside `workspace_root`.
pub fn validate_seed_repo_path(
    workspace_root: &Path,
    seed_repo_path: &str,
) -> Result<PathBuf, HarnessError> {
    let trimmed = seed_repo_path.trim();
    if trimmed.is_empty() {
        return Err(HarnessError::ValidationFailed {
            reason: "SEED_REPO_PATH_EMPTY: seed_repo_path must not be empty".to_string(),
        });
    }
    let candidate = Path::new(trimmed);
    for comp in candidate.components() {
        if matches!(comp, std::path::Component::ParentDir) {
            return Err(HarnessError::PathTraversalDenied {
                path: trimmed.to_string(),
            });
        }
    }
    let resolved = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        workspace_root.join(candidate)
    };
    reject_if_symlink(&resolved)?;
    let canon_root =
        workspace_root
            .canonicalize()
            .map_err(|_| HarnessError::PathTraversalDenied {
                path: workspace_root.display().to_string(),
            })?;
    let canon_seed = resolved
        .canonicalize()
        .map_err(|_| HarnessError::ValidationFailed {
            reason: format!(
                "SEED_REPO_NOT_FOUND: '{}' does not exist",
                resolved.display()
            ),
        })?;
    if !canon_seed.starts_with(&canon_root) {
        return Err(HarnessError::PathTraversalDenied {
            path: trimmed.to_string(),
        });
    }
    Ok(canon_seed)
}

/// Extracts repository context from a validated `seed_repo_dir` (`README.md`, `package.json`,
/// `Cargo.toml`) while rejecting symlinks (`O_NOFOLLOW`/symlink check).
fn extract_seed_repo_context(seed_repo_dir: &Path) -> Result<String, HarnessError> {
    reject_if_symlink(seed_repo_dir)?;
    let mut parts = Vec::new();
    for candidate_name in ["README.md", "package.json", "Cargo.toml"] {
        let file_path = seed_repo_dir.join(candidate_name);
        if let Ok(meta) = fs::symlink_metadata(&file_path) {
            if meta.file_type().is_symlink() {
                return Err(HarnessError::PathTraversalDenied {
                    path: file_path.display().to_string(),
                });
            }
            if meta.file_type().is_file() {
                if let Ok(text) = fs::read_to_string(&file_path) {
                    let snippet: String = text.trim().chars().take(240).collect();
                    if !snippet.is_empty() {
                        parts.push(format!("[{candidate_name}]: {snippet}"));
                    }
                }
            }
        }
    }
    if parts.is_empty() {
        let dir_name = seed_repo_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("seeded-repo");
        Ok(format!("Seeded repository directory '{dir_name}'"))
    } else {
        Ok(parts.join(" | "))
    }
}

/// Counts `[CONFIRMED]`, `[INFERRED]`, and `[MISSING]` provenance markers in a synthesized spec.
pub fn count_provenance_markers(markdown: &str) -> (u32, u32, u32) {
    let confirmed = markdown.matches("[CONFIRMED]").count() as u32;
    let inferred = markdown.matches("[INFERRED]").count() as u32;
    let missing = markdown.matches("[MISSING]").count() as u32;
    (confirmed, inferred, missing)
}

/// Synthesizes all 9 Foundational Project Context Specs (`01-product-overview.md` .. `09-data-and-copy.md`)
/// with deterministic provenance tagging (`[CONFIRMED]`, `[INFERRED]`, `[MISSING]`), cross-project brand
/// reuse (`04-architecture.md` / design direction & `08-security-tenancy.md` / brand guidelines), and
/// idempotent no-op when content is unchanged.
pub fn synthesize_nine_specs(
    base_registry: &SpecRegistry,
    project_id: Uuid,
    req: &SynthesizeSpecsRequest,
) -> Result<SynthesizeSpecsResponse, HarnessError> {
    let (project_registry, _) = resolve_project_scoped_registry(base_registry, project_id)?;
    let daemon_canon_root =
        validate_and_canonicalize_workspace_root(base_registry.pool().workspace_root())?;

    let seed_context = if let Some(ref seed_path) = req.seed_repo_path {
        let canon_seed = validate_seed_repo_path(&daemon_canon_root, seed_path)?;
        Some(extract_seed_repo_context(&canon_seed)?)
    } else {
        None
    };

    let (db_brief, db_inherit_proj) =
        load_project_brief_and_lineage(&project_registry, project_id)?;
    let explicit_brief = req
        .product_brief
        .as_deref()
        .unwrap_or(&db_brief)
        .trim()
        .to_string();

    if explicit_brief.is_empty() && seed_context.is_none() {
        return Err(HarnessError::ValidationFailed {
            reason: "BRIEF_EMPTY: product_brief must not be empty when seed_repo_path is omitted"
                .to_string(),
        });
    }

    let effective_brief = match (&explicit_brief.is_empty(), &seed_context) {
        (false, Some(seed_txt)) => format!("{explicit_brief} (Seed Repo Context: {seed_txt})"),
        (true, Some(seed_txt)) => format!("Seed Repo Context: {seed_txt}"),
        _ => explicit_brief,
    };

    let inherit_from = req.inherit_brand_from_project_id.or(db_inherit_proj);
    if let Some(upstream_id) = inherit_from {
        verify_upstream_project_exists(&project_registry, upstream_id)?;
    }

    let is_sparse = effective_brief.chars().count() < SPARSE_BRIEF_MIN_CHARS;
    let open_questions = if is_sparse {
        vec![
            "What are the primary target user personas and core user journeys?".to_string(),
            "Which visual design preset, brand palette, and typography tokens should be enforced?"
                .to_string(),
            "Which multi-portal surfaces (Web Workbench, Presentation Deck, Executive Report) are in scope?"
                .to_string(),
        ]
    } else {
        Vec::new()
    };

    let actor_id = req
        .actor_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("cowok-synthesizer")
        .to_string();

    let mut total_confirmed = 0u32;
    let mut total_inferred = 0u32;
    let mut total_missing = 0u32;
    let mut out_specs = Vec::with_capacity(CanonicalSpecKey::ALL.len());

    for spec_key in CanonicalSpecKey::ALL {
        let is_brand_inheritable = matches!(
            spec_key,
            CanonicalSpecKey::UiUxSpec | CanonicalSpecKey::DataAndCopy
        );

        let (markdown, src_proj_id, src_rev_sha, status_override) =
            if let (true, Some(upstream_id)) = (is_brand_inheritable, inherit_from) {
                match load_upstream_spec_content(base_registry, upstream_id, spec_key)? {
                    Some((up_md, up_sha)) => (
                        up_md,
                        Some(upstream_id),
                        Some(up_sha),
                        Some(ProvenanceStatus::Confirmed),
                    ),
                    None => {
                        let md = render_spec_template(spec_key, &effective_brief, is_sparse);
                        (md, None, None, None)
                    }
                }
            } else {
                let md = render_spec_template(spec_key, &effective_brief, is_sparse);
                (md, None, None, None)
            };

        let (c_cnt, i_cnt, m_cnt) = if is_sparse && src_proj_id.is_none() {
            let (_, ic, mc) = count_provenance_markers(&markdown);
            (0, ic, mc)
        } else {
            count_provenance_markers(&markdown)
        };

        let status = status_override.unwrap_or(if is_sparse {
            if m_cnt > 0 && i_cnt == 0 {
                ProvenanceStatus::Missing
            } else {
                ProvenanceStatus::Inferred
            }
        } else if c_cnt > 0 {
            ProvenanceStatus::Confirmed
        } else {
            ProvenanceStatus::Inferred
        });

        total_confirmed += c_cnt;
        total_inferred += i_cnt;
        total_missing += m_cnt;

        let prov_json = serde_json::json!({
            "origin": "synthesis_pipeline",
            "actor": actor_id,
            "CONFIRMED": c_cnt,
            "INFERRED": i_cnt,
            "MISSING": m_cnt,
        })
        .to_string();

        // Check existing spec for idempotency (`SYN-04`): if SHA-256 is identical, skip write
        let new_sha = compute_sha256_hex(markdown.as_bytes());
        let existing_spec: Option<SpecRecord> = project_registry.get_spec(project_id, spec_key)?;

        let record = match existing_spec {
            Some(ref cur)
                if cur.content_sha256 == new_sha
                    && cur.source_project_id == src_proj_id
                    && cur.source_revision_sha256 == src_rev_sha =>
            {
                cur.clone()
            }
            Some(ref cur) => {
                let (rec, _) = project_registry.upsert_spec(UpsertSpecInput {
                    project_id,
                    spec_key,
                    markdown_content: markdown,
                    expected_sha256: Some(cur.content_sha256.clone()),
                    status,
                    provenance_summary_json: prov_json,
                    source_project_id: src_proj_id,
                    source_revision_sha256: src_rev_sha.clone(),
                    actor_kind: "synthesis_pipeline".to_string(),
                    actor_id: actor_id.clone(),
                    change_summary: format!("Synthesized {}", spec_key.as_str()),
                })?;
                rec
            }
            None => {
                let (rec, _) = project_registry.upsert_spec(UpsertSpecInput {
                    project_id,
                    spec_key,
                    markdown_content: markdown,
                    expected_sha256: None,
                    status,
                    provenance_summary_json: prov_json,
                    source_project_id: src_proj_id,
                    source_revision_sha256: src_rev_sha.clone(),
                    actor_kind: "synthesis_pipeline".to_string(),
                    actor_id: actor_id.clone(),
                    change_summary: format!("Initial synthesis of {}", spec_key.as_str()),
                })?;
                rec
            }
        };

        out_specs.push(SynthesizedSpecItemDto {
            spec_key,
            status: record.status,
            version: record.version,
            content_sha256: record.content_sha256,
            confirmed_count: c_cnt,
            inferred_count: i_cnt,
            missing_count: m_cnt,
            source_project_id: record.source_project_id,
            source_revision_sha256: record.source_revision_sha256,
        });
    }

    Ok(SynthesizeSpecsResponse {
        project_id,
        sparse_brief_detected: is_sparse,
        total_confirmed_count: total_confirmed,
        total_inferred_count: total_inferred,
        total_missing_count: total_missing,
        open_questions,
        specs: out_specs,
    })
}

fn render_spec_template(spec_key: CanonicalSpecKey, brief: &str, is_sparse: bool) -> String {
    let title = spec_key.as_str();
    let domain_section = match spec_key {
        CanonicalSpecKey::ProductOverview => (
            "Product Scope, Personas & Value Proposition",
            "- User Personas, Core Jobs-to-be-Done, and Success Metrics",
            "Primary target personas and measurable north-star KPIs require product confirmation.",
        ),
        CanonicalSpecKey::DatabaseSpec => (
            "Database Schema, WAL Persistence & Sync Contracts",
            "- Relational Tables, Foreign Keys, Indexes & Optimistic Concurrency",
            "Entity retention policies and tenant isolation indices need schema verification.",
        ),
        CanonicalSpecKey::UiUxSpec => (
            "Visual Design System, Palette & Typography Tokens",
            "- Accessible WCAG 2.2 AA contrast tokens, typography scale & Flagship Preset",
            "Brand accent palette, typography family selection, and motion preferences are not locked.",
        ),
        CanonicalSpecKey::Architecture => (
            "System Architecture & Storage Boundaries",
            "- Loopback Rust Daemon + SQLite WAL + React/Vite BFF Isolation",
            "Project-specific deployment topology, scaling limits, and sync cadence need sign-off.",
        ),
        CanonicalSpecKey::EpicsAndStories => (
            "Execution Backlog, Epics & Vertical Story Slices",
            "- Prioritized Vertical Slice Backlog & Traceable Acceptance Criteria",
            "Sprint capacity ordering and non-MVP deferrals need PM prioritization.",
        ),
        CanonicalSpecKey::CrossDependencies => (
            "Cross-Surface Dependencies & Integration Matrix",
            "- Shared Package Contracts, Portals & External MCP/API Consumers",
            "External webhook SLAs and third-party dependency fallbacks are not yet specified.",
        ),
        CanonicalSpecKey::ApiGuidelines => (
            "API Contracts, Zod Schemas & BFF Routing Standards",
            "- Typed Request/Response DTOs, Idempotency Keys & RFC Problem Details",
            "Endpoint rate limits, pagination cursors, and deprecation headers need review.",
        ),
        CanonicalSpecKey::SecurityTenancy => (
            "Security Controls, Authentication & Tenant Isolation",
            "- Fail-Closed Loopback HMAC-SHA256 v2, Per-Project Token & Path Jail",
            "Project-specific RBAC matrix, secret rotation cadence, and compliance scope need review.",
        ),
        CanonicalSpecKey::DataAndCopy => (
            "Data Entities, Wording Slots & Locale Catalog",
            "- Zod Runtime Validation Schemas, Error Codes & Bilingual UI Copy",
            "Canonical brand terminology, tone guidelines, and i18n locale coverage are incomplete.",
        ),
    };

    if is_sparse {
        format!(
            "# {title}: {heading}\n\n\
             ## Domain Summary\n\
             - [INFERRED] Default `{title}` baseline synthesized from sparse brief: \"{brief}\"\n\
             - [INFERRED] Baseline scope covers: {inferred_item}\n\
             - [MISSING] {missing_item}\n",
            heading = domain_section.0,
            inferred_item = domain_section.1,
            missing_item = domain_section.2
        )
    } else {
        format!(
            "# {title}: {heading}\n\n\
             ## Verified Domain Contract\n\
             - [CONFIRMED] Product Brief Mandate: {brief}\n\
             - [CONFIRMED] Canonical Spec Key: `{title}`\n\
             - [INFERRED] Enforced focus area: {inferred_item}\n\
             - [INFERRED] Quality gates enabled: zero-trust boundary validation and WCAG 2.2 AA compliance.\n",
            heading = domain_section.0,
            inferred_item = domain_section.1
        )
    }
}

fn load_project_brief_and_lineage(
    registry: &SpecRegistry,
    project_id: Uuid,
) -> Result<(String, Option<Uuid>), HarnessError> {
    let conn = registry.pool().connection()?;
    let row: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT product_brief, inherit_brand_from_project_id FROM projects WHERE id = ?1",
            params![project_id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "load_project_brief_and_lineage"))?;

    let (brief, inherit_opt) = row.ok_or_else(|| HarnessError::ProjectNotFound {
        project_id: project_id.to_string(),
    })?;
    let parsed_inherit = inherit_opt.and_then(|s| Uuid::parse_str(&s).ok());
    Ok((brief, parsed_inherit))
}

fn verify_upstream_project_exists(
    registry: &SpecRegistry,
    upstream_id: Uuid,
) -> Result<(), HarnessError> {
    let conn = registry.pool().connection()?;
    let exists: Option<String> = conn
        .query_row(
            "SELECT id FROM projects WHERE id = ?1",
            params![upstream_id.to_string()],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "verify_upstream_project_exists"))?;

    if exists.is_none() {
        return Err(HarnessError::ValidationFailed {
            reason: format!(
                "REFERENTIAL_INTEGRITY: upstream brand project '{upstream_id}' does not exist"
            ),
        });
    }
    Ok(())
}

fn load_upstream_spec_content(
    base_registry: &SpecRegistry,
    upstream_id: Uuid,
    spec_key: CanonicalSpecKey,
) -> Result<Option<(String, String)>, HarnessError> {
    let (upstream_registry, upstream_workspace_root) =
        resolve_project_scoped_registry(base_registry, upstream_id)?;
    let Some(up_rec) = upstream_registry.get_spec(upstream_id, spec_key)? else {
        return Ok(None);
    };
    let up_sha = up_rec.content_sha256;

    let bytes = read_jailed_spec_bytes(&upstream_workspace_root, spec_key)?.ok_or_else(|| {
        HarnessError::ValidationFailed {
            reason: format!(
                "UPSTREAM_SPEC_MISSING_ON_DISK: upstream project '{upstream_id}' spec '{}' is missing on disk",
                spec_key.as_str()
            ),
        }
    })?;

    let disk_sha = compute_sha256_hex(&bytes);
    if disk_sha != up_sha {
        return Err(HarnessError::StaleFile {
            path: format!("product/specs/{}", spec_key.as_str()),
            expected_hash: up_sha,
            actual_hash: disk_sha,
        });
    }

    let text = String::from_utf8(bytes).map_err(|e| HarnessError::ValidationFailed {
        reason: format!(
            "UPSTREAM_SPEC_INVALID_UTF8: upstream project '{upstream_id}' spec '{}' is not valid UTF-8: {e}",
            spec_key.as_str()
        ),
    })?;

    Ok(Some((text, up_sha)))
}

fn compute_sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
