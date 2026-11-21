// @story 02.1
// @implements AC-01, AC-02, AC-03, AC-04
use crate::outline::{compress_dom_outline, enforce_ssot_dom_ratio, MAX_OUTLINE_BPE_TOKENS};
use crate::portal_guard::{
    map_horizon, normalize_intent_nfc, scan_text_for_portal, validate_all_portal_inputs,
    validate_portal_isolation,
};
use crate::profile_adapter::{adapt_for_profile, AdaptedProfileResult};
use crate::telemetry::{record_idempotent_or_replay, verify_project_tenant_for_pool};
use crate::types::{
    BlockFormat, CompiledContextBundle, ContextAssembleDataDto, ContextAssemblyCommand,
    ContextCompilerError, ContextReasonCode, Horizon, InspectPromptPayload, ModelProfile,
    OptionalContextBlock, PortalId, SourcesSummaryDto, TokenUsageDto,
};
use chrono::Utc;
use domain::{
    estimate_tokens, truncate_utf8_to_tokens, CanonicalSpecKey, CompilationInput,
    DeterministicPromptCompiler, Portal, TaskHorizonRouter, VisibilityPolicy, TOKEN_BUDGET_CEILING,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use spec_registry::{
    compute_sha256_hex, read_jailed_spec_bytes, ContextSlicer, SpecRegistry, SqlitePool,
    SqliteWalConfig,
};
use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;
use uuid::Uuid;

pub const MAX_TEXT_BPE_TOKENS: usize = TOKEN_BUDGET_CEILING;
pub const DEFAULT_MAX_IMAGE_TOKENS_PER_TURN: usize = 1024;
pub const MAX_IMAGE_TOKENS_CEILING: usize = 10_000;
const MAX_NON_SSOT_INTENT_TOKENS: usize = 420;
const SPEC_SNIPPET_MAX_TOKENS: usize = 72;
const TAIL_HEADER_TOKENS: usize = 32;

const MIN_EXCERPT_FALLBACK_TOKENS: usize = 8;
const TRUNCATION_NOTICE_MARKER: &str = "<!-- [TRUNCATED_BY_TOKEN_GOVERNOR] -->";

const HTML5_VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

fn is_valid_json_block(raw: &str) -> bool {
    serde_json::from_str::<Value>(raw.trim()).is_ok()
}

fn extract_html_tag_name(tag_inner: &str) -> String {
    tag_inner
        .trim_start_matches('/')
        .split(|c: char| c.is_whitespace() || c == '/')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn is_html_void_tag(tag_inner: &str) -> bool {
    let lower = extract_html_tag_name(tag_inner);
    HTML5_VOID_TAGS.iter().any(|v| *v == lower)
}

fn is_html_tag_byte(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'/' || b == b'!'
}

fn is_html_tag_open_at(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'<' || !is_html_tag_byte(bytes[1]) {
        return false;
    }
    if let Some(end) = s.find('>') {
        let inner = &s[1..end];
        let is_autolink = !inner.starts_with('!')
            && !inner.contains(char::is_whitespace)
            && (inner.contains("://") || inner.starts_with("mailto:") || inner.contains('@'));
        if is_autolink {
            return false;
        }
    }
    true
}

fn has_balanced_html_tags(raw: &str) -> bool {
    let mut stack: Vec<String> = Vec::new();
    let mut idx = 0usize;
    let bytes = raw.as_bytes();
    while idx < bytes.len() {
        if bytes[idx] == b'<' && is_html_tag_open_at(&raw[idx..]) {
            let Some(end_rel) = raw[idx..].find('>') else {
                return false;
            };
            let tag_inner = raw[idx + 1..idx + end_rel].trim();
            let name = extract_html_tag_name(tag_inner);
            if name.is_empty() && !tag_inner.starts_with('!') {
                return false;
            }
            if tag_inner.starts_with('/') {
                if is_html_void_tag(tag_inner) || stack.pop().as_deref() != Some(name.as_str()) {
                    return false;
                }
            } else if !tag_inner.ends_with('/')
                && !tag_inner.starts_with('!')
                && !is_html_void_tag(tag_inner)
            {
                stack.push(name);
            }
            idx += end_rel + 1;
        } else {
            idx += 1;
        }
    }
    stack.is_empty()
}

fn is_valid_html_block(raw: &str) -> bool {
    let trimmed = raw.trim();
    is_html_tag_open_at(trimmed) && trimmed.ends_with('>') && has_balanced_html_tags(trimmed)
}

fn contains_inline_structured_syntax(s: &str) -> bool {
    if s.contains('{') || s.contains('}') {
        return true;
    }
    let bytes = s.as_bytes();
    (0..bytes.len()).any(|i| bytes[i] == b'<' && is_html_tag_open_at(&s[i..]))
}

fn looks_like_json_block_start(trimmed: &str) -> bool {
    if trimmed.starts_with('{') {
        return true;
    }
    let first = trimmed.lines().next().unwrap_or("").trim();
    if !first.starts_with('[') || first.contains("](") || first.contains("]:") {
        return false;
    }
    let rest = first[1..].trim_start();
    if rest.is_empty() {
        return true;
    }
    if let Some(after_bracket) = rest.strip_prefix(']') {
        return after_bracket.trim().is_empty();
    }
    if let Some(close) = first.find(']') {
        if !first[close + 1..].trim().is_empty() {
            return false;
        }
    }
    if rest.starts_with('{')
        || rest.starts_with('[')
        || rest.starts_with('"')
        || rest.starts_with('-')
        || rest.as_bytes().first().is_some_and(u8::is_ascii_digit)
    {
        return true;
    }
    ["true", "false", "null"].iter().any(|kw| {
        rest.starts_with(kw)
            && rest[kw.len()..]
                .chars()
                .next()
                .is_none_or(|c| c.is_whitespace() || matches!(c, ',' | ']'))
    })
}

fn invalid_spec_structure_err(msg: &str, field: Option<&str>) -> ContextCompilerError {
    ContextCompilerError::InvalidRequest {
        message: msg.to_string(),
        field: field.map(str::to_string),
    }
}

fn check_whole_atomic_budget(
    trimmed: &str,
    kind: &str,
    max_tokens: usize,
) -> Result<String, ContextCompilerError> {
    let tok = estimate_tokens(trimmed);
    if tok <= max_tokens {
        Ok(trimmed.to_string())
    } else {
        Err(ContextCompilerError::AtomicBlockOverBudget {
            reason: format!("atomic {kind} spec block ({tok} tok) exceeds {max_tokens}"),
        })
    }
}

fn validate_and_fit_atomic_spec(
    trimmed: &str,
    max_tokens: usize,
    _field: Option<&str>,
) -> Option<Result<String, ContextCompilerError>> {
    if looks_like_json_block_start(trimmed) && is_valid_json_block(trimmed) {
        return Some(check_whole_atomic_budget(trimmed, "JSON", max_tokens));
    }
    if is_html_tag_open_at(trimmed) && is_valid_html_block(trimmed) {
        return Some(check_whole_atomic_budget(trimmed, "HTML", max_tokens));
    }
    None
}

fn collect_fenced_unit(
    lines: &[&str],
    start: usize,
    field: Option<&str>,
) -> Result<(String, usize), ContextCompilerError> {
    let lang = lines[start]
        .trim()
        .trim_start_matches('`')
        .trim()
        .to_ascii_lowercase();
    let Some(end) = ((start + 1)..lines.len()).find(|&idx| {
        let t = lines[idx].trim();
        t.starts_with("```") && t.trim_start_matches('`').trim().is_empty()
    }) else {
        return Err(invalid_spec_structure_err(
            "spec contains unclosed fenced code block",
            field,
        ));
    };
    let inner = lines[(start + 1)..end].join("\n");
    if lang == "json" && !is_valid_json_block(&inner) {
        return Err(invalid_spec_structure_err(
            "spec contains malformed JSON fenced block",
            field,
        ));
    }
    if lang == "html" && !is_valid_html_block(&inner) {
        return Err(invalid_spec_structure_err(
            "spec contains malformed HTML fenced block",
            field,
        ));
    }
    Ok((lines[start..=end].join("\n"), end + 1))
}

fn collect_structured_block_unit(
    lines: &[&str],
    start: usize,
    is_json: bool,
    field: Option<&str>,
) -> Result<(String, usize), ContextCompilerError> {
    let mut block = String::new();
    let mut end = start;
    while end < lines.len() {
        let t = lines[end].trim();
        if t.is_empty() || (end > start && (t.starts_with('#') || t.starts_with("```"))) {
            break;
        }
        if !block.is_empty() {
            block.push('\n');
        }
        block.push_str(t);
        end += 1;
        let complete = if is_json {
            is_valid_json_block(&block)
        } else {
            is_valid_html_block(&block)
        };
        if complete {
            return Ok((block, end));
        }
    }
    let kind = if is_json { "JSON" } else { "HTML" };
    Err(invalid_spec_structure_err(
        &format!("spec contains malformed {kind} block"),
        field,
    ))
}

fn parse_markdown_units(
    content: &str,
    field: Option<&str>,
) -> Result<Vec<(String, bool)>, ContextCompilerError> {
    let lines: Vec<&str> = content.lines().collect();
    let mut units = Vec::new();
    let mut idx = 0usize;
    while idx < lines.len() {
        let trimmed = lines[idx].trim();
        if trimmed.is_empty() {
            idx += 1;
        } else if trimmed.starts_with("```") {
            let (block, next) = collect_fenced_unit(&lines, idx, field)?;
            units.push((block, true));
            idx = next;
        } else if looks_like_json_block_start(trimmed) {
            let (block, next) = collect_structured_block_unit(&lines, idx, true, field)?;
            units.push((block, true));
            idx = next;
        } else if is_html_tag_open_at(trimmed) {
            if has_balanced_html_tags(trimmed) {
                units.push((trimmed.to_string(), true));
                idx += 1;
            } else {
                let (block, next) = collect_structured_block_unit(&lines, idx, false, field)?;
                units.push((block, true));
                idx = next;
            }
        } else {
            let atomic = contains_inline_structured_syntax(trimmed);
            units.push((trimmed.to_string(), atomic));
            idx += 1;
        }
    }
    Ok(units)
}

fn append_unit_to_excerpt(
    out: &mut String,
    unit: &str,
    is_atomic: bool,
    max_tokens: usize,
) -> bool {
    let sep = if out.is_empty() { "" } else { " " };
    let candidate = format!("{out}{sep}{unit}");
    if estimate_tokens(&candidate) <= max_tokens {
        *out = candidate;
        return true;
    }
    if is_atomic {
        return true;
    }
    let current_tok = if out.is_empty() {
        0
    } else {
        estimate_tokens(out) + 1
    };
    let remaining = max_tokens.saturating_sub(current_tok);
    if out.is_empty() || remaining >= MIN_EXCERPT_FALLBACK_TOKENS {
        let budget = remaining.saturating_sub(usize::from(!out.is_empty()));
        let clipped = truncate_utf8_to_tokens(unit, budget, "...");
        let fallback = format!("{out}{sep}{clipped}");
        if !clipped.is_empty() && estimate_tokens(&fallback) <= max_tokens {
            *out = fallback;
        }
    }
    false
}

fn format_structured_spec_excerpt(
    raw: &str,
    max_tokens: usize,
    field: Option<&str>,
) -> Result<String, ContextCompilerError> {
    let stripped = raw.trim().trim_end_matches(TRUNCATION_NOTICE_MARKER).trim();
    if stripped.is_empty() {
        return Err(invalid_spec_structure_err("spec content is empty", field));
    }
    if let Some(res) = validate_and_fit_atomic_spec(stripped, max_tokens, field) {
        return res;
    }
    let units = parse_markdown_units(stripped, field)?;
    let mut out = String::new();
    for (unit, is_atomic) in units {
        if !append_unit_to_excerpt(&mut out, &unit, is_atomic, max_tokens) {
            break;
        }
    }
    if out.is_empty() {
        return Err(ContextCompilerError::AtomicBlockOverBudget {
            reason: format!("spec atomic blocks exceed {max_tokens} token excerpt budget"),
        });
    }
    Ok(out)
}

fn build_static_prefix(cmd: &ContextAssemblyCommand, constraints: &[String]) -> (String, String) {
    let brand = if cmd.brand_tokens_prefix.trim().is_empty() {
        "brand_tokens: warm_paper,geist_sans,geist_mono,semantic_tokens_v4,radius_md_12,elevation_subtle"
    } else {
        cmd.brand_tokens_prefix.trim()
    };
    let skill = if cmd.active_skill_prefix.trim().is_empty() {
        "skill_prefix: cowok-design-opt-core-v1,clean-css-vars,zero-tailwind"
    } else {
        cmd.active_skill_prefix.trim()
    };
    let mut sorted = constraints.to_vec();
    sorted.sort();
    sorted.dedup();
    let cons_block = if sorted.is_empty() {
        "constraints: zero_tailwind,strict_portal_boundary,atomic_ssot_sync,bpe_cap_1500"
            .to_string()
    } else {
        format!("constraints: {}", sorted.join(";"))
    };
    let prefix = format!(
        "# STATIC_PREFIX_V1\nportal_policy: {}\n{}\n{}\n{}",
        cmd.portal_id.as_str(),
        brand,
        cons_block,
        skill
    );
    let digest = format!("{:x}", Sha256::digest(prefix.as_bytes()));
    (prefix, digest)
}

fn validate_block_syntax(block: &OptionalContextBlock) -> Result<(), ContextCompilerError> {
    let valid = match block.format {
        BlockFormat::Json => is_valid_json_block(&block.content),
        BlockFormat::Html => is_valid_html_block(&block.content),
        BlockFormat::Text => true,
    };
    if !valid {
        return Err(ContextCompilerError::InvalidRequest {
            message: format!("malformed {:?} block '{}'", block.format, block.id),
            field: Some("optionalBlocks".to_string()),
        });
    }
    Ok(())
}

fn prune_blocks_atomically(
    budget: usize,
    blocks: &[OptionalContextBlock],
) -> Result<(Vec<String>, usize, bool), ContextCompilerError> {
    let mut sorted = blocks.to_vec();
    sorted.sort_by(|a, b| a.priority.cmp(&b.priority).then_with(|| a.id.cmp(&b.id)));
    let mut kept = Vec::new();
    let mut joined = String::new();
    let mut used = 0usize;
    let mut pruned = false;
    for block in &sorted {
        validate_block_syntax(block)?;
        let line = format!("[block:{}] {}", block.id, block.content.trim());
        let candidate = if joined.is_empty() {
            line.clone()
        } else {
            format!("{joined} | {line}")
        };
        let candidate_tok = estimate_tokens(&candidate);
        if candidate_tok <= budget {
            used = candidate_tok;
            joined = candidate;
            kept.push(line);
        } else if block.priority == 0
            && matches!(block.format, BlockFormat::Html | BlockFormat::Json)
        {
            return Err(ContextCompilerError::AtomicBlockOverBudget {
                reason: format!(
                    "atomic {:?} block '{}' ({candidate_tok} tok) exceeds {budget}",
                    block.format, block.id
                ),
            });
        } else {
            pruned = true;
        }
    }
    Ok((kept, used, pruned))
}

fn map_sqlite_error(err: rusqlite::Error) -> ContextCompilerError {
    let msg = err.to_string();
    let lower = msg.to_ascii_lowercase();
    if lower.contains("busy") || lower.contains("locked") {
        ContextCompilerError::TelemetryWriteBusy
    } else {
        ContextCompilerError::TelemetryStorageUnavailable { reason: msg }
    }
}

fn lookup_source_spec_row(
    conn: &rusqlite::Connection,
    project_id: Uuid,
    sid: Uuid,
) -> Result<(CanonicalSpecKey, String, String), ContextCompilerError> {
    let (sk, status, sha): (String, String, String) = conn
        .query_row(
            "SELECT spec_key, status, content_sha256 FROM project_specs WHERE id = ?1 AND project_id = ?2",
            rusqlite::params![sid.to_string(), project_id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => ContextCompilerError::InvalidRequest {
                message: format!("unknown sourceId '{sid}' for project"),
                field: Some("sourceIds".to_string()),
            },
            other => map_sqlite_error(other),
        })?;
    let key = CanonicalSpecKey::parse(&sk).map_err(|_| ContextCompilerError::InvalidRequest {
        message: format!("invalid spec_key '{sk}' for sourceId '{sid}'"),
        field: Some("sourceIds".to_string()),
    })?;
    Ok((key, status, sha))
}

fn read_verified_source_spec_excerpt(
    workspace_root: &Path,
    portal_id: PortalId,
    domain_portal: Portal,
    sid: Uuid,
    spec_key: CanonicalSpecKey,
    expected_sha256: &str,
) -> Result<String, ContextCompilerError> {
    let bytes = read_jailed_spec_bytes(workspace_root, spec_key)
        .map_err(map_harness_error)?
        .ok_or_else(|| ContextCompilerError::TelemetryStorageUnavailable {
            reason: format!(
                "spec file missing on disk for sourceId '{sid}' ({})",
                spec_key.as_str()
            ),
        })?;
    let disk_sha = compute_sha256_hex(&bytes);
    if disk_sha != expected_sha256 {
        return Err(ContextCompilerError::TelemetryStorageUnavailable {
            reason: format!(
                "spec digest mismatch on disk for sourceId '{sid}' ({})",
                spec_key.as_str()
            ),
        });
    }
    let raw_content = String::from_utf8_lossy(&bytes);
    scan_text_for_portal(portal_id, "source_spec", &raw_content)?;
    let safe_content = VisibilityPolicy::apply_terminal_redaction(&raw_content, domain_portal)
        .map_err(map_harness_error)?;
    let excerpt =
        format_structured_spec_excerpt(&safe_content, SPEC_SNIPPET_MAX_TOKENS, Some("sourceIds"))?;
    Ok(format!("[ssot:{}@{sid}] :: {excerpt}", spec_key.as_str()))
}

fn validate_requested_sources_policy(
    registry: Option<&SpecRegistry>,
    cmd: &ContextAssemblyCommand,
    domain_portal: Portal,
    allowed_keys: &[CanonicalSpecKey],
    denied_keys: &[CanonicalSpecKey],
) -> Result<(Vec<String>, usize), ContextCompilerError> {
    let Some(ids) = &cmd.source_ids else {
        return Ok((Vec::new(), 0));
    };
    if ids.is_empty() {
        return Err(ContextCompilerError::InvalidRequest {
            message: "sourceIds must not be empty".to_string(),
            field: Some("sourceIds".to_string()),
        });
    }
    let Some(reg) = registry else {
        return Err(ContextCompilerError::InvalidRequest {
            message: "sourceIds requires an active SpecRegistry".to_string(),
            field: Some("sourceIds".to_string()),
        });
    };
    let workspace_root = reg
        .verify_project_scope(cmd.project_id)
        .map_err(map_harness_error)?;
    let allowed_set: HashSet<CanonicalSpecKey> = allowed_keys.iter().copied().collect();
    let denied_set: HashSet<CanonicalSpecKey> = denied_keys.iter().copied().collect();
    let mut seen_keys = HashSet::new();
    let conn = reg.pool().connection().map_err(map_harness_error)?;
    let mut resolved = Vec::with_capacity(ids.len());
    for sid in ids {
        let (key, status, expected_sha) = lookup_source_spec_row(&conn, cmd.project_id, *sid)?;
        if cmd.portal_id != PortalId::SuperAdmin
            && (denied_set.contains(&key) || !allowed_set.contains(&key))
        {
            return Err(ContextCompilerError::CrossPortalLeakBlocked {
                reason: format!("sourceId '{sid}' points to denied spec '{}'", key.as_str()),
            });
        }
        if status == "FILE_DELETED" || expected_sha.trim().is_empty() {
            return Err(ContextCompilerError::InvalidRequest {
                message: format!("sourceId '{sid}' has no committed spec content"),
                field: Some("sourceIds".to_string()),
            });
        }
        if seen_keys.insert(key) {
            resolved.push(read_verified_source_spec_excerpt(
                &workspace_root,
                cmd.portal_id,
                domain_portal,
                *sid,
                key,
                &expected_sha,
            )?);
        }
    }
    Ok((resolved, seen_keys.len()))
}

fn map_harness_error(err: domain::HarnessError) -> ContextCompilerError {
    match err {
        domain::HarnessError::ProjectNotFound { .. } => {
            ContextCompilerError::TelemetryProjectReferenceInvalid
        }
        domain::HarnessError::BoundaryViolation { .. } => {
            ContextCompilerError::CrossPortalLeakBlocked {
                reason: err.to_string(),
            }
        }
        domain::HarnessError::SqliteBusy { .. } => ContextCompilerError::TelemetryWriteBusy,
        other => {
            let msg = other.to_string();
            if msg.to_ascii_lowercase().contains("busy")
                || msg.to_ascii_lowercase().contains("locked")
            {
                ContextCompilerError::TelemetryWriteBusy
            } else {
                ContextCompilerError::TelemetryStorageUnavailable { reason: msg }
            }
        }
    }
}

fn resolve_untruncated_slice_content(
    workspace_root: &Path,
    portal: Portal,
    sl: &domain::SpecSlice,
) -> Result<String, ContextCompilerError> {
    if !sl.content.contains(TRUNCATION_NOTICE_MARKER) {
        return Ok(sl.content.clone());
    }
    let bytes = read_jailed_spec_bytes(workspace_root, sl.spec_key)
        .map_err(map_harness_error)?
        .ok_or_else(|| ContextCompilerError::TelemetryStorageUnavailable {
            reason: format!("spec file missing on disk ({})", sl.spec_key.as_str()),
        })?;
    let raw = String::from_utf8_lossy(&bytes);
    VisibilityPolicy::apply_terminal_redaction(&raw, portal).map_err(map_harness_error)
}

fn collect_spec_lines(
    registry: Option<&SpecRegistry>,
    cmd: &ContextAssemblyCommand,
    intent: &str,
) -> Result<(Vec<String>, usize), ContextCompilerError> {
    let (portal, dept) = validate_portal_isolation(cmd.portal_id, intent, &cmd.extra_constraints)?;
    let horizon = map_horizon(cmd.horizon);
    let (allowed, denied_pairs) = VisibilityPolicy::evaluate(dept, portal);
    let denied: Vec<CanonicalSpecKey> = denied_pairs.into_iter().map(|(k, _)| k).collect();
    let _classified =
        TaskHorizonRouter::classify(intent, Some(horizon), cmd.dom_outline_nodes.len());
    let (mut lines, explicit_count) =
        validate_requested_sources_policy(registry, cmd, portal, &allowed, &denied)?;
    if let Some(reg) = registry {
        let req = domain::ContextSliceRequest {
            task_input: intent.to_string(),
            override_horizon: Some(horizon),
            department: dept,
            portal,
            affected_artifacts: Vec::new(),
        };
        let sliced = ContextSlicer::new(reg)
            .slice_context(cmd.project_id, &req)
            .map_err(map_harness_error)?;
        if cmd.source_ids.is_none() {
            let workspace_root = reg
                .verify_project_scope(cmd.project_id)
                .map_err(map_harness_error)?;
            for sl in sliced.slices {
                let full_content = resolve_untruncated_slice_content(&workspace_root, portal, &sl)?;
                scan_text_for_portal(cmd.portal_id, "spec_slice", &full_content)?;
                let excerpt =
                    format_structured_spec_excerpt(&full_content, SPEC_SNIPPET_MAX_TOKENS, None)?;
                lines.push(format!(
                    "[spec:{}] {} :: {excerpt}",
                    sl.spec_key.as_str(),
                    sl.title.trim()
                ));
            }
        }
    }
    Ok((lines, explicit_count))
}

fn fit_ssot_lines_to_budget(ssot_lines: &[String], budget: usize) -> (Vec<String>, usize, usize) {
    let mut kept = Vec::new();
    let mut joined = String::new();
    let mut tok_sum = 0usize;
    for line in ssot_lines {
        let candidate = if joined.is_empty() {
            line.clone()
        } else {
            format!("{joined} | {line}")
        };
        let candidate_tok = estimate_tokens(&candidate);
        if candidate_tok <= budget {
            tok_sum = candidate_tok;
            joined = candidate;
            kept.push(line.clone());
        } else {
            break;
        }
    }
    let dropped = ssot_lines.len().saturating_sub(kept.len());
    (kept, tok_sum, dropped)
}

fn build_tail_string(
    cmd: &ContextAssemblyCommand,
    adapted: &AdaptedProfileResult,
    intent_snippet: &str,
    kept_ssot: &[String],
    kept_blocks: &[String],
) -> String {
    format!(
        "# DYNAMIC_TAIL\nhorizon: {}\nprofile: {}\nmode: {}\nintent: {}\nssot_specs: {}\nblocks: {}\noffline: {}",
        cmd.horizon.as_str(),
        cmd.model_profile.as_str(),
        adapted.adapter_mode.as_str(),
        intent_snippet,
        kept_ssot.join(" | "),
        kept_blocks.join(" | "),
        adapted.offline_snippets.join(",")
    )
}

fn validate_command_preconditions(
    cmd: &ContextAssemblyCommand,
) -> Result<String, ContextCompilerError> {
    if cmd.requested_image_tokens > cmd.max_image_tokens_per_turn
        || cmd.requested_image_tokens > MAX_IMAGE_TOKENS_CEILING
    {
        return Err(ContextCompilerError::ImageTokenBudgetExceeded {
            requested: cmd.requested_image_tokens,
            cap: cmd.max_image_tokens_per_turn,
        });
    }
    validate_all_portal_inputs(cmd)?;
    normalize_intent_nfc(&cmd.intent)
}

fn trim_intent_and_ssot(
    static_tok: usize,
    outline_tok: usize,
    offline_tok: usize,
    nfc_intent: &str,
    all_ssot_lines: &[String],
) -> (String, bool, Vec<String>, usize, usize, usize) {
    let tail_budget = MAX_TEXT_BPE_TOKENS.saturating_sub(static_tok + outline_tok);
    let remaining_tail = tail_budget.saturating_sub(TAIL_HEADER_TOKENS + offline_tok);
    let intent_cap = remaining_tail.min(MAX_NON_SSOT_INTENT_TOKENS);
    let intent_tok_raw = estimate_tokens(nfc_intent);
    let (intent_snippet, intent_pruned) = if intent_tok_raw > intent_cap {
        (truncate_utf8_to_tokens(nfc_intent, intent_cap, "..."), true)
    } else {
        (nfc_intent.to_string(), false)
    };
    let ssot_reserve = remaining_tail.saturating_sub(estimate_tokens(&intent_snippet));
    let (kept_ssot, ssot_tok, dropped_ssot) =
        fit_ssot_lines_to_budget(all_ssot_lines, ssot_reserve);
    let opt_budget = ssot_reserve.saturating_sub(ssot_tok + 2);
    (
        intent_snippet,
        intent_pruned,
        kept_ssot,
        ssot_tok,
        dropped_ssot,
        opt_budget,
    )
}

struct BudgetedAssembly {
    outline: String,
    dynamic_tail: String,
    outline_tok: usize,
    dynamic_tok: usize,
    ssot_tok: usize,
    pruned: bool,
    req_cnt: usize,
    inc_cnt: usize,
    adapted: AdaptedProfileResult,
}

fn verify_domain_compiler_guard(
    cmd: &ContextAssemblyCommand,
    static_prefix: &str,
    nfc_intent: &str,
    adapted: &AdaptedProfileResult,
) -> Result<(), ContextCompilerError> {
    DeterministicPromptCompiler::new(MAX_TEXT_BPE_TOKENS)
        .compile(CompilationInput {
            project_id: cmd.project_id.to_string(),
            system_instruction: static_prefix.to_string(),
            project_constraints: Vec::new(),
            context_slice: None,
            task_instructions: truncate_utf8_to_tokens(
                nfc_intent,
                MAX_NON_SSOT_INTENT_TOKENS,
                "...",
            ),
            optional_components: adapted.adapted_tools.clone(),
        })
        .map(|_| ())
        .map_err(|err| ContextCompilerError::AtomicBlockOverBudget {
            reason: err.to_string(),
        })
}

fn compact_tail_with_mandatory_blocks(
    cmd: &ContextAssemblyCommand,
    adapted: &AdaptedProfileResult,
    offline_tok: usize,
    intent_snippet: &str,
    all_ssot_lines: &[String],
    tail_budget: usize,
) -> Result<(String, Vec<String>, usize), ContextCompilerError> {
    let compact_intent_cap = (tail_budget / 12).clamp(16, 48);
    let short_intent = truncate_utf8_to_tokens(intent_snippet, compact_intent_cap, "...");
    let mandatory_blocks: Vec<OptionalContextBlock> = cmd
        .optional_blocks
        .iter()
        .filter(|b| b.priority == 0 && matches!(b.format, BlockFormat::Html | BlockFormat::Json))
        .cloned()
        .collect();
    let non_ssot_tail_tok = TAIL_HEADER_TOKENS + offline_tok + estimate_tokens(&short_intent);
    let mand_budget = tail_budget.saturating_sub(non_ssot_tail_tok);
    let (kept_mandatory, mand_tok, _) = prune_blocks_atomically(mand_budget, &mandatory_blocks)?;
    let ssot_rem_budget = tail_budget.saturating_sub(non_ssot_tail_tok + mand_tok + 2);
    let (refit_ssot, ssot_tok, _) = fit_ssot_lines_to_budget(all_ssot_lines, ssot_rem_budget);
    let tail = build_tail_string(cmd, adapted, &short_intent, &refit_ssot, &kept_mandatory);
    Ok((tail, refit_ssot, ssot_tok))
}

fn assemble_budgeted_payload(
    registry: Option<&SpecRegistry>,
    cmd: &ContextAssemblyCommand,
    nfc_intent: &str,
    static_prefix: &str,
) -> Result<BudgetedAssembly, ContextCompilerError> {
    let static_tok = estimate_tokens(static_prefix);
    if static_tok > MAX_TEXT_BPE_TOKENS {
        return Err(ContextCompilerError::AtomicBlockOverBudget {
            reason: format!("static prefix ({static_tok} tok) exceeds {MAX_TEXT_BPE_TOKENS}"),
        });
    }
    let (all_ssot_lines, explicit_src_req) = collect_spec_lines(registry, cmd, nfc_intent)?;
    let (outline, outline_tok, _) =
        compress_dom_outline(&cmd.dom_outline_nodes, MAX_OUTLINE_BPE_TOKENS);
    let conn_opt = match registry {
        Some(r) => Some(r.pool().connection().map_err(map_harness_error)?),
        None => None,
    };
    let ws_opt = registry.map(|r| r.pool().workspace_root());
    let adapted = adapt_for_profile(
        conn_opt.as_ref(),
        ws_opt,
        cmd.project_id,
        cmd.portal_id,
        cmd.model_profile,
        static_prefix,
        nfc_intent,
    )?;
    verify_domain_compiler_guard(cmd, static_prefix, nfc_intent, &adapted)?;

    let offline_tok = estimate_tokens(&adapted.offline_snippets.join(","));
    let (intent_snippet, intent_pruned, mut kept_ssot, mut ssot_tok, dropped_ssot, opt_budget) =
        trim_intent_and_ssot(
            static_tok,
            outline_tok,
            offline_tok,
            nfc_intent,
            &all_ssot_lines,
        );
    let tail_budget = MAX_TEXT_BPE_TOKENS.saturating_sub(static_tok + outline_tok);
    let (mut dynamic_tail, mut pruned, force_compact) =
        match prune_blocks_atomically(opt_budget, &cmd.optional_blocks) {
            Ok((kept_blocks, _, blocks_pruned)) => {
                let t = build_tail_string(cmd, &adapted, &intent_snippet, &kept_ssot, &kept_blocks);
                (t, intent_pruned || dropped_ssot > 0 || blocks_pruned, false)
            }
            Err(ContextCompilerError::AtomicBlockOverBudget { .. }) => (String::new(), true, true),
            Err(err) => return Err(err),
        };
    let mut dynamic_tok = if force_compact {
        usize::MAX
    } else {
        estimate_tokens(&dynamic_tail)
    };
    if force_compact || static_tok + dynamic_tok + outline_tok > MAX_TEXT_BPE_TOKENS {
        pruned = true;
        let (compacted_tail, new_kept_ssot, new_ssot_tok) = compact_tail_with_mandatory_blocks(
            cmd,
            &adapted,
            offline_tok,
            &intent_snippet,
            &all_ssot_lines,
            tail_budget,
        )?;
        dynamic_tail = compacted_tail;
        kept_ssot = new_kept_ssot;
        ssot_tok = new_ssot_tok;
        dynamic_tok = estimate_tokens(&dynamic_tail);
        if static_tok + dynamic_tok + outline_tok > MAX_TEXT_BPE_TOKENS {
            return Err(ContextCompilerError::AtomicBlockOverBudget {
                reason: format!("total tokens exceed {MAX_TEXT_BPE_TOKENS}"),
            });
        }
    }
    let req_cnt = if explicit_src_req > 0 {
        explicit_src_req
    } else {
        all_ssot_lines.len()
    };
    let inc_cnt = req_cnt.min(kept_ssot.len());
    Ok(BudgetedAssembly {
        outline,
        dynamic_tail,
        outline_tok,
        dynamic_tok,
        ssot_tok,
        pruned,
        req_cnt,
        inc_cnt,
        adapted,
    })
}

fn build_assembled_bundle(
    start: Instant,
    cmd: &ContextAssemblyCommand,
    static_prefix: String,
    static_sha: String,
    ba: BudgetedAssembly,
) -> Result<CompiledContextBundle, ContextCompilerError> {
    let static_tok = estimate_tokens(&static_prefix);
    let text_total = static_tok + ba.dynamic_tok + ba.outline_tok;
    let ssot_dom = (ba.outline_tok + ba.ssot_tok).min(text_total);
    let non_ssot_text = text_total.saturating_sub(ssot_dom);
    let ssot_dom_ratio = enforce_ssot_dom_ratio(ssot_dom, non_ssot_text)?;
    let reason_code = if ba.pruned || ba.inc_cnt < ba.req_cnt {
        ContextReasonCode::PrunedByBudget
    } else {
        ContextReasonCode::AtomicComplete
    };
    let dto = ContextAssembleDataDto {
        telemetry_id: Uuid::new_v4().to_string(),
        project_id: cmd.project_id.to_string(),
        portal_id: cmd.portal_id,
        horizon: cmd.horizon,
        model_profile: cmd.model_profile,
        adapter_mode: ba.adapted.adapter_mode,
        reason_code,
        static_prefix_sha256: static_sha,
        token_usage: TokenUsageDto {
            static_prefix: static_tok,
            dynamic_tail: ba.dynamic_tok,
            outline: ba.outline_tok,
            text_total,
            image: cmd.requested_image_tokens,
            ssot_dom,
            non_ssot_text,
            ssot_dom_ratio,
        },
        sources: SourcesSummaryDto {
            requested: ba.req_cnt,
            included: ba.inc_cnt,
            pruned: ba.req_cnt.saturating_sub(ba.inc_cnt),
        },
        latency_ms: start.elapsed().as_millis() as u64,
        assembled_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    };
    Ok(CompiledContextBundle {
        data: dto,
        static_prefix,
        dynamic_tail: ba.dynamic_tail,
        outline: ba.outline,
        downgraded_to_micro_tools: ba.adapted.downgraded_to_micro_tools,
        adapted_tool_count: ba.adapted.adapted_tool_count,
        offline_fallback_active: ba.adapted.offline_fallback_active,
        adapted_tools: ba.adapted.adapted_tools,
    })
}

pub fn compile_context_only(
    registry: Option<&SpecRegistry>,
    cmd: &ContextAssemblyCommand,
) -> Result<CompiledContextBundle, ContextCompilerError> {
    let start = Instant::now();
    let nfc_intent = validate_command_preconditions(cmd)?;
    if let Some(reg) = registry {
        if cmd.project_id != Uuid::nil() {
            verify_project_tenant_for_pool(reg.pool(), cmd.project_id, &cmd.tenant_id)?;
        }
    }
    let mut all_constraints = cmd.extra_constraints.clone();
    if let Some(reg) = registry {
        if cmd.project_id != Uuid::nil() {
            let loaded = ContextSlicer::new(reg)
                .load_active_constraints(cmd.project_id)
                .map_err(map_harness_error)?;
            all_constraints.extend(
                loaded
                    .into_iter()
                    .map(|c| format!("{}:{}:{}", c.rule_code, c.severity, c.config_json)),
            );
        }
    }
    validate_portal_isolation(cmd.portal_id, &nfc_intent, &all_constraints)?;
    let (static_prefix, static_sha) = build_static_prefix(cmd, &all_constraints);
    let ba = assemble_budgeted_payload(registry, cmd, &nfc_intent, &static_prefix)?;
    build_assembled_bundle(start, cmd, static_prefix, static_sha, ba)
}

pub fn assemble_and_record(
    registry: &SpecRegistry,
    cmd: &ContextAssemblyCommand,
) -> Result<CompiledContextBundle, ContextCompilerError> {
    let compiled = compile_context_only(Some(registry), cmd)?;
    let data = record_idempotent_or_replay(registry.pool(), cmd, &compiled.data)?;
    Ok(CompiledContextBundle { data, ..compiled })
}

fn resolve_cli_project_scope(workspace_root: &Path) -> (Option<SpecRegistry>, Uuid, String) {
    let db_path = workspace_root
        .join(".cowok-design")
        .join("cowok-design.sqlite3");
    if !db_path.is_file() {
        return (None, Uuid::nil(), "local-default".to_string());
    }
    let Ok(pool) = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, workspace_root)) else {
        return (None, Uuid::nil(), "local-default".to_string());
    };
    let latest = pool.connection().ok().and_then(|conn| {
        conn.query_row(
            "SELECT id, tenant_id, workspace_root FROM projects WHERE trash_deleted_at IS NULL ORDER BY updated_at DESC LIMIT 1",
            [],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
        )
        .ok()
        .and_then(|(s, t, ws)| Uuid::parse_str(&s).ok().map(|u| (u, t, ws)))
    });
    match latest {
        Some((uid, tenant, proj_ws)) => {
            let trimmed = proj_ws.trim();
            if !trimmed.is_empty() && trimmed != "." {
                let target_ws = std::path::PathBuf::from(trimmed);
                if target_ws.is_dir() {
                    if let Ok(scoped_pool) =
                        SqlitePool::open_wal(SqliteWalConfig::new(&db_path, &target_ws))
                    {
                        return (Some(SpecRegistry::new(scoped_pool)), uid, tenant);
                    }
                }
            }
            (Some(SpecRegistry::new(pool)), uid, tenant)
        }
        None => (None, Uuid::nil(), "local-default".to_string()),
    }
}

pub fn inspect_prompt_cli(
    workspace_root: &Path,
    portal_id: PortalId,
    horizon: Horizon,
    model_profile: ModelProfile,
    intent: &str,
) -> Result<InspectPromptPayload, ContextCompilerError> {
    let (reg_opt, project_id, tenant_id) = resolve_cli_project_scope(workspace_root);
    let cmd = ContextAssemblyCommand {
        project_id,
        tenant_id,
        idempotency_key: None,
        portal_id,
        horizon,
        model_profile,
        intent: intent.to_string(),
        requested_image_tokens: 0,
        max_image_tokens_per_turn: DEFAULT_MAX_IMAGE_TOKENS_PER_TURN,
        source_ids: None,
        inspect_only: true,
        brand_tokens_prefix: String::new(),
        active_skill_prefix: String::new(),
        extra_constraints: Vec::new(),
        dom_outline_nodes: Vec::new(),
        optional_blocks: Vec::new(),
    };
    let compiled = compile_context_only(reg_opt.as_ref(), &cmd)?;
    Ok(InspectPromptPayload {
        prefix_sha256: compiled.data.static_prefix_sha256,
        static_prefix: compiled.static_prefix,
        dynamic_tail: compiled.dynamic_tail,
        outline: compiled.outline,
        outline_tokens: compiled.data.token_usage.outline,
        total_text_tokens: compiled.data.token_usage.text_total,
        image_tokens: compiled.data.token_usage.image,
        ssot_dom_ratio: compiled.data.token_usage.ssot_dom_ratio,
        reason_code: compiled.data.reason_code,
        atomic_complete: matches!(compiled.data.reason_code, ContextReasonCode::AtomicComplete),
        profile: compiled.data.model_profile,
        adapter_mode: compiled.data.adapter_mode,
        downgraded_to_micro_tools: compiled.downgraded_to_micro_tools,
        adapted_tool_count: compiled.adapted_tool_count,
        offline_fallback_active: compiled.offline_fallback_active,
    })
}

pub fn inspect_prompt_from_args(args: &[String], default_root: &Path) -> Result<String, String> {
    let mut portal = PortalId::DeveloperPortal;
    let mut horizon = Horizon::Mid;
    let mut profile = ModelProfile::A;
    let mut intent = "inspect current workspace prompt".to_string();
    let mut idx = 0usize;
    while idx < args.len() {
        let key = args[idx].as_str();
        let next = args.get(idx + 1).map(|s| s.as_str());
        match (key, next) {
            ("--portal", Some(val)) => {
                portal = serde_json::from_str(&format!("\"{val}\"")).map_err(|e| e.to_string())?;
                idx += 2;
            }
            ("--horizon", Some(val)) => {
                horizon = serde_json::from_str(&format!("\"{val}\"")).map_err(|e| e.to_string())?;
                idx += 2;
            }
            ("--profile", Some(val)) => {
                profile = serde_json::from_str(&format!("\"{val}\"")).map_err(|e| e.to_string())?;
                idx += 2;
            }
            ("--intent", Some(val)) => {
                intent = val.to_string();
                idx += 2;
            }
            _ => idx += 1,
        }
    }
    let payload = inspect_prompt_cli(default_root, portal, horizon, profile, &intent)
        .map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())
}
