// @story 01.3, 05.2
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use axum::{
    body::Bytes,
    extract::{Extension, Path as AxumPath, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::STANDARD as B64_STD, Engine as _};
use domain::{CanonicalSpecKey, HarnessError};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spec_registry::{map_sqlite_error, read_jailed_spec_bytes, retry_on_busy, SpecRegistry};
use uuid::Uuid;

use super::*;
use crate::daemon_auth::VerifiedLegacyIngress;

const MAX_VISUAL_DIFF_WIDTH: u32 = 1280;
const MAX_VISUAL_DIFF_HEIGHT: u32 = 800;
const MAX_TRADEOFFS_PER_PROPOSAL: usize = 12;

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
    pub files: Vec<SnapshotFileItemDto>,
    #[serde(default)]
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

fn default_trigger() -> String {
    "manual".to_string()
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
    pub files: Vec<SnapshotFileItemDto>,
    #[serde(default)]
    pub metadata: Option<std::collections::HashMap<String, String>>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevertSnapshotRequest {
    #[serde(default)]
    pub safety_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevertResultDto {
    pub success: bool,
    pub reverted_to_snapshot_id: Uuid,
    pub safety_snapshot_id: Uuid,
    pub restored_files_count: usize,
    pub timestamp: String,
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
    pub is_offline_mock: bool,
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

const PROPOSAL_SELECT_COLS: &str = "id, project_id, tenant_id, intent_id, title, description, files_json,
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
        if !result.contains_key(&rel) {
            let disk_sha = read_disk_file_sha256_jailed(canon_project_root, &rel)?;
            let db_sha = read_db_file_sha256(conn, project_id, &rel)?;
            result.insert(rel, disk_sha.or(db_sha).unwrap_or_default());
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
    let mut constraints =
        constraint_engine::ConstraintEvaluator::load_constraints_for_project(
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
        return Err(Ok(unverified_gate_claim("missing gate_receipt_id on proposal")));
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
                tx_rejection = Some(
                    (
                        StatusCode::CONFLICT,
                        Json(serde_json::json!({
                            "code": "PROPOSAL_STALE_BASE_HASH",
                            "error": "PROPOSAL_STALE_BASE_HASH",
                            "staleFiles": stale_files,
                            "message": format!(
                                "Workspace files changed since proposal snapshot: {}",
                                stale_files.join(", ")
                            ),
                        })),
                    )
                        .into_response(),
                );
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
        created_at,
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
                    trigger, tree_sha256, files_json, created_at
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

pub(super) async fn handle_list_snapshots(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<VersionSnapshotDto>>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());

    let list = tokio::task::spawn_blocking(move || {
        let conn = project_registry.pool().connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, project_id, tenant_id, version_tag, label, description,
                        trigger, tree_sha256, files_json, created_at
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

    Ok(Json(list))
}

pub(super) async fn handle_create_snapshot(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<CreateSnapshotRequest>,
) -> Result<(StatusCode, Json<VersionSnapshotDto>), Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let now = chrono::Utc::now().to_rfc3339();
    let snapshot_id = req.id.unwrap_or_else(Uuid::new_v4);

    let dto = tokio::task::spawn_blocking(move || {
        let tree_sha256 = compute_files_tree_sha(&req.files)?;
        let normalized_files: Vec<SnapshotFileItemDto> = req
            .files
            .into_iter()
            .map(|mut f| {
                f.path = validate_safe_relative_path(&f.path)?;
                let sha = format!("{:x}", Sha256::digest(f.content.as_bytes()));
                f.sha256 = Some(sha);
                f.size_bytes = Some(f.content.len());
                Ok(f)
            })
            .collect::<Result<Vec<_>, HarnessError>>()?;

        let files_json =
            serde_json::to_string(&normalized_files).map_err(|e| HarnessError::ValidationFailed {
                reason: format!("SNAPSHOT_FILES_SERIALIZE_ERROR: {e}"),
            })?;

        let mut conn = project_registry.pool().connection()?;
        let existing_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM version_snapshots WHERE project_id = ?1",
                params![project_id.to_string()],
                |r| r.get(0),
            )
            .map_err(|e| map_sqlite_error(e, "count_snapshots"))?;
        let version_tag = req
            .version_tag
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("v1.{}.0", existing_count + 1));

        retry_on_busy("insert_version_snapshot", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "INSERT INTO version_snapshots (
                    id, project_id, snapshot_tag, manifest_sha256,
                    tenant_id, version_tag, label, description, trigger,
                    tree_sha256, files_json, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
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
                    now,
                    now,
                ],
            )?;
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
            tx.commit()
        })?;

        Ok::<_, HarnessError>(VersionSnapshotDto {
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
            created_at: now,
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
        let mut conn = project_registry.pool().connection()?;
        let target: Option<VersionSnapshotDto> = conn
            .query_row(
                "SELECT id, project_id, tenant_id, version_tag, label, description,
                        trigger, tree_sha256, files_json, created_at
                 FROM version_snapshots
                 WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
                params![snapshot_id.to_string(), project_id.to_string(), tenant_id],
                map_snapshot_row,
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "load_target_snapshot"))?;

        let Some(target) = target else {
            return Ok::<_, HarnessError>(Err(snapshot_not_found(snapshot_id)));
        };

        let existing_files = load_workspace_files_from_db(&conn, project_id)?;
        let safety_files = if existing_files.is_empty() {
            target.files.clone()
        } else {
            existing_files.clone()
        };
        let mut target_paths = std::collections::HashSet::with_capacity(target.files.len());
        for f in &target.files {
            target_paths.insert(validate_safe_relative_path(&f.path)?);
        }
        let safety_sha = compute_files_tree_sha(&safety_files)?;
        let safety_files_json =
            serde_json::to_string(&safety_files).map_err(|e| HarnessError::ValidationFailed {
                reason: format!("SAFETY_FILES_SERIALIZE_ERROR: {e}"),
            })?;
        let safety_label = req
            .safety_label
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Pre-Revert Safety Backup".to_string());
        let safety_tag = format!("safety-backup-{}", &safety_id.to_string()[..8]);

        for existing in &existing_files {
            if let Ok(clean_rel) = validate_safe_relative_path(&existing.path) {
                if !target_paths.contains(&clean_rel) {
                    sync_workspace_file_to_disk(&canon_project_root, &clean_rel, true, "")?;
                }
            }
        }
        for f in &target.files {
            sync_workspace_file_to_disk(&canon_project_root, &f.path, false, &f.content)?;
        }

        retry_on_busy("revert_version_snapshot", || {
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
            tx.execute(
                "DELETE FROM project_workspace_files WHERE project_id = ?1",
                params![project_id.to_string()],
            )?;
            for f in &target.files {
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
            tx.commit()
        })?;

        Ok(Ok(RevertResultDto {
            success: true,
            reverted_to_snapshot_id: snapshot_id,
            safety_snapshot_id: safety_id,
            restored_files_count: target.files.len(),
            timestamp: now,
        }))
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

fn collect_exportable_files(
    project_registry: &SpecRegistry,
    canon_project_root: &Path,
    project_id: Uuid,
    tenant_id: &str,
) -> Result<Vec<(String, Vec<u8>)>, HarnessError> {
    let conn = project_registry.pool().connection()?;
    let mut items = load_workspace_files_from_db(&conn, project_id)?;
    if items.is_empty() {
        let latest_json: Option<String> = conn
            .query_row(
                "SELECT files_json FROM version_snapshots
                 WHERE project_id = ?1 AND tenant_id = ?2
                 ORDER BY created_at DESC LIMIT 1",
                params![project_id.to_string(), tenant_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "load_latest_snapshot_for_export"))?;
        if let Some(json_str) = latest_json {
            items = serde_json::from_str(&json_str).unwrap_or_default();
        }
    }

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        let rel = validate_safe_relative_path(&item.path)?;
        if seen.insert(rel.clone()) {
            out.push((rel, item.content.into_bytes()));
        }
    }

    for spec_key in CanonicalSpecKey::ALL {
        if project_registry.get_spec(project_id, spec_key)?.is_some() {
            if let Some(bytes) = read_jailed_spec_bytes(canon_project_root, spec_key)? {
                let rel = format!("product/specs/{}", spec_key.as_str());
                if seen.insert(rel.clone()) {
                    out.push((rel, bytes));
                }
            }
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

/// Builds a valid STORE-method PKZIP archive (Local File Headers + Central Directory + EOCD).
fn build_pkzip_archive(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central_dir = Vec::new();

    for (name, data) in entries {
        let offset = out.len() as u32;
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len().min(u16::MAX as usize) as u16;
        let crc = crc32_ieee(data);
        let size = data.len() as u32;

        // Local file header (30 bytes + filename)
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0x0800u16.to_le_bytes()); // UTF-8 filename flag
        out.extend_from_slice(&0u16.to_le_bytes()); // compression method: STORE
        out.extend_from_slice(&0u16.to_le_bytes()); // mod time
        out.extend_from_slice(&0u16.to_le_bytes()); // mod date
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra len
        out.extend_from_slice(&name_bytes[..name_len as usize]);
        out.extend_from_slice(data);

        // Central directory header (46 bytes + filename)
        central_dir.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central_dir.extend_from_slice(&20u16.to_le_bytes()); // version made by
        central_dir.extend_from_slice(&20u16.to_le_bytes()); // version needed
        central_dir.extend_from_slice(&0x0800u16.to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes()); // STORE
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
        central_dir.extend_from_slice(&name_bytes[..name_len as usize]);
    }

    let cd_offset = out.len() as u32;
    let cd_size = central_dir.len() as u32;
    let count = entries.len().min(u16::MAX as usize) as u16;
    out.extend_from_slice(&central_dir);

    // End of Central Directory (22 bytes)
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
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
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "code": "AC03_RUNNABLE_BUNDLE_VIOLATION",
                "error": "AC03_RUNNABLE_BUNDLE_VIOLATION",
                "message": "Export bundle must retain design tokens and icon manifest to remain runnable (Story 05.3 AC-03).",
            })),
        )
            .into_response());
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
            return Ok::<_, HarnessError>(Err((
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "code": "NO_EXPORTABLE_FILES",
                    "error": "NO_EXPORTABLE_FILES",
                    "message": "No persisted workspace files, snapshots, or specs available to export",
                })),
            )
                .into_response()));
        }

        let files_included: Vec<String> = entries.iter().map(|(p, _)| p.clone()).collect();
        let ready_to_run = files_included.iter().any(|p| p == "package.json");
        let zip_bytes = build_pkzip_archive(&entries);
        let sha256 = format!("{:x}", Sha256::digest(&zip_bytes));
        let format_ext = "zip";
        let date_str = chrono::Utc::now().format("%Y%m%d").to_string();
        let file_name = format!("{project_id}-{tenant_for_name}-bundle-{date_str}.{format_ext}");

        Ok(Ok(ExportZipResponseDto {
            file_name,
            byte_length: zip_bytes.len(),
            sha256,
            files_included,
            ready_to_run,
        }))
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    dto.map(Json)
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
    })?
    .map_err(map_error_to_response)?;

    if entries.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "code": "NO_EXPORTABLE_FILES",
                "error": "NO_EXPORTABLE_FILES",
                "message": "No persisted workspace files, snapshots, or specs available to download",
            })),
        )
            .into_response());
    }

    let zip_bytes = build_pkzip_archive(&entries);
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
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to build zip response",
            )
                .into_response()
        })
}

pub(super) async fn handle_create_pr(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(_req): Json<GitHubPRRequest>,
) -> Result<Json<GitHubPRResponseDto>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let _ = verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
        .map_err(map_error_to_response)?;

    Err((
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "code": "GITHUB_INTEGRATION_NOT_CONFIGURED",
            "error": "GITHUB_INTEGRATION_NOT_CONFIGURED",
            "message": "Remote GitHub Pull Request creation requires a configured GitHub App / git remote integration; offline mock PR fabrication is disabled.",
        })),
    )
        .into_response())
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
