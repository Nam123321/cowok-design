// @story 02.1
// @implements AC-04
use crate::portal_guard::{map_portal_and_department, scan_text_for_portal};
use crate::types::{AdapterMode, ContextCompilerError, ModelProfile, PortalId};
use domain::{
    AdaptationRequest, AdaptationResponse, CanonicalSpecKey, ModelCapabilityAdapter,
    ModelProfileTier, ModelToolDefinition, VisibilityPolicy,
};
use rusqlite::{params, Connection};
use spec_registry::read_jailed_spec_bytes;
use std::collections::HashSet;
use std::path::Path;
use uuid::Uuid;

const MAX_OFFLINE_FTS_ROWS: usize = 5;
const MICRO_TOOL_LIMIT: usize = 5;
const MAX_SNIPPET_BODY_CHARS: usize = 4096;

#[derive(Debug, Clone)]
pub struct AdaptedProfileResult {
    pub profile: ModelProfile,
    pub adapter_mode: AdapterMode,
    pub downgraded_to_micro_tools: bool,
    pub adapted_tool_count: usize,
    pub offline_fallback_active: bool,
    pub system_prompt_summary: String,
    pub adapted_tools: Vec<String>,
    pub offline_snippets: Vec<String>,
}

fn canonical_tool_catalog() -> Vec<ModelToolDefinition> {
    vec![
        make_tool("query_ssot_specs", true, &["project_id", "spec_key"]),
        make_tool("patch_dom_ff_id", true, &["ff_id", "prop", "value"]),
        make_tool("validate_design_tokens", true, &["source", "mode"]),
        make_tool("inspect_component_registry", true, &["portal_id"]),
        make_tool("read_portal_policy", true, &["portal_id"]),
        make_tool(
            "run_structural_diff_batch",
            false,
            &["project_id", "nodes", "edges", "tokens", "filters", "mode"],
        ),
        make_tool(
            "synthesize_multi_portal_tree",
            false,
            &[
                "project_id",
                "portals",
                "constraints",
                "theme",
                "layout",
                "spec",
            ],
        ),
    ]
}

fn make_tool(name: &str, is_micro_tool: bool, prop_names: &[&str]) -> ModelToolDefinition {
    let mut props = serde_json::Map::new();
    for p in prop_names {
        props.insert((*p).to_string(), serde_json::json!({ "type": "string" }));
    }
    ModelToolDefinition {
        name: name.to_string(),
        description: format!("Deterministic tool {name}"),
        parameters: serde_json::json!({ "type": "object", "properties": props }),
        is_micro_tool,
    }
}

fn map_tier(profile: ModelProfile) -> ModelProfileTier {
    match profile {
        ModelProfile::A => ModelProfileTier::ProfileA_Frontier,
        ModelProfile::B => ModelProfileTier::ProfileB_FastCoding,
        ModelProfile::C => ModelProfileTier::ProfileC_Compact,
    }
}

fn build_fts_query_terms(intent: &str) -> String {
    let terms: Vec<String> = intent
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|w| w.chars().count() >= 3)
        .take(6)
        .map(|w| format!("\"{}\"", w.replace('"', "")))
        .collect();
    if terms.is_empty() {
        "\"spec\"".to_string()
    } else {
        terms.join(" OR ")
    }
}

fn resolve_spec_body(
    workspace_root: Option<&Path>,
    spec_key: &str,
    summary_digest: &str,
    sha256: &str,
) -> String {
    if let (Some(ws), Some(canonical)) = (workspace_root, CanonicalSpecKey::parse(spec_key).ok()) {
        if let Ok(Some(bytes)) = read_jailed_spec_bytes(ws, canonical) {
            let decoded = String::from_utf8_lossy(&bytes);
            let compact: String = decoded.chars().take(MAX_SNIPPET_BODY_CHARS).collect();
            if !compact.trim().is_empty() {
                return compact;
            }
        }
    }
    let digest_part = summary_digest.trim();
    let sha_part: String = sha256.chars().take(8).collect();
    format!("{spec_key} {digest_part} {sha_part}")
}

fn storage_err(e: rusqlite::Error) -> ContextCompilerError {
    ContextCompilerError::TelemetryStorageUnavailable {
        reason: e.to_string(),
    }
}

fn collect_offline_fts_snippets(
    conn: &Connection,
    workspace_root: Option<&Path>,
    project_id: Uuid,
    portal: PortalId,
    intent: &str,
) -> Result<Vec<String>, ContextCompilerError> {
    let (domain_portal, dept) = map_portal_and_department(portal);
    let (allowed_keys, _) = VisibilityPolicy::evaluate(dept, domain_portal);
    let allowed_set: HashSet<&str> = allowed_keys.iter().map(|k| k.as_str()).collect();

    let mut stmt = conn
        .prepare(
            "SELECT spec_key, content_sha256, summary_digest
             FROM project_specs
             WHERE project_id = ?1
             ORDER BY spec_key ASC",
        )
        .map_err(storage_err)?;
    let raw_rows = stmt
        .query_map(params![project_id.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(storage_err)?;

    let fts_conn = Connection::open_in_memory().map_err(storage_err)?;
    fts_conn
        .execute_batch("CREATE VIRTUAL TABLE spec_fts USING fts5(spec_key, body);")
        .map_err(storage_err)?;

    let mut fallback_rows = Vec::new();
    for item in raw_rows {
        let (spec_key, sha, summary) = item.map_err(storage_err)?;
        if portal != PortalId::SuperAdmin && !allowed_set.contains(spec_key.as_str()) {
            continue;
        }
        let body = resolve_spec_body(workspace_root, &spec_key, &summary, &sha);
        if scan_text_for_portal(portal, "offline_spec", &body).is_err() {
            continue;
        }
        fts_conn
            .execute(
                "INSERT INTO spec_fts (spec_key, body) VALUES (?1, ?2)",
                params![spec_key, body],
            )
            .map_err(storage_err)?;
        let sha_short: String = sha.chars().take(8).collect();
        if fallback_rows.len() < MAX_OFFLINE_FTS_ROWS {
            fallback_rows.push(format!("{spec_key}:{sha_short}"));
        }
    }

    let match_query = build_fts_query_terms(intent);
    let mut fts_stmt = fts_conn
        .prepare(
            "SELECT spec_key, snippet(spec_fts, 1, '[', ']', '...', 8)
             FROM spec_fts
             WHERE spec_fts MATCH ?1
             ORDER BY bm25(spec_fts) ASC, spec_key ASC
             LIMIT ?2",
        )
        .map_err(storage_err)?;
    let hits = fts_stmt
        .query_map(params![match_query, MAX_OFFLINE_FTS_ROWS as i64], |r| {
            let key: String = r.get(0)?;
            let snip: String = r.get(1)?;
            Ok(format!("{key}:{}", snip.replace('\n', " ").trim()))
        })
        .map_err(storage_err)?;
    let mut out = Vec::new();
    for hit in hits {
        let entry = hit.map_err(storage_err)?;
        if scan_text_for_portal(portal, "offline_snippet", &entry).is_ok() {
            out.push(entry);
        }
    }
    if out.is_empty() {
        return Ok(fallback_rows);
    }
    Ok(out)
}

pub fn adapt_for_profile(
    conn: Option<&Connection>,
    workspace_root: Option<&Path>,
    project_id: Uuid,
    portal: PortalId,
    profile: ModelProfile,
    raw_prompt: &str,
    intent: &str,
) -> Result<AdaptedProfileResult, ContextCompilerError> {
    let req = AdaptationRequest {
        project_id,
        requested_profile: map_tier(profile),
        raw_system_prompt: raw_prompt.to_string(),
        tools: canonical_tool_catalog(),
        disallowed_profiles: Vec::new(),
        syntax_error_reported: matches!(profile, ModelProfile::B),
        max_budget_override: None,
    };
    let adapted: AdaptationResponse = ModelCapabilityAdapter::new().adapt(&req).map_err(|err| {
        ContextCompilerError::InvalidRequest {
            message: err.to_string(),
            field: Some("modelProfile".to_string()),
        }
    })?;
    let offline_snippets = if matches!(profile, ModelProfile::C) {
        match conn {
            Some(c) => collect_offline_fts_snippets(c, workspace_root, project_id, portal, intent)?,
            None => vec!["offline_fts5:local_cache".to_string()],
        }
    } else {
        Vec::new()
    };
    let mut adapted_tools: Vec<String> = adapted.tools.into_iter().map(|t| t.name).collect();
    if matches!(profile, ModelProfile::B) && adapted_tools.len() > MICRO_TOOL_LIMIT {
        adapted_tools.truncate(MICRO_TOOL_LIMIT);
    }
    let downgraded_to_micro_tools = matches!(profile, ModelProfile::B) || adapted.degraded;
    Ok(AdaptedProfileResult {
        profile,
        adapter_mode: profile.adapter_mode(),
        downgraded_to_micro_tools,
        adapted_tool_count: adapted_tools.len(),
        offline_fallback_active: matches!(profile, ModelProfile::C),
        system_prompt_summary: adapted.system_prompt,
        adapted_tools,
        offline_snippets,
    })
}
