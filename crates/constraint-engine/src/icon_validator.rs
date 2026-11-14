// @story 03.2
// @implements AC-01, AC-02, AC-03
use domain::HarnessError;
use rusqlite::{params, Connection};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

pub use domain::{
    ComponentRegistryContract, ComponentRegistryEntry, ComponentValidationReport, IconManifest,
    IconManifestEntry, IconValidationReport, ProjectDesignContracts, TypographyContract,
    TypographyValidationReport,
};

/// Embedded canonical flagship 143-icon catalog (crate-local copy for isolated cargo/Docker builds).
pub const FLAGSHIP_ICON_MANIFEST_JSON: &str = include_str!("../assets/flagship-icon-manifest.json");

/// Sentinel value stored in `constraint_profiles.icon_manifest_path` to select the built-in flagship catalog.
pub const FLAGSHIP_MANIFEST_SENTINEL: &str = "builtin://flagship";

/// Hard upper bounds preventing O(m * n) manifest DoS vectors.
pub const MAX_MANIFEST_ICONS: usize = 2048;
pub const MAX_IDENTIFIER_LEN: usize = 64;
pub const MAX_COMPONENT_REGISTRY_ENTRIES: usize = 1024;

pub struct IconValidator;

impl IconValidator {
    /// Strips control, zero-width, and bidi override characters before trimming.
    pub fn sanitize_identifier(raw: &str) -> String {
        raw.chars()
            .filter(|&c| {
                !c.is_control()
                    && !matches!(
                        c,
                        '\u{200B}'..='\u{200F}' | '\u{FEFF}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{206F}'
                    )
            })
            .collect::<String>()
            .trim()
            .to_string()
    }

    /// Parses and strictly validates an icon manifest JSON string.
    /// Enforces entry count parity (`manifest.count == manifest.icons.len()`) and DoS bounds.
    pub fn parse_manifest(json_str: &str) -> Result<IconManifest, HarnessError> {
        let manifest = serde_json::from_str::<IconManifest>(json_str).map_err(|e| {
            HarnessError::ValidationFailed {
                reason: format!("Failed to parse icon manifest JSON: {}", e),
            }
        })?;

        if manifest.icons.len() > MAX_MANIFEST_ICONS {
            return Err(HarnessError::BoundaryViolation {
                reason: format!(
                    "ICON_MANIFEST_TOO_LARGE: {} entries exceeds maximum {}",
                    manifest.icons.len(),
                    MAX_MANIFEST_ICONS
                ),
            });
        }

        if manifest.count != manifest.icons.len() {
            return Err(HarnessError::ValidationFailed {
                reason: format!(
                    "ICON_MANIFEST_COUNT_MISMATCH: declared count ({}) does not match actual icons length ({})",
                    manifest.count,
                    manifest.icons.len()
                ),
            });
        }

        for entry in &manifest.icons {
            let clean = Self::sanitize_identifier(&entry.name);
            if clean.is_empty() || clean.chars().count() > MAX_IDENTIFIER_LEN {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "INVALID_MANIFEST_ICON_NAME: '{}' must be 1..={} characters",
                        entry.name, MAX_IDENTIFIER_LEN
                    ),
                });
            }
        }

        Ok(manifest)
    }

    /// Loads the embedded flagship cowok-brand-product-system-v4.3 143-icon catalog.
    pub fn get_flagship_manifest() -> IconManifest {
        Self::parse_manifest(FLAGSHIP_ICON_MANIFEST_JSON)
            .expect("Embedded flagship icon manifest must be valid JSON")
    }

    /// Loads an icon manifest from an arbitrary path (intended for verified local files/tests).
    /// Prefer `resolve_jailed_manifest_path` + `load_project_design_context` at daemon runtime.
    pub fn load_from_file(path: &Path) -> Result<IconManifest, HarnessError> {
        let content =
            std::fs::read_to_string(path).map_err(|e| HarnessError::ValidationFailed {
                reason: format!(
                    "Failed to read icon manifest from '{}': {e}",
                    path.display()
                ),
            })?;
        Self::parse_manifest(&content)
    }

    /// Strictly resolves a relative `icon_manifest_path` inside `project_root`.
    /// Rejects absolute paths, `..` traversal, missing files, and symlink escapes (FAIL-CLOSED).
    pub fn resolve_jailed_manifest_path(
        project_root: &Path,
        rel_path: &str,
    ) -> Result<PathBuf, HarnessError> {
        let cleaned = Self::sanitize_identifier(rel_path);
        if cleaned.is_empty() || rel_path.contains('\0') {
            return Err(HarnessError::BoundaryViolation {
                reason: "INVALID_MANIFEST_PATH: Path cannot be empty or contain null bytes"
                    .to_string(),
            });
        }

        let rel = Path::new(&cleaned);
        if rel.is_absolute() {
            return Err(HarnessError::BoundaryViolation {
                reason: format!(
                    "PATH_TRAVERSAL_BLOCKED: Absolute icon manifest path '{}' is forbidden",
                    cleaned
                ),
            });
        }
        for comp in rel.components() {
            match comp {
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(HarnessError::BoundaryViolation {
                        reason: format!(
                            "PATH_TRAVERSAL_BLOCKED: Icon manifest path '{}' cannot escape project root",
                            cleaned
                        ),
                    });
                }
                _ => {}
            }
        }

        let canon_root =
            project_root
                .canonicalize()
                .map_err(|e| HarnessError::BoundaryViolation {
                    reason: format!(
                        "PROJECT_ROOT_UNRESOLVABLE: Failed to canonicalize '{}': {e}",
                        project_root.display()
                    ),
                })?;

        let candidate = canon_root.join(rel);
        let canon_target = candidate.canonicalize().map_err(|e| {
            HarnessError::ValidationFailed {
                reason: format!(
                    "ICON_MANIFEST_NOT_FOUND: Configured manifest '{}' could not be resolved inside project: {e}",
                    cleaned
                ),
            }
        })?;

        if !canon_target.starts_with(&canon_root) {
            return Err(HarnessError::BoundaryViolation {
                reason: format!(
                    "SYMLINK_JAIL_ESCAPE_BLOCKED: '{}' resolves outside project root",
                    cleaned
                ),
            });
        }

        if !canon_target.is_file() {
            return Err(HarnessError::ValidationFailed {
                reason: format!(
                    "ICON_MANIFEST_NOT_A_FILE: '{}' is not a regular file",
                    cleaned
                ),
            });
        }

        Ok(canon_target)
    }

    /// Applies an optional `allowed_icon_set` restriction filter to a manifest.
    pub fn apply_allowed_icon_filter(
        mut manifest: IconManifest,
        allowed_set: &[String],
    ) -> IconManifest {
        if allowed_set.is_empty() {
            return manifest;
        }
        let allowed_norm: HashSet<String> = allowed_set
            .iter()
            .map(|s| Self::sanitize_identifier(s).to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        if allowed_norm.is_empty() {
            return manifest;
        }
        manifest
            .icons
            .retain(|entry| allowed_norm.contains(&entry.name.to_lowercase()));
        manifest.count = manifest.icons.len();
        manifest
    }

    fn load_raw_design_contracts(
        conn: &Connection,
        project_root: &Path,
        project_id: &str,
    ) -> Result<ProjectDesignContracts, HarnessError> {
        let profile_row: Option<(String, String, String, f64)> = conn
            .query_row(
                "SELECT icon_manifest_path, allowed_icon_set_json, typography_contract_json, min_wcag_contrast
                 FROM constraint_profiles
                 WHERE project_id = ?1
                 ORDER BY updated_at DESC
                 LIMIT 1",
                params![project_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .ok();

        let (manifest_path_raw, allowed_json, typography_json, min_contrast) = profile_row
            .unwrap_or_else(|| {
                (
                    FLAGSHIP_MANIFEST_SENTINEL.to_string(),
                    "[]".to_string(),
                    "{}".to_string(),
                    4.5,
                )
            });

        let cleaned_path = Self::sanitize_identifier(&manifest_path_raw);
        let icon_manifest_path = if cleaned_path.is_empty() {
            FLAGSHIP_MANIFEST_SENTINEL.to_string()
        } else {
            cleaned_path
        };

        let allowed_trimmed = allowed_json.trim();
        let allowed_icon_set: Vec<String> = if allowed_trimmed.is_empty() || allowed_trimmed == "[]"
        {
            Vec::new()
        } else {
            serde_json::from_str::<Vec<String>>(allowed_trimmed).map_err(|e| {
                HarnessError::ValidationFailed {
                    reason: format!("INVALID_ALLOWED_ICON_SET_JSON: {e}"),
                }
            })?
        };

        let typo_trimmed = typography_json.trim();
        let mut typography_contract = if typo_trimmed.is_empty() || typo_trimmed == "{}" {
            TypographyContract::default()
        } else {
            serde_json::from_str::<TypographyContract>(typo_trimmed).map_err(|e| {
                HarnessError::ValidationFailed {
                    reason: format!("INVALID_TYPOGRAPHY_CONTRACT_JSON: {e}"),
                }
            })?
        };
        if min_contrast > 0.0 && typo_trimmed == "{}" {
            typography_contract.min_wcag_contrast = min_contrast;
        }

        let comp_rule_json: Option<String> = conn
            .query_row(
                "SELECT r.config_json
                 FROM constraint_rules r
                 JOIN constraint_profiles p ON r.profile_id = p.id
                 WHERE p.project_id = ?1 AND UPPER(r.rule_code) = 'ENFORCE_COMPONENT_REGISTRY' AND r.enabled = 1
                 ORDER BY r.updated_at DESC
                 LIMIT 1",
                params![project_id],
                |r| r.get(0),
            )
            .ok();

        let component_registry = if let Some(raw_cfg) = comp_rule_json {
            let cfg_trimmed = raw_cfg.trim();
            if cfg_trimmed.is_empty() || cfg_trimmed == "{}" {
                Self::load_component_registry_from_project_specs(
                    conn,
                    project_root,
                    project_id,
                    true,
                )
            } else {
                serde_json::from_str::<ComponentRegistryContract>(cfg_trimmed).map_err(|e| {
                    HarnessError::ValidationFailed {
                        reason: format!("INVALID_COMPONENT_REGISTRY_JSON: {e}"),
                    }
                })?
            }
        } else {
            Self::load_component_registry_from_project_specs(conn, project_root, project_id, false)
        };

        Ok(ProjectDesignContracts {
            icon_manifest_path,
            allowed_icon_set,
            typography_contract,
            component_registry,
        })
    }

    /// Loads the stored `ProjectDesignContracts` metadata for a project from SQLite.
    pub fn load_project_design_contracts(
        conn: &Connection,
        project_root: &Path,
        project_id: &str,
    ) -> Result<ProjectDesignContracts, HarnessError> {
        Self::load_raw_design_contracts(conn, project_root, project_id)
    }

    /// Single canonical fail-closed loader for a project's IconManifest, TypographyContract,
    /// and ComponentRegistryContract from SQLite `constraint_profiles` + `constraint_rules`.
    pub fn load_project_design_context(
        conn: &Connection,
        project_root: &Path,
        project_id: &str,
    ) -> Result<(IconManifest, TypographyContract, ComponentRegistryContract), HarnessError> {
        let contracts = Self::load_raw_design_contracts(conn, project_root, project_id)?;

        // 1. Resolve IconManifest (FAIL-CLOSED if explicit path fails jail, IO, or JSON parse)
        let base_manifest = if contracts.icon_manifest_path == FLAGSHIP_MANIFEST_SENTINEL {
            Self::get_flagship_manifest()
        } else {
            let jailed_path =
                Self::resolve_jailed_manifest_path(project_root, &contracts.icon_manifest_path)?;
            Self::load_from_file(&jailed_path)?
        };

        // 2. Enforce `allowed_icon_set_json` if non-empty
        let effective_manifest =
            Self::apply_allowed_icon_filter(base_manifest, &contracts.allowed_icon_set);

        Ok((
            effective_manifest,
            contracts.typography_contract,
            contracts.component_registry,
        ))
    }

    fn load_component_registry_from_project_specs(
        conn: &Connection,
        project_root: &Path,
        project_id: &str,
        forbid_unregistered: bool,
    ) -> ComponentRegistryContract {
        let rel_path: Option<String> = conn
            .query_row(
                "SELECT file_path FROM project_specs
                 WHERE project_id = ?1 AND spec_key = '03-ui-ux-spec.md' AND file_path != ''
                 LIMIT 1",
                params![project_id],
                |r| r.get(0),
            )
            .ok();

        if let Some(rel) = rel_path {
            if let Ok(jailed) = Self::resolve_jailed_manifest_path(project_root, &rel) {
                if let Ok(md) = std::fs::read_to_string(jailed) {
                    return Self::extract_component_registry_from_markdown(
                        &md,
                        forbid_unregistered,
                    );
                }
            }
        }

        ComponentRegistryContract {
            components: Vec::new(),
            forbid_unregistered,
        }
    }

    /// Persists `ProjectDesignContracts` safely into `constraint_profiles` & `constraint_rules`.
    /// Validates custom manifest paths against the canonical project jail at write time.
    pub fn save_project_design_contracts(
        conn: &mut Connection,
        project_root: &Path,
        project_id: &str,
        contracts: &ProjectDesignContracts,
    ) -> Result<(), HarnessError> {
        let cleaned_path = Self::sanitize_identifier(&contracts.icon_manifest_path);
        let stored_path = if cleaned_path.is_empty() || cleaned_path == FLAGSHIP_MANIFEST_SENTINEL {
            FLAGSHIP_MANIFEST_SENTINEL.to_string()
        } else {
            let jailed = Self::resolve_jailed_manifest_path(project_root, &cleaned_path)?;
            let _ = Self::load_from_file(&jailed)?;
            cleaned_path
        };

        if contracts.typography_contract.min_wcag_contrast.is_nan()
            || contracts.typography_contract.min_wcag_contrast < 1.0
            || contracts.typography_contract.min_wcag_contrast > 21.0
        {
            return Err(HarnessError::ValidationFailed {
                reason: "INVALID_WCAG_CONTRAST: min_wcag_contrast must be between 1.0 and 21.0"
                    .to_string(),
            });
        }

        if contracts.component_registry.components.len() > MAX_COMPONENT_REGISTRY_ENTRIES {
            return Err(HarnessError::BoundaryViolation {
                reason: format!(
                    "COMPONENT_REGISTRY_TOO_LARGE: max {} entries allowed",
                    MAX_COMPONENT_REGISTRY_ENTRIES
                ),
            });
        }

        let allowed_json = serde_json::to_string(&contracts.allowed_icon_set).map_err(|e| {
            HarnessError::ValidationFailed {
                reason: format!("Failed to serialize allowed_icon_set: {e}"),
            }
        })?;
        let typography_json =
            serde_json::to_string(&contracts.typography_contract).map_err(|e| {
                HarnessError::ValidationFailed {
                    reason: format!("Failed to serialize typography_contract: {e}"),
                }
            })?;
        let comp_json = serde_json::to_string(&contracts.component_registry).map_err(|e| {
            HarnessError::ValidationFailed {
                reason: format!("Failed to serialize component_registry: {e}"),
            }
        })?;

        let now_ms = chrono_now_millis();
        let prof_id = uuid::Uuid::new_v4().to_string();

        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| HarnessError::SqliteBusy {
                details: format!("Failed to open design contracts tx: {e}"),
            })?;

        tx.execute(
            "INSERT INTO constraint_profiles (id, project_id, preset_profile, styling_policy, forbid_tailwind, forbid_raw_hex, tokens_css_path, icon_manifest_path, allowed_icon_set_json, typography_contract_json, min_wcag_contrast, updated_at)
             VALUES (?1, ?2, 'custom', 'tokens_strict', 1, 1, 'tokens.css', ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(project_id, preset_profile) DO UPDATE SET
                icon_manifest_path = excluded.icon_manifest_path,
                allowed_icon_set_json = excluded.allowed_icon_set_json,
                typography_contract_json = excluded.typography_contract_json,
                min_wcag_contrast = excluded.min_wcag_contrast,
                updated_at = excluded.updated_at",
            params![
                prof_id,
                project_id,
                stored_path,
                allowed_json,
                typography_json,
                contracts.typography_contract.min_wcag_contrast,
                now_ms
            ],
        )
        .map_err(|e| HarnessError::SqliteBusy {
            details: format!("Failed to upsert constraint_profiles design contracts: {e}"),
        })?;

        let effective_prof_id: String = tx
            .query_row(
                "SELECT id FROM constraint_profiles WHERE project_id = ?1 ORDER BY updated_at DESC LIMIT 1",
                params![project_id],
                |r| r.get(0),
            )
            .map_err(|e| HarnessError::SqliteBusy {
                details: format!("Failed to resolve profile id: {e}"),
            })?;

        tx.execute(
            "INSERT INTO constraint_rules (id, profile_id, rule_code, severity, config_json, enabled, updated_at)
             VALUES (?1, ?2, 'ENFORCE_COMPONENT_REGISTRY', 'BLOCKER', ?3, 1, ?4)
             ON CONFLICT(profile_id, rule_code) DO UPDATE SET config_json = excluded.config_json, enabled = 1, updated_at = excluded.updated_at",
            params![uuid::Uuid::new_v4().to_string(), effective_prof_id, comp_json, now_ms],
        )
        .map_err(|e| HarnessError::SqliteBusy {
            details: format!("Failed to upsert ENFORCE_COMPONENT_REGISTRY rule: {e}"),
        })?;

        tx.commit().map_err(|e| HarnessError::SqliteBusy {
            details: format!("Failed to commit design contracts tx: {e}"),
        })?;

        Ok(())
    }

    /// Validates an icon name against the given manifest.
    /// If the icon is not found, calculates normalized Levenshtein distances and
    /// returns the Top-3 nearest valid suggestions (AC-03).
    pub fn validate_icon(manifest: &IconManifest, requested_icon: &str) -> IconValidationReport {
        let sanitized = Self::sanitize_identifier(requested_icon);
        let norm_req = sanitized.to_lowercase();

        if norm_req.is_empty() || norm_req.chars().count() > MAX_IDENTIFIER_LEN {
            let default_suggestions = manifest
                .icons
                .iter()
                .take(3)
                .map(|i| i.name.clone())
                .collect();
            return IconValidationReport {
                requested_icon: sanitized,
                is_valid: false,
                suggestions: default_suggestions,
            };
        }

        // Direct match
        if manifest
            .icons
            .iter()
            .any(|i| i.name.to_lowercase() == norm_req)
        {
            return IconValidationReport {
                requested_icon: sanitized,
                is_valid: true,
                suggestions: Vec::new(),
            };
        }

        let candidates: Vec<&str> = manifest.icons.iter().map(|e| e.name.as_str()).collect();
        let suggestions = Self::top_k_suggestions(&norm_req, &candidates, 3);

        IconValidationReport {
            requested_icon: sanitized,
            is_valid: false,
            suggestions,
        }
    }

    /// Validates a JSX component tag against `ComponentRegistryContract` (`07_COMPONENT_REGISTRY`).
    /// Lowercase intrinsic HTML/SVG tags and React `Fragment` are always valid; PascalCase
    /// components must exist in `contract.components` when `forbid_unregistered` is true.
    pub fn validate_component(
        contract: &ComponentRegistryContract,
        requested_component: &str,
    ) -> ComponentValidationReport {
        let sanitized = Self::sanitize_identifier(requested_component);
        if sanitized.is_empty() || sanitized.chars().count() > MAX_IDENTIFIER_LEN {
            return ComponentValidationReport {
                requested_component: sanitized,
                is_valid: false,
                suggestions: contract
                    .components
                    .iter()
                    .take(3)
                    .map(|c| c.name.clone())
                    .collect(),
            };
        }

        // Intrinsic HTML/SVG tags start with lowercase ASCII; React.Fragment / Fragment allowed
        let first_char = sanitized.chars().next().unwrap_or('a');
        if first_char.is_ascii_lowercase()
            || sanitized == "Fragment"
            || sanitized == "React.Fragment"
        {
            return ComponentValidationReport {
                requested_component: sanitized,
                is_valid: true,
                suggestions: Vec::new(),
            };
        }

        if !contract.forbid_unregistered {
            return ComponentValidationReport {
                requested_component: sanitized,
                is_valid: true,
                suggestions: Vec::new(),
            };
        }

        let direct_match = contract
            .components
            .iter()
            .any(|c| c.name.eq_ignore_ascii_case(&sanitized));

        if direct_match {
            return ComponentValidationReport {
                requested_component: sanitized,
                is_valid: true,
                suggestions: Vec::new(),
            };
        }

        let candidates: Vec<&str> = contract
            .components
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        let norm_req = sanitized.to_lowercase();
        let suggestions = Self::top_k_suggestions(&norm_req, &candidates, 3);

        ComponentValidationReport {
            requested_component: sanitized,
            is_valid: false,
            suggestions,
        }
    }

    /// Extracts PascalCase registered UI components from `07_COMPONENT_REGISTRY` sections
    /// or inline `<Component />` / `` `ComponentName` `` tokens in `03-ui-ux-spec.md`.
    pub fn extract_component_registry_from_markdown(
        spec_md: &str,
        forbid_unregistered: bool,
    ) -> ComponentRegistryContract {
        let mut seen = HashSet::new();
        let mut components = Vec::new();

        // Scope strictly to 07_COMPONENT_REGISTRY / Component Registry section if present,
        // terminating at the next top/second-level heading (`# ` or `## `).
        let after_start = if let Some(idx) = spec_md.find("07_COMPONENT_REGISTRY") {
            &spec_md[idx..]
        } else if let Some(idx) = spec_md.find("Component Registry") {
            &spec_md[idx..]
        } else {
            spec_md
        };

        let mut section_end = after_start.len();
        let mut cursor = 0usize;
        for (line_idx, line) in after_start.lines().enumerate() {
            if line_idx > 0 {
                let trimmed_line = line.trim_start();
                if trimmed_line.starts_with("# ") || trimmed_line.starts_with("## ") {
                    section_end = cursor;
                    break;
                }
            }
            cursor += line.len() + 1;
        }
        let target_slice = &after_start[..section_end.min(after_start.len())];

        for seg in target_slice.split('`').skip(1).step_by(2) {
            let trimmed = seg
                .trim()
                .trim_start_matches('<')
                .trim_end_matches("/>")
                .trim_end_matches('>')
                .split_whitespace()
                .next()
                .unwrap_or("");
            if Self::is_pascal_component_ident(trimmed) && seen.insert(trimmed.to_string()) {
                components.push(ComponentRegistryEntry {
                    name: trimmed.to_string(),
                    source_spec: Some("07_COMPONENT_REGISTRY".to_string()),
                    allowed_props: Vec::new(),
                });
            }
        }

        ComponentRegistryContract {
            components,
            forbid_unregistered,
        }
    }

    fn is_pascal_component_ident(ident: &str) -> bool {
        let mut chars = ident.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        if !first.is_ascii_uppercase() || ident.len() < 2 || ident.len() > MAX_IDENTIFIER_LEN {
            return false;
        }
        // Reject ALL_CAPS constants like `PROHIBITED_TOKENS` or `AC01`
        let has_lowercase = ident.chars().any(|c| c.is_ascii_lowercase());
        has_lowercase && ident.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
    }

    fn top_k_suggestions(norm_req: &str, candidates: &[&str], k: usize) -> Vec<String> {
        let mut scored: Vec<(&str, usize, bool)> = candidates
            .iter()
            .map(|&name| {
                let lower_name = name.to_lowercase();
                let dist = Self::levenshtein_distance(norm_req, &lower_name);
                let is_prefix =
                    lower_name.starts_with(norm_req) || norm_req.starts_with(&lower_name);
                (name, dist, is_prefix)
            })
            .collect();

        scored.sort_by(|a, b| {
            a.1.cmp(&b.1)
                .then_with(|| b.2.cmp(&a.2))
                .then_with(|| a.0.cmp(b.0))
        });

        scored
            .into_iter()
            .take(k)
            .map(|(name, _, _)| name.to_string())
            .collect()
    }

    /// Efficient Wagner-Fischer algorithm for Levenshtein distance with stack arrays for length <= 64.
    #[allow(clippy::needless_range_loop)]
    pub fn levenshtein_distance(a: &str, b: &str) -> usize {
        let m = a.chars().count();
        let n = b.chars().count();

        if m == 0 {
            return n;
        }
        if n == 0 {
            return m;
        }

        if m > 64 || n > 64 {
            let a_chars: Vec<char> = a.chars().collect();
            let b_chars: Vec<char> = b.chars().collect();
            let mut prev_row: Vec<usize> = (0..=n).collect();
            let mut curr_row: Vec<usize> = vec![0; n + 1];

            for i in 1..=m {
                curr_row[0] = i;
                for j in 1..=n {
                    let cost = if a_chars[i - 1] == b_chars[j - 1] {
                        0
                    } else {
                        1
                    };
                    curr_row[j] = (prev_row[j] + 1)
                        .min(curr_row[j - 1] + 1)
                        .min(prev_row[j - 1] + cost);
                }
                prev_row.copy_from_slice(&curr_row);
            }
            return prev_row[n];
        }

        let mut a_chars = ['\0'; 65];
        for (i, ch) in a.chars().enumerate() {
            a_chars[i] = ch;
        }
        let mut b_chars = ['\0'; 65];
        for (j, ch) in b.chars().enumerate() {
            b_chars[j] = ch;
        }

        let mut prev_row = [0usize; 65];
        let mut curr_row = [0usize; 65];

        for j in 0..=n {
            prev_row[j] = j;
        }

        for i in 1..=m {
            curr_row[0] = i;
            for j in 1..=n {
                let cost = if a_chars[i - 1] == b_chars[j - 1] {
                    0
                } else {
                    1
                };
                curr_row[j] = (prev_row[j] + 1)
                    .min(curr_row[j - 1] + 1)
                    .min(prev_row[j - 1] + cost);
            }
            prev_row[..=n].copy_from_slice(&curr_row[..=n]);
        }

        prev_row[n]
    }

    /// Parses CSS font-size into normalized pixels (16px baseline for `rem` and unitless numbers)
    /// so `1rem` and `16.0px` match `"16px"` in `allowed_font_sizes`.
    fn normalize_css_font_size_px(raw: &str) -> Option<f64> {
        let s = raw.trim().to_ascii_lowercase();
        if let Some(num) = s.strip_suffix("px") {
            num.trim().parse::<f64>().ok()
        } else if let Some(num) = s.strip_suffix("rem") {
            num.trim().parse::<f64>().ok().map(|v| v * 16.0)
        } else if let Some(num) = s.strip_suffix("em") {
            num.trim().parse::<f64>().ok().map(|v| v * 16.0)
        } else if let Some(num) = s.strip_suffix("pt") {
            num.trim().parse::<f64>().ok().map(|v| v * (96.0 / 72.0))
        } else {
            None
        }
    }

    /// Validates a font style against the project typography contract.
    pub fn validate_typography(
        contract: &TypographyContract,
        font_family: &str,
        font_weight: u32,
        font_size: Option<&str>,
        contrast_ratio: f64,
    ) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        let family_clean = Self::sanitize_identifier(font_family)
            .trim_matches('\'')
            .trim_matches('"')
            .to_string();
        if !contract
            .allowed_font_families
            .iter()
            .any(|f| f.eq_ignore_ascii_case(&family_clean))
        {
            errors.push(format!(
                "Font family '{}' is not in allowed list: {:?}",
                font_family, contract.allowed_font_families
            ));
        }

        if !contract.allowed_font_weights.contains(&font_weight) {
            errors.push(format!(
                "Font weight '{}' is not in allowed list: {:?}",
                font_weight, contract.allowed_font_weights
            ));
        }

        if let Some(size) = font_size {
            let size_clean = Self::sanitize_identifier(size);
            let exact_match = contract
                .allowed_font_sizes
                .iter()
                .any(|s| s.eq_ignore_ascii_case(&size_clean));
            let normalized_match =
                Self::normalize_css_font_size_px(&size_clean).is_some_and(|req_px| {
                    contract.allowed_font_sizes.iter().any(|allowed| {
                        Self::normalize_css_font_size_px(allowed)
                            .is_some_and(|allowed_px| (req_px - allowed_px).abs() < 1e-4)
                    })
                });

            if !exact_match && !normalized_match {
                errors.push(format!(
                    "Font size '{}' is not in allowed list: {:?}",
                    size, contract.allowed_font_sizes
                ));
            }
        }

        if contrast_ratio.is_nan() || contrast_ratio < contract.min_wcag_contrast {
            errors.push(format!(
                "Contrast ratio {:.2} fails minimum WCAG threshold of {:.2}",
                contrast_ratio, contract.min_wcag_contrast
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Convenience wrapper returning a serializable `TypographyValidationReport`.
    pub fn validate_typography_report(
        contract: &TypographyContract,
        font_family: &str,
        font_weight: u32,
        font_size: Option<&str>,
        contrast_ratio: f64,
    ) -> TypographyValidationReport {
        match Self::validate_typography(
            contract,
            font_family,
            font_weight,
            font_size,
            contrast_ratio,
        ) {
            Ok(()) => TypographyValidationReport {
                is_valid: true,
                errors: Vec::new(),
            },
            Err(errors) => TypographyValidationReport {
                is_valid: false,
                errors,
            },
        }
    }
}

fn chrono_now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
