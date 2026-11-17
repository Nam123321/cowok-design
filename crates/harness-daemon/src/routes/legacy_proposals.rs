// @story 01.3, 05.2, 05.3
use std::collections::{BTreeMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use axum::{
    body::Bytes,
    extract::{Extension, Path as AxumPath, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::STANDARD as B64_STD, Engine as _};
use domain::{CanonicalSpecKey, HarnessError};
use rusqlite::{ffi as sqlite_ffi, params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spec_registry::{
    is_sqlite_busy_or_locked, map_sqlite_error, read_jailed_spec_bytes, retry_on_busy, SpecRegistry,
};
use uuid::Uuid;

use super::*;
use crate::daemon_auth::VerifiedLegacyIngress;
use crate::vault::VaultStore;

const MAX_VISUAL_DIFF_WIDTH: u32 = 1280;
const MAX_VISUAL_DIFF_HEIGHT: u32 = 800;
const MAX_TRADEOFFS_PER_PROPOSAL: usize = 12;
const MAX_ZIP_FILES: usize = 2_000;
const MAX_SINGLE_FILE_BYTES: usize = 16 * 1024 * 1024;
const MAX_ZIP_TOTAL_BYTES: usize = 100 * 1024 * 1024;
const DEFAULT_PR_CMD_TIMEOUT_MS: u64 = 30_000;

fn default_content_mode() -> String {
    "full_file".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiffItemDto {
    pub path: String,
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
    pub patch: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationCheckDto {
    pub check_type: String,
    pub status: String,
    pub message: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTradeoffInput {
    #[serde(default)]
    pub node_id: Option<String>,
    pub element_ff_id: String,
    pub badge_index: i32,
    #[serde(default)]
    pub design_note: String,
    #[serde(default)]
    pub instead_label: String,
    #[serde(default)]
    pub instead_patch: Option<serde_json::Value>,
    #[serde(default)]
    pub ask_prompt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalGateReceiptDto {
    pub id: Uuid,
    pub proposal_id: Uuid,
    pub diff_sha256: String,
    pub constraints_sha256: String,
    pub workspace_head_sha256: String,
    pub exit_code: i32,
    pub receipt_hmac: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalTradeoffDto {
    pub id: Uuid,
    pub proposal_id: Uuid,
    pub node_id: Option<String>,
    pub element_ff_id: String,
    pub badge_index: i32,
    pub design_note: String,
    pub instead_label: String,
    pub instead_patch: Option<serde_json::Value>,
    pub decision: String,
    pub ask_prompt: Option<String>,
    pub decided_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualDiffBboxDto {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualDiffResultDto {
    pub status: String,
    pub ssim: f64,
    pub mismatch_ratio: f64,
    pub mismatched_pixels: u64,
    pub total_pixels: u64,
    pub width: u32,
    pub height: u32,
    pub changed_bbox: Option<VisualDiffBboxDto>,
    pub reason: Option<String>,
    #[serde(default)]
    pub structural_diff_summary: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListProposalsQuery {
    #[serde(default, alias = "node_id")]
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProposalRequest {
    #[serde(default)]
    pub id: Option<Uuid>,
    #[serde(default)]
    pub intent_id: Option<Uuid>,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub files: Vec<FileDiffItemDto>,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub batch_id: Option<String>,
    #[serde(default)]
    pub node_ids: Option<Vec<String>>,
    #[serde(default)]
    pub content_mode: Option<String>,
    #[serde(default)]
    pub base_file_hashes: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub tradeoffs: Option<Vec<CreateTradeoffInput>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateProposalRequest {
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub rebase_base_hashes: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffProposalRequest {
    #[serde(default)]
    pub before_rgba_base64: Option<String>,
    #[serde(default)]
    pub after_rgba_base64: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub threshold: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecideTradeoffRequest {
    pub decision: String,
    #[serde(default)]
    pub ask_prompt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeProposalDto {
    pub id: Uuid,
    #[serde(alias = "tenantId")]
    pub tenant_id: String,
    #[serde(alias = "projectId")]
    pub project_id: Uuid,
    #[serde(default)]
    pub intent_id: Option<Uuid>,
    pub title: String,
    pub description: String,
    pub files: Vec<FileDiffItemDto>,
    pub validation_status: String,
    pub validation_checks: Vec<ValidationCheckDto>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub batch_id: Option<String>,
    #[serde(default)]
    pub composite_scope_key: String,
    #[serde(default = "default_content_mode")]
    pub content_mode: String,
    #[serde(default)]
    pub base_file_hashes: BTreeMap<String, String>,
    #[serde(default)]
    pub node_ids: Vec<String>,
    #[serde(default)]
    pub gate_receipt_id: Option<String>,
    #[serde(default)]
    pub gate_receipt: Option<ProposalGateReceiptDto>,
    #[serde(default)]
    pub visual_diff: Option<VisualDiffResultDto>,
    #[serde(default)]
    pub tradeoffs: Vec<ProposalTradeoffDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyProposalRequest {
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotFileItemDto {
    pub path: String,
    pub content: String,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSnapshotRequest {
    #[serde(default)]
    pub id: Option<Uuid>,
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version_tag: Option<String>,
    #[serde(default = "default_trigger")]
    pub trigger: String,
    #[serde(default)]
    pub files: Vec<SnapshotFileItemDto>,
    #[serde(default)]
    pub metadata: Option<std::collections::HashMap<String, String>>,
    #[serde(default, alias = "proposal_id")]
    pub proposal_id: Option<String>,
    #[serde(default, alias = "parent_snapshot_id")]
    pub parent_snapshot_id: Option<String>,
    #[serde(default, alias = "chat_artifact_snapshot_id")]
    pub chat_artifact_snapshot_id: Option<String>,
}

fn default_trigger() -> String {
    "manual".to_string()
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSnapshotsQuery {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default, alias = "node_id")]
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionSnapshotDto {
    pub id: Uuid,
    #[serde(alias = "tenantId")]
    pub tenant_id: String,
    #[serde(alias = "projectId")]
    pub project_id: Uuid,
    pub version_tag: String,
    pub label: String,
    pub description: String,
    pub trigger: String,
    pub tree_sha256: String,
    #[serde(default)]
    pub files: Vec<SnapshotFileItemDto>,
    #[serde(default)]
    pub metadata: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub proposal_id: Option<String>,
    #[serde(default)]
    pub parent_snapshot_id: Option<String>,
    #[serde(default)]
    pub chat_artifact_snapshot_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatArtifactSnapshotDto {
    pub id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub session_id: String,
    pub message_id: String,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub proposal_id: Option<String>,
    #[serde(default)]
    pub parent_snapshot_id: Option<String>,
    pub artifact_type: String,
    pub content_hash: String,
    pub manifest_json: String,
    pub node_snapshot_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevertSnapshotRequest {
    #[serde(default)]
    pub safety_label: Option<String>,
    #[serde(default, alias = "restore_missing_nodes")]
    pub restore_missing_nodes: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevertResultDto {
    pub success: bool,
    pub reverted_to_snapshot_id: Uuid,
    pub safety_snapshot_id: Uuid,
    pub restored_files_count: usize,
    #[serde(default)]
    pub restored_node_ids: Vec<String>,
    pub timestamp: String,
}

fn default_true_bool() -> bool {
    true
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkTurnSnapshotRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default = "default_true_bool")]
    pub include_node_scope: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnForkResultDto {
    pub forked_session_id: String,
    pub forked_from_session_id: String,
    pub forked_from_message_id: String,
    pub turn_snapshot_id: String,
    pub copied_messages_count: usize,
    pub detached_node_ids: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportZipRequest {
    #[serde(default = "default_export_bool")]
    pub include_fixtures: bool,
    #[serde(default = "default_export_bool")]
    pub include_tokens: bool,
    #[serde(default = "default_export_bool")]
    pub include_icons: bool,
    #[serde(default = "default_export_format")]
    pub format: String,
}

fn default_export_bool() -> bool {
    true
}

fn default_export_format() -> String {
    "zip".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportZipResponseDto {
    pub file_name: String,
    pub byte_length: usize,
    pub sha256: String,
    pub files_included: Vec<String>,
    pub ready_to_run: bool,
    #[serde(default)]
    pub handoff_manifest_included: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubPRRequest {
    #[serde(alias = "branch_name")]
    pub branch_name: String,
    #[serde(alias = "pr_title")]
    pub pr_title: String,
    #[serde(default, alias = "pr_body")]
    pub pr_body: String,
    #[serde(default = "default_target_branch", alias = "target_branch")]
    pub target_branch: String,
    #[serde(default, alias = "snapshot_id")]
    pub snapshot_id: Option<String>,
    #[serde(default, alias = "include_visual_diff_table")]
    pub include_visual_diff_table: bool,
}

fn default_target_branch() -> String {
    "main".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubPRResponseDto {
    pub pr_url: String,
    pub pr_number: u64,
    pub branch_name: String,
    pub status: String,
    #[serde(default)]
    pub snapshot_id: Option<String>,
    #[serde(default)]
    pub handoff_manifest_sha256: String,
    #[serde(default)]
    pub is_offline_mock: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenIdeHandoffRequest {
    pub ide: String,
    #[serde(default, alias = "relative_entry_path")]
    pub relative_entry_path: Option<String>,
    #[serde(default, alias = "snapshot_id")]
    pub snapshot_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenIdeHandoffResponseDto {
    pub ide: String,
    pub launch_uri: String,
    pub cli_fallback_command: String,
    pub manifest_path: String,
    pub manifest_sha256: String,
}

fn resolve_tenant_id(
    registry: &SpecRegistry,
    project_id: Uuid,
    caller: Option<&VerifiedLegacyIngress>,
) -> String {
    if let Some(verified) = caller {
        if matches!(
            verified.kind,
            crate::daemon_auth::LegacyAuthKind::HmacV2
                | crate::daemon_auth::LegacyAuthKind::SessionToken
        ) && !verified.principal.tenant_id.trim().is_empty()
        {
            return verified.principal.tenant_id.trim().to_string();
        }
    }
    if let Ok(conn) = registry.pool().connection() {
        if let Ok(t) = conn.query_row(
            "SELECT tenant_id FROM projects WHERE id = ?1",
            params![project_id.to_string()],
            |r| r.get::<_, String>(0),
        ) {
            if !t.trim().is_empty() {
                return t;
            }
        }
    }
    "local-default".to_string()
}

fn reconstruct_lintable_patch_source(raw_patch: &str) -> String {
    let looks_like_diff = raw_patch.starts_with("--- ")
        || raw_patch.contains("\n@@ ")
        || raw_patch.starts_with("@@ ")
        || raw_patch.lines().any(|line| {
            [
                "+const ",
                "+let ",
                "+var ",
                "+export ",
                "+import ",
                "+function ",
                "+interface ",
                "+type ",
                "+return ",
                "+<",
                "+//",
            ]
            .iter()
            .any(|pfx| line.starts_with(pfx))
        });
    if !looks_like_diff {
        return raw_patch.to_string();
    }
    raw_patch
        .lines()
        .filter_map(|line| {
            if line.starts_with("+++ ") || line.starts_with("--- ") || line.starts_with("@@ ") {
                None
            } else if let Some(added) = line.strip_prefix('+') {
                Some(added)
            } else if let Some(ctx) = line.strip_prefix(' ') {
                Some(ctx)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn validate_safe_relative_path(raw: &str) -> Result<String, HarnessError> {
    let trimmed = raw.trim().replace('\\', "/");
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || trimmed.contains('\0')
        || trimmed
            .split('/')
            .any(|seg| seg.is_empty() || seg == "." || seg == "..")
    {
        return Err(HarnessError::PathTraversalDenied {
            path: raw.to_string(),
        });
    }
    Ok(trimmed)
}

fn has_forbidden_dynamic_execution_pattern(patch: &str) -> bool {
    let lower_patch = patch.to_ascii_lowercase();
    let react_unsafe_html = concat!("dangerously", "setinnerhtml");
    if lower_patch.contains(react_unsafe_html) || lower_patch.contains("__html") {
        return true;
    }
    // Check eval(...) with arbitrary whitespace
    let mut search_from = 0;
    while let Some(rel_pos) = lower_patch[search_from..].find("eval") {
        let idx = search_from + rel_pos;
        let before_ok = if idx == 0 {
            true
        } else {
            let prev = lower_patch[..idx].chars().last().unwrap_or(' ');
            !prev.is_alphanumeric() && prev != '_' && prev != '$'
        };
        if before_ok {
            let rest = &lower_patch[idx + 4..];
            let trimmed = rest.trim_start();
            if trimmed.starts_with('(') {
                return true;
            }
        }
        search_from = idx + 4;
    }
    // Check Function(...) constructor with arbitrary whitespace (capital F)
    let mut search_from_f = 0;
    while let Some(rel_pos) = patch[search_from_f..].find("Function") {
        let idx = search_from_f + rel_pos;
        let before_ok = if idx == 0 {
            true
        } else {
            let prev = patch[..idx].chars().last().unwrap_or(' ');
            !prev.is_alphanumeric() && prev != '_' && prev != '$'
        };
        if before_ok {
            let rest = &patch[idx + 8..];
            let trimmed = rest.trim_start();
            if trimmed.starts_with('(') {
                return true;
            }
        }
        search_from_f = idx + 8;
    }
    false
}

fn has_type_mismatch(patch: &str) -> bool {
    if patch.contains("TYPE_ERROR_TRIGGER") || patch.contains("SYNTAX_ERROR_TRIGGER") {
        return true;
    }
    for line in patch.lines() {
        let t = line.trim();
        let target = if let Some(stripped) = t.strip_prefix('+') {
            stripped.trim()
        } else {
            t
        };

        // Check for literal string = number/bool/obj/arr or semantic mismatch
        if let Some(pos) = target.find(": string") {
            let after = target[pos + 8..].trim_start();
            if let Some(eq_val) = after.strip_prefix('=') {
                let v = eq_val.trim_start();
                if v.starts_with(|c: char| c.is_ascii_digit())
                    || v.starts_with("true")
                    || v.starts_with("false")
                    || v.starts_with('{')
                    || v.starts_with('[')
                {
                    return true;
                }
                if let Some(paren_pos) = v.find('(') {
                    let fn_name = v[..paren_pos].trim().to_ascii_lowercase();
                    if fn_name.contains("numeric")
                        || fn_name.contains("count")
                        || fn_name.contains("amount")
                        || fn_name.contains("timestamp")
                    {
                        return true;
                    }
                }
            }
        }
        // Check for literal number = string/bool/obj/arr or semantic mismatch
        if let Some(pos) = target.find(": number") {
            let after = target[pos + 8..].trim_start();
            if let Some(eq_val) = after.strip_prefix('=') {
                let v = eq_val.trim_start();
                if v.starts_with('\'')
                    || v.starts_with('"')
                    || v.starts_with('`')
                    || v.starts_with("true")
                    || v.starts_with("false")
                    || v.starts_with('{')
                    || v.starts_with('[')
                {
                    return true;
                }
                if let Some(paren_pos) = v.find('(') {
                    let fn_name = v[..paren_pos].trim().to_ascii_lowercase();
                    if fn_name.contains("string")
                        || fn_name.contains("text")
                        || fn_name.contains("name")
                        || fn_name.contains("format")
                    {
                        return true;
                    }
                }
            }
        }
        // Check for literal boolean = string/number/obj/arr or semantic mismatch
        if let Some(pos) = target.find(": boolean") {
            let after = target[pos + 9..].trim_start();
            if let Some(eq_val) = after.strip_prefix('=') {
                let v = eq_val.trim_start();
                if v.starts_with('\'')
                    || v.starts_with('"')
                    || v.starts_with('`')
                    || v.starts_with(|c: char| c.is_ascii_digit())
                    || v.starts_with('{')
                    || v.starts_with('[')
                {
                    return true;
                }
                if let Some(paren_pos) = v.find('(') {
                    let fn_name = v[..paren_pos].trim().to_ascii_lowercase();
                    if fn_name.contains("numeric")
                        || fn_name.contains("string")
                        || fn_name.contains("array")
                    {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn has_syntax_errors(files: &[FileDiffItemDto]) -> bool {
    for f in files {
        if f.patch.contains("SYNTAX_ERROR_TRIGGER") {
            return true;
        }
        let added_lines: Vec<&str> = f
            .patch
            .lines()
            .filter(|l| l.starts_with('+'))
            .map(|l| &l[1..])
            .collect();
        let added = if added_lines.is_empty() {
            f.patch.as_str()
        } else {
            let mut s = String::new();
            for l in &added_lines {
                s.push_str(l);
                s.push('\n');
            }
            if s.contains("const =") || s.contains("let =") || s.contains("var =") {
                return true;
            }
            let open_braces = s.chars().filter(|&c| c == '{').count();
            let close_braces = s.chars().filter(|&c| c == '}').count();
            if open_braces != close_braces {
                return true;
            }
            let open_parens = s.chars().filter(|&c| c == '(').count();
            let close_parens = s.chars().filter(|&c| c == ')').count();
            if open_parens != close_parens {
                return true;
            }
            let open_brackets = s.chars().filter(|&c| c == '[').count();
            let close_brackets = s.chars().filter(|&c| c == ']').count();
            if open_brackets != close_brackets {
                return true;
            }
            continue;
        };

        if added.contains("const =") || added.contains("let =") || added.contains("var =") {
            return true;
        }
        let open_braces = added.chars().filter(|&c| c == '{').count();
        let close_braces = added.chars().filter(|&c| c == '}').count();
        if open_braces != close_braces {
            return true;
        }
        let open_parens = added.chars().filter(|&c| c == '(').count();
        let close_parens = added.chars().filter(|&c| c == ')').count();
        if open_parens != close_parens {
            return true;
        }
        let open_brackets = added.chars().filter(|&c| c == '[').count();
        let close_brackets = added.chars().filter(|&c| c == ']').count();
        if open_brackets != close_brackets {
            return true;
        }
    }
    false
}

fn extract_relative_import_specifiers(patch: &str) -> Vec<String> {
    let mut specs = Vec::new();
    for line in patch.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('-') && !trimmed.starts_with("---") {
            continue;
        }
        let body = trimmed.strip_prefix('+').unwrap_or(trimmed);
        for marker in ["from '", "from \"", "import('", "import(\""] {
            let quote = if marker.ends_with('\'') { '\'' } else { '"' };
            let mut rest = body;
            while let Some(idx) = rest.find(marker) {
                let after = &rest[idx + marker.len()..];
                if let Some(end_idx) = after.find(quote) {
                    let spec = after[..end_idx].trim();
                    if spec.starts_with("./") || spec.starts_with("../") {
                        specs.push(spec.to_string());
                    }
                    rest = &after[end_idx + 1..];
                } else {
                    break;
                }
            }
        }
    }
    specs
}

fn normalize_rel_path(base_file: &str, rel_spec: &str) -> Option<String> {
    let mut parts: Vec<String> = base_file
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .map(String::from)
        .collect();
    // drop filename of base_file
    parts.pop();
    for seg in rel_spec.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other.to_string()),
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn relative_import_candidates(norm: &str) -> Vec<String> {
    let lower = norm.to_ascii_lowercase();
    if lower.ends_with(".ts")
        || lower.ends_with(".tsx")
        || lower.ends_with(".js")
        || lower.ends_with(".jsx")
        || lower.ends_with(".json")
    {
        vec![norm.to_string()]
    } else {
        vec![
            norm.to_string(),
            format!("{norm}.ts"),
            format!("{norm}.tsx"),
            format!("{norm}.js"),
            format!("{norm}.jsx"),
            format!("{norm}.json"),
            format!("{norm}/index.ts"),
            format!("{norm}/index.tsx"),
            format!("{norm}/index.js"),
        ]
    }
}

fn sync_workspace_file_to_disk(
    canon_project_root: &Path,
    rel_path: &str,
    is_deleted: bool,
    content: &str,
) -> Result<(), HarnessError> {
    let clean_rel = validate_safe_relative_path(rel_path)?;
    let mut rel_parts = clean_rel.split('/').collect::<Vec<_>>();
    let Some(file_name) = rel_parts.pop() else {
        return Err(HarnessError::PathTraversalDenied {
            path: rel_path.to_string(),
        });
    };

    #[cfg(unix)]
    {
        use std::ffi::{CString, OsStr};
        use std::os::fd::AsRawFd;
        let mut dir_fd = crate::routes::fs_jail::open_dir_abs_nofollow(canon_project_root)?;
        for seg in &rel_parts {
            if is_deleted {
                match crate::routes::fs_jail::open_child_dir_fd_nofollow(&dir_fd, OsStr::new(seg))?
                {
                    Some(child) => dir_fd = child,
                    None => return Ok(()),
                }
            } else {
                dir_fd =
                    crate::routes::fs_jail::ensure_child_dir_fd(&dir_fd, OsStr::new(seg), 0o755)?;
            }
        }
        if is_deleted {
            let c_name = CString::new(file_name.as_bytes()).map_err(|_| {
                HarnessError::PathTraversalDenied {
                    path: rel_path.to_string(),
                }
            })?;
            let mut st: libc::stat = unsafe { std::mem::zeroed() };
            let rc = unsafe {
                libc::fstatat(
                    dir_fd.as_raw_fd(),
                    c_name.as_ptr(),
                    &mut st,
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            };
            if rc != 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::ENOENT) {
                    return Ok(());
                }
                return Err(HarnessError::ValidationFailed {
                    reason: format!("DISK_UNLINK_STAT_FAILED for '{rel_path}': {err}"),
                });
            }
            let kind = st.st_mode & libc::S_IFMT;
            if kind == libc::S_IFLNK || kind == libc::S_IFDIR {
                return Err(HarnessError::PathTraversalDenied {
                    path: rel_path.to_string(),
                });
            }
            let u_rc = unsafe { libc::unlinkat(dir_fd.as_raw_fd(), c_name.as_ptr(), 0) };
            if u_rc != 0 {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "DISK_UNLINK_FAILED for '{rel_path}': {}",
                        std::io::Error::last_os_error()
                    ),
                });
            }
            return Ok(());
        }
        crate::routes::fs_jail::write_file_in_dir_fd(
            &dir_fd,
            OsStr::new(file_name),
            content.as_bytes(),
            0o644,
        )
    }

    #[cfg(not(unix))]
    {
        let mut cur = canon_project_root.to_path_buf();
        for seg in &rel_parts {
            cur = cur.join(seg);
        }
        let target_path = cur.join(file_name);
        if is_deleted {
            if target_path.exists() {
                std::fs::remove_file(&target_path).map_err(|e| HarnessError::ValidationFailed {
                    reason: format!("DISK_UNLINK_FAILED for '{}': {e}", target_path.display()),
                })?;
            }
            return Ok(());
        }
        std::fs::create_dir_all(&cur).map_err(|e| HarnessError::ValidationFailed {
            reason: format!("DISK_MKDIR_FAILED for '{}': {e}", cur.display()),
        })?;
        std::fs::write(&target_path, content.as_bytes()).map_err(|e| {
            HarnessError::ValidationFailed {
                reason: format!("DISK_WRITE_FAILED for '{}': {e}", target_path.display()),
            }
        })?;
        Ok(())
    }
}

fn has_missing_relative_imports_with_context(
    files: &[FileDiffItemDto],
    canon_project_root: Option<&Path>,
    db_paths: &HashSet<String>,
) -> bool {
    let mut batch_paths = HashSet::new();
    for f in files {
        if !f.status.eq_ignore_ascii_case("deleted") {
            if let Ok(clean) = validate_safe_relative_path(&f.path) {
                batch_paths.insert(clean);
            }
        }
    }

    for f in files {
        if f.patch.contains("MISSING_IMPORT_TRIGGER") {
            return true;
        }
        if f.status.eq_ignore_ascii_case("deleted") {
            continue;
        }
        let base_rel = match validate_safe_relative_path(&f.path) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for spec in extract_relative_import_specifiers(&f.patch) {
            let Some(norm) = normalize_rel_path(&base_rel, &spec) else {
                return true;
            };
            let candidates = relative_import_candidates(&norm);
            let exists_in_batch = candidates.iter().any(|c| batch_paths.contains(c));
            let exists_in_db = candidates.iter().any(|c| db_paths.contains(c));
            let exists_on_disk = canon_project_root.is_some_and(|root| {
                candidates.iter().any(|c| {
                    let p = root.join(c);
                    std::fs::symlink_metadata(&p)
                        .map(|m| m.file_type().is_file() && !m.file_type().is_symlink())
                        .unwrap_or(false)
                })
            });
            if !exists_in_batch && !exists_in_db && !exists_on_disk {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
fn has_missing_relative_imports(files: &[FileDiffItemDto]) -> bool {
    let empty_db = HashSet::new();
    has_missing_relative_imports_with_context(files, None, &empty_db)
}

fn run_proposal_validation_checks(
    registry: &SpecRegistry,
    canon_project_root: &Path,
    project_id: Uuid,
    files: &[FileDiffItemDto],
    timestamp: &str,
) -> Result<(String, Vec<ValidationCheckDto>), HarnessError> {
    if files.is_empty() {
        return Err(HarnessError::ValidationFailed {
            reason: "PROPOSAL_EMPTY: at least one file patch is required".to_string(),
        });
    }
    for f in files {
        validate_safe_relative_path(&f.path)?;
    }

    let conn = registry.pool().connection()?;
    let proj_str = project_id.to_string();
    let constraints =
        constraint_engine::ConstraintEvaluator::load_constraints_for_project(&conn, &proj_str)?;
    let (icon_manifest, typography_contract, component_registry) =
        constraint_engine::IconValidator::load_project_design_context(
            &conn,
            canon_project_root,
            &proj_str,
        )?;

    let mut ast_failures = Vec::new();
    for f in files {
        if has_forbidden_dynamic_execution_pattern(&f.patch) {
            ast_failures.push(format!(
                "{}: forbidden dynamic execution/HTML pattern",
                f.path
            ));
        }
        let lower_path = f.path.to_ascii_lowercase();
        let is_js_ts = lower_path.ends_with(".tsx")
            || lower_path.ends_with(".ts")
            || lower_path.ends_with(".jsx")
            || lower_path.ends_with(".js");
        if is_js_ts {
            let source_to_lint = reconstruct_lintable_patch_source(&f.patch);
            let violations = constraint_engine::AstLinter::lint_source_with_context(
                &source_to_lint,
                &constraints,
                Some(&icon_manifest),
                Some(&typography_contract),
                Some(&component_registry),
            )?;
            for v in violations {
                if matches!(
                    v.severity.to_ascii_uppercase().as_str(),
                    "BLOCKER" | "WARN" | "WARNING"
                ) {
                    ast_failures.push(format!("{}: [{}] {}", f.path, v.rule_code, v.message));
                }
            }
        } else if lower_path.ends_with(".json")
            && serde_json::from_str::<serde_json::Value>(&f.patch).is_err()
        {
            ast_failures.push(format!("{}: invalid JSON payload", f.path));
        }
    }

    let ast_passed = ast_failures.is_empty();
    let ast_msg = if ast_passed {
        format!(
            "AST and design contracts verified across {} file(s)",
            files.len()
        )
    } else {
        ast_failures.join("; ")
    };

    let existing_workspace_paths: HashSet<String> =
        load_workspace_files_from_db(&conn, project_id)?
            .into_iter()
            .filter_map(|item| validate_safe_relative_path(&item.path).ok())
            .collect();
    let type_mismatch = files.iter().any(|f| has_type_mismatch(&f.patch));
    let missing_imports = has_missing_relative_imports_with_context(
        files,
        Some(canon_project_root),
        &existing_workspace_paths,
    );
    let typecheck_failed = type_mismatch || missing_imports;

    let syntax_err = has_syntax_errors(files);
    let build_failed = syntax_err || missing_imports || type_mismatch;

    let typecheck_status = if typecheck_failed { "failed" } else { "passed" };
    let typecheck_msg = if typecheck_failed {
        "TypeScript static contract validation failed"
    } else {
        "Type contracts verified"
    };

    let build_status = if build_failed { "failed" } else { "passed" };
    let build_msg = if build_failed {
        "Build packaging and syntax validation failed"
    } else {
        "Build artifact packaging verified"
    };

    let checks = vec![
        ValidationCheckDto {
            check_type: "ast_lint".to_string(),
            status: if ast_passed { "passed" } else { "failed" }.to_string(),
            message: ast_msg,
            timestamp: timestamp.to_string(),
        },
        ValidationCheckDto {
            check_type: "typecheck".to_string(),
            status: typecheck_status.to_string(),
            message: typecheck_msg.to_string(),
            timestamp: timestamp.to_string(),
        },
        ValidationCheckDto {
            check_type: "build".to_string(),
            status: build_status.to_string(),
            message: build_msg.to_string(),
            timestamp: timestamp.to_string(),
        },
    ];

    let all_passed = ast_passed && !typecheck_failed && !build_failed;
    let validation_status = if all_passed { "passed" } else { "failed" }.to_string();
    Ok((validation_status, checks))
}

fn map_proposal_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChangeProposalDto> {
    let id_str: String = row.get(0)?;
    let project_id_str: String = row.get(1)?;
    let tenant_id: String = row.get(2)?;
    let intent_id_str: Option<String> = row.get(3)?;
    let title: String = row.get(4)?;
    let description: String = row.get(5)?;
    let files_json: String = row.get(6)?;
    let validation_status: String = row.get(7)?;
    let checks_json: String = row.get(8)?;
    let status: String = row.get(9)?;
    let created_at: String = row.get(10)?;
    let updated_at: String = row.get(11)?;
    let node_id: Option<String> = row.get(12)?;
    let batch_id: Option<String> = row.get(13)?;
    let composite_scope_key: String = row.get(14)?;
    let content_mode: String = row.get(15)?;
    let base_file_hashes_json: String = row.get(16)?;
    let visual_diff_json: String = row.get(17)?;
    let gate_receipt_id: Option<String> = row.get(18)?;

    let visual_diff: Option<VisualDiffResultDto> = if visual_diff_json.trim().is_empty()
        || visual_diff_json.trim() == "{}"
        || visual_diff_json.trim() == "null"
    {
        None
    } else {
        serde_json::from_str(&visual_diff_json).ok()
    };

    Ok(ChangeProposalDto {
        id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::nil()),
        tenant_id,
        project_id: Uuid::parse_str(&project_id_str).unwrap_or_else(|_| Uuid::nil()),
        intent_id: intent_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
        title,
        description,
        files: serde_json::from_str(&files_json).unwrap_or_default(),
        validation_status,
        validation_checks: serde_json::from_str(&checks_json).unwrap_or_default(),
        status,
        created_at,
        updated_at,
        node_id,
        batch_id,
        composite_scope_key,
        content_mode: if content_mode.trim().is_empty() {
            default_content_mode()
        } else {
            content_mode
        },
        base_file_hashes: serde_json::from_str(&base_file_hashes_json).unwrap_or_default(),
        node_ids: Vec::new(),
        gate_receipt_id,
        gate_receipt: None,
        visual_diff,
        tradeoffs: Vec::new(),
    })
}

fn map_tradeoff_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProposalTradeoffDto> {
    let id_str: String = row.get(0)?;
    let proposal_id_str: String = row.get(1)?;
    let node_id: Option<String> = row.get(2)?;
    let element_ff_id: String = row.get(3)?;
    let badge_index: i32 = row.get(4)?;
    let design_note: String = row.get(5)?;
    let instead_label: String = row.get(6)?;
    let instead_patch_json: String = row.get(7)?;
    let decision: String = row.get(8)?;
    let ask_prompt: Option<String> = row.get(9)?;
    let decided_at: Option<String> = row.get(10)?;
    let created_at: String = row.get(11)?;

    let instead_patch = match serde_json::from_str::<serde_json::Value>(&instead_patch_json) {
        Ok(serde_json::Value::Null) | Err(_) => None,
        Ok(val) => Some(val),
    };

    Ok(ProposalTradeoffDto {
        id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::nil()),
        proposal_id: Uuid::parse_str(&proposal_id_str).unwrap_or_else(|_| Uuid::nil()),
        node_id,
        element_ff_id,
        badge_index,
        design_note,
        instead_label,
        instead_patch,
        decision,
        ask_prompt,
        decided_at,
        created_at,
    })
}

fn hydrate_proposal_details(
    conn: &rusqlite::Connection,
    proposal: &mut ChangeProposalDto,
) -> Result<(), HarnessError> {
    let pid_str = proposal.id.to_string();
    let proj_str = proposal.project_id.to_string();

    {
        let mut stmt = conn
            .prepare(
                "SELECT node_id FROM proposal_target_nodes
                 WHERE proposal_id = ?1 AND project_id = ?2 AND tenant_id = ?3
                 ORDER BY node_id ASC",
            )
            .map_err(|e| map_sqlite_error(e, "prepare_load_target_nodes"))?;
        let rows = stmt
            .query_map(params![pid_str, proj_str, proposal.tenant_id], |r| r.get(0))
            .map_err(|e| map_sqlite_error(e, "query_load_target_nodes"))?;
        let mut node_ids = Vec::new();
        for r in rows {
            node_ids.push(r.map_err(|e| map_sqlite_error(e, "row_load_target_nodes"))?);
        }
        proposal.node_ids = node_ids;
    }

    if let Some(ref receipt_id) = proposal.gate_receipt_id {
        proposal.gate_receipt = conn
            .query_row(
                "SELECT id, proposal_id, diff_sha256, constraints_sha256,
                        workspace_head_sha256, exit_code, receipt_hmac, created_at
                 FROM proposal_gate_receipts
                 WHERE id = ?1 AND proposal_id = ?2 AND project_id = ?3 AND tenant_id = ?4",
                params![receipt_id, pid_str, proj_str, proposal.tenant_id],
                |r| {
                    let id_s: String = r.get(0)?;
                    let prop_s: String = r.get(1)?;
                    Ok(ProposalGateReceiptDto {
                        id: Uuid::parse_str(&id_s).unwrap_or_else(|_| Uuid::nil()),
                        proposal_id: Uuid::parse_str(&prop_s).unwrap_or_else(|_| Uuid::nil()),
                        diff_sha256: r.get(2)?,
                        constraints_sha256: r.get(3)?,
                        workspace_head_sha256: r.get(4)?,
                        exit_code: r.get(5)?,
                        receipt_hmac: r.get(6)?,
                        created_at: r.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "load_proposal_gate_receipt"))?;
    }

    {
        let mut stmt = conn
            .prepare(
                "SELECT id, proposal_id, node_id, element_ff_id, badge_index,
                        design_note, instead_label, instead_patch_json,
                        decision, ask_prompt, decided_at, created_at
                 FROM proposal_tradeoffs
                 WHERE proposal_id = ?1 AND project_id = ?2 AND tenant_id = ?3
                 ORDER BY badge_index ASC",
            )
            .map_err(|e| map_sqlite_error(e, "prepare_load_tradeoffs"))?;
        let rows = stmt
            .query_map(
                params![pid_str, proj_str, proposal.tenant_id],
                map_tradeoff_row,
            )
            .map_err(|e| map_sqlite_error(e, "query_load_tradeoffs"))?;
        let mut tradeoffs = Vec::new();
        for r in rows {
            tradeoffs.push(r.map_err(|e| map_sqlite_error(e, "row_load_tradeoffs"))?);
        }
        proposal.tradeoffs = tradeoffs;
    }

    Ok(())
}

fn proposal_not_found(id: Uuid) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "code": "PROPOSAL_NOT_FOUND",
            "error": "PROPOSAL_NOT_FOUND",
            "message": format!("ChangeProposal '{id}' was not found for this project/tenant"),
        })),
    )
        .into_response()
}

fn snapshot_not_found(id: Uuid) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "code": "SNAPSHOT_NOT_FOUND",
            "error": "SNAPSHOT_NOT_FOUND",
            "message": format!("VersionSnapshot '{id}' was not found for this project/tenant"),
        })),
    )
        .into_response()
}

const PROPOSAL_SELECT_COLS: &str =
    "id, project_id, tenant_id, intent_id, title, description, files_json,
    validation_status, validation_checks_json, status, created_at, updated_at,
    node_id, batch_id, composite_scope_key, content_mode, base_file_hashes_json,
    visual_diff_json, gate_receipt_id";

fn load_proposal_by_id(
    conn: &rusqlite::Connection,
    project_id: Uuid,
    proposal_id: Uuid,
    tenant_id: &str,
) -> Result<Option<ChangeProposalDto>, HarnessError> {
    let sql = format!(
        "SELECT {PROPOSAL_SELECT_COLS}
         FROM change_proposals
         WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3"
    );
    let mut found = conn
        .query_row(
            &sql,
            params![proposal_id.to_string(), project_id.to_string(), tenant_id],
            map_proposal_row,
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "load_proposal_by_id"))?;
    if let Some(ref mut proposal) = found {
        hydrate_proposal_details(conn, proposal)?;
    }
    Ok(found)
}

fn proposal_receipt_hmac_key(state: &DaemonAppState) -> Vec<u8> {
    if let Ok(lock) = crate::lockfile::read_daemon_lock(state.registry.pool().workspace_root()) {
        if let Some(bytes) = decode_hex(lock.hmac_key_hex.trim()) {
            if bytes.len() >= 16 {
                return bytes;
            }
        }
    }
    static PROCESS_KEY: std::sync::OnceLock<[u8; 32]> = std::sync::OnceLock::new();
    let base = PROCESS_KEY.get_or_init(|| {
        use rand::RngCore;
        let mut buf = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut buf);
        buf
    });
    let mut hasher = Sha256::new();
    hasher.update(base);
    hasher.update(b":");
    hasher.update(state.auth.daemon_nonce.as_bytes());
    hasher.finalize().to_vec()
}

fn validate_content_mode(raw: Option<&str>) -> Result<String, HarnessError> {
    let Some(val) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok("full_file".to_string());
    };
    if matches!(val, "full_file" | "unified_diff" | "surgical_patch") {
        Ok(val.to_string())
    } else {
        Err(HarnessError::ValidationFailed {
            reason: format!(
                "INVALID_CONTENT_MODE: '{val}' must be one of full_file, unified_diff, surgical_patch"
            ),
        })
    }
}

fn validate_tradeoffs_input(tradeoffs: &[CreateTradeoffInput]) -> Result<(), HarnessError> {
    if tradeoffs.len() > MAX_TRADEOFFS_PER_PROPOSAL {
        return Err(HarnessError::ValidationFailed {
            reason: format!(
                "TRADEOFF_LIMIT_EXCEEDED: at most {MAX_TRADEOFFS_PER_PROPOSAL} tradeoffs allowed per proposal"
            ),
        });
    }
    let mut seen = HashSet::new();
    for t in tradeoffs {
        if !(1..=12).contains(&t.badge_index) {
            return Err(HarnessError::ValidationFailed {
                reason: format!(
                    "INVALID_BADGE_INDEX: badgeIndex {} must be between 1 and 12",
                    t.badge_index
                ),
            });
        }
        if !seen.insert(t.badge_index) {
            return Err(HarnessError::ValidationFailed {
                reason: format!(
                    "DUPLICATE_BADGE_INDEX: badgeIndex {} appears more than once",
                    t.badge_index
                ),
            });
        }
        if t.element_ff_id.trim().is_empty() {
            return Err(HarnessError::ValidationFailed {
                reason: "INVALID_ELEMENT_FF_ID: elementFfId must not be empty".to_string(),
            });
        }
    }
    Ok(())
}

fn read_disk_file_bytes_jailed(
    canon_project_root: &Path,
    rel_path: &str,
) -> Result<Option<Vec<u8>>, HarnessError> {
    let clean_rel = validate_safe_relative_path(rel_path)?;
    let mut parts: Vec<&str> = clean_rel.split('/').collect();
    let Some(file_name) = parts.pop() else {
        return Err(HarnessError::PathTraversalDenied {
            path: rel_path.to_string(),
        });
    };

    #[cfg(unix)]
    {
        use std::ffi::{CString, OsStr};
        use std::io::Read;
        use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
        let mut dir_fd = crate::routes::fs_jail::open_dir_abs_nofollow(canon_project_root)?;
        for seg in parts {
            match crate::routes::fs_jail::open_child_dir_fd_nofollow(&dir_fd, OsStr::new(seg))? {
                Some(child) => dir_fd = child,
                None => return Ok(None),
            }
        }
        let c_name =
            CString::new(file_name.as_bytes()).map_err(|_| HarnessError::PathTraversalDenied {
                path: rel_path.to_string(),
            })?;
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        let rc = unsafe {
            libc::fstatat(
                dir_fd.as_raw_fd(),
                c_name.as_ptr(),
                &mut st,
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if rc != 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::ENOENT) {
                return Ok(None);
            }
            return Err(HarnessError::ValidationFailed {
                reason: format!("DISK_STAT_FAILED for '{rel_path}': {err}"),
            });
        }
        let kind = st.st_mode & libc::S_IFMT;
        if kind == libc::S_IFLNK {
            return Err(HarnessError::PathTraversalDenied {
                path: rel_path.to_string(),
            });
        }
        if kind != libc::S_IFREG {
            return Ok(None);
        }
        let fd = unsafe {
            libc::openat(
                dir_fd.as_raw_fd(),
                c_name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW,
            )
        };
        if fd < 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::ELOOP) {
                return Err(HarnessError::PathTraversalDenied {
                    path: rel_path.to_string(),
                });
            }
            return Err(HarnessError::ValidationFailed {
                reason: format!("DISK_READ_FAILED for '{rel_path}': {err}"),
            });
        }
        let owned = unsafe { OwnedFd::from_raw_fd(fd) };
        let mut file = std::fs::File::from(owned);
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .map_err(|e| HarnessError::ValidationFailed {
                reason: format!("DISK_READ_BYTES_FAILED for '{rel_path}': {e}"),
            })?;
        Ok(Some(buf))
    }

    #[cfg(not(unix))]
    {
        let mut target = canon_project_root.to_path_buf();
        for seg in parts {
            target = target.join(seg);
        }
        target = target.join(file_name);
        match std::fs::symlink_metadata(&target) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(HarnessError::PathTraversalDenied {
                        path: rel_path.to_string(),
                    });
                }
                if !meta.file_type().is_file() {
                    return Ok(None);
                }
                let bytes = std::fs::read(&target).map_err(|e| HarnessError::ValidationFailed {
                    reason: format!("DISK_READ_FAILED for '{rel_path}': {e}"),
                })?;
                Ok(Some(bytes))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(HarnessError::ValidationFailed {
                reason: format!("DISK_STAT_FAILED for '{rel_path}': {e}"),
            }),
        }
    }
}

fn read_disk_file_sha256_jailed(
    canon_project_root: &Path,
    rel_path: &str,
) -> Result<Option<String>, HarnessError> {
    let bytes = read_disk_file_bytes_jailed(canon_project_root, rel_path)?;
    Ok(bytes.map(|b| format!("{:x}", Sha256::digest(&b))))
}

fn read_db_file_sha256(
    conn: &rusqlite::Connection,
    project_id: Uuid,
    rel_path: &str,
) -> Result<Option<String>, HarnessError> {
    conn.query_row(
        "SELECT file_sha256 FROM project_workspace_files
         WHERE project_id = ?1 AND relative_path = ?2",
        params![project_id.to_string(), rel_path],
        |r| r.get(0),
    )
    .optional()
    .map_err(|e| map_sqlite_error(e, "read_db_file_sha256"))
}

fn snapshot_current_hashes_for_paths<'a, I>(
    conn: &rusqlite::Connection,
    canon_project_root: &Path,
    project_id: Uuid,
    paths: I,
) -> Result<BTreeMap<String, String>, HarnessError>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut hashes = BTreeMap::new();
    for raw in paths {
        let rel = validate_safe_relative_path(raw)?;
        if hashes.contains_key(&rel) {
            continue;
        }
        let disk_sha = read_disk_file_sha256_jailed(canon_project_root, &rel)?;
        let db_sha = read_db_file_sha256(conn, project_id, &rel)?;
        hashes.insert(rel, disk_sha.or(db_sha).unwrap_or_default());
    }
    Ok(hashes)
}

fn resolve_base_file_hashes(
    conn: &rusqlite::Connection,
    canon_project_root: &Path,
    project_id: Uuid,
    files: &[FileDiffItemDto],
    provided: Option<&BTreeMap<String, String>>,
) -> Result<BTreeMap<String, String>, HarnessError> {
    let mut result = BTreeMap::new();
    if let Some(map) = provided {
        for (raw_path, hash) in map {
            let rel = validate_safe_relative_path(raw_path)?;
            result.insert(rel, hash.trim().to_string());
        }
    }
    for f in files {
        let rel = validate_safe_relative_path(&f.path)?;
        if let std::collections::btree_map::Entry::Vacant(e) = result.entry(rel) {
            let disk_sha = read_disk_file_sha256_jailed(canon_project_root, e.key())?;
            let db_sha = read_db_file_sha256(conn, project_id, e.key())?;
            e.insert(disk_sha.or(db_sha).unwrap_or_default());
        }
    }
    Ok(result)
}

fn compute_diff_sha256(files: &[FileDiffItemDto]) -> Result<String, HarnessError> {
    let mut sorted = files.to_vec();
    for f in &mut sorted {
        f.path = validate_safe_relative_path(&f.path)?;
    }
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let canonical = serde_json::to_vec(&sorted).map_err(|e| HarnessError::ValidationFailed {
        reason: format!("DIFF_SHA_SERIALIZE_ERROR: {e}"),
    })?;
    Ok(format!("{:x}", Sha256::digest(&canonical)))
}

fn compute_constraints_sha256(
    conn: &rusqlite::Connection,
    project_id: Uuid,
) -> Result<String, HarnessError> {
    let mut constraints = constraint_engine::ConstraintEvaluator::load_constraints_for_project(
        conn,
        &project_id.to_string(),
    )?;
    constraints.sort_by(|a, b| a.rule_code.cmp(&b.rule_code));
    let canonical_items: Vec<(&str, &str, &str)> = constraints
        .iter()
        .map(|c| {
            (
                c.rule_code.as_str(),
                c.severity.as_str(),
                c.config_json.as_str(),
            )
        })
        .collect();
    let bytes =
        serde_json::to_vec(&canonical_items).map_err(|e| HarnessError::ValidationFailed {
            reason: format!("CONSTRAINTS_SHA_SERIALIZE_ERROR: {e}"),
        })?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

fn compute_scoped_workspace_head_sha256(
    conn: &rusqlite::Connection,
    canon_project_root: &Path,
    project_id: Uuid,
    files: &[FileDiffItemDto],
    base_hashes: &BTreeMap<String, String>,
) -> Result<String, HarnessError> {
    let paths: Vec<&str> = files
        .iter()
        .map(|f| f.path.as_str())
        .chain(base_hashes.keys().map(String::as_str))
        .collect();
    let current = snapshot_current_hashes_for_paths(conn, canon_project_root, project_id, paths)?;
    let bytes = serde_json::to_vec(&current).map_err(|e| HarnessError::ValidationFailed {
        reason: format!("WORKSPACE_HEAD_SERIALIZE_ERROR: {e}"),
    })?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

#[allow(clippy::too_many_arguments)]
fn compute_receipt_hmac_hex(
    hmac_key: &[u8],
    receipt_id: Uuid,
    tenant_id: &str,
    project_id: Uuid,
    proposal_id: Uuid,
    diff_sha256: &str,
    constraints_sha256: &str,
    workspace_head_sha256: &str,
    checks_json: &str,
    exit_code: i32,
    created_at: &str,
) -> String {
    let canonical = format!(
        "{receipt_id}\n{tenant_id}\n{project_id}\n{proposal_id}\n{diff_sha256}\n{constraints_sha256}\n{workspace_head_sha256}\n{checks_json}\n{exit_code}\n{created_at}"
    );
    hmac_sha256(hmac_key, canonical.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn build_gate_receipt(
    state: &DaemonAppState,
    conn: &rusqlite::Connection,
    canon_project_root: &Path,
    tenant_id: &str,
    project_id: Uuid,
    proposal_id: Uuid,
    files: &[FileDiffItemDto],
    base_hashes: &BTreeMap<String, String>,
    checks_json: &str,
    exit_code: i32,
    created_at: &str,
) -> Result<ProposalGateReceiptDto, HarnessError> {
    let receipt_id = Uuid::new_v4();
    let diff_sha256 = compute_diff_sha256(files)?;
    let constraints_sha256 = compute_constraints_sha256(conn, project_id)?;
    let workspace_head_sha256 = compute_scoped_workspace_head_sha256(
        conn,
        canon_project_root,
        project_id,
        files,
        base_hashes,
    )?;
    let key = proposal_receipt_hmac_key(state);
    let receipt_hmac = compute_receipt_hmac_hex(
        &key,
        receipt_id,
        tenant_id,
        project_id,
        proposal_id,
        &diff_sha256,
        &constraints_sha256,
        &workspace_head_sha256,
        checks_json,
        exit_code,
        created_at,
    );
    Ok(ProposalGateReceiptDto {
        id: receipt_id,
        proposal_id,
        diff_sha256,
        constraints_sha256,
        workspace_head_sha256,
        exit_code,
        receipt_hmac,
        created_at: created_at.to_string(),
    })
}

fn insert_gate_receipt_tx(
    tx: &rusqlite::Transaction<'_>,
    tenant_id: &str,
    project_id: Uuid,
    receipt: &ProposalGateReceiptDto,
    checks_json: &str,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO proposal_gate_receipts (
            id, tenant_id, project_id, proposal_id, diff_sha256,
            constraints_sha256, workspace_head_sha256, checks_json,
            exit_code, receipt_hmac, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            receipt.id.to_string(),
            tenant_id,
            project_id.to_string(),
            receipt.proposal_id.to_string(),
            receipt.diff_sha256,
            receipt.constraints_sha256,
            receipt.workspace_head_sha256,
            checks_json,
            receipt.exit_code,
            receipt.receipt_hmac,
            receipt.created_at,
        ],
    )?;
    Ok(())
}

fn unverified_gate_claim(reason: &str) -> Response {
    (
        StatusCode::PRECONDITION_FAILED,
        Json(serde_json::json!({
            "code": "UNVERIFIED_GATE_CLAIM",
            "error": "UNVERIFIED_GATE_CLAIM",
            "message": format!("Gate receipt verification failed: {reason}"),
        })),
    )
        .into_response()
}

fn verify_receipt_cryptography(
    state: &DaemonAppState,
    conn: &rusqlite::Connection,
    proposal: &ChangeProposalDto,
) -> Result<(ProposalGateReceiptDto, String), Result<Response, HarnessError>> {
    let Some(ref receipt_id) = proposal.gate_receipt_id else {
        return Err(Ok(unverified_gate_claim(
            "missing gate_receipt_id on proposal",
        )));
    };
    let row_opt = conn
        .query_row(
            "SELECT id, proposal_id, diff_sha256, constraints_sha256,
                    workspace_head_sha256, checks_json, exit_code, receipt_hmac, created_at
             FROM proposal_gate_receipts
             WHERE id = ?1 AND proposal_id = ?2 AND project_id = ?3 AND tenant_id = ?4",
            params![
                receipt_id,
                proposal.id.to_string(),
                proposal.project_id.to_string(),
                proposal.tenant_id
            ],
            |r| {
                let id_s: String = r.get(0)?;
                let prop_s: String = r.get(1)?;
                let checks_json: String = r.get(5)?;
                Ok((
                    ProposalGateReceiptDto {
                        id: Uuid::parse_str(&id_s).unwrap_or_else(|_| Uuid::nil()),
                        proposal_id: Uuid::parse_str(&prop_s).unwrap_or_else(|_| Uuid::nil()),
                        diff_sha256: r.get(2)?,
                        constraints_sha256: r.get(3)?,
                        workspace_head_sha256: r.get(4)?,
                        exit_code: r.get(6)?,
                        receipt_hmac: r.get(7)?,
                        created_at: r.get(8)?,
                    },
                    checks_json,
                ))
            },
        )
        .optional()
        .map_err(|e| Err(map_sqlite_error(e, "verify_receipt_lookup")))?;

    let Some((receipt, checks_json)) = row_opt else {
        return Err(Ok(unverified_gate_claim(
            "referenced gate receipt record does not exist",
        )));
    };
    if receipt.exit_code != 0 {
        return Err(Ok(unverified_gate_claim(
            "gate receipt exit_code is non-zero",
        )));
    }
    let key = proposal_receipt_hmac_key(state);
    let expected_hmac = compute_receipt_hmac_hex(
        &key,
        receipt.id,
        &proposal.tenant_id,
        proposal.project_id,
        proposal.id,
        &receipt.diff_sha256,
        &receipt.constraints_sha256,
        &receipt.workspace_head_sha256,
        &checks_json,
        receipt.exit_code,
        &receipt.created_at,
    );
    if !constant_time_eq(expected_hmac.as_bytes(), receipt.receipt_hmac.as_bytes()) {
        return Err(Ok(unverified_gate_claim("receipt HMAC signature mismatch")));
    }

    let expected_diff_sha = compute_diff_sha256(&proposal.files).map_err(Err)?;
    if !constant_time_eq(expected_diff_sha.as_bytes(), receipt.diff_sha256.as_bytes()) {
        return Err(Ok(unverified_gate_claim(
            "proposal diff_sha256 does not match gate receipt",
        )));
    }
    let expected_constraints_sha =
        compute_constraints_sha256(conn, proposal.project_id).map_err(Err)?;
    if !constant_time_eq(
        expected_constraints_sha.as_bytes(),
        receipt.constraints_sha256.as_bytes(),
    ) {
        return Err(Ok(unverified_gate_claim(
            "project constraints_sha256 does not match gate receipt",
        )));
    }

    Ok((receipt, checks_json))
}

fn find_stale_base_files(
    conn: &rusqlite::Connection,
    canon_project_root: &Path,
    project_id: Uuid,
    base_hashes: &BTreeMap<String, String>,
) -> Result<Vec<String>, HarnessError> {
    let mut stale = Vec::new();
    for (rel_path, expected_hash) in base_hashes {
        let clean_rel = validate_safe_relative_path(rel_path)?;
        let disk_sha = read_disk_file_sha256_jailed(canon_project_root, &clean_rel)?;
        let db_sha = read_db_file_sha256(conn, project_id, &clean_rel)?;
        let actual_sha = disk_sha.or(db_sha);
        let is_stale = if expected_hash.is_empty() {
            actual_sha.is_some()
        } else {
            actual_sha.as_deref() != Some(expected_hash.as_str())
        };
        if is_stale {
            stale.push(clean_rel);
        }
    }
    Ok(stale)
}

fn blend_rgba_over_white(rgba: &[u8]) -> (f64, f64, f64) {
    let a = f64::from(rgba[3]) / 255.0;
    let r = 255.0 + (f64::from(rgba[0]) - 255.0) * a;
    let g = 255.0 + (f64::from(rgba[1]) - 255.0) * a;
    let b = 255.0 + (f64::from(rgba[2]) - 255.0) * a;
    (r, g, b)
}

fn compute_visual_diff_from_rgba(
    before: &[u8],
    after: &[u8],
    width: u32,
    height: u32,
    threshold: f64,
    structural_diff_summary: Vec<String>,
) -> VisualDiffResultDto {
    let w = width as usize;
    let h = height as usize;
    let total_pixels = (w as u64) * (h as u64);
    let max_delta = 35_215.0 * threshold * threshold;

    let mut lum_before = vec![0.0_f64; w * h];
    let mut lum_after = vec![0.0_f64; w * h];
    let mut mismatched_pixels = 0_u64;
    let mut min_x = u32::MAX;
    let mut min_y = u32::MAX;
    let mut max_x = 0_u32;
    let mut max_y = 0_u32;

    for y in 0..h {
        for x in 0..w {
            let p_idx = y * w + x;
            let off = p_idx * 4;
            let b_px = &before[off..off + 4];
            let a_px = &after[off..off + 4];

            let (r1, g1, b1) = blend_rgba_over_white(b_px);
            let (r2, g2, b2) = blend_rgba_over_white(a_px);
            lum_before[p_idx] = 0.299 * r1 + 0.587 * g1 + 0.114 * b1;
            lum_after[p_idx] = 0.299 * r2 + 0.587 * g2 + 0.114 * b2;

            if b_px != a_px {
                let dr = r1 - r2;
                let dg = g1 - g2;
                let db = b1 - b2;
                let yiq_y = dr * 0.29889531 + dg * 0.58662247 + db * 0.11448223;
                let yiq_i = dr * 0.59597799 - dg * 0.27417610 - db * 0.32180189;
                let yiq_q = dr * 0.21147017 - dg * 0.52261711 + db * 0.31114694;
                let delta = 0.5053 * yiq_y * yiq_y + 0.299 * yiq_i * yiq_i + 0.1957 * yiq_q * yiq_q;
                if delta > max_delta {
                    mismatched_pixels += 1;
                    let ux = x as u32;
                    let uy = y as u32;
                    min_x = min_x.min(ux);
                    min_y = min_y.min(uy);
                    max_x = max_x.max(ux);
                    max_y = max_y.max(uy);
                }
            }
        }
    }

    let win = 8usize;
    let build_spans = |dim: usize| -> Vec<(usize, usize)> {
        if dim <= win {
            return vec![(0, dim)];
        }
        let mut spans = Vec::new();
        let mut start = 0;
        while start + win <= dim {
            spans.push((start, win));
            start += win;
        }
        if !dim.is_multiple_of(win) {
            spans.push((dim - win, win));
        }
        spans
    };

    let xs = build_spans(w);
    let ys = build_spans(h);
    let c1 = (0.01 * 255.0_f64).powi(2);
    let c2 = (0.03 * 255.0_f64).powi(2);
    let mut ssim_sum = 0.0_f64;
    let mut win_count = 0_usize;

    for &(y0, hh) in &ys {
        for &(x0, ww) in &xs {
            let n = (ww * hh) as f64;
            let mut sum_a = 0.0;
            let mut sum_b = 0.0;
            for yy in y0..(y0 + hh) {
                let row_off = yy * w;
                for xx in x0..(x0 + ww) {
                    sum_a += lum_before[row_off + xx];
                    sum_b += lum_after[row_off + xx];
                }
            }
            let mu_a = sum_a / n;
            let mu_b = sum_b / n;

            let mut var_a = 0.0;
            let mut var_b = 0.0;
            let mut cov_ab = 0.0;
            for yy in y0..(y0 + hh) {
                let row_off = yy * w;
                for xx in x0..(x0 + ww) {
                    let da = lum_before[row_off + xx] - mu_a;
                    let db = lum_after[row_off + xx] - mu_b;
                    var_a += da * da;
                    var_b += db * db;
                    cov_ab += da * db;
                }
            }
            let denom_n = if n > 1.0 { n - 1.0 } else { 1.0 };
            var_a /= denom_n;
            var_b /= denom_n;
            cov_ab /= denom_n;

            let num = (2.0 * mu_a * mu_b + c1) * (2.0 * cov_ab + c2);
            let den = (mu_a * mu_a + mu_b * mu_b + c1) * (var_a + var_b + c2);
            let ssim_win = if den == 0.0 { 1.0 } else { num / den };
            ssim_sum += ssim_win;
            win_count += 1;
        }
    }

    let ssim = if win_count == 0 {
        1.0
    } else {
        (ssim_sum / win_count as f64).clamp(0.0, 1.0)
    };
    let mismatch_ratio = if total_pixels == 0 {
        0.0
    } else {
        mismatched_pixels as f64 / total_pixels as f64
    };
    let changed_bbox = if mismatched_pixels > 0 {
        Some(VisualDiffBboxDto {
            x: min_x,
            y: min_y,
            width: max_x - min_x + 1,
            height: max_y - min_y + 1,
        })
    } else {
        None
    };

    VisualDiffResultDto {
        status: "COMPUTED".to_string(),
        ssim,
        mismatch_ratio,
        mismatched_pixels,
        total_pixels,
        width,
        height,
        changed_bbox,
        reason: None,
        structural_diff_summary,
    }
}

pub(super) async fn handle_list_proposals(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<ListProposalsQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<ChangeProposalDto>>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let filter_node_id = query
        .node_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);

    let list = tokio::task::spawn_blocking(move || {
        let conn = project_registry.pool().connection()?;
        let mut out = Vec::new();
        if let Some(ref nid) = filter_node_id {
            let sql = format!(
                "SELECT {PROPOSAL_SELECT_COLS}
                 FROM change_proposals
                 WHERE project_id = ?1 AND tenant_id = ?2
                   AND (
                       node_id = ?3
                       OR EXISTS (
                           SELECT 1 FROM proposal_target_nodes ptn
                           WHERE ptn.proposal_id = change_proposals.id
                             AND ptn.tenant_id = change_proposals.tenant_id
                             AND ptn.project_id = change_proposals.project_id
                             AND ptn.node_id = ?3
                       )
                   )
                 ORDER BY updated_at DESC"
            );
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| map_sqlite_error(e, "prepare_list_proposals_by_node"))?;
            let rows = stmt
                .query_map(
                    params![project_id.to_string(), tenant_id, nid],
                    map_proposal_row,
                )
                .map_err(|e| map_sqlite_error(e, "query_list_proposals_by_node"))?;
            for r in rows {
                let mut dto = r.map_err(|e| map_sqlite_error(e, "row_list_proposals_by_node"))?;
                hydrate_proposal_details(&conn, &mut dto)?;
                out.push(dto);
            }
        } else {
            let sql = format!(
                "SELECT {PROPOSAL_SELECT_COLS}
                 FROM change_proposals
                 WHERE project_id = ?1 AND tenant_id = ?2
                 ORDER BY updated_at DESC"
            );
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| map_sqlite_error(e, "prepare_list_proposals"))?;
            let rows = stmt
                .query_map(params![project_id.to_string(), tenant_id], map_proposal_row)
                .map_err(|e| map_sqlite_error(e, "query_list_proposals"))?;
            for r in rows {
                let mut dto = r.map_err(|e| map_sqlite_error(e, "row_list_proposals"))?;
                hydrate_proposal_details(&conn, &mut dto)?;
                out.push(dto);
            }
        }
        Ok::<_, HarnessError>(out)
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    Ok(Json(list))
}

pub(super) async fn handle_get_proposal(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath((id, proposal_id_raw)): AxumPath<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<ChangeProposalDto>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let proposal_id = parse_project_uuid(&proposal_id_raw).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());

    let found = tokio::task::spawn_blocking(move || {
        let conn = project_registry.pool().connection()?;
        load_proposal_by_id(&conn, project_id, proposal_id, &tenant_id)
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    match found {
        Some(p) => Ok(Json(p)),
        None => Err(proposal_not_found(proposal_id)),
    }
}

pub(super) async fn handle_create_proposal(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<CreateProposalRequest>,
) -> Result<(StatusCode, Json<ChangeProposalDto>), Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let base_registry = state.registry.clone();
    let now = chrono::Utc::now().to_rfc3339();
    let proposal_id = req.id.unwrap_or_else(Uuid::new_v4);
    let state_for_receipt = state.clone();

    let dto = tokio::task::spawn_blocking(move || {
        let content_mode = validate_content_mode(req.content_mode.as_deref())?;
        let tradeoffs_input = req.tradeoffs.unwrap_or_default();
        validate_tradeoffs_input(&tradeoffs_input)?;

        let node_id = req
            .node_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from);
        let batch_id = req
            .batch_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from);

        let mut node_ids = Vec::new();
        let mut seen_nodes = HashSet::new();
        if let Some(raw_nodes) = req.node_ids {
            for raw in raw_nodes {
                let trimmed = raw.trim();
                if !trimmed.is_empty() && seen_nodes.insert(trimmed.to_string()) {
                    node_ids.push(trimmed.to_string());
                }
            }
        }

        let composite_scope_key = if let Some(ref nid) = node_id {
            format!("{tenant_id}::{project_id}::{nid}")
        } else if let Some(first_nid) = node_ids.first() {
            format!("{tenant_id}::{project_id}::{first_nid}")
        } else if let Some(ref bid) = batch_id {
            format!("{tenant_id}::{project_id}::batch::{bid}")
        } else {
            String::new()
        };

        let (validation_status, checks) = run_proposal_validation_checks(
            &base_registry,
            &canon_project_root,
            project_id,
            &req.files,
            &now,
        )?;
        let status = if validation_status == "passed" {
            "ready".to_string()
        } else {
            "draft".to_string()
        };

        let files_json =
            serde_json::to_string(&req.files).map_err(|e| HarnessError::ValidationFailed {
                reason: format!("PROPOSAL_SERIALIZE_ERROR: {e}"),
            })?;
        let checks_json =
            serde_json::to_string(&checks).map_err(|e| HarnessError::ValidationFailed {
                reason: format!("PROPOSAL_CHECKS_SERIALIZE_ERROR: {e}"),
            })?;

        let mut conn = project_registry.pool().connection()?;
        let base_file_hashes = resolve_base_file_hashes(
            &conn,
            &canon_project_root,
            project_id,
            &req.files,
            req.base_file_hashes.as_ref(),
        )?;
        let base_hashes_json =
            serde_json::to_string(&base_file_hashes).map_err(|e| {
                HarnessError::ValidationFailed {
                    reason: format!("BASE_HASHES_SERIALIZE_ERROR: {e}"),
                }
            })?;

        let gate_receipt = if validation_status == "passed" {
            Some(build_gate_receipt(
                &state_for_receipt,
                &conn,
                &canon_project_root,
                &tenant_id,
                project_id,
                proposal_id,
                &req.files,
                &base_file_hashes,
                &checks_json,
                0,
                &now,
            )?)
        } else {
            None
        };
        let gate_receipt_id = gate_receipt.as_ref().map(|r| r.id.to_string());

        let mut tradeoffs_out = Vec::with_capacity(tradeoffs_input.len());
        for t in tradeoffs_input {
            let tid = Uuid::new_v4();
            let t_node_id = t
                .node_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .or_else(|| node_id.clone());
            tradeoffs_out.push(ProposalTradeoffDto {
                id: tid,
                proposal_id,
                node_id: t_node_id,
                element_ff_id: t.element_ff_id.trim().to_string(),
                badge_index: t.badge_index,
                design_note: t.design_note,
                instead_label: t.instead_label,
                instead_patch: t.instead_patch,
                decision: "pending".to_string(),
                ask_prompt: t.ask_prompt,
                decided_at: None,
                created_at: now.clone(),
            });
        }

        let title_trimmed = req.title.trim().to_string();
        let spec_key = req
            .files
            .first()
            .map(|f| f.path.clone())
            .unwrap_or_else(|| "proposal".to_string());

        retry_on_busy("insert_change_proposal", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "INSERT INTO change_proposals (
                    id, project_id, spec_key, diff_content, tenant_id, intent_id,
                    title, description, files_json, validation_status,
                    validation_checks_json, status, node_id, batch_id,
                    composite_scope_key, content_mode, base_file_hashes_json,
                    visual_diff_json, gate_receipt_id, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, '{}', ?18, ?19, ?20)",
                params![
                    proposal_id.to_string(),
                    project_id.to_string(),
                    spec_key,
                    files_json,
                    tenant_id,
                    req.intent_id.map(|u| u.to_string()),
                    title_trimmed,
                    req.description,
                    files_json,
                    validation_status,
                    checks_json,
                    status,
                    node_id,
                    batch_id,
                    composite_scope_key,
                    content_mode,
                    base_hashes_json,
                    gate_receipt_id,
                    now,
                    now,
                ],
            )?;

            for nid in &node_ids {
                tx.execute(
                    "INSERT INTO proposal_target_nodes (proposal_id, tenant_id, project_id, node_id)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![
                        proposal_id.to_string(),
                        tenant_id,
                        project_id.to_string(),
                        nid
                    ],
                )?;
            }

            if let Some(ref receipt) = gate_receipt {
                insert_gate_receipt_tx(&tx, &tenant_id, project_id, receipt, &checks_json)?;
            }

            for t in &tradeoffs_out {
                let patch_json = match &t.instead_patch {
                    Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "null".to_string()),
                    None => "null".to_string(),
                };
                tx.execute(
                    "INSERT INTO proposal_tradeoffs (
                        id, tenant_id, project_id, proposal_id, node_id,
                        element_ff_id, badge_index, design_note, instead_label,
                        instead_patch_json, decision, ask_prompt, decided_at, created_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'pending', ?11, NULL, ?12)",
                    params![
                        t.id.to_string(),
                        tenant_id,
                        project_id.to_string(),
                        proposal_id.to_string(),
                        t.node_id,
                        t.element_ff_id,
                        t.badge_index,
                        t.design_note,
                        t.instead_label,
                        patch_json,
                        t.ask_prompt,
                        t.created_at,
                    ],
                )?;
            }

            tx.commit()
        })?;

        Ok::<_, HarnessError>(ChangeProposalDto {
            id: proposal_id,
            tenant_id,
            project_id,
            intent_id: req.intent_id,
            title: title_trimmed,
            description: req.description,
            files: req.files,
            validation_status,
            validation_checks: checks,
            status,
            created_at: now.clone(),
            updated_at: now,
            node_id,
            batch_id,
            composite_scope_key,
            content_mode,
            base_file_hashes,
            node_ids,
            gate_receipt_id,
            gate_receipt,
            visual_diff: None,
            tradeoffs: tradeoffs_out,
        })
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    Ok((StatusCode::CREATED, Json(dto)))
}

pub(super) async fn handle_validate_proposal(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath((id, proposal_id_raw)): AxumPath<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<ChangeProposalDto>, Response> {
    let req: ValidateProposalRequest = if body.is_empty() {
        ValidateProposalRequest::default()
    } else {
        serde_json::from_slice(&body).map_err(|e| {
            map_error_to_response(HarnessError::ValidationFailed {
                reason: format!("INVALID_VALIDATE_REQUEST: {e}"),
            })
        })?
    };
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let proposal_id = parse_project_uuid(&proposal_id_raw).map_err(map_error_to_response)?;
    let (project_registry, canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let base_registry = state.registry.clone();
    let now = chrono::Utc::now().to_rfc3339();
    let state_for_receipt = state.clone();

    let result = tokio::task::spawn_blocking(move || {
        let mut conn = project_registry.pool().connection()?;
        let Some(mut proposal) =
            load_proposal_by_id(&conn, project_id, proposal_id, &tenant_id)?
        else {
            return Ok::<_, HarnessError>(Err(proposal_not_found(proposal_id)));
        };

        if proposal.status == "rejected" || proposal.status == "applied" {
            return Ok(Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "INVALID_STATE",
                    "message": format!("Cannot validate ChangeProposal in terminal '{}' status.", proposal.status)
                })),
            )
                .into_response()));
        }

        let (validation_status, checks) = run_proposal_validation_checks(
            &base_registry,
            &canon_project_root,
            project_id,
            &proposal.files,
            &now,
        )?;
        let next_status = if validation_status == "passed" {
            "ready".to_string()
        } else {
            "draft".to_string()
        };
        let checks_json = serde_json::to_string(&checks).map_err(|e| {
            HarnessError::ValidationFailed {
                reason: format!("PROPOSAL_CHECKS_SERIALIZE_ERROR: {e}"),
            }
        })?;

        let updated_base_hashes = if req.rebase_base_hashes.unwrap_or(false) {
            let paths: Vec<&str> = proposal
                .files
                .iter()
                .map(|f| f.path.as_str())
                .chain(proposal.base_file_hashes.keys().map(String::as_str))
                .collect();
            snapshot_current_hashes_for_paths(&conn, &canon_project_root, project_id, paths)?
        } else {
            proposal.base_file_hashes.clone()
        };
        let base_hashes_json =
            serde_json::to_string(&updated_base_hashes).map_err(|e| {
                HarnessError::ValidationFailed {
                    reason: format!("BASE_HASHES_SERIALIZE_ERROR: {e}"),
                }
            })?;

        let exit_code = if validation_status == "passed" { 0 } else { 1 };
        let receipt = build_gate_receipt(
            &state_for_receipt,
            &conn,
            &canon_project_root,
            &tenant_id,
            project_id,
            proposal_id,
            &proposal.files,
            &updated_base_hashes,
            &checks_json,
            exit_code,
            &now,
        )?;
        let receipt_id_opt = if exit_code == 0 {
            Some(receipt.id.to_string())
        } else {
            None
        };

        retry_on_busy("validate_change_proposal", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            insert_gate_receipt_tx(&tx, &tenant_id, project_id, &receipt, &checks_json)?;
            tx.execute(
                "UPDATE change_proposals
                 SET status = ?1, validation_status = ?2, validation_checks_json = ?3,
                     base_file_hashes_json = ?4, gate_receipt_id = ?5, updated_at = ?6
                 WHERE id = ?7 AND project_id = ?8 AND tenant_id = ?9",
                params![
                    next_status,
                    validation_status,
                    checks_json,
                    base_hashes_json,
                    receipt_id_opt,
                    now,
                    proposal_id.to_string(),
                    project_id.to_string(),
                    tenant_id,
                ],
            )?;
            tx.commit()
        })?;

        proposal.status = next_status;
        proposal.validation_status = validation_status;
        proposal.validation_checks = checks;
        proposal.base_file_hashes = updated_base_hashes;
        proposal.gate_receipt_id = receipt_id_opt;
        proposal.gate_receipt = if exit_code == 0 { Some(receipt) } else { None };
        proposal.updated_at = now;
        Ok(Ok(proposal))
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    result.map(Json)
}

fn mark_conflicting_proposals_stale_tx(
    tx: &rusqlite::Transaction<'_>,
    project_id: Uuid,
    tenant_id: &str,
    applied_id: Uuid,
    applied: &ChangeProposalDto,
    now: &str,
) -> rusqlite::Result<()> {
    let applied_paths: std::collections::HashSet<String> =
        applied.files.iter().map(|f| f.path.clone()).collect();
    let mut stmt = tx.prepare(
        "SELECT id, node_id, files_json FROM change_proposals
         WHERE project_id = ?1 AND tenant_id = ?2 AND id != ?3
           AND status IN ('draft', 'ready', 'validating')",
    )?;
    let candidates: Vec<(String, Option<String>, String)> = stmt
        .query_map(
            params![project_id.to_string(), tenant_id, applied_id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    for (cand_id, cand_node_id, cand_files_json) in candidates {
        let same_node = applied.node_id.is_some() && applied.node_id == cand_node_id;
        let cand_files: Vec<FileDiffItemDto> =
            serde_json::from_str(&cand_files_json).unwrap_or_default();
        let overlapping_file = cand_files.iter().any(|f| applied_paths.contains(&f.path));
        if same_node || overlapping_file {
            tx.execute(
                "UPDATE change_proposals SET status = 'stale', updated_at = ?1
                 WHERE id = ?2 AND project_id = ?3 AND tenant_id = ?4",
                params![now, cand_id, project_id.to_string(), tenant_id],
            )?;
        }
    }
    Ok(())
}

fn stale_base_hash_conflict_response(stale_files: Vec<String>) -> Response {
    let summary = if stale_files.is_empty() {
        "Proposal is stale due to concurrent workspace apply.".to_string()
    } else {
        format!(
            "Workspace files changed since proposal snapshot: {}",
            stale_files.join(", ")
        )
    };
    (
        StatusCode::CONFLICT,
        Json(serde_json::json!({
            "code": "PROPOSAL_STALE_BASE_HASH",
            "error": "PROPOSAL_STALE_BASE_HASH",
            "staleFiles": stale_files,
            "message": summary,
        })),
    )
        .into_response()
}

pub(super) async fn handle_apply_proposal(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath((id, proposal_id_raw)): AxumPath<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<ChangeProposalDto>, Response> {
    let _req: ApplyProposalRequest = if body.is_empty() {
        ApplyProposalRequest { force: false }
    } else {
        serde_json::from_slice(&body).map_err(|e| {
            map_error_to_response(HarnessError::ValidationFailed {
                reason: format!("INVALID_APPLY_REQUEST: {e}"),
            })
        })?
    };
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let proposal_id = parse_project_uuid(&proposal_id_raw).map_err(map_error_to_response)?;
    let (project_registry, canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let now = chrono::Utc::now().to_rfc3339();
    let state_for_verify = state.clone();

    let result = tokio::task::spawn_blocking(move || {
        let mut conn = project_registry.pool().connection()?;
        let Some(mut proposal) =
            load_proposal_by_id(&conn, project_id, proposal_id, &tenant_id)?
        else {
            return Ok::<_, HarnessError>(Err(proposal_not_found(proposal_id)));
        };

        if proposal.status == "rejected" || proposal.status == "applied" {
            return Ok(Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "INVALID_STATE",
                    "message": format!("Cannot apply ChangeProposal in '{}' status. Only 'ready' proposals can be applied.", proposal.status)
                })),
            )
                .into_response()));
        }

        let has_failed = proposal.validation_checks.iter().any(|c| c.status == "failed");
        if proposal.validation_status != "passed" || has_failed {
            return Ok(Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "PRECONDITION_FAILED",
                    "message": "Cannot apply ChangeProposal: AST/constraint validation checks must pass first."
                })),
            )
                .into_response()));
        }

        if proposal.status == "stale" {
            let stale_paths = proposal.files.iter().map(|f| f.path.clone()).collect();
            return Ok(Err(stale_base_hash_conflict_response(stale_paths)));
        }

        if proposal.status != "ready" {
            return Ok(Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "INVALID_STATE",
                    "message": format!("Cannot apply ChangeProposal in '{}' status. Only 'ready' proposals can be applied.", proposal.status)
                })),
            )
                .into_response()));
        }

        for f in &proposal.files {
            validate_safe_relative_path(&f.path)?;
        }

        let receipt = match verify_receipt_cryptography(&state_for_verify, &conn, &proposal) {
            Ok((receipt, _)) => receipt,
            Err(Ok(resp)) => return Ok(Err(resp)),
            Err(Err(e)) => return Err(e),
        };

        let mut tx_rejection: Option<Response> = None;
        let mut domain_err: Option<HarnessError> = None;
        retry_on_busy("apply_change_proposal", || {
            tx_rejection = None;
            domain_err = None;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

            let current_status: Option<String> = tx
                .query_row(
                    "SELECT status FROM change_proposals WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
                    params![proposal_id.to_string(), project_id.to_string(), tenant_id],
                    |r| r.get(0),
                )
                .optional()?;
            if current_status.as_deref() == Some("stale") {
                let stale_paths = proposal.files.iter().map(|f| f.path.clone()).collect();
                tx_rejection = Some(stale_base_hash_conflict_response(stale_paths));
                return Ok(());
            }
            if current_status.as_deref() != Some("ready") {
                tx_rejection = Some(
                    (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "error": "INVALID_STATE",
                            "message": "Proposal is no longer in 'ready' status."
                        })),
                    )
                        .into_response(),
                );
                return Ok(());
            }

            let stale_files = match find_stale_base_files(
                &tx,
                &canon_project_root,
                project_id,
                &proposal.base_file_hashes,
            ) {
                Ok(s) => s,
                Err(e) => {
                    domain_err = Some(e);
                    return Ok(());
                }
            };
            if !stale_files.is_empty() {
                tx_rejection = Some(stale_base_hash_conflict_response(stale_files));
                return Ok(());
            }

            let current_head_sha = match compute_scoped_workspace_head_sha256(
                &tx,
                &canon_project_root,
                project_id,
                &proposal.files,
                &proposal.base_file_hashes,
            ) {
                Ok(sha) => sha,
                Err(e) => {
                    domain_err = Some(e);
                    return Ok(());
                }
            };
            if !constant_time_eq(
                current_head_sha.as_bytes(),
                receipt.workspace_head_sha256.as_bytes(),
            ) {
                tx_rejection = Some(unverified_gate_claim(
                    "workspace_head_sha256 does not match gate receipt",
                ));
                return Ok(());
            }

            // Capture automatic pre_apply_safety snapshot before mutating workspace
            let mut safety_files = match load_workspace_files_from_db(&tx, project_id) {
                Ok(f) => f,
                Err(e) => {
                    domain_err = Some(e);
                    return Ok(());
                }
            };
            let tracked: HashSet<String> =
                safety_files.iter().map(|f| f.path.clone()).collect();
            for f in &proposal.files {
                if let Ok(clean_rel) = validate_safe_relative_path(&f.path) {
                    if !tracked.contains(&clean_rel) {
                        if let Ok(Some(bytes)) =
                            read_disk_file_bytes_jailed(&canon_project_root, &clean_rel)
                        {
                            let content = String::from_utf8_lossy(&bytes).into_owned();
                            let sha = format!("{:x}", Sha256::digest(content.as_bytes()));
                            let sz = content.len();
                            safety_files.push(SnapshotFileItemDto {
                                path: clean_rel,
                                content,
                                sha256: Some(sha),
                                size_bytes: Some(sz),
                            });
                        }
                    }
                }
            }
            let safety_sha = match compute_files_tree_sha(&safety_files) {
                Ok(s) => s,
                Err(e) => {
                    domain_err = Some(e);
                    return Ok(());
                }
            };
            let safety_files_json = match serde_json::to_string(&safety_files) {
                Ok(j) => j,
                Err(e) => {
                    domain_err = Some(HarnessError::ValidationFailed {
                        reason: format!("SAFETY_FILES_SERIALIZE_ERROR: {e}"),
                    });
                    return Ok(());
                }
            };
            let safety_id = Uuid::new_v4();
            let safety_tag = format!(
                "pre-apply-{}-{}",
                &proposal_id.to_string()[..8],
                &safety_id.to_string()[..8]
            );
            tx.execute(
                "INSERT INTO version_snapshots (
                    id, project_id, snapshot_tag, manifest_sha256,
                    tenant_id, version_tag, label, description, trigger,
                    tree_sha256, files_json, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pre_apply_safety', ?9, ?10, ?11, ?12)",
                params![
                    safety_id.to_string(),
                    project_id.to_string(),
                    safety_tag,
                    safety_sha,
                    tenant_id,
                    "pre-apply-safety",
                    format!("Pre-Apply Safety Backup ({})", proposal.title),
                    format!("Auto safety snapshot before applying ChangeProposal {proposal_id}"),
                    safety_sha,
                    safety_files_json,
                    now,
                    now,
                ],
            )?;

            for file in &proposal.files {
                if let Err(e) = sync_workspace_file_to_disk(
                    &canon_project_root,
                    &file.path,
                    file.status.eq_ignore_ascii_case("deleted"),
                    &file.patch,
                ) {
                    domain_err = Some(e);
                    return Ok(());
                }
                let rel_path = validate_safe_relative_path(&file.path)
                    .map_err(|_| rusqlite::Error::InvalidParameterName(file.path.clone()))?;
                if file.status.eq_ignore_ascii_case("deleted") {
                    tx.execute(
                        "DELETE FROM project_workspace_files WHERE project_id = ?1 AND relative_path = ?2",
                        params![project_id.to_string(), rel_path],
                    )?;
                } else {
                    let sha = format!("{:x}", Sha256::digest(file.patch.as_bytes()));
                    let byte_size = file.patch.len() as i64;
                    tx.execute(
                        "INSERT INTO project_workspace_files (
                            id, project_id, relative_path, file_sha256, byte_size, content, created_at, updated_at
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                         ON CONFLICT(project_id, relative_path) DO UPDATE SET
                            file_sha256 = excluded.file_sha256,
                            byte_size = excluded.byte_size,
                            content = excluded.content,
                            updated_at = excluded.updated_at",
                        params![
                            Uuid::new_v4().to_string(),
                            project_id.to_string(),
                            rel_path,
                            sha,
                            byte_size,
                            file.patch,
                            now,
                            now,
                        ],
                    )?;
                }
            }
            tx.execute(
                "UPDATE change_proposals SET status = 'applied', updated_at = ?1
                 WHERE id = ?2 AND project_id = ?3 AND tenant_id = ?4",
                params![now, proposal_id.to_string(), project_id.to_string(), tenant_id],
            )?;
            mark_conflicting_proposals_stale_tx(
                &tx,
                project_id,
                &tenant_id,
                proposal_id,
                &proposal,
                &now,
            )?;
            let apply_turn_files: Vec<SnapshotFileItemDto> = proposal
                .files
                .iter()
                .filter(|f| !f.status.eq_ignore_ascii_case("deleted"))
                .filter_map(|f| {
                    validate_safe_relative_path(&f.path).ok().map(|rel| SnapshotFileItemDto {
                        path: rel,
                        content: f.patch.clone(),
                        sha256: Some(format!("{:x}", Sha256::digest(f.patch.as_bytes()))),
                        size_bytes: Some(f.patch.len()),
                    })
                })
                .collect();
            let apply_manifest_json =
                serde_json::to_string(&apply_turn_files).unwrap_or_else(|_| "[]".to_string());
            let apply_hash =
                compute_files_tree_sha(&apply_turn_files).unwrap_or_else(|_| safety_sha.clone());
            let _ = insert_immutable_turn_snapshot_tx(
                &tx,
                project_id,
                &tenant_id,
                proposal.node_id.as_deref(),
                Some(&proposal_id.to_string()),
                Some(&safety_id.to_string()),
                &proposal.title,
                &apply_manifest_json,
                &apply_hash,
                chrono::Utc::now().timestamp_millis(),
            )?;
            tx.commit()
        })?;

        if let Some(err) = domain_err {
            return Err(err);
        }
        if let Some(resp) = tx_rejection {
            return Ok(Err(resp));
        }

        proposal.status = "applied".to_string();
        proposal.updated_at = now;
        Ok(Ok(proposal))
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    result.map(Json)
}

pub(super) async fn handle_diff_proposal(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath((id, proposal_id_raw)): AxumPath<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<VisualDiffResultDto>, Response> {
    let req: DiffProposalRequest = if body.is_empty() {
        DiffProposalRequest::default()
    } else {
        serde_json::from_slice(&body).map_err(|e| {
            map_error_to_response(HarnessError::ValidationFailed {
                reason: format!("INVALID_DIFF_REQUEST: {e}"),
            })
        })?
    };
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let proposal_id = parse_project_uuid(&proposal_id_raw).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let now = chrono::Utc::now().to_rfc3339();

    let result = tokio::task::spawn_blocking(move || {
        let mut conn = project_registry.pool().connection()?;
        let Some(proposal) =
            load_proposal_by_id(&conn, project_id, proposal_id, &tenant_id)?
        else {
            return Ok::<_, HarnessError>(Err(proposal_not_found(proposal_id)));
        };

        let structural_diff_summary: Vec<String> = proposal
            .files
            .iter()
            .map(|f| format!("{} ({}: +{} -{})", f.path, f.status, f.additions, f.deletions))
            .collect();

        let before_b64 = req
            .before_rgba_base64
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let after_b64 = req
            .after_rgba_base64
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());

        let diff_result = match (before_b64, after_b64) {
            (Some(b_str), Some(a_str)) => {
                let width = req.width.ok_or_else(|| HarnessError::ValidationFailed {
                    reason: "VISUAL_DIFF_DIMENSIONS_REQUIRED: width is required when RGBA buffers are provided".to_string(),
                })?;
                let height = req.height.ok_or_else(|| HarnessError::ValidationFailed {
                    reason: "VISUAL_DIFF_DIMENSIONS_REQUIRED: height is required when RGBA buffers are provided".to_string(),
                })?;
                if width == 0
                    || height == 0
                    || width > MAX_VISUAL_DIFF_WIDTH
                    || height > MAX_VISUAL_DIFF_HEIGHT
                {
                    return Err(HarnessError::ValidationFailed {
                        reason: format!(
                            "VISUAL_DIFF_BOUNDS_EXCEEDED: dimensions {width}x{height} must be within 1..={MAX_VISUAL_DIFF_WIDTH} x 1..={MAX_VISUAL_DIFF_HEIGHT}"
                        ),
                    });
                }
                let threshold = req.threshold.unwrap_or(0.1);
                if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
                    return Err(HarnessError::ValidationFailed {
                        reason: format!(
                            "VISUAL_DIFF_INVALID_THRESHOLD: threshold {threshold} must be within 0.0..=1.0"
                        ),
                    });
                }

                let before_bytes = B64_STD.decode(b_str).map_err(|e| {
                    HarnessError::ValidationFailed {
                        reason: format!("VISUAL_DIFF_INVALID_BASE64: beforeRgbaBase64: {e}"),
                    }
                })?;
                let after_bytes = B64_STD.decode(a_str).map_err(|e| {
                    HarnessError::ValidationFailed {
                        reason: format!("VISUAL_DIFF_INVALID_BASE64: afterRgbaBase64: {e}"),
                    }
                })?;

                let expected_len = (width as usize) * (height as usize) * 4;
                if before_bytes.len() != expected_len || after_bytes.len() != expected_len {
                    return Err(HarnessError::ValidationFailed {
                        reason: format!(
                            "VISUAL_DIFF_BUFFER_SIZE_MISMATCH: expected {expected_len} bytes ({width}x{height}x4), got before={} after={}",
                            before_bytes.len(),
                            after_bytes.len()
                        ),
                    });
                }

                compute_visual_diff_from_rgba(
                    &before_bytes,
                    &after_bytes,
                    width,
                    height,
                    threshold,
                    structural_diff_summary,
                )
            }
            _ => VisualDiffResultDto {
                status: "RASTERIZER_UNAVAILABLE".to_string(),
                ssim: 1.0,
                mismatch_ratio: 0.0,
                mismatched_pixels: 0,
                total_pixels: 0,
                width: req.width.unwrap_or(0),
                height: req.height.unwrap_or(0),
                changed_bbox: None,
                reason: Some("Headless rasterizer RGBA buffers not provided".to_string()),
                structural_diff_summary,
            },
        };

        let diff_json = serde_json::to_string(&diff_result).map_err(|e| {
            HarnessError::ValidationFailed {
                reason: format!("VISUAL_DIFF_SERIALIZE_ERROR: {e}"),
            }
        })?;
        retry_on_busy("persist_visual_diff", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "UPDATE change_proposals SET visual_diff_json = ?1, updated_at = ?2
                 WHERE id = ?3 AND project_id = ?4 AND tenant_id = ?5",
                params![
                    diff_json,
                    now,
                    proposal_id.to_string(),
                    project_id.to_string(),
                    tenant_id,
                ],
            )?;
            tx.commit()
        })?;

        Ok(Ok(diff_result))
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    result.map(Json)
}

pub(super) async fn handle_decide_tradeoff(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath((id, proposal_id_raw, tradeoff_id_raw)): AxumPath<(String, String, String)>,
    headers: HeaderMap,
    Json(req): Json<DecideTradeoffRequest>,
) -> Result<Json<ProposalTradeoffDto>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let proposal_id = parse_project_uuid(&proposal_id_raw).map_err(map_error_to_response)?;
    let tradeoff_id = parse_project_uuid(&tradeoff_id_raw).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let now = chrono::Utc::now().to_rfc3339();

    let decision = req.decision.trim().to_string();
    if !matches!(
        decision.as_str(),
        "agreed" | "disagreed_instead" | "asked" | "skipped"
    ) {
        return Err(map_error_to_response(HarnessError::ValidationFailed {
            reason: format!(
                "INVALID_TRADEOFF_DECISION: '{decision}' must be one of agreed, disagreed_instead, asked, skipped"
            ),
        }));
    }
    let ask_prompt = req
        .ask_prompt
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);

    let result = tokio::task::spawn_blocking(move || {
        let mut conn = project_registry.pool().connection()?;
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM change_proposals WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
                params![proposal_id.to_string(), project_id.to_string(), tenant_id],
                |_| Ok(()),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "check_proposal_for_tradeoff"))?
            .is_some();
        if !exists {
            return Ok::<_, HarnessError>(Err(proposal_not_found(proposal_id)));
        }

        let mut updated_count = 0usize;
        retry_on_busy("decide_proposal_tradeoff", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            updated_count = tx.execute(
                "UPDATE proposal_tradeoffs
                 SET decision = ?1, ask_prompt = COALESCE(?2, ask_prompt), decided_at = ?3
                 WHERE id = ?4 AND proposal_id = ?5 AND project_id = ?6 AND tenant_id = ?7",
                params![
                    decision,
                    ask_prompt,
                    now,
                    tradeoff_id.to_string(),
                    proposal_id.to_string(),
                    project_id.to_string(),
                    tenant_id,
                ],
            )?;
            tx.commit()
        })?;

        if updated_count == 0 {
            return Ok(Err((
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "code": "TRADEOFF_NOT_FOUND",
                    "error": "TRADEOFF_NOT_FOUND",
                    "message": format!("ProposalTradeoff '{tradeoff_id}' was not found for this proposal/project/tenant"),
                })),
            )
                .into_response()));
        }

        let updated = conn
            .query_row(
                "SELECT id, proposal_id, node_id, element_ff_id, badge_index,
                        design_note, instead_label, instead_patch_json,
                        decision, ask_prompt, decided_at, created_at
                 FROM proposal_tradeoffs
                 WHERE id = ?1 AND proposal_id = ?2 AND project_id = ?3 AND tenant_id = ?4",
                params![
                    tradeoff_id.to_string(),
                    proposal_id.to_string(),
                    project_id.to_string(),
                    tenant_id,
                ],
                map_tradeoff_row,
            )
            .map_err(|e| map_sqlite_error(e, "reload_tradeoff_after_decide"))?;
        Ok(Ok(updated))
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    result.map(Json)
}

pub(super) async fn handle_reject_proposal(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath((id, proposal_id_raw)): AxumPath<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<ChangeProposalDto>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let proposal_id = parse_project_uuid(&proposal_id_raw).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let now = chrono::Utc::now().to_rfc3339();

    let result = tokio::task::spawn_blocking(move || {
        let mut conn = project_registry.pool().connection()?;
        let Some(mut proposal) = load_proposal_by_id(&conn, project_id, proposal_id, &tenant_id)?
        else {
            return Ok::<_, HarnessError>(Err(proposal_not_found(proposal_id)));
        };

        if proposal.status == "applied" {
            return Ok(Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "INVALID_STATE",
                    "message": "Cannot reject an already applied ChangeProposal."
                })),
            )
                .into_response()));
        }

        retry_on_busy("reject_change_proposal", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "UPDATE change_proposals SET status = 'rejected', updated_at = ?1
                 WHERE id = ?2 AND project_id = ?3 AND tenant_id = ?4",
                params![
                    now,
                    proposal_id.to_string(),
                    project_id.to_string(),
                    tenant_id
                ],
            )?;
            tx.commit()
        })?;

        proposal.status = "rejected".to_string();
        proposal.updated_at = now;
        Ok(Ok(proposal))
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    result.map(Json)
}

pub(crate) fn json_err_resp(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({
            "code": code,
            "error": code,
            "message": message,
        })),
    )
        .into_response()
}

fn map_sre_constraint_error(err: &rusqlite::Error, context: &str) -> Option<Response> {
    if let rusqlite::Error::SqliteFailure(ffi_err, msg_opt) = err {
        let msg = msg_opt.as_deref().unwrap_or("constraint violation");
        match ffi_err.extended_code {
            sqlite_ffi::SQLITE_CONSTRAINT_UNIQUE | sqlite_ffi::SQLITE_CONSTRAINT_PRIMARYKEY => {
                return Some(
                    (
                        StatusCode::CONFLICT,
                        Json(serde_json::json!({
                            "code": "P2002",
                            "error": "CONSTRAINT_CONFLICT",
                            "constraint": "P2002",
                            "message": format!("UNIQUE_CONSTRAINT [{context}]: {msg}"),
                        })),
                    )
                        .into_response(),
                );
            }
            sqlite_ffi::SQLITE_CONSTRAINT_FOREIGNKEY => {
                return Some(
                    (
                        StatusCode::UNPROCESSABLE_ENTITY,
                        Json(serde_json::json!({
                            "code": "P2003",
                            "error": "INVALID_FOREIGN_KEY",
                            "constraint": "P2003",
                            "message": format!("REFERENTIAL_INTEGRITY [{context}]: {msg}"),
                        })),
                    )
                        .into_response(),
                );
            }
            _ => {}
        }
    }
    None
}

fn retry_on_busy_053<T, F>(context: &str, mut op: F) -> Result<T, Response>
where
    F: FnMut() -> Result<T, rusqlite::Error>,
{
    let op_hash = context.bytes().fold(0u64, |acc, b| {
        acc.wrapping_mul(31).wrapping_add(u64::from(b))
    });
    let jitter = op_hash % 15;
    for attempt in 0..5u32 {
        match op() {
            Ok(val) => return Ok(val),
            Err(err) => {
                if let Some(resp) = map_sre_constraint_error(&err, context) {
                    return Err(resp);
                }
                if is_sqlite_busy_or_locked(&err) && attempt < 4 {
                    let backoff = (25u64.saturating_mul(1u64 << attempt) + jitter).min(250);
                    std::thread::sleep(Duration::from_millis(backoff));
                    continue;
                }
                if is_sqlite_busy_or_locked(&err) {
                    return Err(json_err_resp(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "DATABASE_BUSY_TIMEOUT",
                        &format!("{context}: database busy timeout after 5 attempts: {err}"),
                    ));
                }
                return Err(map_error_to_response(map_sqlite_error(err, context)));
            }
        }
    }
    Err(json_err_resp(
        StatusCode::SERVICE_UNAVAILABLE,
        "DATABASE_BUSY_TIMEOUT",
        &format!("{context}: database busy timeout"),
    ))
}

fn epoch_ms_to_rfc3339(ms: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)
        .unwrap_or_else(chrono::Utc::now)
        .to_rfc3339()
}

fn extract_idempotency_key(headers: &HeaderMap) -> Option<String> {
    headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

enum IdemReservation053 {
    New,
    Replay(u16, String),
    Conflict(String),
}

fn reserve_idempotency_053(
    conn: &mut rusqlite::Connection,
    tenant_id: &str,
    actor_id: &str,
    idem_key: &str,
    request_sha: &str,
    now_ms: i64,
) -> Result<IdemReservation053, Response> {
    type Idem053Row = (String, String, i64, Option<i64>, Option<String>, i64);
    retry_on_busy_053("reserve_idempotency_053", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<Idem053Row> = tx
            .query_row(
                "SELECT request_sha256, status, lease_expires_at, response_status, response_body_json, expires_at
                 FROM idempotency_records
                 WHERE tenant_id = ?1 AND actor_id = ?2 AND idempotency_key = ?3",
                params![tenant_id, actor_id, idem_key],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .optional()?;
        if let Some((saved_sha, status, lease_exp, resp_st, resp_body, expires_at)) = existing {
            if saved_sha != request_sha {
                tx.rollback().ok();
                return Ok(IdemReservation053::Conflict(
                    "IDEMPOTENCY_KEY_REUSED_WITH_DIFFERENT_PAYLOAD".into(),
                ));
            }
            if status == "COMPLETED" && now_ms < expires_at {
                tx.rollback().ok();
                return Ok(IdemReservation053::Replay(
                    resp_st.unwrap_or(200) as u16,
                    resp_body.unwrap_or_default(),
                ));
            }
            if status == "IN_PROGRESS" && now_ms < lease_exp {
                tx.rollback().ok();
                return Ok(IdemReservation053::Conflict(
                    "IDEMPOTENCY_REQUEST_IN_PROGRESS".into(),
                ));
            }
            tx.execute(
                "UPDATE idempotency_records
                 SET status = 'IN_PROGRESS', lease_expires_at = ?1, response_status = NULL, response_body_json = NULL, expires_at = ?2
                 WHERE tenant_id = ?3 AND actor_id = ?4 AND idempotency_key = ?5",
                params![now_ms + 30_000, now_ms + 86_400_000, tenant_id, actor_id, idem_key],
            )?;
            tx.commit()?;
            return Ok(IdemReservation053::New);
        }
        tx.execute(
            "INSERT INTO idempotency_records (
                tenant_id, actor_id, idempotency_key, request_sha256, status,
                lease_expires_at, response_status, response_body_json, expires_at
             ) VALUES (?1, ?2, ?3, ?4, 'IN_PROGRESS', ?5, NULL, NULL, ?6)",
            params![
                tenant_id,
                actor_id,
                idem_key,
                request_sha,
                now_ms + 30_000,
                now_ms + 86_400_000
            ],
        )?;
        tx.commit()?;
        Ok(IdemReservation053::New)
    })
}

fn finalize_idempotency_053(
    conn: &mut rusqlite::Connection,
    tenant_id: &str,
    actor_id: &str,
    idem_key: &str,
    status: u16,
    body_json: &str,
) {
    let _ = retry_on_busy_053("finalize_idempotency_053", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE idempotency_records
             SET status = 'COMPLETED', response_status = ?1, response_body_json = ?2
             WHERE tenant_id = ?3 AND actor_id = ?4 AND idempotency_key = ?5",
            params![i64::from(status), body_json, tenant_id, actor_id, idem_key],
        )?;
        tx.commit()
    });
}

fn release_idempotency_053(
    conn: &mut rusqlite::Connection,
    tenant_id: &str,
    actor_id: &str,
    idem_key: &str,
) {
    let _ = retry_on_busy_053("release_idempotency_053", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "DELETE FROM idempotency_records
             WHERE tenant_id = ?1 AND actor_id = ?2 AND idempotency_key = ?3 AND status = 'IN_PROGRESS'",
            params![tenant_id, actor_id, idem_key],
        )?;
        tx.commit()
    });
}

fn replay_idempotent_json(status_code: u16, body_json: &str) -> Response {
    let st = StatusCode::from_u16(status_code).unwrap_or(StatusCode::OK);
    let val: serde_json::Value =
        serde_json::from_str(body_json).unwrap_or_else(|_| serde_json::json!({}));
    (st, Json(val)).into_response()
}

fn ensure_audit_job_id_tx(
    tx: &rusqlite::Transaction<'_>,
    project_id: Uuid,
    tenant_id: &str,
    now_ms: i64,
) -> Result<String, rusqlite::Error> {
    let pid_str = project_id.to_string();
    if let Some(existing_job) = tx
        .query_row(
            "SELECT id FROM node_execution_jobs WHERE project_id = ?1 AND tenant_id = ?2 ORDER BY created_at DESC LIMIT 1",
            params![pid_str, tenant_id],
            |r| r.get::<_, String>(0),
        )
        .optional()?
    {
        return Ok(existing_job);
    }
    let audit_sess_id = format!("audit-sess-{pid_str}");
    let audit_job_id = format!("audit-job-{pid_str}");
    tx.execute(
        "INSERT OR IGNORE INTO chat_sessions (id, tenant_id, project_id, title, model_provider, archived_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'Workspace Audit Journal', 'system', ?4, ?4, ?4)",
        params![audit_sess_id, tenant_id, pid_str, now_ms],
    )?;
    tx.execute(
        "INSERT OR IGNORE INTO node_execution_jobs (
            id, tenant_id, project_id, session_id, node_id, composite_scope_key,
            daemon_nonce, status, fencing_token, priority, created_at, completed_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, '__workspace__', ?5, '', 'COMPLETED', 1, 10, ?6, ?6, ?6)",
        params![
            audit_job_id,
            tenant_id,
            pid_str,
            audit_sess_id,
            format!("{pid_str}::{audit_sess_id}::__workspace__"),
            now_ms
        ],
    )?;
    Ok(audit_job_id)
}

fn append_execution_event_tx(
    tx: &rusqlite::Transaction<'_>,
    tenant_id: &str,
    project_id: Uuid,
    event_kind: &str,
    payload_text: &str,
    now_ms: i64,
) -> Result<String, rusqlite::Error> {
    let pid_str = project_id.to_string();
    let job_id = ensure_audit_job_id_tx(tx, project_id, tenant_id, now_ms)?;
    let seq_no: i64 = tx.query_row(
        "SELECT COALESCE(MAX(seq_no), 0) + 1 FROM execution_events WHERE job_id = ?1",
        params![job_id],
        |r| r.get(0),
    )?;
    let proj_seq: i64 = tx.query_row(
        "SELECT COALESCE(MAX(project_seq), 0) + 1 FROM execution_events WHERE project_id = ?1",
        params![pid_str],
        |r| r.get(0),
    )?;
    let event_id = Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO execution_events (
            id, tenant_id, project_id, job_id, node_id, composite_scope_key,
            event_kind, payload_text, seq_no, project_seq, created_at
         ) VALUES (?1, ?2, ?3, ?4, '__workspace__', '', ?5, ?6, ?7, ?8, ?9)",
        params![
            event_id,
            tenant_id,
            pid_str,
            job_id,
            event_kind,
            payload_text,
            seq_no,
            proj_seq,
            now_ms
        ],
    )?;
    Ok(event_id)
}

fn map_snapshot_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<VersionSnapshotDto> {
    let id_str: String = row.get(0)?;
    let proj_id_str: String = row.get(1)?;
    let tenant_id: String = row.get(2)?;
    let version_tag: String = row.get(3)?;
    let label: String = row.get(4)?;
    let description: String = row.get(5)?;
    let trigger: String = row.get(6)?;
    let tree_sha256: String = row.get(7)?;
    let files_json: String = row.get(8)?;
    let created_at: String = row.get(9)?;
    let proposal_id: Option<String> = row.get(10)?;
    let parent_snapshot_id: Option<String> = row.get(11)?;
    let chat_artifact_snapshot_id: Option<String> = row.get(12)?;

    Ok(VersionSnapshotDto {
        id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::nil()),
        tenant_id,
        project_id: Uuid::parse_str(&proj_id_str).unwrap_or_else(|_| Uuid::nil()),
        version_tag,
        label,
        description,
        trigger,
        tree_sha256,
        files: serde_json::from_str(&files_json).unwrap_or_default(),
        metadata: None,
        proposal_id,
        parent_snapshot_id,
        chat_artifact_snapshot_id,
        created_at,
    })
}

fn map_turn_snapshot_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChatArtifactSnapshotDto> {
    let created_ms: i64 = row.get(12)?;
    Ok(ChatArtifactSnapshotDto {
        id: row.get(0)?,
        tenant_id: row.get(1)?,
        project_id: row.get(2)?,
        session_id: row.get(3)?,
        message_id: row.get(4)?,
        node_id: row.get(5)?,
        proposal_id: row.get(6)?,
        parent_snapshot_id: row.get(7)?,
        artifact_type: row.get(8)?,
        content_hash: row.get(9)?,
        manifest_json: row.get(10)?,
        node_snapshot_json: row.get(11)?,
        created_at: epoch_ms_to_rfc3339(created_ms),
    })
}

fn compute_files_tree_sha(files: &[SnapshotFileItemDto]) -> Result<String, HarnessError> {
    let mut sorted = files.to_vec();
    for f in &mut sorted {
        f.path = validate_safe_relative_path(&f.path)?;
    }
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let canonical = serde_json::to_string(&sorted).map_err(|e| HarnessError::ValidationFailed {
        reason: format!("SNAPSHOT_SERIALIZE_ERROR: {e}"),
    })?;
    Ok(format!("{:x}", Sha256::digest(canonical.as_bytes())))
}

pub(super) async fn handle_get_snapshot(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath((id, snapshot_id_raw)): AxumPath<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<VersionSnapshotDto>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let snapshot_id = parse_project_uuid(&snapshot_id_raw).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());

    let found = tokio::task::spawn_blocking(move || {
        let conn = project_registry.pool().connection()?;
        conn.query_row(
            "SELECT id, project_id, tenant_id, version_tag, label, description,
                    trigger, tree_sha256, files_json, created_at,
                    proposal_id, parent_snapshot_id, chat_artifact_snapshot_id
             FROM version_snapshots
             WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
            params![snapshot_id.to_string(), project_id.to_string(), tenant_id],
            map_snapshot_row,
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "get_snapshot_by_id"))
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    match found {
        Some(snap) => Ok(Json(snap)),
        None => Err(snapshot_not_found(snapshot_id)),
    }
}

fn load_turn_snapshots_from_db(
    conn: &rusqlite::Connection,
    project_id: Uuid,
    tenant_id: &str,
    node_id_filter: Option<&str>,
) -> Result<Vec<ChatArtifactSnapshotDto>, HarnessError> {
    let sql = if node_id_filter.is_some() {
        "SELECT id, tenant_id, project_id, session_id, message_id, node_id,
                proposal_id, parent_snapshot_id, artifact_type, content_hash,
                manifest_json, node_snapshot_json, created_at
         FROM chat_artifact_snapshots
         WHERE project_id = ?1 AND tenant_id = ?2 AND node_id = ?3
         ORDER BY created_at ASC"
    } else {
        "SELECT id, tenant_id, project_id, session_id, message_id, node_id,
                proposal_id, parent_snapshot_id, artifact_type, content_hash,
                manifest_json, node_snapshot_json, created_at
         FROM chat_artifact_snapshots
         WHERE project_id = ?1 AND tenant_id = ?2
         ORDER BY created_at ASC"
    };
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| map_sqlite_error(e, "prepare_list_turn_snapshots"))?;
    let rows = if let Some(nid) = node_id_filter {
        stmt.query_map(
            params![project_id.to_string(), tenant_id, nid],
            map_turn_snapshot_row,
        )
    } else {
        stmt.query_map(
            params![project_id.to_string(), tenant_id],
            map_turn_snapshot_row,
        )
    }
    .map_err(|e| map_sqlite_error(e, "query_list_turn_snapshots"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| map_sqlite_error(e, "row_list_turn_snapshots"))?);
    }
    Ok(out)
}

pub(super) async fn handle_list_turn_snapshots(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<ListSnapshotsQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<ChatArtifactSnapshotDto>>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let node_filter = query
        .node_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);

    let list = tokio::task::spawn_blocking(move || {
        let conn = project_registry.pool().connection()?;
        load_turn_snapshots_from_db(&conn, project_id, &tenant_id, node_filter.as_deref())
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    Ok(Json(list))
}

pub(super) async fn handle_list_snapshots(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<ListSnapshotsQuery>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());

    if query.kind.as_deref() == Some("turns") {
        let node_filter = query
            .node_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from);
        let turns = tokio::task::spawn_blocking(move || {
            let conn = project_registry.pool().connection()?;
            load_turn_snapshots_from_db(&conn, project_id, &tenant_id, node_filter.as_deref())
        })
        .await
        .map_err(|e| {
            map_error_to_response(HarnessError::ValidationFailed {
                reason: format!("BLOCKING_TASK_FAILED: {e}"),
            })
        })?
        .map_err(map_error_to_response)?;
        return Ok(Json(turns).into_response());
    }

    let list = tokio::task::spawn_blocking(move || {
        let conn = project_registry.pool().connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, project_id, tenant_id, version_tag, label, description,
                        trigger, tree_sha256, files_json, created_at,
                        proposal_id, parent_snapshot_id, chat_artifact_snapshot_id
                 FROM version_snapshots
                 WHERE project_id = ?1 AND tenant_id = ?2
                 ORDER BY created_at ASC",
            )
            .map_err(|e| map_sqlite_error(e, "prepare_list_snapshots"))?;
        let rows = stmt
            .query_map(params![project_id.to_string(), tenant_id], map_snapshot_row)
            .map_err(|e| map_sqlite_error(e, "query_list_snapshots"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| map_sqlite_error(e, "row_list_snapshots"))?);
        }
        Ok::<_, HarnessError>(out)
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    Ok(Json(list).into_response())
}

fn verify_snapshot_parent_refs(
    conn: &rusqlite::Connection,
    project_id: Uuid,
    tenant_id: &str,
    proposal_id: Option<&str>,
    parent_snapshot_id: Option<&str>,
    chat_artifact_snapshot_id: Option<&str>,
) -> Result<(), Response> {
    let pid = project_id.to_string();
    if let Some(prop_id) = proposal_id {
        let ok: bool = conn
            .query_row(
                "SELECT 1 FROM change_proposals WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
                params![prop_id, pid, tenant_id],
                |_| Ok(()),
            )
            .optional()
            .map_err(|e| map_error_to_response(map_sqlite_error(e, "check_proposal_owner")))?
            .is_some();
        if !ok {
            return Err(json_err_resp(
                StatusCode::NOT_FOUND,
                "REFERENCE_NOT_FOUND",
                &format!("ChangeProposal '{prop_id}' was not found for this project/tenant"),
            ));
        }
    }
    if let Some(parent_id) = parent_snapshot_id {
        let ok: bool = conn
            .query_row(
                "SELECT 1 FROM version_snapshots WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
                params![parent_id, pid, tenant_id],
                |_| Ok(()),
            )
            .optional()
            .map_err(|e| map_error_to_response(map_sqlite_error(e, "check_parent_snapshot_owner")))?
            .is_some();
        if !ok {
            return Err(json_err_resp(
                StatusCode::NOT_FOUND,
                "REFERENCE_NOT_FOUND",
                &format!(
                    "Parent VersionSnapshot '{parent_id}' was not found for this project/tenant"
                ),
            ));
        }
    }
    if let Some(turn_id) = chat_artifact_snapshot_id {
        let ok: bool = conn
            .query_row(
                "SELECT 1 FROM chat_artifact_snapshots WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
                params![turn_id, pid, tenant_id],
                |_| Ok(()),
            )
            .optional()
            .map_err(|e| map_error_to_response(map_sqlite_error(e, "check_turn_snapshot_owner")))?
            .is_some();
        if !ok {
            return Err(json_err_resp(
                StatusCode::NOT_FOUND,
                "REFERENCE_NOT_FOUND",
                &format!("ChatArtifactSnapshot '{turn_id}' was not found for this project/tenant"),
            ));
        }
    }
    Ok(())
}

fn normalize_opt_ref(raw: Option<String>) -> Option<String> {
    raw.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn resolve_or_create_chat_session_tx(
    tx: &rusqlite::Transaction<'_>,
    project_id: Uuid,
    tenant_id: &str,
    now_ms: i64,
) -> Result<String, rusqlite::Error> {
    let pid = project_id.to_string();
    if let Some(sid) = tx
        .query_row(
            "SELECT id FROM chat_sessions WHERE project_id = ?1 AND tenant_id = ?2 ORDER BY updated_at DESC LIMIT 1",
            params![pid, tenant_id],
            |r| r.get::<_, String>(0),
        )
        .optional()?
    {
        return Ok(sid);
    }
    let sid = Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO chat_sessions (id, tenant_id, project_id, title, model_provider, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'Workbench Session', 'cowok-swarm', ?4, ?4)",
        params![sid, tenant_id, pid, now_ms],
    )?;
    Ok(sid)
}

#[allow(clippy::too_many_arguments)]
fn insert_immutable_turn_snapshot_tx(
    tx: &rusqlite::Transaction<'_>,
    project_id: Uuid,
    tenant_id: &str,
    node_id: Option<&str>,
    proposal_id: Option<&str>,
    parent_snapshot_id: Option<&str>,
    label: &str,
    manifest_json: &str,
    content_hash: &str,
    now_ms: i64,
) -> Result<String, rusqlite::Error> {
    let pid = project_id.to_string();
    let sid = resolve_or_create_chat_session_tx(tx, project_id, tenant_id, now_ms)?;
    let mid = Uuid::new_v4().to_string();
    let turn_snap_id = Uuid::new_v4().to_string();
    let node_snapshot_json = if let Some(nid) = node_id {
        tx.query_row(
            "SELECT graph_id, screen_key, title, node_type, route_path FROM flow_nodes
             WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
            params![nid, pid, tenant_id],
            |r| {
                Ok(serde_json::json!({
                    "id": nid,
                    "graphId": r.get::<_, String>(0)?,
                    "screenKey": r.get::<_, String>(1)?,
                    "title": r.get::<_, String>(2)?,
                    "nodeType": r.get::<_, String>(3)?,
                    "routePath": r.get::<_, String>(4)?,
                })
                .to_string())
            },
        )
        .optional()?
        .unwrap_or_else(|| "{}".to_string())
    } else {
        "{}".to_string()
    };
    let receipt_json = serde_json::json!({
        "turnSnapshotId": turn_snap_id,
        "contentHash": content_hash,
        "label": label,
    })
    .to_string();
    tx.execute(
        "INSERT INTO chat_messages (
            id, tenant_id, session_id, role, prose_markdown, turn_receipt_json, created_at
         ) VALUES (?1, ?2, ?3, 'assistant', ?4, ?5, ?6)",
        params![
            mid,
            tenant_id,
            sid,
            format!("Turn Snapshot: {label}"),
            receipt_json,
            now_ms
        ],
    )?;
    if let Some(nid) = node_id {
        let scope_key = format!("{pid}::{sid}::{nid}");
        tx.execute(
            "INSERT INTO chat_message_node_scopes (
                id, tenant_id, message_id, project_id, session_id, node_id, composite_scope_key, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![Uuid::new_v4().to_string(), tenant_id, mid, pid, sid, nid, scope_key, now_ms],
        )?;
    }
    let valid_parent_turn = if let Some(par) = parent_snapshot_id {
        tx.query_row(
            "SELECT 1 FROM chat_artifact_snapshots WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
            params![par, pid, tenant_id],
            |_| Ok(()),
        )
        .optional()?
        .map(|_| par.to_string())
    } else {
        None
    };
    tx.execute(
        "INSERT INTO chat_artifact_snapshots (
            id, tenant_id, project_id, session_id, message_id, node_id, proposal_id,
            parent_snapshot_id, artifact_type, content_hash, manifest_json, node_snapshot_json, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'html_state', ?9, ?10, ?11, ?12)",
        params![
            turn_snap_id,
            tenant_id,
            pid,
            sid,
            mid,
            node_id,
            proposal_id,
            valid_parent_turn,
            content_hash,
            manifest_json,
            node_snapshot_json,
            now_ms
        ],
    )?;
    Ok(turn_snap_id)
}

fn collect_live_snapshot_files(
    conn: &rusqlite::Connection,
    project_registry: &SpecRegistry,
    canon_project_root: &Path,
    project_id: Uuid,
) -> Result<Vec<SnapshotFileItemDto>, Response> {
    let mut files =
        load_workspace_files_from_db(conn, project_id).map_err(map_error_to_response)?;
    let mut seen: HashSet<String> = files.iter().map(|f| f.path.clone()).collect();

    for spec_key in CanonicalSpecKey::ALL {
        if project_registry
            .get_spec(project_id, spec_key)
            .map_err(map_error_to_response)?
            .is_some()
        {
            if let Some(bytes) = read_jailed_spec_bytes(canon_project_root, spec_key)
                .map_err(map_error_to_response)?
            {
                if let Ok(content) = String::from_utf8(bytes) {
                    let rel = format!("product/specs/{}", spec_key.as_str());
                    if !is_secret_path(&rel) && seen.insert(rel.clone()) {
                        files.push(SnapshotFileItemDto {
                            path: rel,
                            content,
                            sha256: None,
                            size_bytes: None,
                        });
                    }
                }
            }
        }
    }

    if files.is_empty() {
        let proj_row: Option<(String, String, String, String)> = conn
            .query_row(
                "SELECT slug, name, product_brief, status FROM projects WHERE id = ?1",
                params![project_id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
            .map_err(|e| map_error_to_response(map_sqlite_error(e, "load_project_for_snapshot")))?;
        let mut node_rows = Vec::new();
        if let Ok(mut stmt) = conn.prepare(
            "SELECT id, screen_key, title, route_path, viewport_preset FROM flow_nodes_v3 WHERE project_id = ?1 ORDER BY screen_key ASC",
        ) {
            if let Ok(mapped) = stmt.query_map(params![project_id.to_string()], |r| {
                Ok(serde_json::json!({
                    "id": r.get::<_, String>(0)?,
                    "screenKey": r.get::<_, String>(1)?,
                    "title": r.get::<_, String>(2)?,
                    "routePath": r.get::<_, String>(3)?,
                    "viewportPreset": r.get::<_, String>(4)?,
                }))
            }) {
                for row in mapped.flatten() {
                    node_rows.push(row);
                }
            }
        }
        let manifest = serde_json::json!({
            "projectId": project_id.to_string(),
            "project": proj_row.map(|(slug, name, brief, status)| serde_json::json!({
                "slug": slug, "name": name, "productBrief": brief, "status": status,
            })),
            "flowNodes": node_rows,
        });
        let content = serde_json::to_string_pretty(&manifest).unwrap_or_else(|_| "{}".to_string());
        files.push(SnapshotFileItemDto {
            path: "workspace-state.json".to_string(),
            content,
            sha256: None,
            size_bytes: None,
        });
    }

    Ok(files)
}

pub(super) async fn handle_create_snapshot(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<CreateSnapshotRequest>,
) -> Result<(StatusCode, Json<VersionSnapshotDto>), Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let now = chrono::Utc::now().to_rfc3339();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let snapshot_id = req.id.unwrap_or_else(Uuid::new_v4);
    let proposal_id = normalize_opt_ref(req.proposal_id);
    let parent_snapshot_id = normalize_opt_ref(req.parent_snapshot_id);
    let chat_artifact_snapshot_id = normalize_opt_ref(req.chat_artifact_snapshot_id);

    let dto = tokio::task::spawn_blocking(move || {
        let mut conn = project_registry.pool().connection().map_err(map_error_to_response)?;
        let should_sync_workspace = !req.files.is_empty();
        let raw_files = if req.files.is_empty() {
            collect_live_snapshot_files(&conn, &project_registry, &canon_project_root, project_id)?
        } else {
            req.files
        };
        let tree_sha256 = compute_files_tree_sha(&raw_files).map_err(map_error_to_response)?;
        let normalized_files: Vec<SnapshotFileItemDto> = raw_files
            .into_iter()
            .map(|mut f| {
                f.path = validate_safe_relative_path(&f.path)?;
                let sha = format!("{:x}", Sha256::digest(f.content.as_bytes()));
                f.sha256 = Some(sha);
                f.size_bytes = Some(f.content.len());
                Ok(f)
            })
            .collect::<Result<Vec<_>, HarnessError>>()
            .map_err(map_error_to_response)?;
        let files_json = serde_json::to_string(&normalized_files).map_err(|e| {
            map_error_to_response(HarnessError::ValidationFailed {
                reason: format!("SNAPSHOT_FILES_SERIALIZE_ERROR: {e}"),
            })
        })?;

        verify_snapshot_parent_refs(
            &conn,
            project_id,
            &tenant_id,
            proposal_id.as_deref(),
            parent_snapshot_id.as_deref(),
            chat_artifact_snapshot_id.as_deref(),
        )?;
        let existing_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM version_snapshots WHERE project_id = ?1",
                params![project_id.to_string()],
                |r| r.get(0),
            )
            .map_err(|e| map_error_to_response(map_sqlite_error(e, "count_snapshots")))?;
        let version_tag = req
            .version_tag
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("v1.{}.0", existing_count + 1));
        let mut effective_turn_snap_id = chat_artifact_snapshot_id.clone();

        retry_on_busy_053("insert_version_snapshot", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            if effective_turn_snap_id.is_none() {
                effective_turn_snap_id = Some(insert_immutable_turn_snapshot_tx(
                    &tx,
                    project_id,
                    &tenant_id,
                    None,
                    proposal_id.as_deref(),
                    None,
                    &req.label,
                    &files_json,
                    &tree_sha256,
                    now_ms,
                )?);
            }
            tx.execute(
                "INSERT INTO version_snapshots (
                    id, project_id, snapshot_tag, manifest_sha256,
                    tenant_id, version_tag, label, description, trigger,
                    tree_sha256, files_json, proposal_id, parent_snapshot_id,
                    chat_artifact_snapshot_id, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                params![
                    snapshot_id.to_string(),
                    project_id.to_string(),
                    format!("{version_tag}-{}", &snapshot_id.to_string()[..8]),
                    tree_sha256,
                    tenant_id,
                    version_tag,
                    req.label,
                    req.description,
                    req.trigger,
                    tree_sha256,
                    files_json,
                    proposal_id,
                    parent_snapshot_id,
                    effective_turn_snap_id,
                    now,
                    now,
                ],
            )?;
            if should_sync_workspace {
                for f in &normalized_files {
                    let sha = f
                        .sha256
                        .clone()
                        .unwrap_or_else(|| format!("{:x}", Sha256::digest(f.content.as_bytes())));
                    tx.execute(
                        "INSERT INTO project_workspace_files (
                            id, project_id, relative_path, file_sha256, byte_size, content, created_at, updated_at
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                         ON CONFLICT(project_id, relative_path) DO UPDATE SET
                            file_sha256 = excluded.file_sha256,
                            byte_size = excluded.byte_size,
                            content = excluded.content,
                            updated_at = excluded.updated_at",
                        params![
                            Uuid::new_v4().to_string(),
                            project_id.to_string(),
                            f.path,
                            sha,
                            f.content.len() as i64,
                            f.content,
                            now,
                            now,
                        ],
                    )?;
                }
            }
            tx.commit()
        })?;

        Ok::<_, Response>(VersionSnapshotDto {
            id: snapshot_id,
            tenant_id,
            project_id,
            version_tag,
            label: req.label,
            description: req.description,
            trigger: req.trigger,
            tree_sha256,
            files: normalized_files,
            metadata: req.metadata,
            proposal_id,
            parent_snapshot_id,
            chat_artifact_snapshot_id: effective_turn_snap_id,
            created_at: now,
        })
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })??;

    Ok((StatusCode::CREATED, Json(dto)))
}

fn load_workspace_files_from_db(
    conn: &rusqlite::Connection,
    project_id: Uuid,
) -> Result<Vec<SnapshotFileItemDto>, HarnessError> {
    let mut stmt = conn
        .prepare(
            "SELECT relative_path, content, file_sha256, byte_size
             FROM project_workspace_files
             WHERE project_id = ?1
             ORDER BY relative_path ASC",
        )
        .map_err(|e| map_sqlite_error(e, "prepare_load_workspace_files"))?;
    let rows = stmt
        .query_map(params![project_id.to_string()], |row| {
            Ok(SnapshotFileItemDto {
                path: row.get(0)?,
                content: row.get(1)?,
                sha256: row.get::<_, Option<String>>(2)?,
                size_bytes: row.get::<_, Option<i64>>(3)?.map(|n| n.max(0) as usize),
            })
        })
        .map_err(|e| map_sqlite_error(e, "query_load_workspace_files"))?;
    let mut files = Vec::new();
    for r in rows {
        files.push(r.map_err(|e| map_sqlite_error(e, "row_load_workspace_files"))?);
    }
    Ok(files)
}

struct StagingDirGuard {
    path: PathBuf,
}

impl StagingDirGuard {
    fn create(canon_project_root: &Path) -> Result<Self, HarnessError> {
        let staging_parent = canon_project_root.join(".cowok-staging");
        crate::lockfile::reject_if_symlink(&staging_parent)?;
        std::fs::create_dir_all(&staging_parent).map_err(|e| HarnessError::ValidationFailed {
            reason: format!("STAGING_PARENT_CREATE_FAILED: {e}"),
        })?;
        crate::lockfile::reject_if_symlink(&staging_parent)?;
        let stage_dir = staging_parent.join(Uuid::new_v4().to_string());
        std::fs::create_dir_all(&stage_dir).map_err(|e| HarnessError::ValidationFailed {
            reason: format!("STAGING_DIR_CREATE_FAILED: {e}"),
        })?;
        Ok(Self { path: stage_dir })
    }
}

impl Drop for StagingDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
}

#[derive(Debug, Clone)]
struct NodeRestorePayload {
    id: String,
    graph_id: String,
    screen_key: String,
    title: String,
    node_type: String,
    route_path: String,
    viewport_preset: String,
    pos_x: f64,
    pos_y: f64,
}

fn extract_node_restores_from_json(
    conn: &rusqlite::Connection,
    project_id: Uuid,
    tenant_id: &str,
    linked_node_id: Option<&str>,
    node_snapshot_json: &str,
) -> Result<Vec<NodeRestorePayload>, HarnessError> {
    let fallback_graph: Option<String> = conn
        .query_row(
            "SELECT id FROM flow_graphs WHERE project_id = ?1 AND tenant_id = ?2 ORDER BY updated_at DESC LIMIT 1",
            params![project_id.to_string(), tenant_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "lookup_fallback_graph_for_restore"))?;

    let parsed: serde_json::Value =
        serde_json::from_str(node_snapshot_json).unwrap_or(serde_json::Value::Null);
    let mut items = Vec::new();
    let push_obj = |obj: &serde_json::Map<String, serde_json::Value>,
                    out: &mut Vec<NodeRestorePayload>| {
        let nid = obj
            .get("id")
            .or_else(|| obj.get("nodeId"))
            .and_then(|v| v.as_str())
            .or(linked_node_id)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from);
        let gid = obj
            .get("graphId")
            .or_else(|| obj.get("graph_id"))
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| fallback_graph.clone());
        if let (Some(id), Some(graph_id)) = (nid, gid) {
            out.push(NodeRestorePayload {
                screen_key: obj
                    .get("screenKey")
                    .or_else(|| obj.get("screen_key"))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or(&id)
                    .to_string(),
                title: obj
                    .get("title")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or("Restored Screen")
                    .to_string(),
                node_type: obj
                    .get("nodeType")
                    .or_else(|| obj.get("node_type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("screen")
                    .to_string(),
                route_path: obj
                    .get("routePath")
                    .or_else(|| obj.get("route_path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("/")
                    .to_string(),
                viewport_preset: obj
                    .get("viewportPreset")
                    .or_else(|| obj.get("viewport_preset"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("desktop-1440")
                    .to_string(),
                pos_x: obj.get("posX").and_then(|v| v.as_f64()).unwrap_or(0.0),
                pos_y: obj.get("posY").and_then(|v| v.as_f64()).unwrap_or(0.0),
                id,
                graph_id,
            });
        }
    };
    if let Some(arr) = parsed.get("nodes").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(obj) = item.as_object() {
                push_obj(obj, &mut items);
            }
        }
    } else if let Some(obj) = parsed.as_object() {
        push_obj(obj, &mut items);
    } else if let (Some(nid), Some(gid)) = (linked_node_id, fallback_graph.clone()) {
        items.push(NodeRestorePayload {
            id: nid.to_string(),
            graph_id: gid,
            screen_key: nid.to_string(),
            title: "Restored Screen".to_string(),
            node_type: "screen".to_string(),
            route_path: "/".to_string(),
            viewport_preset: "desktop-1440".to_string(),
            pos_x: 0.0,
            pos_y: 0.0,
        });
    }
    Ok(items)
}

fn check_or_prepare_missing_nodes(
    conn: &rusqlite::Connection,
    project_id: Uuid,
    tenant_id: &str,
    turn_snapshot_id: Option<&str>,
    restore_missing_nodes: bool,
) -> Result<Vec<NodeRestorePayload>, Response> {
    let Some(tsid) = turn_snapshot_id else {
        return Ok(Vec::new());
    };
    let row: Option<(Option<String>, String)> = conn
        .query_row(
            "SELECT node_id, node_snapshot_json FROM chat_artifact_snapshots
             WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
            params![tsid, project_id.to_string(), tenant_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| map_error_to_response(map_sqlite_error(e, "load_turn_node_snapshot")))?;
    let Some((linked_nid, node_snap_json)) = row else {
        return Ok(Vec::new());
    };
    let candidates = extract_node_restores_from_json(
        conn,
        project_id,
        tenant_id,
        linked_nid.as_deref(),
        &node_snap_json,
    )
    .map_err(map_error_to_response)?;

    let mut missing = Vec::new();
    for c in candidates {
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM flow_nodes WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
                params![c.id, project_id.to_string(), tenant_id],
                |_| Ok(()),
            )
            .optional()
            .map_err(|e| map_error_to_response(map_sqlite_error(e, "check_live_flow_node")))?
            .is_some();
        if !exists {
            missing.push(c);
        }
    }
    if missing.is_empty() {
        return Ok(Vec::new());
    }
    if !restore_missing_nodes {
        let ids: Vec<&str> = missing.iter().map(|m| m.id.as_str()).collect();
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "code": "NODE_DELETED_REQUIRES_GRAPH_RESTORE",
                "error": "NODE_DELETED_REQUIRES_GRAPH_RESTORE",
                "missingNodeIds": ids,
                "message": "Target snapshot references deleted FlowNode(s). Pass restoreMissingNodes: true to restore them.",
            })),
        )
            .into_response());
    }
    Ok(missing)
}

fn apply_files_to_disk_jail(
    canon_project_root: &Path,
    existing_files: &[SnapshotFileItemDto],
    target_files: &[SnapshotFileItemDto],
) -> Result<(), HarnessError> {
    let mut target_paths = HashSet::with_capacity(target_files.len());
    for f in target_files {
        target_paths.insert(validate_safe_relative_path(&f.path)?);
    }
    for existing in existing_files {
        if let Ok(clean_rel) = validate_safe_relative_path(&existing.path) {
            if !target_paths.contains(&clean_rel) {
                sync_workspace_file_to_disk(canon_project_root, &clean_rel, true, "")?;
            }
        }
    }
    for f in target_files {
        sync_workspace_file_to_disk(canon_project_root, &f.path, false, &f.content)?;
    }
    Ok(())
}

fn update_revert_journal_status(
    conn: &mut rusqlite::Connection,
    event_id: &str,
    status: &str,
    target_snapshot_id: Uuid,
    safety_snapshot_id: Uuid,
    target_tree_sha256: &str,
) -> Result<(), Response> {
    let payload = serde_json::json!({
        "status": status,
        "target_snapshot_id": target_snapshot_id.to_string(),
        "safety_snapshot_id": safety_snapshot_id.to_string(),
        "target_tree_sha256": target_tree_sha256,
    })
    .to_string();
    retry_on_busy_053("update_revert_journal", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE execution_events SET payload_text = ?1 WHERE id = ?2",
            params![payload, event_id],
        )?;
        tx.commit()
    })
}

fn reconcile_incomplete_revert_journals(
    conn: &mut rusqlite::Connection,
    canon_project_root: &Path,
    project_id: Uuid,
    tenant_id: &str,
) -> Result<(), Response> {
    let rows: Vec<(String, String)> = {
        let mut stmt = conn
            .prepare(
                "SELECT id, payload_text FROM execution_events
                 WHERE project_id = ?1 AND tenant_id = ?2 AND event_kind = 'revert_tx_journal'
                 ORDER BY created_at ASC, seq_no ASC",
            )
            .map_err(|e| map_error_to_response(map_sqlite_error(e, "query_revert_journals")))?;
        let mapped = stmt
            .query_map(params![project_id.to_string(), tenant_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| map_error_to_response(map_sqlite_error(e, "map_revert_journals")))?;
        mapped.filter_map(Result::ok).collect()
    };

    for (event_id, payload_text) in rows {
        let val: serde_json::Value =
            serde_json::from_str(&payload_text).unwrap_or(serde_json::Value::Null);
        let status = val.get("status").and_then(|s| s.as_str()).unwrap_or("");
        if status == "COMPENSATION_FAILED" {
            return Err(json_err_resp(
                StatusCode::CONFLICT,
                "WORKSPACE_RECONCILIATION_REQUIRED",
                "Unresolved COMPENSATION_FAILED revert journal blocks workspace export/revert until reconciled.",
            ));
        }
        if status == "PENDING_DISK_SWAP" {
            let committed_files =
                load_workspace_files_from_db(conn, project_id).map_err(map_error_to_response)?;
            let target_id = val
                .get("target_snapshot_id")
                .and_then(|s| s.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .unwrap_or_else(Uuid::nil);
            let safety_id = val
                .get("safety_snapshot_id")
                .and_then(|s| s.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .unwrap_or_else(Uuid::nil);
            let sha = val
                .get("target_tree_sha256")
                .and_then(|s| s.as_str())
                .unwrap_or("");
            if apply_files_to_disk_jail(canon_project_root, &[], &committed_files).is_ok() {
                update_revert_journal_status(
                    conn,
                    &event_id,
                    "COMMITTED_COMPLETE",
                    target_id,
                    safety_id,
                    sha,
                )?;
            } else {
                update_revert_journal_status(
                    conn,
                    &event_id,
                    "COMPENSATION_FAILED",
                    target_id,
                    safety_id,
                    sha,
                )?;
                return Err(json_err_resp(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "DISK_REVERT_COMPENSATION_FAILED",
                    "Failed reconciling incomplete revert journal to disk",
                ));
            }
        }
    }
    Ok(())
}

fn load_revert_target_snapshot(
    conn: &rusqlite::Connection,
    project_id: Uuid,
    tenant_id: &str,
    snapshot_id: Uuid,
) -> Result<Option<VersionSnapshotDto>, Response> {
    let vs: Option<VersionSnapshotDto> = conn
        .query_row(
            "SELECT id, project_id, tenant_id, version_tag, label, description,
                    trigger, tree_sha256, files_json, created_at,
                    proposal_id, parent_snapshot_id, chat_artifact_snapshot_id
             FROM version_snapshots
             WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
            params![snapshot_id.to_string(), project_id.to_string(), tenant_id],
            map_snapshot_row,
        )
        .optional()
        .map_err(|e| map_error_to_response(map_sqlite_error(e, "load_target_snapshot")))?;
    if vs.is_some() {
        return Ok(vs);
    }
    let turn_row: Option<(String, String, i64)> = conn
        .query_row(
            "SELECT content_hash, manifest_json, created_at
             FROM chat_artifact_snapshots
             WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
            params![snapshot_id.to_string(), project_id.to_string(), tenant_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| map_error_to_response(map_sqlite_error(e, "load_turn_for_revert")))?;
    let Some((content_hash, manifest_json, created_ms)) = turn_row else {
        return Ok(None);
    };
    let manifest_val: serde_json::Value =
        serde_json::from_str(&manifest_json).unwrap_or(serde_json::Value::Null);
    let files: Vec<SnapshotFileItemDto> = if manifest_val.is_array() {
        serde_json::from_value(manifest_val).unwrap_or_default()
    } else {
        manifest_val
            .get("files")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default()
    };
    if files.is_empty() {
        return Err(json_err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_TURN_MANIFEST",
            "Turn snapshot manifest contains zero restorable files",
        ));
    }
    Ok(Some(VersionSnapshotDto {
        id: snapshot_id,
        tenant_id: tenant_id.to_string(),
        project_id,
        version_tag: "turn-snapshot".to_string(),
        label: "Turn Snapshot".to_string(),
        description: String::new(),
        trigger: "turn_receipt".to_string(),
        tree_sha256: content_hash,
        files,
        metadata: None,
        proposal_id: None,
        parent_snapshot_id: None,
        chat_artifact_snapshot_id: Some(snapshot_id.to_string()),
        created_at: epoch_ms_to_rfc3339(created_ms),
    }))
}

fn restore_safety_files_to_disk(
    canon_project_root: &Path,
    safety_files: &[SnapshotFileItemDto],
    target_files: &[SnapshotFileItemDto],
) -> Result<(), HarnessError> {
    let mut safety_paths = HashSet::with_capacity(safety_files.len());
    for f in safety_files {
        safety_paths.insert(validate_safe_relative_path(&f.path)?);
    }
    for t in target_files {
        if let Ok(clean_rel) = validate_safe_relative_path(&t.path) {
            if !safety_paths.contains(&clean_rel)
                && std::fs::symlink_metadata(canon_project_root.join(&clean_rel))
                    .is_ok_and(|m| m.file_type().is_file())
            {
                sync_workspace_file_to_disk(canon_project_root, &clean_rel, true, "")?;
            }
        }
    }
    for f in safety_files {
        sync_workspace_file_to_disk(canon_project_root, &f.path, false, &f.content)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn compensate_failed_revert_swap(
    conn: &mut rusqlite::Connection,
    canon_project_root: &Path,
    project_id: Uuid,
    event_id: &str,
    snapshot_id: Uuid,
    safety_id: Uuid,
    safety_files: &[SnapshotFileItemDto],
    target_files: &[SnapshotFileItemDto],
    target_tree_sha: &str,
) -> Response {
    let now_str = chrono::Utc::now().to_rfc3339();
    let db_res = retry_on_busy_053("compensate_revert_db", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        rewrite_workspace_files_tx(&tx, project_id, safety_files, &now_str)?;
        tx.commit()
    });
    if db_res.is_err()
        || restore_safety_files_to_disk(canon_project_root, safety_files, target_files).is_err()
    {
        let _ = update_revert_journal_status(
            conn,
            event_id,
            "COMPENSATION_FAILED",
            snapshot_id,
            safety_id,
            target_tree_sha,
        );
        return json_err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DISK_REVERT_COMPENSATION_FAILED",
            "Disk swap failed and safety compensation failed; manual reconciliation required.",
        );
    }
    let _ = update_revert_journal_status(
        conn,
        event_id,
        "COMPENSATED",
        snapshot_id,
        safety_id,
        target_tree_sha,
    );
    json_err_resp(
        StatusCode::INTERNAL_SERVER_ERROR,
        "DISK_REVERT_COMPENSATED",
        "Disk swap failed after DB commit; state compensated back to pre_revert_safety.",
    )
}

fn insert_restored_nodes_tx(
    tx: &rusqlite::Transaction<'_>,
    tenant_id: &str,
    project_id: Uuid,
    node_restores: &[NodeRestorePayload],
    now_ms: i64,
) -> Result<(), rusqlite::Error> {
    for nr in node_restores {
        tx.execute(
            "INSERT OR IGNORE INTO flow_nodes (
                id, graph_id, tenant_id, project_id, portal_id, screen_key,
                title, node_type, route_path, viewport_preset, pos_x, pos_y,
                revision, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 'default', ?5, ?6, ?7, ?8, ?9, ?10, ?11, 1, ?12, ?12)",
            params![
                nr.id,
                nr.graph_id,
                tenant_id,
                project_id.to_string(),
                nr.screen_key,
                nr.title,
                nr.node_type,
                nr.route_path,
                nr.viewport_preset,
                nr.pos_x,
                nr.pos_y,
                now_ms,
            ],
        )?;
    }
    Ok(())
}

fn rewrite_workspace_files_tx(
    tx: &rusqlite::Transaction<'_>,
    project_id: Uuid,
    files: &[SnapshotFileItemDto],
    now: &str,
) -> Result<(), rusqlite::Error> {
    tx.execute(
        "DELETE FROM project_workspace_files WHERE project_id = ?1",
        params![project_id.to_string()],
    )?;
    for f in files {
        let rel_path = validate_safe_relative_path(&f.path)
            .map_err(|_| rusqlite::Error::InvalidParameterName(f.path.clone()))?;
        let sha = format!("{:x}", Sha256::digest(f.content.as_bytes()));
        tx.execute(
            "INSERT INTO project_workspace_files (
                id, project_id, relative_path, file_sha256, byte_size, content, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                Uuid::new_v4().to_string(),
                project_id.to_string(),
                rel_path,
                sha,
                f.content.len() as i64,
                f.content,
                now,
                now,
            ],
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn execute_revert_tx(
    conn: &mut rusqlite::Connection,
    project_id: Uuid,
    tenant_id: &str,
    snapshot_id: Uuid,
    safety_id: Uuid,
    safety_label: &str,
    safety_sha: &str,
    safety_files_json: &str,
    target: &VersionSnapshotDto,
    node_restores: &[NodeRestorePayload],
    now: &str,
) -> Result<String, Response> {
    let safety_tag = format!("safety-backup-{}", &safety_id.to_string()[..8]);
    let now_ms = chrono::Utc::now().timestamp_millis();
    let journal_payload = serde_json::json!({
        "status": "PENDING_DISK_SWAP",
        "target_snapshot_id": snapshot_id.to_string(),
        "safety_snapshot_id": safety_id.to_string(),
        "target_tree_sha256": target.tree_sha256,
    })
    .to_string();
    retry_on_busy_053("revert_version_snapshot", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO version_snapshots (
                id, project_id, snapshot_tag, manifest_sha256,
                tenant_id, version_tag, label, description, trigger,
                tree_sha256, files_json, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pre_revert_safety', ?9, ?10, ?11, ?12)",
            params![
                safety_id.to_string(),
                project_id.to_string(),
                safety_tag,
                safety_sha,
                tenant_id,
                "safety-backup",
                safety_label,
                format!("Auto safety backup before reverting to {snapshot_id}"),
                safety_sha,
                safety_files_json,
                now,
                now,
            ],
        )?;
        insert_restored_nodes_tx(&tx, tenant_id, project_id, node_restores, now_ms)?;
        rewrite_workspace_files_tx(&tx, project_id, &target.files, now)?;
        let journal_event_id = append_execution_event_tx(
            &tx,
            tenant_id,
            project_id,
            "revert_tx_journal",
            &journal_payload,
            now_ms,
        )?;
        tx.commit()?;
        Ok(journal_event_id)
    })
}

pub(super) async fn handle_revert_snapshot(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath((id, snapshot_id_raw)): AxumPath<(String, String)>,
    headers: HeaderMap,
    Json(req): Json<RevertSnapshotRequest>,
) -> Result<Json<RevertResultDto>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let snapshot_id = parse_project_uuid(&snapshot_id_raw).map_err(map_error_to_response)?;
    let (project_registry, canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let now = chrono::Utc::now().to_rfc3339();
    let safety_id = Uuid::new_v4();

    let result = tokio::task::spawn_blocking(move || {
        let mut conn = project_registry
            .pool()
            .connection()
            .map_err(map_error_to_response)?;
        reconcile_incomplete_revert_journals(
            &mut conn,
            &canon_project_root,
            project_id,
            &tenant_id,
        )?;
        let Some(target) = load_revert_target_snapshot(&conn, project_id, &tenant_id, snapshot_id)?
        else {
            return Err(snapshot_not_found(snapshot_id));
        };
        let node_restores = check_or_prepare_missing_nodes(
            &conn,
            project_id,
            &tenant_id,
            target.chat_artifact_snapshot_id.as_deref(),
            req.restore_missing_nodes,
        )?;
        let existing_files =
            load_workspace_files_from_db(&conn, project_id).map_err(map_error_to_response)?;
        let safety_files = if existing_files.is_empty() {
            target.files.clone()
        } else {
            existing_files.clone()
        };
        let safety_sha = compute_files_tree_sha(&safety_files).map_err(map_error_to_response)?;
        let safety_files_json = serde_json::to_string(&safety_files).map_err(|e| {
            map_error_to_response(HarnessError::ValidationFailed {
                reason: format!("SAFETY_FILES_SERIALIZE_ERROR: {e}"),
            })
        })?;
        let safety_label = req
            .safety_label
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Pre-Revert Safety Backup".to_string());

        let staging_guard =
            StagingDirGuard::create(&canon_project_root).map_err(map_error_to_response)?;
        for f in &target.files {
            sync_workspace_file_to_disk(&staging_guard.path, &f.path, false, &f.content)
                .map_err(map_error_to_response)?;
        }

        let journal_event_id = execute_revert_tx(
            &mut conn,
            project_id,
            &tenant_id,
            snapshot_id,
            safety_id,
            &safety_label,
            &safety_sha,
            &safety_files_json,
            &target,
            &node_restores,
            &now,
        )?;

        if apply_files_to_disk_jail(&canon_project_root, &existing_files, &target.files).is_err() {
            return Err(compensate_failed_revert_swap(
                &mut conn,
                &canon_project_root,
                project_id,
                &journal_event_id,
                snapshot_id,
                safety_id,
                &safety_files,
                &target.files,
                &target.tree_sha256,
            ));
        }
        drop(staging_guard);
        update_revert_journal_status(
            &mut conn,
            &journal_event_id,
            "COMMITTED_COMPLETE",
            snapshot_id,
            safety_id,
            &target.tree_sha256,
        )?;

        Ok(RevertResultDto {
            success: true,
            reverted_to_snapshot_id: snapshot_id,
            safety_snapshot_id: safety_id,
            restored_files_count: target.files.len(),
            restored_node_ids: node_restores.into_iter().map(|n| n.id).collect(),
            timestamp: now,
        })
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })??;

    Ok(Json(result))
}

struct ForkOriginMeta {
    orig_sid: String,
    orig_mid: String,
    snap_node_id: Option<String>,
    orig_title: String,
    orig_provider: String,
    cutoff_created_at: i64,
}

fn load_fork_origin_meta(
    conn: &rusqlite::Connection,
    pid_str: &str,
    tenant_id: &str,
    turn_snapshot_id: &str,
) -> Result<ForkOriginMeta, Response> {
    let snap_meta: Option<(String, String, Option<String>)> = conn
        .query_row(
            "SELECT session_id, message_id, node_id FROM chat_artifact_snapshots
             WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
            params![turn_snapshot_id, pid_str, tenant_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| map_error_to_response(map_sqlite_error(e, "lookup_turn_snapshot")))?;
    let Some((orig_sid, orig_mid, snap_node_id)) = snap_meta else {
        return Err(json_err_resp(
            StatusCode::NOT_FOUND,
            "REFERENCE_NOT_FOUND",
            &format!("TurnSnapshot '{turn_snapshot_id}' was not found for this project/tenant"),
        ));
    };
    let orig_sess: Option<(String, String)> = conn
        .query_row(
            "SELECT title, model_provider FROM chat_sessions WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
            params![orig_sid, pid_str, tenant_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| map_error_to_response(map_sqlite_error(e, "lookup_orig_session")))?;
    let Some((orig_title, orig_provider)) = orig_sess else {
        return Err(json_err_resp(
            StatusCode::NOT_FOUND,
            "REFERENCE_NOT_FOUND",
            "Original chat session not found for project/tenant",
        ));
    };
    let cutoff_ts: Option<i64> = conn
        .query_row(
            "SELECT created_at FROM chat_messages WHERE id = ?1 AND session_id = ?2 AND tenant_id = ?3",
            params![orig_mid, orig_sid, tenant_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| map_error_to_response(map_sqlite_error(e, "lookup_orig_message")))?;
    let Some(cutoff_created_at) = cutoff_ts else {
        return Err(json_err_resp(
            StatusCode::NOT_FOUND,
            "REFERENCE_NOT_FOUND",
            "Original chat message not found for session/tenant",
        ));
    };
    Ok(ForkOriginMeta {
        orig_sid,
        orig_mid,
        snap_node_id,
        orig_title,
        orig_provider,
        cutoff_created_at,
    })
}

fn is_flow_node_live_tx(
    tx: &rusqlite::Transaction<'_>,
    node_id: &str,
    pid_str: &str,
    tenant_id: &str,
) -> Result<bool, rusqlite::Error> {
    tx.query_row(
        "SELECT 1 FROM flow_nodes WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
        params![node_id, pid_str, tenant_id],
        |_| Ok(()),
    )
    .optional()
    .map(|opt| opt.is_some())
}

#[allow(clippy::too_many_arguments)]
fn copy_fork_message_scopes_tx(
    tx: &rusqlite::Transaction<'_>,
    pid_str: &str,
    tenant_id: &str,
    old_mid: &str,
    new_mid: &str,
    new_sid: &str,
    detached_set: &mut HashSet<String>,
    detached_node_ids: &mut Vec<String>,
) -> Result<(), rusqlite::Error> {
    let scopes: Vec<(String, i64)> = {
        let mut s_stmt = tx.prepare(
            "SELECT node_id, created_at FROM chat_message_node_scopes
             WHERE message_id = ?1 AND project_id = ?2 AND tenant_id = ?3",
        )?;
        let mapped = s_stmt.query_map(params![old_mid, pid_str, tenant_id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?;
        mapped.collect::<Result<Vec<_>, _>>()?
    };
    for (nid, s_cr) in scopes {
        let live = is_flow_node_live_tx(tx, &nid, pid_str, tenant_id)?;
        let scope_key = if live {
            format!("{pid_str}::{new_sid}::{nid}")
        } else {
            if detached_set.insert(nid.clone()) {
                detached_node_ids.push(nid.clone());
            }
            format!("{pid_str}::{new_sid}::{nid}::detached")
        };
        tx.execute(
            "INSERT INTO chat_message_node_scopes (
                id, tenant_id, message_id, project_id, session_id, node_id, composite_scope_key, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                Uuid::new_v4().to_string(),
                tenant_id,
                new_mid,
                pid_str,
                new_sid,
                nid,
                scope_key,
                s_cr
            ],
        )?;
    }
    Ok(())
}

fn execute_fork_turn_tx(
    conn: &mut rusqlite::Connection,
    project_id: Uuid,
    tenant_id: &str,
    turn_snapshot_id: &str,
    title_override: Option<&str>,
    include_node_scope: bool,
    now_ms: i64,
) -> Result<TurnForkResultDto, Response> {
    let pid_str = project_id.to_string();
    let meta = load_fork_origin_meta(conn, &pid_str, tenant_id, turn_snapshot_id)?;
    let new_sid = Uuid::new_v4().to_string();
    let fork_title = title_override
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or_else(|| format!("{} (Fork)", meta.orig_title));

    retry_on_busy_053("fork_turn_snapshot", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO chat_sessions (
                id, tenant_id, project_id, title, model_provider,
                forked_from_session_id, forked_from_message_id, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                new_sid,
                tenant_id,
                pid_str,
                fork_title,
                meta.orig_provider,
                meta.orig_sid,
                meta.orig_mid,
                now_ms
            ],
        )?;
        type ForkMessageRow = (String, String, String, Option<String>, Option<String>, i64);
        let msgs: Vec<ForkMessageRow> = {
            let mut stmt = tx.prepare(
                "SELECT id, role, prose_markdown, parent_message_id, turn_receipt_json, created_at
                 FROM chat_messages
                 WHERE session_id = ?1 AND tenant_id = ?2 AND created_at <= ?3
                 ORDER BY created_at ASC, rowid ASC",
            )?;
            let mapped = stmt.query_map(
                params![meta.orig_sid, tenant_id, meta.cutoff_created_at],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                },
            )?;
            mapped.collect::<Result<Vec<_>, _>>()?
        };
        let mut msg_id_map = std::collections::HashMap::with_capacity(msgs.len());
        let mut detached_set = HashSet::new();
        let mut detached_node_ids = Vec::new();
        if include_node_scope {
            if let Some(ref snid) = meta.snap_node_id {
                if !is_flow_node_live_tx(&tx, snid, &pid_str, tenant_id)?
                    && detached_set.insert(snid.clone())
                {
                    detached_node_ids.push(snid.clone());
                }
            }
        }
        for (old_mid, role, prose, old_parent, receipt_json, cr_at) in &msgs {
            let new_mid = Uuid::new_v4().to_string();
            let remapped_parent = old_parent
                .as_ref()
                .and_then(|op| msg_id_map.get(op).cloned());
            msg_id_map.insert(old_mid.clone(), new_mid.clone());
            tx.execute(
                "INSERT INTO chat_messages (
                    id, tenant_id, session_id, role, prose_markdown, parent_message_id, turn_receipt_json, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![new_mid, tenant_id, new_sid, role, prose, remapped_parent, receipt_json, cr_at],
            )?;
            if include_node_scope {
                copy_fork_message_scopes_tx(
                    &tx,
                    &pid_str,
                    tenant_id,
                    old_mid,
                    &new_mid,
                    &new_sid,
                    &mut detached_set,
                    &mut detached_node_ids,
                )?;
            }
        }
        tx.commit()?;
        Ok(TurnForkResultDto {
            forked_session_id: new_sid.clone(),
            forked_from_session_id: meta.orig_sid.clone(),
            forked_from_message_id: meta.orig_mid.clone(),
            turn_snapshot_id: turn_snapshot_id.to_string(),
            copied_messages_count: msgs.len(),
            detached_node_ids,
            created_at: epoch_ms_to_rfc3339(now_ms),
        })
    })
}

pub(super) async fn handle_fork_turn_snapshot(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath((id, turn_snapshot_id_raw)): AxumPath<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let turn_snapshot_id = turn_snapshot_id_raw.trim().to_string();
    if turn_snapshot_id.is_empty()
        || turn_snapshot_id.contains('/')
        || turn_snapshot_id.contains("..")
    {
        return Err(json_err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid turnSnapshotId",
        ));
    }
    let req: ForkTurnSnapshotRequest = if body.is_empty() {
        ForkTurnSnapshotRequest {
            title: None,
            include_node_scope: true,
        }
    } else {
        serde_json::from_slice(&body).map_err(|e| {
            json_err_resp(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                &format!("Invalid fork payload: {e}"),
            )
        })?
    };
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let actor_id = extract_actor_id(&headers);
    let idem_key = extract_idempotency_key(&headers);
    let now_ms = chrono::Utc::now().timestamp_millis();
    let req_sha = format!(
        "{:x}",
        Sha256::digest(
            format!(
                "{project_id}:{turn_snapshot_id}:{}:{}",
                req.title.as_deref().unwrap_or(""),
                req.include_node_scope
            )
            .as_bytes()
        )
    );

    tokio::task::spawn_blocking(move || {
        let mut conn = project_registry
            .pool()
            .connection()
            .map_err(map_error_to_response)?;
        if let Some(ref key) = idem_key {
            match reserve_idempotency_053(&mut conn, &tenant_id, &actor_id, key, &req_sha, now_ms)?
            {
                IdemReservation053::Replay(st, body_json) => {
                    return Ok(replay_idempotent_json(st, &body_json))
                }
                IdemReservation053::Conflict(msg) => {
                    return Err(json_err_resp(
                        StatusCode::CONFLICT,
                        "IDEMPOTENCY_CONFLICT",
                        &msg,
                    ));
                }
                IdemReservation053::New => {}
            }
        }
        let res = execute_fork_turn_tx(
            &mut conn,
            project_id,
            &tenant_id,
            &turn_snapshot_id,
            req.title.as_deref(),
            req.include_node_scope,
            now_ms,
        );
        match res {
            Ok(dto) => {
                if let Some(ref key) = idem_key {
                    if let Ok(json_str) = serde_json::to_string(&dto) {
                        finalize_idempotency_053(
                            &mut conn, &tenant_id, &actor_id, key, 201, &json_str,
                        );
                    }
                }
                Ok((StatusCode::CREATED, Json(dto)).into_response())
            }
            Err(resp) => {
                if let Some(ref key) = idem_key {
                    release_idempotency_053(&mut conn, &tenant_id, &actor_id, key);
                }
                Err(resp)
            }
        }
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
}

fn is_secret_path(rel_path: &str) -> bool {
    let lower = rel_path.to_ascii_lowercase();
    for seg in lower.split('/') {
        if seg == ".env"
            || seg.starts_with(".env.")
            || seg == ".npmrc"
            || seg == ".netrc"
            || seg == ".git-credentials"
            || seg.ends_with(".pem")
            || seg.ends_with(".key")
            || seg.starts_with("id_rsa")
            || seg.starts_with("id_ed25519")
            || seg.starts_with("id_ecdsa")
        {
            return true;
        }
    }
    let basename = lower.rsplit('/').next().unwrap_or("");
    basename.contains("secret") && !basename.contains("secret-note")
}

fn verify_disk_path_not_escaping_jail(
    canon_project_root: &Path,
    rel_path: &str,
) -> Result<(), HarnessError> {
    let mut cur = canon_project_root.to_path_buf();
    for seg in rel_path.split('/') {
        cur.push(seg);
        if let Ok(meta) = std::fs::symlink_metadata(&cur) {
            if meta.file_type().is_symlink() {
                return Err(HarnessError::PathTraversalDenied {
                    path: rel_path.to_string(),
                });
            }
        }
    }
    Ok(())
}

fn build_handoff_manifest_bytes(
    project_id: Uuid,
    tenant_id: &str,
    snapshot_id: Option<&str>,
    exported_at: &str,
    entries: &[(String, Vec<u8>)],
    entry_path: Option<&str>,
) -> Vec<u8> {
    let tokens_path = entries
        .iter()
        .find(|(p, _)| p == "src/theme/tokens.css" || p == "src/styles/tokens.css")
        .map(|(p, _)| p.as_str())
        .unwrap_or("src/styles/tokens.css");
    let icon_manifest_path = entries
        .iter()
        .find(|(p, _)| p == "src/assets/icon-manifest.json" || p == "icons/icon-manifest.json")
        .map(|(p, _)| p.as_str())
        .unwrap_or("src/assets/icon-manifest.json");
    let package_json_present = entries.iter().any(|(p, _)| p == "package.json");
    let manifest = serde_json::json!({
        "schemaVersion": "1.0",
        "projectId": project_id.to_string(),
        "tenantId": tenant_id,
        "snapshotId": snapshot_id,
        "exportedAt": exported_at,
        "tokensPath": tokens_path,
        "iconManifestPath": icon_manifest_path,
        "packageJsonPresent": package_json_present,
        "entryPath": entry_path,
    });
    serde_json::to_vec_pretty(&manifest).unwrap_or_else(|_| b"{}".to_vec())
}

fn collect_exportable_files(
    project_registry: &SpecRegistry,
    canon_project_root: &Path,
    project_id: Uuid,
    tenant_id: &str,
) -> Result<Vec<(String, Vec<u8>)>, Response> {
    let mut conn = project_registry
        .pool()
        .connection()
        .map_err(map_error_to_response)?;
    reconcile_incomplete_revert_journals(&mut conn, canon_project_root, project_id, tenant_id)?;

    let (items, latest_snap_id, exported_at) = {
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(|e| map_error_to_response(map_sqlite_error(e, "begin_export_mvcc_read")))?;
        let mut items =
            load_workspace_files_from_db(&tx, project_id).map_err(map_error_to_response)?;
        let latest_snap: Option<(String, String, String)> = tx
            .query_row(
                "SELECT id, files_json, created_at FROM version_snapshots
                 WHERE project_id = ?1 AND tenant_id = ?2
                 ORDER BY created_at DESC LIMIT 1",
                params![project_id.to_string(), tenant_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(|e| {
                map_error_to_response(map_sqlite_error(e, "load_latest_snapshot_for_export"))
            })?;
        let mut snap_id_opt = None;
        let mut det_ts = "1970-01-01T00:00:00Z".to_string();
        if let Some((sid, json_str, cr_at)) = latest_snap {
            snap_id_opt = Some(sid);
            det_ts = cr_at;
            if items.is_empty() {
                items = serde_json::from_str(&json_str).unwrap_or_default();
            }
        }
        tx.commit()
            .map_err(|e| map_error_to_response(map_sqlite_error(e, "commit_export_mvcc_read")))?;
        (items, snap_id_opt, det_ts)
    };

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        let rel = validate_safe_relative_path(&item.path).map_err(map_error_to_response)?;
        if is_secret_path(&rel) {
            continue;
        }
        verify_disk_path_not_escaping_jail(canon_project_root, &rel)
            .map_err(map_error_to_response)?;
        if seen.insert(rel.clone()) {
            out.push((rel, item.content.into_bytes()));
        }
    }

    for spec_key in CanonicalSpecKey::ALL {
        if project_registry
            .get_spec(project_id, spec_key)
            .map_err(map_error_to_response)?
            .is_some()
        {
            if let Some(bytes) = read_jailed_spec_bytes(canon_project_root, spec_key)
                .map_err(map_error_to_response)?
            {
                let rel = format!("product/specs/{}", spec_key.as_str());
                if !is_secret_path(&rel) && seen.insert(rel.clone()) {
                    out.push((rel, bytes));
                }
            }
        }
    }

    if !out.is_empty() && !seen.contains("cowok-handoff-manifest.json") {
        let manifest_bytes = build_handoff_manifest_bytes(
            project_id,
            tenant_id,
            latest_snap_id.as_deref(),
            &exported_at,
            &out,
            None,
        );
        out.push(("cowok-handoff-manifest.json".to_string(), manifest_bytes));
    }

    if out.len() > MAX_ZIP_FILES {
        return Err(json_err_resp(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            &format!(
                "Export entry count {} exceeds MAX_ZIP_FILES ({MAX_ZIP_FILES})",
                out.len()
            ),
        ));
    }
    let mut total_bytes: usize = 0;
    for (rel, data) in &out {
        if data.len() > MAX_SINGLE_FILE_BYTES {
            return Err(json_err_resp(
                StatusCode::PAYLOAD_TOO_LARGE,
                "PAYLOAD_TOO_LARGE",
                &format!("File '{rel}' exceeds MAX_SINGLE_FILE_BYTES ({MAX_SINGLE_FILE_BYTES})"),
            ));
        }
        total_bytes = total_bytes.saturating_add(data.len());
        if total_bytes > MAX_ZIP_TOTAL_BYTES {
            return Err(json_err_resp(
                StatusCode::PAYLOAD_TOO_LARGE,
                "PAYLOAD_TOO_LARGE",
                &format!("Total export size exceeds MAX_ZIP_TOTAL_BYTES ({MAX_ZIP_TOTAL_BYTES})"),
            ));
        }
    }

    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn crc32_ieee(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Builds a valid STORE-method PKZIP archive with strict `u32::MAX` / `u16::MAX` bounds.
fn build_pkzip_archive(entries: &[(String, Vec<u8>)]) -> Result<Vec<u8>, Response> {
    let count = u16::try_from(entries.len()).map_err(|_| {
        json_err_resp(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "PKZIP entry count exceeds u16::MAX",
        )
    })?;
    let mut out = Vec::new();
    let mut central_dir = Vec::new();

    for (name, data) in entries {
        let offset = u32::try_from(out.len()).map_err(|_| {
            json_err_resp(
                StatusCode::PAYLOAD_TOO_LARGE,
                "PAYLOAD_TOO_LARGE",
                "PKZIP offset exceeds u32::MAX",
            )
        })?;
        let name_bytes = name.as_bytes();
        let name_len = u16::try_from(name_bytes.len()).map_err(|_| {
            json_err_resp(
                StatusCode::PAYLOAD_TOO_LARGE,
                "PAYLOAD_TOO_LARGE",
                "PKZIP filename exceeds u16::MAX",
            )
        })?;
        let crc = crc32_ieee(data);
        let size = u32::try_from(data.len()).map_err(|_| {
            json_err_resp(
                StatusCode::PAYLOAD_TOO_LARGE,
                "PAYLOAD_TOO_LARGE",
                "PKZIP entry exceeds u32::MAX",
            )
        })?;

        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0x0800u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(data);

        central_dir.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central_dir.extend_from_slice(&20u16.to_le_bytes());
        central_dir.extend_from_slice(&20u16.to_le_bytes());
        central_dir.extend_from_slice(&0x0800u16.to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes());
        central_dir.extend_from_slice(&crc.to_le_bytes());
        central_dir.extend_from_slice(&size.to_le_bytes());
        central_dir.extend_from_slice(&size.to_le_bytes());
        central_dir.extend_from_slice(&name_len.to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes());
        central_dir.extend_from_slice(&0u32.to_le_bytes());
        central_dir.extend_from_slice(&offset.to_le_bytes());
        central_dir.extend_from_slice(name_bytes);
    }

    let cd_offset = u32::try_from(out.len()).map_err(|_| {
        json_err_resp(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "PKZIP central directory offset exceeds u32::MAX",
        )
    })?;
    let cd_size = u32::try_from(central_dir.len()).map_err(|_| {
        json_err_resp(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "PKZIP central directory size exceeds u32::MAX",
        )
    })?;
    out.extend_from_slice(&central_dir);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    Ok(out)
}

fn compute_export_ready_to_run(entries: &[(String, Vec<u8>)]) -> bool {
    let mut has_tokens = false;
    let mut has_icons = false;
    let mut has_fixtures = false;
    let mut has_manifest = false;
    let mut has_source = false;
    let mut has_build_script = false;

    for (path, bytes) in entries {
        let lower = path.to_ascii_lowercase();
        if lower.ends_with("tokens.css") {
            has_tokens = true;
        }
        if lower.ends_with("icon-manifest.json") {
            has_icons = true;
        }
        if lower.contains("fixture") {
            has_fixtures = true;
        }
        if path == "cowok-handoff-manifest.json" {
            has_manifest = true;
        }
        if lower.contains("src/")
            && (lower.ends_with(".tsx")
                || lower.ends_with(".ts")
                || lower.ends_with(".jsx")
                || lower.ends_with(".js"))
        {
            has_source = true;
        }
        if path == "package.json" || lower.ends_with("/package.json") {
            if let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(bytes) {
                if let Some(script) = parsed
                    .get("scripts")
                    .and_then(|s| s.get("build"))
                    .and_then(|b| b.as_str())
                {
                    if !script.trim().is_empty() {
                        has_build_script = true;
                    }
                }
            }
        }
    }

    has_tokens && has_icons && has_fixtures && has_manifest && has_source && has_build_script
}

pub(super) async fn handle_export_snapshots(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<ExportZipRequest>,
) -> Result<Json<ExportZipResponseDto>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;

    if !req.include_tokens || !req.include_icons {
        return Err(json_err_resp(
            StatusCode::BAD_REQUEST,
            "AC03_RUNNABLE_BUNDLE_VIOLATION",
            "Export bundle must retain design tokens and icon manifest to remain runnable (Story 05.3 AC-03).",
        ));
    }

    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let tenant_for_name = tenant_id.clone();

    let dto = tokio::task::spawn_blocking(move || {
        let entries = collect_exportable_files(
            &project_registry,
            &canon_project_root,
            project_id,
            &tenant_id,
        )?;
        if entries.is_empty() {
            return Err(json_err_resp(
                StatusCode::NOT_FOUND,
                "NO_EXPORTABLE_FILES",
                "No persisted workspace files, snapshots, or specs available to export",
            ));
        }

        let files_included: Vec<String> = entries.iter().map(|(p, _)| p.clone()).collect();
        let has_manifest = files_included
            .iter()
            .any(|p| p == "cowok-handoff-manifest.json");
        let ready_to_run = compute_export_ready_to_run(&entries);
        let zip_bytes = build_pkzip_archive(&entries)?;
        let sha256 = format!("{:x}", Sha256::digest(&zip_bytes));
        let date_str = chrono::Utc::now().format("%Y%m%d").to_string();
        let file_name = format!("{project_id}-{tenant_for_name}-bundle-{date_str}.zip");

        Ok(ExportZipResponseDto {
            file_name,
            byte_length: zip_bytes.len(),
            sha256,
            files_included,
            ready_to_run,
            handoff_manifest_included: has_manifest,
        })
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })??;

    Ok(Json(dto))
}

pub(super) async fn handle_download_export(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let tenant_for_collect = tenant_id.clone();

    let entries = tokio::task::spawn_blocking(move || {
        collect_exportable_files(
            &project_registry,
            &canon_project_root,
            project_id,
            &tenant_for_collect,
        )
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })??;

    if entries.is_empty() {
        return Err(json_err_resp(
            StatusCode::NOT_FOUND,
            "NO_EXPORTABLE_FILES",
            "No persisted workspace files, snapshots, or specs available to download",
        ));
    }

    let zip_bytes = build_pkzip_archive(&entries)?;
    let sha256 = format!("{:x}", Sha256::digest(&zip_bytes));
    let date_str = chrono::Utc::now().format("%Y%m%d").to_string();
    let safe_tenant: String = tenant_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let file_name = format!("{project_id}-{safe_tenant}-bundle-{date_str}.zip");

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{file_name}\""),
        )
        .header("x-cowok-bundle-sha256", sha256)
        .body(axum::body::Body::from(zip_bytes))
        .map_err(|_| {
            json_err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "ZIP_RESPONSE_ERROR",
                "failed to build zip response",
            )
        })
}

fn validate_git_ref(raw: &str) -> Result<String, Response> {
    let s = raw.trim();
    let bytes = s.as_bytes();
    let valid_len = !bytes.is_empty() && bytes.len() <= 121;
    let valid_first = bytes.first().is_some_and(|b| b.is_ascii_alphanumeric());
    let valid_chars = bytes
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'/' | b'-'));
    let valid_seq = !s.contains("..")
        && !s.contains("@{")
        && !s.contains("//")
        && !s.ends_with(".lock")
        && !s.ends_with('/')
        && !s.ends_with('.');
    if !(valid_len && valid_first && valid_chars && valid_seq) {
        return Err(json_err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            &format!("Invalid git branch ref '{s}'"),
        ));
    }
    Ok(s.to_string())
}

fn scrub_secrets(raw: &str, vault_token: Option<&str>) -> String {
    let mut out = raw.to_string();
    if let Some(tok) = vault_token.map(str::trim).filter(|t| !t.is_empty()) {
        out = out.replace(tok, "[REDACTED]");
    }
    for prefix in ["ghp_", "gho_", "ghu_", "ghs_", "github_pat_"] {
        while let Some(idx) = out.find(prefix) {
            let end = out[idx..]
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .map(|rel| idx + rel)
                .unwrap_or(out.len());
            out.replace_range(idx..end, "[REDACTED]");
        }
    }
    out
}

fn pr_cmd_timeout_ms() -> u64 {
    std::env::var("COWOK_PR_CMD_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(|ms| ms.clamp(50, DEFAULT_PR_CMD_TIMEOUT_MS))
        .unwrap_or(DEFAULT_PR_CMD_TIMEOUT_MS)
}

struct ProcessGroupGuard {
    pid: u32,
    active: bool,
}

impl Drop for ProcessGroupGuard {
    fn drop(&mut self) {
        if self.active && self.pid > 0 {
            #[cfg(unix)]
            unsafe {
                libc::kill(-(self.pid as libc::pid_t), libc::SIGKILL);
            }
        }
    }
}

fn spawn_pipe_drain_bounded<R: Read + Send + 'static>(
    mut reader: R,
) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        while let Ok(n) = reader.read(&mut chunk) {
            if n == 0 {
                break;
            }
            let avail = (64 * 1024usize).saturating_sub(buf.len());
            if avail > 0 {
                buf.extend_from_slice(&chunk[..n.min(avail)]);
            }
        }
        String::from_utf8_lossy(&buf).to_string()
    })
}

pub(crate) struct CmdRunOutput {
    pub(crate) success: bool,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) timed_out: bool,
    pub(crate) not_found: bool,
}

pub(crate) fn run_bounded_cmd_with_pgid(
    bin: &str,
    args: &[&str],
    cwd: &Path,
    envs: &[(&str, &str)],
) -> CmdRunOutput {
    let mut cmd = Command::new(bin);
    cmd.args(args)
        .current_dir(cwd)
        .env("CI", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GH_PROMPT_DISABLED", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return CmdRunOutput {
                success: false,
                stdout: String::new(),
                stderr: e.to_string(),
                timed_out: false,
                not_found: e.kind() == std::io::ErrorKind::NotFound,
            };
        }
    };
    let mut guard = ProcessGroupGuard {
        pid: child.id(),
        active: true,
    };
    let out_drain = child.stdout.take().map(spawn_pipe_drain_bounded);
    let err_drain = child.stderr.take().map(spawn_pipe_drain_bounded);
    let deadline = Instant::now() + Duration::from_millis(pr_cmd_timeout_ms());
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                guard.active = false;
                let stdout = out_drain.and_then(|h| h.join().ok()).unwrap_or_default();
                let stderr = err_drain.and_then(|h| h.join().ok()).unwrap_or_default();
                return CmdRunOutput {
                    success: st.success(),
                    stdout,
                    stderr,
                    timed_out: false,
                    not_found: false,
                };
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    drop(guard);
                    let _ = child.wait();
                    let stdout = out_drain.and_then(|h| h.join().ok()).unwrap_or_default();
                    let stderr = err_drain.and_then(|h| h.join().ok()).unwrap_or_default();
                    return CmdRunOutput {
                        success: false,
                        stdout,
                        stderr,
                        timed_out: true,
                        not_found: false,
                    };
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                drop(guard);
                let _ = child.wait();
                return CmdRunOutput {
                    success: false,
                    stdout: String::new(),
                    stderr: e.to_string(),
                    timed_out: false,
                    not_found: false,
                };
            }
        }
    }
}

fn record_pr_audit_event(
    conn: &mut rusqlite::Connection,
    project_id: Uuid,
    tenant_id: &str,
    branch: &str,
    outcome: &str,
    detail: &str,
    token: Option<&str>,
) {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let payload = serde_json::json!({
        "branch": branch,
        "outcome": outcome,
        "detail": scrub_secrets(detail, token),
    })
    .to_string();
    let _ = retry_on_busy_053("record_github_pr_attempt", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        append_execution_event_tx(
            &tx,
            tenant_id,
            project_id,
            "github_pr_attempt",
            &payload,
            now_ms,
        )?;
        tx.commit()
    });
}

fn map_cmd_failure_response(
    out: &CmdRunOutput,
    code: &str,
    msg: &str,
    token: Option<&str>,
) -> Response {
    if out.timed_out {
        return json_err_resp(
            StatusCode::GATEWAY_TIMEOUT,
            "SUBPROCESS_TIMEOUT",
            &format!("{msg}: subprocess timed out"),
        );
    }
    let clean_err = scrub_secrets(&out.stderr, token);
    json_err_resp(
        StatusCode::BAD_GATEWAY,
        code,
        &format!("{msg}: {clean_err}"),
    )
}

fn parse_pr_number_from_url(url: &str) -> Option<u64> {
    url.trim()
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .and_then(|s| s.parse::<u64>().ok())
}

struct EphemeralGhConfigDir {
    path: PathBuf,
}

impl EphemeralGhConfigDir {
    fn create() -> Result<Self, Response> {
        let path = std::env::temp_dir().join(format!("cowok-gh-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).map_err(|e| {
            json_err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "GH_CONFIG_DIR_FAILED",
                &e.to_string(),
            )
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700));
        }
        Ok(Self { path })
    }
}

impl Drop for EphemeralGhConfigDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn preflight_pr_env_and_token(
    conn: &mut rusqlite::Connection,
    daemon_workspace_root: &Path,
    canon_project_root: &Path,
    project_id: Uuid,
    tenant_id: &str,
    req: &GitHubPRRequest,
) -> Result<(EphemeralGhConfigDir, String), Response> {
    if let Some(ref sid) = req.snapshot_id {
        let ok: bool = conn
            .query_row(
                "SELECT 1 FROM version_snapshots WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
                params![sid, project_id.to_string(), tenant_id],
                |_| Ok(()),
            )
            .optional()
            .map_err(|e| map_error_to_response(map_sqlite_error(e, "check_pr_snapshot_owner")))?
            .is_some();
        if !ok {
            return Err(json_err_resp(
                StatusCode::NOT_FOUND,
                "REFERENCE_NOT_FOUND",
                &format!("VersionSnapshot '{sid}' was not found for this project/tenant"),
            ));
        }
    }
    let top_out = run_bounded_cmd_with_pgid(
        "git",
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "rev-parse",
            "--show-toplevel",
        ],
        canon_project_root,
        &[],
    );
    let is_standalone_repo_root = top_out.success
        && std::fs::canonicalize(top_out.stdout.trim()).ok().as_deref() == Some(canon_project_root);
    if !is_standalone_repo_root {
        record_pr_audit_event(
            conn,
            project_id,
            tenant_id,
            &req.branch_name,
            "failed",
            "GIT_REMOTE_MISSING",
            None,
        );
        return Err(json_err_resp(
                StatusCode::NOT_IMPLEMENTED,
                "GIT_REMOTE_MISSING",
                "Scratch workspace has no standalone git repository; link a git repository or download ZIP bundle instead.",
            ));
    }
    let remote_out = run_bounded_cmd_with_pgid(
        "git",
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "remote",
            "get-url",
            "origin",
        ],
        canon_project_root,
        &[],
    );
    if remote_out.timed_out {
        record_pr_audit_event(
            conn,
            project_id,
            tenant_id,
            &req.branch_name,
            "timeout",
            "git remote timed out",
            None,
        );
        return Err(map_cmd_failure_response(
            &remote_out,
            "GITHUB_PR_FAILED",
            "git remote failed",
            None,
        ));
    }
    if !remote_out.success || remote_out.stdout.trim().is_empty() {
        record_pr_audit_event(
            conn,
            project_id,
            tenant_id,
            &req.branch_name,
            "failed",
            "GIT_REMOTE_MISSING",
            None,
        );
        return Err(json_err_resp(
            StatusCode::NOT_IMPLEMENTED,
            "GIT_REMOTE_MISSING",
            "Project workspace has no configured git 'origin' remote; download ZIP bundle instead.",
        ));
    }
    let gh_ver = run_bounded_cmd_with_pgid("gh", &["--version"], canon_project_root, &[]);
    if gh_ver.not_found || (!gh_ver.success && !gh_ver.timed_out) {
        record_pr_audit_event(
            conn,
            project_id,
            tenant_id,
            &req.branch_name,
            "failed",
            "GH_CLI_MISSING",
            None,
        );
        return Err(json_err_resp(
            StatusCode::NOT_IMPLEMENTED,
            "GH_CLI_MISSING",
            "GitHub CLI ('gh') is not installed on PATH; download ZIP bundle instead.",
        ));
    }
    if gh_ver.timed_out {
        record_pr_audit_event(
            conn,
            project_id,
            tenant_id,
            &req.branch_name,
            "timeout",
            "gh --version timed out",
            None,
        );
        return Err(map_cmd_failure_response(
            &gh_ver,
            "GITHUB_PR_FAILED",
            "gh version failed",
            None,
        ));
    }
    let vault = VaultStore::from_workspace_root(daemon_workspace_root);
    let secrets = vault.unseal_secrets(None).unwrap_or_default();
    let Some(gh_token) = secrets
        .get("github")
        .map(String::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        record_pr_audit_event(
            conn,
            project_id,
            tenant_id,
            &req.branch_name,
            "failed",
            "GH_UNAUTHENTICATED_NO_VAULT",
            None,
        );
        return Err(json_err_resp(
            StatusCode::NOT_IMPLEMENTED,
            "GH_UNAUTHENTICATED",
            "GitHub Vault token is missing; authenticate GitHub in Vault or download ZIP bundle.",
        ));
    };
    let gh_cfg_dir = EphemeralGhConfigDir::create()?;
    let gh_cfg_str = gh_cfg_dir.path.to_string_lossy().to_string();
    let gh_envs = [
        ("GH_CONFIG_DIR", gh_cfg_str.as_str()),
        ("GH_TOKEN", gh_token),
    ];
    let auth_out =
        run_bounded_cmd_with_pgid("gh", &["auth", "status"], canon_project_root, &gh_envs);
    if auth_out.timed_out {
        record_pr_audit_event(
            conn,
            project_id,
            tenant_id,
            &req.branch_name,
            "timeout",
            &auth_out.stderr,
            Some(gh_token),
        );
        return Err(map_cmd_failure_response(
            &auth_out,
            "GITHUB_PR_FAILED",
            "gh auth status timed out",
            Some(gh_token),
        ));
    }
    if !auth_out.success {
        record_pr_audit_event(
            conn,
            project_id,
            tenant_id,
            &req.branch_name,
            "failed",
            &auth_out.stderr,
            Some(gh_token),
        );
        return Err(json_err_resp(
            StatusCode::NOT_IMPLEMENTED,
            "GH_UNAUTHENTICATED",
            "GitHub CLI authentication failed or token revoked; download ZIP bundle instead.",
        ));
    }
    Ok((gh_cfg_dir, gh_token.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn check_remote_branch_and_push(
    conn: &mut rusqlite::Connection,
    canon_project_root: &Path,
    project_id: Uuid,
    tenant_id: &str,
    req: &GitHubPRRequest,
    export_entries: &[(String, Vec<u8>)],
    gh_envs: &[(&str, &str)],
    gh_token: &str,
) -> Result<Vec<serde_json::Value>, Response> {
    let branch_name = &req.branch_name;
    let ls_out = run_bounded_cmd_with_pgid(
        "git",
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "ls-remote",
            "--heads",
            "origin",
            branch_name,
        ],
        canon_project_root,
        gh_envs,
    );
    if ls_out.timed_out || !ls_out.success {
        record_pr_audit_event(
            conn,
            project_id,
            tenant_id,
            branch_name,
            "failed",
            &ls_out.stderr,
            Some(gh_token),
        );
        return Err(map_cmd_failure_response(
            &ls_out,
            "GITHUB_PR_FAILED",
            "git ls-remote failed",
            Some(gh_token),
        ));
    }
    let pr_list_out = run_bounded_cmd_with_pgid(
        "gh",
        &[
            "pr",
            "list",
            "--head",
            branch_name,
            "--state",
            "open",
            "--json",
            "number,url",
        ],
        canon_project_root,
        gh_envs,
    );
    if pr_list_out.timed_out || !pr_list_out.success {
        record_pr_audit_event(
            conn,
            project_id,
            tenant_id,
            branch_name,
            "failed",
            &pr_list_out.stderr,
            Some(gh_token),
        );
        return Err(map_cmd_failure_response(
            &pr_list_out,
            "GITHUB_PR_FAILED",
            "gh pr list failed",
            Some(gh_token),
        ));
    }
    let open_prs: Vec<serde_json::Value> =
        serde_json::from_str(pr_list_out.stdout.trim()).unwrap_or_default();
    if !ls_out.stdout.trim().is_empty() {
        let fetch_out = run_bounded_cmd_with_pgid(
            "git",
            &[
                "-c",
                "core.hooksPath=/dev/null",
                "fetch",
                "origin",
                branch_name,
            ],
            canon_project_root,
            gh_envs,
        );
        if fetch_out.timed_out || !fetch_out.success {
            record_pr_audit_event(
                conn,
                project_id,
                tenant_id,
                branch_name,
                "failed",
                &fetch_out.stderr,
                Some(gh_token),
            );
            return Err(map_cmd_failure_response(
                &fetch_out,
                "GITHUB_PR_FAILED",
                "git fetch failed",
                Some(gh_token),
            ));
        }
        let anc_out = run_bounded_cmd_with_pgid(
            "git",
            &[
                "-c",
                "core.hooksPath=/dev/null",
                "merge-base",
                "--is-ancestor",
                "FETCH_HEAD",
                "HEAD",
            ],
            canon_project_root,
            &[],
        );
        if !anc_out.success {
            record_pr_audit_event(
                conn,
                project_id,
                tenant_id,
                branch_name,
                "conflict",
                "GIT_BRANCH_EXISTS",
                Some(gh_token),
            );
            return Err(json_err_resp(
                StatusCode::CONFLICT,
                "GIT_BRANCH_EXISTS",
                &format!(
                    "Remote branch '{branch_name}' already exists with non-fast-forward history"
                ),
            ));
        }
    }
    let chk_out = run_bounded_cmd_with_pgid(
        "git",
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "checkout",
            "-B",
            branch_name,
        ],
        canon_project_root,
        &[],
    );
    if !chk_out.success {
        record_pr_audit_event(
            conn,
            project_id,
            tenant_id,
            branch_name,
            "failed",
            &chk_out.stderr,
            Some(gh_token),
        );
        return Err(map_cmd_failure_response(
            &chk_out,
            "GITHUB_PR_FAILED",
            "git checkout failed",
            Some(gh_token),
        ));
    }
    let reset_out = run_bounded_cmd_with_pgid(
        "git",
        &["-c", "core.hooksPath=/dev/null", "reset", "--quiet"],
        canon_project_root,
        &[],
    );
    if !reset_out.success {
        record_pr_audit_event(
            conn,
            project_id,
            tenant_id,
            branch_name,
            "failed",
            &reset_out.stderr,
            Some(gh_token),
        );
        return Err(map_cmd_failure_response(
            &reset_out,
            "GITHUB_PR_FAILED",
            "git reset failed",
            Some(gh_token),
        ));
    }
    for (rel_path, content_bytes) in export_entries {
        if is_secret_path(rel_path) {
            continue;
        }
        if let Ok(text) = std::str::from_utf8(content_bytes) {
            if let Err(err) = sync_workspace_file_to_disk(canon_project_root, rel_path, false, text)
            {
                return Err(map_error_to_response(err));
            }
        }
    }
    let safe_paths: Vec<&str> = export_entries
        .iter()
        .map(|(p, _)| p.as_str())
        .filter(|p| !is_secret_path(p) && canon_project_root.join(p).is_file())
        .collect();
    if !safe_paths.is_empty() {
        let mut add_args: Vec<&str> = vec!["-c", "core.hooksPath=/dev/null", "add", "--"];
        add_args.extend(safe_paths.iter().copied());
        let add_out = run_bounded_cmd_with_pgid("git", &add_args, canon_project_root, &[]);
        if !add_out.success {
            record_pr_audit_event(
                conn,
                project_id,
                tenant_id,
                branch_name,
                "failed",
                &add_out.stderr,
                Some(gh_token),
            );
            return Err(map_cmd_failure_response(
                &add_out,
                "GITHUB_PR_FAILED",
                "git add failed",
                Some(gh_token),
            ));
        }
    }
    let staged_out = run_bounded_cmd_with_pgid(
        "git",
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "diff",
            "--cached",
            "--name-only",
        ],
        canon_project_root,
        &[],
    );
    for staged_file in staged_out.stdout.lines() {
        if is_secret_path(staged_file.trim()) {
            let _ = run_bounded_cmd_with_pgid(
                "git",
                &["-c", "core.hooksPath=/dev/null", "reset", "--quiet"],
                canon_project_root,
                &[],
            );
            record_pr_audit_event(
                conn,
                project_id,
                tenant_id,
                branch_name,
                "blocked",
                "SECRET_PATH_IN_STAGED_INDEX",
                Some(gh_token),
            );
            return Err(json_err_resp(
                StatusCode::BAD_REQUEST,
                "SECRET_PATH_BLOCKED",
                "Refusing to commit sensitive path detected in git staging area",
            ));
        }
    }
    let diff_cached = run_bounded_cmd_with_pgid(
        "git",
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "diff",
            "--cached",
            "--quiet",
        ],
        canon_project_root,
        &[],
    );
    if !diff_cached.success {
        let commit_msg = format!("feat(design): {}", req.pr_title.trim());
        let commit_out = run_bounded_cmd_with_pgid(
            "git",
            &[
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "user.name=cowok-design",
                "-c",
                "user.email=noreply@cowok.design",
                "commit",
                "-m",
                &commit_msg,
            ],
            canon_project_root,
            &[],
        );
        if !commit_out.success {
            record_pr_audit_event(
                conn,
                project_id,
                tenant_id,
                branch_name,
                "failed",
                &commit_out.stderr,
                Some(gh_token),
            );
            return Err(map_cmd_failure_response(
                &commit_out,
                "GITHUB_PR_FAILED",
                "git commit failed",
                Some(gh_token),
            ));
        }
    }
    let push_out = run_bounded_cmd_with_pgid(
        "git",
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "push",
            "-u",
            "origin",
            branch_name,
        ],
        canon_project_root,
        gh_envs,
    );
    if push_out.timed_out || !push_out.success {
        record_pr_audit_event(
            conn,
            project_id,
            tenant_id,
            branch_name,
            "failed",
            &push_out.stderr,
            Some(gh_token),
        );
        return Err(map_cmd_failure_response(
            &push_out,
            "GITHUB_PR_FAILED",
            "git push failed",
            Some(gh_token),
        ));
    }
    Ok(open_prs)
}

#[allow(clippy::too_many_arguments)]
fn create_or_update_gh_pr(
    conn: &mut rusqlite::Connection,
    canon_project_root: &Path,
    project_id: Uuid,
    tenant_id: &str,
    req: &GitHubPRRequest,
    open_prs: &[serde_json::Value],
    gh_envs: &[(&str, &str)],
    gh_token: &str,
) -> Result<(String, StatusCode, u64, String), Response> {
    if let Some(first_pr) = open_prs.first() {
        let num = first_pr.get("number").and_then(|v| v.as_u64()).unwrap_or(1);
        let num_str = num.to_string();
        let edit_out = run_bounded_cmd_with_pgid(
            "gh",
            &[
                "pr",
                "edit",
                &num_str,
                "--title",
                &req.pr_title,
                "--body",
                &req.pr_body,
            ],
            canon_project_root,
            gh_envs,
        );
        if edit_out.timed_out || !edit_out.success {
            record_pr_audit_event(
                conn,
                project_id,
                tenant_id,
                &req.branch_name,
                "failed",
                &edit_out.stderr,
                Some(gh_token),
            );
            return Err(map_cmd_failure_response(
                &edit_out,
                "GITHUB_PR_FAILED",
                "gh pr edit failed",
                Some(gh_token),
            ));
        }
        let url = first_pr
            .get("url")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(edit_out.stdout.trim())
            .to_string();
        return Ok(("updated".to_string(), StatusCode::OK, num, url));
    }
    let cr_out = run_bounded_cmd_with_pgid(
        "gh",
        &[
            "pr",
            "create",
            "--base",
            &req.target_branch,
            "--head",
            &req.branch_name,
            "--title",
            &req.pr_title,
            "--body",
            &req.pr_body,
        ],
        canon_project_root,
        gh_envs,
    );
    if cr_out.timed_out || !cr_out.success {
        record_pr_audit_event(
            conn,
            project_id,
            tenant_id,
            &req.branch_name,
            "failed",
            &cr_out.stderr,
            Some(gh_token),
        );
        return Err(map_cmd_failure_response(
            &cr_out,
            "GITHUB_PR_FAILED",
            "gh pr create failed",
            Some(gh_token),
        ));
    }
    let url = cr_out
        .stdout
        .lines()
        .last()
        .unwrap_or("")
        .trim()
        .to_string();
    let num = parse_pr_number_from_url(&url).unwrap_or(1);
    Ok(("created".to_string(), StatusCode::CREATED, num, url))
}

fn execute_create_pr_blocking(
    project_registry: &SpecRegistry,
    daemon_workspace_root: &Path,
    canon_project_root: &Path,
    project_id: Uuid,
    tenant_id: &str,
    req: &GitHubPRRequest,
) -> Result<(StatusCode, GitHubPRResponseDto), Response> {
    let mut conn = project_registry
        .pool()
        .connection()
        .map_err(map_error_to_response)?;
    let (gh_cfg_dir, gh_token) = preflight_pr_env_and_token(
        &mut conn,
        daemon_workspace_root,
        canon_project_root,
        project_id,
        tenant_id,
        req,
    )?;
    let gh_cfg_str = gh_cfg_dir.path.to_string_lossy().to_string();
    let gh_envs = [
        ("GH_CONFIG_DIR", gh_cfg_str.as_str()),
        ("GH_TOKEN", gh_token.as_str()),
    ];
    let entries =
        collect_exportable_files(project_registry, canon_project_root, project_id, tenant_id)
            .unwrap_or_default();
    let manifest_bytes = entries
        .iter()
        .find(|(p, _)| p == "cowok-handoff-manifest.json")
        .map(|(_, b)| b.clone())
        .unwrap_or_else(|| {
            build_handoff_manifest_bytes(
                project_id,
                tenant_id,
                req.snapshot_id.as_deref(),
                &chrono::Utc::now().to_rfc3339(),
                &entries,
                None,
            )
        });
    let manifest_sha = format!("{:x}", Sha256::digest(&manifest_bytes));
    let open_prs = check_remote_branch_and_push(
        &mut conn,
        canon_project_root,
        project_id,
        tenant_id,
        req,
        &entries,
        &gh_envs,
        &gh_token,
    )?;
    let (status_str, http_status, pr_number, pr_url) = create_or_update_gh_pr(
        &mut conn,
        canon_project_root,
        project_id,
        tenant_id,
        req,
        &open_prs,
        &gh_envs,
        &gh_token,
    )?;
    let now = chrono::Utc::now().to_rfc3339();
    retry_on_busy_053("persist_github_pr", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO github_pull_requests (
                id, tenant_id, project_id, pr_number, branch_name, title, pr_url,
                status, snapshot_id, handoff_manifest_sha256, visual_diff_summary_json,
                created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'OPEN', ?8, ?9, '{}', ?10, ?10)
             ON CONFLICT(project_id, pr_number) DO UPDATE SET
                branch_name = excluded.branch_name,
                title = excluded.title,
                pr_url = excluded.pr_url,
                snapshot_id = excluded.snapshot_id,
                handoff_manifest_sha256 = excluded.handoff_manifest_sha256,
                updated_at = excluded.updated_at",
            params![
                Uuid::new_v4().to_string(),
                tenant_id,
                project_id.to_string(),
                pr_number as i64,
                req.branch_name,
                req.pr_title,
                pr_url,
                req.snapshot_id,
                manifest_sha,
                now,
            ],
        )?;
        append_execution_event_tx(
            &tx,
            tenant_id,
            project_id,
            "github_pr_attempt",
            &serde_json::json!({ "branch": req.branch_name, "outcome": status_str, "prNumber": pr_number }).to_string(),
            chrono::Utc::now().timestamp_millis(),
        )?;
        tx.commit()
    })?;
    Ok((
        http_status,
        GitHubPRResponseDto {
            pr_url,
            pr_number,
            branch_name: req.branch_name.clone(),
            status: status_str,
            snapshot_id: req.snapshot_id.clone(),
            handoff_manifest_sha256: manifest_sha,
            is_offline_mock: false,
        },
    ))
}

pub(super) async fn handle_create_pr(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(mut req): Json<GitHubPRRequest>,
) -> Result<Response, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    req.branch_name = validate_git_ref(&req.branch_name)?;
    req.target_branch = validate_git_ref(&req.target_branch)?;
    req.snapshot_id = normalize_opt_ref(req.snapshot_id);
    if req.pr_title.trim().is_empty() {
        return Err(json_err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "prTitle must not be empty",
        ));
    }

    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let actor_id = extract_actor_id(&headers);
    let idem_key = extract_idempotency_key(&headers);
    let daemon_ws_root = state.registry.pool().workspace_root().to_path_buf();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let req_sha = format!(
        "{:x}",
        Sha256::digest(serde_json::to_string(&req).unwrap_or_default().as_bytes())
    );

    tokio::task::spawn_blocking(move || {
        let mut conn = project_registry
            .pool()
            .connection()
            .map_err(map_error_to_response)?;
        if let Some(ref key) = idem_key {
            match reserve_idempotency_053(&mut conn, &tenant_id, &actor_id, key, &req_sha, now_ms)?
            {
                IdemReservation053::Replay(st, body_json) => {
                    return Ok(replay_idempotent_json(st, &body_json))
                }
                IdemReservation053::Conflict(msg) => {
                    return Err(json_err_resp(
                        StatusCode::CONFLICT,
                        "IDEMPOTENCY_CONFLICT",
                        &msg,
                    ));
                }
                IdemReservation053::New => {}
            }
        }
        let res = execute_create_pr_blocking(
            &project_registry,
            &daemon_ws_root,
            &canon_project_root,
            project_id,
            &tenant_id,
            &req,
        );
        match res {
            Ok((st, dto)) => {
                if let Some(ref key) = idem_key {
                    if let Ok(json_str) = serde_json::to_string(&dto) {
                        finalize_idempotency_053(
                            &mut conn,
                            &tenant_id,
                            &actor_id,
                            key,
                            st.as_u16(),
                            &json_str,
                        );
                    }
                }
                Ok((st, Json(dto)).into_response())
            }
            Err(resp) => {
                if let Some(ref key) = idem_key {
                    release_idempotency_053(&mut conn, &tenant_id, &actor_id, key);
                }
                Err(resp)
            }
        }
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
}

fn validate_ide_entry_path(canon_project_root: &Path, raw: &str) -> Result<String, Response> {
    let rel = validate_safe_relative_path(raw).map_err(|_| {
        json_err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            &format!("Invalid relativeEntryPath '{raw}'"),
        )
    })?;
    verify_disk_path_not_escaping_jail(canon_project_root, &rel).map_err(|_| {
        json_err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            &format!("relativeEntryPath '{rel}' escapes project jail"),
        )
    })?;
    Ok(rel)
}

pub(super) async fn handle_open_ide_handoff(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<OpenIdeHandoffRequest>,
) -> Result<Json<OpenIdeHandoffResponseDto>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let ide = req.ide.trim().to_ascii_lowercase();
    if !matches!(
        ide.as_str(),
        "antigravity" | "cursor" | "vscode" | "claude-code"
    ) {
        return Err(json_err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "ide must be one of: antigravity, cursor, vscode, claude-code",
        ));
    }
    let rel_entry = match normalize_opt_ref(req.relative_entry_path) {
        Some(raw) => Some(validate_ide_entry_path(&canon_project_root, &raw)?),
        None => None,
    };
    let snapshot_id = normalize_opt_ref(req.snapshot_id);
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());

    let dto = tokio::task::spawn_blocking(move || {
        let conn = project_registry.pool().connection().map_err(map_error_to_response)?;
        if let Some(ref sid) = snapshot_id {
            let ok: bool = conn
                .query_row(
                    "SELECT 1 FROM version_snapshots WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
                    params![sid, project_id.to_string(), tenant_id],
                    |_| Ok(()),
                )
                .optional()
                .map_err(|e| map_error_to_response(map_sqlite_error(e, "check_ide_snapshot_owner")))?
                .is_some();
            if !ok {
                return Err(json_err_resp(
                    StatusCode::NOT_FOUND,
                    "REFERENCE_NOT_FOUND",
                    &format!("VersionSnapshot '{sid}' was not found for this project/tenant"),
                ));
            }
        }
        let entries = collect_exportable_files(&project_registry, &canon_project_root, project_id, &tenant_id)
            .unwrap_or_default();
        let manifest_bytes = build_handoff_manifest_bytes(
            project_id,
            &tenant_id,
            snapshot_id.as_deref(),
            &chrono::Utc::now().to_rfc3339(),
            &entries,
            rel_entry.as_deref(),
        );
        let manifest_content = String::from_utf8_lossy(&manifest_bytes).to_string();
        sync_workspace_file_to_disk(
            &canon_project_root,
            "cowok-handoff-manifest.json",
            false,
            &manifest_content,
        )
        .map_err(map_error_to_response)?;
        let manifest_sha256 = format!("{:x}", Sha256::digest(&manifest_bytes));
        let target_path = match &rel_entry {
            Some(rel) => canon_project_root.join(rel),
            None => canon_project_root.clone(),
        };
        let target_disp = target_path.display().to_string();
        let (launch_uri, cli_fallback_command) = match ide.as_str() {
            "antigravity" => (
                format!("antigravity://file/{target_disp}"),
                format!("antigravity \"{target_disp}\""),
            ),
            "cursor" => (
                format!("cursor://file/{target_disp}"),
                format!("cursor \"{target_disp}\""),
            ),
            "vscode" => (
                format!("vscode://file/{target_disp}"),
                format!("code \"{target_disp}\""),
            ),
            _ => (
                format!("claude-code://open?path={target_disp}"),
                format!("claude \"{}\"", canon_project_root.display()),
            ),
        };
        Ok(OpenIdeHandoffResponseDto {
            ide,
            launch_uri,
            cli_fallback_command,
            manifest_path: "cowok-handoff-manifest.json".to_string(),
            manifest_sha256,
        })
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })??;

    Ok(Json(dto))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forbidden_execution_tokens_detection() {
        assert!(has_forbidden_dynamic_execution_pattern(concat!(
            "ev",
            "al('alert(1)')"
        )));
        assert!(has_forbidden_dynamic_execution_pattern(concat!(
            "ev",
            "al  ('alert(1)')"
        )));
        assert!(has_forbidden_dynamic_execution_pattern(concat!(
            "window.ev",
            "al('x')"
        )));
        assert!(has_forbidden_dynamic_execution_pattern(
            "Function('return 1')()"
        ));
        assert!(has_forbidden_dynamic_execution_pattern(concat!(
            "new Func",
            "tion('return 1')"
        )));
        assert!(has_forbidden_dynamic_execution_pattern(concat!(
            "dangerously",
            "SetInnerHTML"
        )));
        assert!(has_forbidden_dynamic_execution_pattern("__html: 'xss'"));

        // Clean code should not trigger
        assert!(!has_forbidden_dynamic_execution_pattern(
            "export function MyComponent() { return <div>OK</div>; }"
        ));
        assert!(!has_forbidden_dynamic_execution_pattern(
            "const evaluate = 1;"
        ));
    }

    #[test]
    fn test_type_mismatch_detection() {
        assert!(has_type_mismatch("const x: string = 123;"));
        assert!(has_type_mismatch("const b: string = true;"));
        assert!(has_type_mismatch("const n: number = 'hello';"));
        assert!(has_type_mismatch("const flag: boolean = 'true';"));
        assert!(has_type_mismatch("const str: string = getNumericValue();"));
        assert!(has_type_mismatch("TYPE_ERROR_TRIGGER"));

        // Clean types
        assert!(!has_type_mismatch("const x: string = 'valid';"));
        assert!(!has_type_mismatch("const n: number = 42;"));
        assert!(!has_type_mismatch("const b: boolean = true;"));
    }

    #[test]
    fn test_syntax_errors_and_delimiters() {
        let bad_braces = vec![FileDiffItemDto {
            path: "src/Bad.tsx".to_string(),
            status: "added".to_string(),
            additions: 1,
            deletions: 0,
            patch: "+function test() { if (true) { return 1; }".to_string(),
        }];
        assert!(has_syntax_errors(&bad_braces));

        let bad_statement = vec![FileDiffItemDto {
            path: "src/Bad2.tsx".to_string(),
            status: "added".to_string(),
            additions: 1,
            deletions: 0,
            patch: "+const = ;".to_string(),
        }];
        assert!(has_syntax_errors(&bad_statement));

        let good = vec![FileDiffItemDto {
            path: "src/Good.tsx".to_string(),
            status: "added".to_string(),
            additions: 1,
            deletions: 0,
            patch: "+function test() { if (true) { return 1; } }".to_string(),
        }];
        assert!(!has_syntax_errors(&good));
    }

    #[test]
    fn test_missing_relative_imports() {
        let missing = vec![FileDiffItemDto {
            path: "src/Bad.ts".to_string(),
            status: "added".to_string(),
            additions: 1,
            deletions: 0,
            patch: "+import { missing } from './does-not-exist';".to_string(),
        }];
        assert!(has_missing_relative_imports(&missing));

        let clean = vec![FileDiffItemDto {
            path: "src/Good.ts".to_string(),
            status: "added".to_string(),
            additions: 1,
            deletions: 0,
            patch: "+import { Button } from './Button';".to_string(),
        }];
        // Without DB or disk context, ./Button is unresolved.
        assert!(has_missing_relative_imports(&clean));

        // With src/Button.tsx tracked in DB or materialized on disk, ./Button resolves cleanly.
        let mut db_paths = HashSet::new();
        db_paths.insert("src/Button.tsx".to_string());
        assert!(!has_missing_relative_imports_with_context(
            &clean, None, &db_paths
        ));

        let dir = tempfile::tempdir().expect("tempdir");
        let root = std::fs::canonicalize(dir.path()).expect("canonical root");
        sync_workspace_file_to_disk(
            &root,
            "src/Button.tsx",
            false,
            "export const Button = () => null;",
        )
        .expect("seed Button.tsx");
        assert!(!has_missing_relative_imports_with_context(
            &clean,
            Some(&root),
            &HashSet::new()
        ));
    }

    #[test]
    fn test_sync_workspace_file_to_disk_writes_unlinks_and_blocks_symlinks() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = std::fs::canonicalize(dir.path()).expect("canonical root");

        sync_workspace_file_to_disk(
            &root,
            "src/Button.tsx",
            false,
            "export const Button = () => null;\n",
        )
        .expect("write inside workspace");
        assert_eq!(
            std::fs::read_to_string(root.join("src/Button.tsx")).expect("read file"),
            "export const Button = () => null;\n"
        );

        sync_workspace_file_to_disk(&root, "src/Button.tsx", true, "").expect("unlink file");
        assert!(!root.join("src/Button.tsx").exists());

        #[cfg(unix)]
        {
            let outside = tempfile::tempdir().expect("outside");
            std::os::unix::fs::symlink(outside.path(), root.join("escape")).expect("symlink");
            let err = sync_workspace_file_to_disk(&root, "escape/pwn.ts", false, "bad");
            assert!(err.is_err(), "symlink directory escape must be denied");
        }
    }
}
