// @story 08.5
// @implements AC-03 (Auto-Package User Skills into JIT Progressive Disclosure <=100 tok + V20 user_skill_registry)
use std::net::{IpAddr, Ipv4Addr};

use chrono::Utc;
use domain::token_budget::count_exact_bpe_tokens;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::repackager::{
    scan_skill_markdown_safety, validate_safe_segment, validate_skill_metadata_text,
};
use crate::tool_jail::scan_ephemeral_script_capabilities;
use crate::validator::estimate_token_count;

pub const USER_SKILL_MAX_HEADER_TOKENS: usize = 100;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UserSkillPackagerError {
    #[error("SSRF_URL_BLOCKED: {0}")]
    SsrfBlocked(String),
    #[error("LICENSE_REJECTED: license '{license}' is disallowed for skill import")]
    LicenseRejected { license: String },
    #[error("SECURITY_SCAN_REJECTED: {0}")]
    SecurityRejected(String),
    #[error("INVALID_SKILL_SLUG: {0}")]
    InvalidSlug(String),
    #[error("STORAGE_ERROR: {0}")]
    Storage(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SecurityScanVerdict {
    Pass,
    Quarantined,
    Reject,
}

impl SecurityScanVerdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Quarantined => "QUARANTINED",
            Self::Reject => "REJECT",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LicenseClassification {
    AdoptPriorArt,
    ReferenceOnly,
    Reject,
}

impl LicenseClassification {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AdoptPriorArt => "ADOPT_PRIOR_ART",
            Self::ReferenceOnly => "REFERENCE_ONLY",
            Self::Reject => "REJECT",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportUserSkillRequest {
    pub project_id: Option<String>,
    pub skill_slug: String,
    pub repo_source_url: String,
    pub license_spdx: String,
    pub description: String,
    pub body_markdown: String,
    #[serde(default)]
    pub attached_scripts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagedUserSkillReceipt {
    pub id: String,
    pub tenant_id: String,
    pub project_id: Option<String>,
    pub skill_slug: String,
    pub repo_source_url: String,
    pub header_tokens: usize,
    pub security_scan_verdict: SecurityScanVerdict,
    pub license_classification: LicenseClassification,
    pub skill_md_path: String,
    pub skill_md_preview: String,
    pub created_at: i64,
}

fn is_forbidden_ipv4(v4: Ipv4Addr) -> bool {
    let oct = v4.octets();
    v4.is_loopback()
        || v4.is_private()
        || v4.is_link_local()
        || v4.is_unspecified()
        || v4.is_broadcast()
        || v4.is_multicast()
        || (oct[0] == 100 && (oct[1] & 0b1100_0000) == 64)
        || oct[0] == 0
}

fn is_non_canonical_numeric_ipv4_host(host: &str) -> bool {
    if host.is_empty() {
        return false;
    }
    // Single decimal/hex/octal integer hostname like `2130706433` or `0x7f000001`
    if host.bytes().all(|b| b.is_ascii_digit()) {
        return true;
    }
    if let Some(hex_body) = host.strip_prefix("0x").or_else(|| host.strip_prefix("0X")) {
        if !hex_body.is_empty() && hex_body.bytes().all(|b| b.is_ascii_hexdigit()) {
            return true;
        }
    }
    // Dotted numeric forms (short IPv4 `127.1`, octal `0177.0.0.1`, hex `0x7f.0.0.1`)
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() >= 2 && labels.len() <= 4 {
        let all_numeric = labels.iter().all(|lbl| {
            !lbl.is_empty()
                && (lbl.bytes().all(|b| b.is_ascii_digit())
                    || lbl
                        .strip_prefix("0x")
                        .or_else(|| lbl.strip_prefix("0X"))
                        .is_some_and(|h| !h.is_empty() && h.bytes().all(|b| b.is_ascii_hexdigit())))
        });
        if all_numeric {
            if labels.len() < 4 {
                return true;
            }
            // Any leading-zero octal segment or hex segment
            if labels.iter().any(|lbl| {
                (lbl.len() > 1 && lbl.starts_with('0'))
                    || lbl.starts_with("0x")
                    || lbl.starts_with("0X")
            }) {
                return true;
            }
        }
    }
    false
}

/// Validates that an import URL is strictly public HTTPS (blocks loopback, RFC1918, link-local,
/// IPv4-mapped IPv6, IPv6 fe80::/10 & ff00::/8, non-canonical numeric IPv4, trailing dot bypass,
/// control/whitespace/backslash chars, file://, metadata IPs).
pub fn validate_import_repo_ssrf(url: &str) -> Result<(), UserSkillPackagerError> {
    let trimmed = url.trim();
    if trimmed.is_empty() || trimmed.len() > 512 {
        return Err(UserSkillPackagerError::SsrfBlocked(
            "URL must be 1..512 characters".to_string(),
        ));
    }
    if trimmed
        .chars()
        .any(|c| c.is_control() || c.is_whitespace() || c == '\\')
    {
        return Err(UserSkillPackagerError::SsrfBlocked(
            "Control characters, whitespace, or backslashes forbidden in URL".to_string(),
        ));
    }
    let after_scheme = trimmed.strip_prefix("https://").ok_or_else(|| {
        UserSkillPackagerError::SsrfBlocked(
            "Only https:// repository URLs are permitted (http/file/gopher rejected)".to_string(),
        )
    })?;
    let authority = after_scheme.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return Err(UserSkillPackagerError::SsrfBlocked(
            "Credentials in URL or empty host rejected".to_string(),
        ));
    }
    let is_bracketed_ipv6 = authority.starts_with('[');
    let raw_host = if is_bracketed_ipv6 {
        let Some(end_idx) = authority.find(']') else {
            return Err(UserSkillPackagerError::SsrfBlocked(
                "Malformed bracketed IPv6 authority rejected".to_string(),
            ));
        };
        &authority[1..end_idx]
    } else {
        authority.split(':').next().unwrap_or("")
    };
    let lower_host = raw_host.trim_end_matches('.').to_ascii_lowercase();
    if is_bracketed_ipv6 && lower_host.parse::<std::net::Ipv6Addr>().is_err() {
        return Err(UserSkillPackagerError::SsrfBlocked(format!(
            "Invalid or zone-scoped bracketed IPv6 host '{raw_host}' blocked"
        )));
    }
    if lower_host.is_empty()
        || lower_host == "localhost"
        || lower_host.ends_with(".localhost")
        || lower_host.ends_with(".local")
        || lower_host.ends_with(".internal")
        || lower_host == "metadata.google.internal"
    {
        return Err(UserSkillPackagerError::SsrfBlocked(format!(
            "Local/internal host '{raw_host}' blocked"
        )));
    }
    if is_non_canonical_numeric_ipv4_host(&lower_host) {
        return Err(UserSkillPackagerError::SsrfBlocked(format!(
            "Non-canonical numeric IPv4 host '{raw_host}' blocked"
        )));
    }
    if let Ok(ip) = lower_host.parse::<IpAddr>() {
        let forbidden = match ip {
            IpAddr::V4(v4) => is_forbidden_ipv4(v4),
            IpAddr::V6(v6) => {
                let segs = v6.segments();
                let is_link_local = (segs[0] & 0xffc0) == 0xfe80;
                let is_multicast = (segs[0] & 0xff00) == 0xff00;
                let mapped_forbidden = v6.to_ipv4_mapped().is_some_and(is_forbidden_ipv4);
                let compat_v4_forbidden = segs[0..6] == [0, 0, 0, 0, 0, 0]
                    && is_forbidden_ipv4(Ipv4Addr::from(
                        ((segs[6] as u32) << 16) | (segs[7] as u32),
                    ));
                v6.is_loopback()
                    || v6.is_unspecified()
                    || v6.is_unique_local()
                    || is_link_local
                    || is_multicast
                    || mapped_forbidden
                    || compat_v4_forbidden
            }
        };
        if forbidden {
            return Err(UserSkillPackagerError::SsrfBlocked(format!(
                "Non-public IP address '{ip}' blocked"
            )));
        }
    }
    Ok(())
}

/// Classifies SPDX license identifiers into ADOPT_PRIOR_ART, REFERENCE_ONLY, or REJECT.
pub fn classify_spdx_license(spdx: &str) -> LicenseClassification {
    let norm = spdx.trim().to_ascii_uppercase();
    match norm.as_str() {
        "MIT" | "APACHE-2.0" | "BSD-2-CLAUSE" | "BSD-3-CLAUSE" | "ISC" | "CC0-1.0"
        | "UNLICENSE" => LicenseClassification::AdoptPriorArt,
        "MPL-2.0" | "LGPL-3.0" | "LGPL-2.1" | "CC-BY-4.0" | "CC-BY-SA-4.0" => {
            LicenseClassification::ReferenceOnly
        }
        _ => LicenseClassification::Reject,
    }
}

fn exact_header_token_count(header: &str) -> usize {
    std::cmp::max(count_exact_bpe_tokens(header), estimate_token_count(header))
}

/// Builds a `SKILL.md` frontmatter header strictly `<= 100` tokens (verified via exact BPE + heuristic max)
/// with YAML backslash/quote escaping for JIT progressive disclosure.
pub fn build_jit_skill_markdown(
    slug: &str,
    license: LicenseClassification,
    description: &str,
    body_markdown: &str,
) -> (String, usize) {
    let clean_desc: String = description
        .replace('\\', "\\\\")
        .replace(['\r', '\n', '"'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut words: Vec<&str> = clean_desc.split_whitespace().collect();
    if words.is_empty() {
        words.push("User");
        words.push("skill");
    }

    let mut header: String;
    let mut tokens: usize;
    loop {
        let joined_desc = words.join(" ");
        header = format!(
            "---\nname: {slug}\nlicense_class: {}\ndescription: \"{joined_desc}\"\n---",
            license.as_str()
        );
        tokens = exact_header_token_count(&header);
        if tokens <= USER_SKILL_MAX_HEADER_TOKENS || words.len() <= 1 {
            break;
        }
        let next_len = (words.len() * 3) / 4;
        words.truncate(next_len.max(1));
    }

    if tokens > USER_SKILL_MAX_HEADER_TOKENS {
        header = format!(
            "---\nname: {slug}\nlicense_class: {}\ndescription: \"Packaged user skill\"\n---",
            license.as_str()
        );
        tokens = exact_header_token_count(&header);
    }

    let full_md = format!("{header}\n\n{}\n", body_markdown.trim());
    (full_md, tokens)
}

/// Validates SSRF, License/ToS, metadata YAML safety, and AST/Markdown security, packages `SKILL.md`
/// (`<=100` exact BPE tokens), and persists the entry into SQLite `V20 user_skill_registry`.
pub fn package_and_register_user_skill(
    conn: &Connection,
    tenant_id: &str,
    req: &ImportUserSkillRequest,
) -> Result<PackagedUserSkillReceipt, UserSkillPackagerError> {
    let slug = req.skill_slug.trim().to_ascii_lowercase();
    validate_safe_segment("skill_slug", &slug)
        .map_err(|e| UserSkillPackagerError::InvalidSlug(e.to_string()))?;

    validate_import_repo_ssrf(&req.repo_source_url)?;

    let license_classification = classify_spdx_license(&req.license_spdx);
    if license_classification == LicenseClassification::Reject {
        return Err(UserSkillPackagerError::LicenseRejected {
            license: req.license_spdx.clone(),
        });
    }

    // H-03: Validate description metadata against YAML delimiter breakout and IPI markers
    if let Err(err) = validate_skill_metadata_text("description", &req.description) {
        return Err(UserSkillPackagerError::SecurityRejected(err.to_string()));
    }
    if let Err(err) = scan_skill_markdown_safety("SKILL.md", &req.body_markdown) {
        return Err(UserSkillPackagerError::SecurityRejected(err.to_string()));
    }
    for script in &req.attached_scripts {
        if let Err(err) = scan_ephemeral_script_capabilities(script) {
            return Err(UserSkillPackagerError::SecurityRejected(err.to_string()));
        }
    }

    let verdict = if license_classification == LicenseClassification::ReferenceOnly {
        SecurityScanVerdict::Quarantined
    } else {
        SecurityScanVerdict::Pass
    };

    let (skill_md_preview, header_tokens) = build_jit_skill_markdown(
        &slug,
        license_classification,
        &req.description,
        &req.body_markdown,
    );
    let skill_md_path = format!(".cowok-design/user-skills/{slug}/SKILL.md");
    let id = Uuid::new_v4().to_string();
    let created_at = Utc::now().timestamp_millis();
    let trimmed_proj = req
        .project_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    // M-01: Handle NULL project_id cleanly via partial unique index uq_user_skill_registry_tenant_global_slug
    if let Some(pid) = trimmed_proj {
        conn.execute(
            "INSERT INTO user_skill_registry (
                id, tenant_id, project_id, skill_slug, repo_source_url,
                header_tokens, security_scan_verdict, license_classification, skill_md_path, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(tenant_id, project_id, skill_slug) DO UPDATE SET
                repo_source_url = excluded.repo_source_url,
                header_tokens = excluded.header_tokens,
                security_scan_verdict = excluded.security_scan_verdict,
                license_classification = excluded.license_classification,
                skill_md_path = excluded.skill_md_path,
                created_at = excluded.created_at",
            params![
                id,
                tenant_id,
                pid,
                slug,
                req.repo_source_url.trim(),
                header_tokens as i64,
                verdict.as_str(),
                license_classification.as_str(),
                skill_md_path,
                created_at
            ],
        )
        .map_err(|e| UserSkillPackagerError::Storage(e.to_string()))?;
    } else {
        conn.execute(
            "INSERT INTO user_skill_registry (
                id, tenant_id, project_id, skill_slug, repo_source_url,
                header_tokens, security_scan_verdict, license_classification, skill_md_path, created_at
             ) VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(tenant_id, skill_slug) WHERE project_id IS NULL DO UPDATE SET
                repo_source_url = excluded.repo_source_url,
                header_tokens = excluded.header_tokens,
                security_scan_verdict = excluded.security_scan_verdict,
                license_classification = excluded.license_classification,
                skill_md_path = excluded.skill_md_path,
                created_at = excluded.created_at",
            params![
                id,
                tenant_id,
                slug,
                req.repo_source_url.trim(),
                header_tokens as i64,
                verdict.as_str(),
                license_classification.as_str(),
                skill_md_path,
                created_at
            ],
        )
        .map_err(|e| UserSkillPackagerError::Storage(e.to_string()))?;
    }

    let persisted_id: String = conn
        .query_row(
            "SELECT id FROM user_skill_registry
             WHERE tenant_id = ?1 AND project_id IS ?2 AND skill_slug = ?3",
            params![tenant_id, trimmed_proj, slug],
            |r| r.get(0),
        )
        .unwrap_or(id);

    Ok(PackagedUserSkillReceipt {
        id: persisted_id,
        tenant_id: tenant_id.to_string(),
        project_id: trimmed_proj.map(str::to_string),
        skill_slug: slug,
        repo_source_url: req.repo_source_url.trim().to_string(),
        header_tokens,
        security_scan_verdict: verdict,
        license_classification,
        skill_md_path,
        skill_md_preview,
        created_at,
    })
}

pub fn list_user_skills_scoped(
    conn: &Connection,
    tenant_id: &str,
    project_id: Option<&str>,
) -> Result<Vec<PackagedUserSkillReceipt>, UserSkillPackagerError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, tenant_id, project_id, skill_slug, repo_source_url,
                    header_tokens, security_scan_verdict, license_classification, skill_md_path, created_at
             FROM user_skill_registry
             WHERE tenant_id = ?1 AND (?2 IS NULL OR project_id = ?2)
             ORDER BY created_at DESC, id DESC",
        )
        .map_err(|e| UserSkillPackagerError::Storage(e.to_string()))?;

    let rows = stmt
        .query_map(params![tenant_id, project_id], |r| {
            let verdict_str: String = r.get(6)?;
            let lic_str: String = r.get(7)?;
            Ok(PackagedUserSkillReceipt {
                id: r.get(0)?,
                tenant_id: r.get(1)?,
                project_id: r.get(2)?,
                skill_slug: r.get(3)?,
                repo_source_url: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                header_tokens: r.get::<_, i64>(5)?.max(0) as usize,
                security_scan_verdict: match verdict_str.as_str() {
                    "PASS" => SecurityScanVerdict::Pass,
                    "QUARANTINED" => SecurityScanVerdict::Quarantined,
                    _ => SecurityScanVerdict::Reject,
                },
                license_classification: match lic_str.as_str() {
                    "ADOPT_PRIOR_ART" => LicenseClassification::AdoptPriorArt,
                    "REFERENCE_ONLY" => LicenseClassification::ReferenceOnly,
                    _ => LicenseClassification::Reject,
                },
                skill_md_path: r.get(8)?,
                skill_md_preview: String::new(),
                created_at: r.get(9)?,
            })
        })
        .map_err(|e| UserSkillPackagerError::Storage(e.to_string()))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| UserSkillPackagerError::Storage(e.to_string()))?);
    }
    Ok(out)
}
