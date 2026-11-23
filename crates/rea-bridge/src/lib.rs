// @story 09.1
// @implements AC-01, AC-02, AC-03, AC-04 (crates/rea-bridge Two-Store /rea + /awesome-re Bridge)
pub mod catalog;
pub mod firewall;
pub mod importer;
pub mod store;

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub use catalog::{AwesomeReCatalogItem, AwesomeReCatalogResponse};

pub use firewall::{
    evaluate_license_and_tos, is_forbidden_ip, is_prohibited_port, sanitize_design_json,
    sanitize_design_only_text, validate_public_url_ssrf, validate_socket_addr_ssrf,
    FirewallDecision, FirewallError, FirewallVerdict, SanitizationReport, PROHIBITED_PORTS,
};
pub use importer::{
    compute_proof_payload_and_sha256, evaluate_figma_proof_export, extract_web_clone_l1_l6,
    import_design_payload, read_zip_safely, FigmaProofExport, ImportedDesignArtifact,
    ImporterError, SourceFormat, WebCloneLayers, ZipEntryData, MAX_ZIP_ENTRIES,
    MAX_ZIP_EXPANSION_RATIO, MAX_ZIP_UNCOMPRESSED_BYTES,
};
pub use store::{
    count_deterministic_tokens, sanitize_fts5_query, truncate_to_token_cap, ProofStatus,
    ReaReferenceRecord, ReaSearchHit, ReaSearchTier, ReaStoreError, ReaTwoStore,
    UpsertReaReference, CACHE_DB_FILENAME, CACHE_RELATIVE_DIR, TIER1A_MAX_TOKENS,
};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaIngestRequest {
    pub slug: String,
    pub source_url: String,
    pub source_format: SourceFormat,
    #[serde(default)]
    pub license_spdx: String,
    #[serde(default)]
    pub summary_hint: Option<String>,
    #[serde(default)]
    pub payload_text: Option<String>,
    #[serde(default)]
    pub payload_base64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaIngestOutcome {
    pub record: ReaReferenceRecord,
    pub firewall: FirewallDecision,
    pub stripped_categories: Vec<String>,
    pub extracted_files: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaSearchRequest {
    #[serde(default)]
    pub q: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
    #[serde(default = "default_search_tier")]
    pub tier: ReaSearchTier,
}

fn default_search_limit() -> usize {
    20
}

fn default_search_tier() -> ReaSearchTier {
    ReaSearchTier::Tier1a
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaSearchResponse {
    pub tier: ReaSearchTier,
    pub query: String,
    pub sanitized_fts_query: String,
    pub max_tier1a_tokens: usize,
    pub total: usize,
    pub items: Vec<ReaSearchHit>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaExportProofRequest {
    pub reference_id: String,
    #[serde(default)]
    pub roundtrip_checksum: Option<String>,
}

#[derive(Debug, Error)]
pub enum ReaBridgeError {
    #[error("Invalid slug: {0}")]
    InvalidSlug(String),
    #[error("SSRF blocked: {0}")]
    Ssrf(#[from] FirewallError),
    #[error("License/ToS firewall rejected source ({policy_rule}): {reason}")]
    FirewallRejected {
        quarantine_id: String,
        license_spdx: String,
        policy_rule: String,
        reason: String,
    },
    #[error("Importer error: {0}")]
    Importer(#[from] ImporterError),
    #[error("Store error: {0}")]
    Store(#[from] ReaStoreError),
}

fn validate_slug(raw: &str) -> Result<String, ReaBridgeError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 96 {
        return Err(ReaBridgeError::InvalidSlug(
            "Slug must be 1..96 characters".to_string(),
        ));
    }
    if !trimmed
        .bytes()
        .next()
        .is_some_and(|b| b.is_ascii_alphanumeric())
    {
        return Err(ReaBridgeError::InvalidSlug(
            "Slug must start with an ASCII alphanumeric character".to_string(),
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err(ReaBridgeError::InvalidSlug(
            "Slug may only contain ASCII alphanumeric, '-', '_', or '.'".to_string(),
        ));
    }
    if trimmed.contains("..") {
        return Err(ReaBridgeError::InvalidSlug(
            "Slug cannot contain '..'".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn is_valid_local_source_uri(raw: &str) -> bool {
    let Some(rest) = raw.strip_prefix("local://").or_else(|| {
        if raw.len() >= 8 && raw[..8].eq_ignore_ascii_case("local://") {
            Some(&raw[8..])
        } else {
            None
        }
    }) else {
        return false;
    };
    if rest.is_empty()
        || rest.len() > 256
        || rest.starts_with('/')
        || rest.contains("..")
        || rest.contains('?')
        || rest.contains('#')
        || rest.contains('\\')
    {
        return false;
    }
    rest.bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'/'))
}

fn merge_categories(target: &mut Vec<String>, incoming: Vec<String>) {
    for cat in incoming {
        if !target.iter().any(|c| c == &cat) {
            target.push(cat);
        }
    }
}

/// High-level facade coordinating Firewall, Lossless Importer, Design-Only Sanitizer,
/// Honest Proof Exporter, and the Two-Store SQLite FTS5 cache.
#[derive(Debug, Clone)]
pub struct ReaBridgeService {
    store: ReaTwoStore,
}

impl ReaBridgeService {
    pub fn open(base_dir: &Path) -> Result<Self, ReaBridgeError> {
        Ok(Self {
            store: ReaTwoStore::open(base_dir)?,
        })
    }

    pub fn store(&self) -> &ReaTwoStore {
        &self.store
    }

    pub fn ingest(
        &self,
        tenant_id: &str,
        req: ReaIngestRequest,
    ) -> Result<ReaIngestOutcome, ReaBridgeError> {
        let slug = validate_slug(&req.slug)?;
        let trimmed_url = req.source_url.trim();
        let is_network_format =
            matches!(req.source_format, SourceFormat::Url | SourceFormat::Github);
        if is_network_format || !is_valid_local_source_uri(trimmed_url) {
            validate_public_url_ssrf(trimmed_url)?;
        }

        let hint_text = format!(
            "{}\n{}",
            req.summary_hint.as_deref().unwrap_or(""),
            req.payload_text.as_deref().unwrap_or("")
        );
        let mut decision = evaluate_license_and_tos(&req.license_spdx, trimmed_url, &hint_text);
        if decision.verdict == FirewallVerdict::Reject {
            return Err(ReaBridgeError::FirewallRejected {
                quarantine_id: format!("quarantine-{}", Uuid::new_v4()),
                license_spdx: decision.normalized_license,
                policy_rule: decision.policy_rule,
                reason: decision.reason,
            });
        }

        let mut total_secrets: usize = 0;
        let mut categories: Vec<String> = Vec::new();
        let sanitized_payload_text = req.payload_text.as_deref().map(|raw| {
            let report = sanitize_design_only_text(raw);
            total_secrets += report.sanitized_secrets_count;
            merge_categories(&mut categories, report.stripped_categories);
            report.sanitized_text
        });

        let imported = import_design_payload(
            req.source_format,
            sanitized_payload_text.as_deref(),
            req.payload_base64.as_deref(),
        )?;

        // F-01: Re-run License/ToS Firewall over extracted archive entries, extracted text,
        // serialized design_dna, and raw_fields BEFORE persisting to Two-Store.
        let dna_raw_str = serde_json::to_string(&imported.design_dna).unwrap_or_default();
        let raw_fields_raw_str = imported
            .raw_fields
            .as_ref()
            .and_then(|rf| serde_json::to_string(rf).ok())
            .unwrap_or_default();
        let extracted_archive_text = format!(
            "{}\n{}\n{}\n{}",
            imported.extracted_files.join("\n"),
            imported.extracted_text,
            dna_raw_str,
            raw_fields_raw_str,
        );
        let post_decision =
            evaluate_license_and_tos(&req.license_spdx, trimmed_url, &extracted_archive_text);
        if post_decision.verdict == FirewallVerdict::Reject {
            return Err(ReaBridgeError::FirewallRejected {
                quarantine_id: format!("quarantine-{}", Uuid::new_v4()),
                license_spdx: post_decision.normalized_license,
                policy_rule: post_decision.policy_rule,
                reason: post_decision.reason,
            });
        }
        if decision.verdict == FirewallVerdict::AdoptPriorArt
            && post_decision.verdict == FirewallVerdict::ReferenceOnly
        {
            decision = post_decision;
        }

        // F-04: Sanitize source_url, design_dna (keys+values), raw_fields (keys+values),
        // summary, and extracted_files before persisting/returning.
        let source_url_report = sanitize_design_only_text(trimmed_url);
        total_secrets += source_url_report.sanitized_secrets_count;
        merge_categories(&mut categories, source_url_report.stripped_categories);

        let (sanitized_dna, dna_secrets, dna_cats) = sanitize_design_json(&imported.design_dna);
        total_secrets += dna_secrets;
        merge_categories(&mut categories, dna_cats);

        let sanitized_raw_json = if let Some(ref rf) = imported.raw_fields {
            let (s_rf, rf_secrets, rf_cats) = sanitize_design_json(rf);
            total_secrets += rf_secrets;
            merge_categories(&mut categories, rf_cats);
            Some(serde_json::to_string(&s_rf).unwrap_or_else(|_| "{}".to_string()))
        } else {
            None
        };

        let sanitized_extracted_files: Vec<String> = imported
            .extracted_files
            .iter()
            .map(|name| {
                let report = sanitize_design_only_text(name);
                total_secrets += report.sanitized_secrets_count;
                merge_categories(&mut categories, report.stripped_categories);
                report.sanitized_text
            })
            .collect();

        let raw_summary = req
            .summary_hint
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|hint| format!("{hint} {}", imported.auto_summary))
            .unwrap_or(imported.auto_summary);
        let summary_report = sanitize_design_only_text(&raw_summary);
        total_secrets += summary_report.sanitized_secrets_count;
        merge_categories(&mut categories, summary_report.stripped_categories);

        let design_dna_json =
            serde_json::to_string(&sanitized_dna).unwrap_or_else(|_| "{}".to_string());

        let record = self.store.upsert_reference(UpsertReaReference {
            tenant_id: tenant_id.to_string(),
            slug,
            source_url: source_url_report.sanitized_text,
            source_format: req.source_format.as_str().to_string(),
            license_spdx: decision.normalized_license.clone(),
            firewall_verdict: decision.verdict,
            firewall_reason: decision.reason.clone(),
            tier1a_summary: summary_report.sanitized_text,
            design_dna_json,
            raw_fields_json: sanitized_raw_json,
            proof_status: ProofStatus::AwaitingVerification,
            sanitized_secrets_count: total_secrets,
        })?;

        Ok(ReaIngestOutcome {
            record,
            firewall: decision,
            stripped_categories: categories,
            extracted_files: sanitized_extracted_files,
        })
    }

    pub fn search(
        &self,
        tenant_id: &str,
        req: ReaSearchRequest,
    ) -> Result<ReaSearchResponse, ReaBridgeError> {
        let sanitized_fts_query = sanitize_fts5_query(&req.q);
        let items = self.store.search(tenant_id, &req.q, req.tier, req.limit)?;
        let total = items.len();
        Ok(ReaSearchResponse {
            tier: req.tier,
            query: req.q,
            sanitized_fts_query,
            max_tier1a_tokens: TIER1A_MAX_TOKENS,
            total,
            items,
        })
    }

    pub fn search_catalog(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<AwesomeReCatalogResponse, ReaBridgeError> {
        Ok(self.store.search_awesome_re_catalog(query, limit)?)
    }

    pub fn export_proof(
        &self,
        tenant_id: &str,
        req: ReaExportProofRequest,
    ) -> Result<FigmaProofExport, ReaBridgeError> {
        let trimmed_id = req.reference_id.trim();
        let Some(record) = self.store.get_by_id(tenant_id, trimmed_id)? else {
            return Err(ReaBridgeError::Store(ReaStoreError::NotFound(
                trimmed_id.to_string(),
            )));
        };
        let export = evaluate_figma_proof_export(&record, req.roundtrip_checksum.as_deref());
        if export.proof_status == ProofStatus::ProofVerified
            && record.proof_status != ProofStatus::ProofVerified
        {
            self.store
                .update_proof_status(tenant_id, &record.id, ProofStatus::ProofVerified)?;
        }
        Ok(export)
    }
}
