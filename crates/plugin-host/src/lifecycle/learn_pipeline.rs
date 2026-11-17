// @story 08.3
// @implements AC-02, AC-04, AC-06
use crate::models::LintIssue;
use crate::repackager::security_scanner::{scan_skill_markdown_safety, validate_skill_metadata_text};
use domain::models::GateReport;
use domain::token_budget::get_cl100k_bpe;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use thiserror::Error;

pub const MAX_LEARN_PROMPT_TOKENS: usize = 1_500;
pub const MAX_CARD_TITLE_BYTES: usize = 160;
pub const MAX_CARD_CONTENT_BYTES: usize = 4_096;
pub const MAX_CARD_EVIDENCE_BYTES: usize = 512;
pub const MAX_SCOPE_SEGMENT_BYTES: usize = 128;
pub const MAX_APPROVAL_TTL_SECONDS: i64 = 900;
pub const MAX_CARDS_PER_SCOPE: usize = 5_000;
pub const MAX_NONCES_PER_SCOPE: usize = 10_000;
const APPROVER_HMAC_DOMAIN: &str = "cowok-learn-approver-v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(
        export,
        export_to = "../../../packages/shared-types/src/generated/",
        rename_all = "SCREAMING_SNAKE_CASE"
    )
)]
pub enum LearnCardCategory {
    Rule,
    Pattern,
    AntiPattern,
    Lesson,
    Flow,
}

impl LearnCardCategory {
    pub fn as_wire_str(&self) -> &'static str {
        match self {
            Self::Rule => "RULE",
            Self::Pattern => "PATTERN",
            Self::AntiPattern => "ANTI_PATTERN",
            Self::Lesson => "LESSON",
            Self::Flow => "FLOW",
        }
    }

    pub fn parse_compat(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "RULE" => Ok(Self::Rule),
            "PATTERN" => Ok(Self::Pattern),
            "ANTI_PATTERN" | "ANTIPATTERN" => Ok(Self::AntiPattern),
            "LESSON" => Ok(Self::Lesson),
            "FLOW" => Ok(Self::Flow),
            _ => Err(format!("Unknown LearnCardCategory '{raw}'")),
        }
    }
}

impl Serialize for LearnCardCategory {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_wire_str())
    }
}

impl<'de> Deserialize<'de> for LearnCardCategory {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::parse_compat(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(
        export,
        export_to = "../../../packages/shared-types/src/generated/",
        rename_all = "SCREAMING_SNAKE_CASE"
    )
)]
pub enum CardStatus {
    Proposed,
    Accepted,
    Rejected,
}

impl CardStatus {
    pub fn as_wire_str(&self) -> &'static str {
        match self {
            Self::Proposed => "PROPOSED",
            Self::Accepted => "ACCEPTED",
            Self::Rejected => "REJECTED",
        }
    }

    pub fn parse_compat(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "PROPOSED" => Ok(Self::Proposed),
            "ACCEPTED" => Ok(Self::Accepted),
            "REJECTED" => Ok(Self::Rejected),
            _ => Err(format!("Unknown CardStatus '{raw}'")),
        }
    }
}

impl Serialize for CardStatus {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_wire_str())
    }
}

impl<'de> Deserialize<'de> for CardStatus {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::parse_compat(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub enum CardDecisionAction {
    Accept,
    Reject,
}

impl CardDecisionAction {
    pub fn as_wire_str(self) -> &'static str {
        match self {
            Self::Accept => "ACCEPT",
            Self::Reject => "REJECT",
        }
    }

    pub fn target_status(self) -> CardStatus {
        match self {
            Self::Accept => CardStatus::Accepted,
            Self::Reject => CardStatus::Rejected,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub enum ActorKind {
    HumanWorkbenchSession,
    AgentHmac,
    McpClient,
    SuperadminCli,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct HumanApproverToken {
    kid: String,
    actor_kind: ActorKind,
    actor_id: String,
    tenant_id: String,
    project_id: String,
    card_id: String,
    content_sha256: String,
    action: CardDecisionAction,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    expected_revision: u64,
    reason: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    issued_at_s: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    expires_at_s: i64,
    nonce: String,
    hmac_sha256: String,
}

impl HumanApproverToken {
    pub fn kid(&self) -> &str {
        &self.kid
    }
    pub fn actor_kind(&self) -> ActorKind {
        self.actor_kind
    }
    pub fn actor_id(&self) -> &str {
        &self.actor_id
    }
    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }
    pub fn project_id(&self) -> &str {
        &self.project_id
    }
    pub fn card_id(&self) -> &str {
        &self.card_id
    }
    pub fn content_sha256(&self) -> &str {
        &self.content_sha256
    }
    pub fn action(&self) -> CardDecisionAction {
        self.action
    }
    pub fn expected_revision(&self) -> u64 {
        self.expected_revision
    }
    pub fn reason(&self) -> &str {
        &self.reason
    }
    pub fn issued_at_s(&self) -> i64 {
        self.issued_at_s
    }
    pub fn expires_at_s(&self) -> i64 {
        self.expires_at_s
    }
    pub fn nonce(&self) -> &str {
        &self.nonce
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MintTokenInput<'a> {
    pub actor_kind: ActorKind,
    pub actor_id: &'a str,
    pub tenant_id: &'a str,
    pub project_id: &'a str,
    pub card_id: &'a str,
    pub content_sha256: &'a str,
    pub action: CardDecisionAction,
    pub expected_revision: u64,
    pub reason: &'a str,
    pub now_s: i64,
    pub ttl_s: i64,
    pub nonce: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct ConsumedNonceRecord {
    pub tenant_id: String,
    pub project_id: String,
    pub nonce: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub expires_at_s: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct KnowledgeCard {
    pub id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub idempotency_key: Option<String>,
    pub category: LearnCardCategory,
    pub title: String,
    pub content: String,
    pub content_sha256: String,
    pub status: CardStatus,
    pub source_evidence: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub revision: u64,
    pub reviewed_by: Option<String>,
    pub decision_reason: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub created_at: i64,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub accepted_at: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
pub struct ProposeCardInput<'a> {
    pub tenant_id: &'a str,
    pub project_id: &'a str,
    pub id: &'a str,
    pub idempotency_key: Option<&'a str>,
    pub category: LearnCardCategory,
    pub title: &'a str,
    pub content: &'a str,
    pub source_evidence: &'a str,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct PromptInjectionResult {
    pub card_ids: Vec<String>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total_tokens: usize,
    pub fenced_xml: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "name", rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub enum StorageConstraintTarget {
    PluginActiveSlug,
    KnowledgeCardPrimaryKey,
    KnowledgeCardContentHash,
    CronScheduleLease,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct StorageConstraintError {
    pub code: String,
    pub target_kind: StorageConstraintTarget,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub enum DomainStorageAction {
    ConflictRetryable {
        error: String,
        retry_hint: String,
    },
    IdempotentDuplicate {
        error: String,
    },
    ParentNotFound {
        error: String,
    },
    RetryWithBoundedBackoff {
        max_attempts: u8,
        #[cfg_attr(feature = "ts", ts(type = "Array<number>"))]
        backoff_ms: Vec<u64>,
    },
    UnmappedFatal {
        error: String,
    },
}

pub fn map_storage_constraint_error(err: &StorageConstraintError) -> DomainStorageAction {
    let code_upper = err.code.to_ascii_uppercase();
    if code_upper == "P2002" || code_upper == "SQLITE_CONSTRAINT_UNIQUE" {
        return match &err.target_kind {
            StorageConstraintTarget::KnowledgeCardContentHash
            | StorageConstraintTarget::KnowledgeCardPrimaryKey => {
                DomainStorageAction::IdempotentDuplicate {
                    error: "DuplicateKnowledgeCard".to_string(),
                }
            }
            StorageConstraintTarget::PluginActiveSlug
            | StorageConstraintTarget::CronScheduleLease => DomainStorageAction::ConflictRetryable {
                error: "ActivePluginConflict".to_string(),
                retry_hint: "Re-read active revision & isolate breaking candidate into Staging"
                    .to_string(),
            },
            StorageConstraintTarget::Other(name) => DomainStorageAction::UnmappedFatal {
                error: format!("Unclassified unique constraint on '{name}': {}", err.detail),
            },
        };
    }
    if code_upper == "P2003" || code_upper == "SQLITE_CONSTRAINT_FOREIGNKEY" {
        return DomainStorageAction::ParentNotFound {
            error: "ProjectOrPluginNotFound".to_string(),
        };
    }
    if code_upper == "SQLITE_BUSY" || code_upper == "P2034" {
        return DomainStorageAction::RetryWithBoundedBackoff {
            max_attempts: 3,
            backoff_ms: vec![10, 25, 50],
        };
    }
    DomainStorageAction::UnmappedFatal {
        error: err.detail.clone(),
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LearnError {
    #[error("InvalidCardInput: {0}")]
    InvalidInput(String),
    #[error("PromptInjectionBlocked: {0}")]
    PromptInjectionBlocked(String),
    #[error("DuplicateCardId: '{0}' already exists in scope")]
    DuplicateCardId(String),
    #[error("DuplicateActiveContent: matching active/proposed content hash already exists")]
    DuplicateActiveContent,
    #[error("IdempotentReplay: request already materialized as card '{0}'")]
    IdempotentReplay(String),
    #[error("IdempotencyPayloadMismatch: idempotency_key reused with conflicting card payload")]
    IdempotencyPayloadMismatch,
    #[error("CardNotFound: '{0}'")]
    CardNotFound(String),
    #[error("UnauthorizedApprover: {0}")]
    UnauthorizedApprover(String),
    #[error("ReplayedApprovalNonce: nonce '{0}' already consumed")]
    ReplayedApprovalNonce(String),
    #[error("InvalidCardTransition: cannot move card from {from:?} to {to:?}")]
    InvalidCardTransition { from: CardStatus, to: CardStatus },
    #[error("RevisionConflict: expected {expected}, actual {actual}")]
    RevisionConflict { expected: u64, actual: u64 },
    #[error("CapacityExceeded: {0}")]
    CapacityExceeded(String),
    #[error("EntropyUnavailable: OS CSPRNG failed")]
    EntropyUnavailable,
}

fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut out = String::with_capacity(64);
    for b in digest {
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

fn hmac_sha256_hex(key: &[u8; 32], message: &[u8]) -> String {
    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..32 {
        ipad[i] ^= key[i];
        opad[i] ^= key[i];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(message);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_hash);
    let digest = outer.finalize();
    let mut out = String::with_capacity(64);
    for b in digest {
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

fn ct_eq_hex(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.as_bytes().iter().zip(b.as_bytes().iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn zeroize_key_bytes(key: &mut [u8; 32]) {
    for b in key.iter_mut() {
        // SAFETY: valid mutable reference within array bounds; volatile prevents dead-store elimination.
        unsafe { std::ptr::write_volatile(b, 0) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

fn read_os_csprng_32() -> Result<[u8; 32], LearnError> {
    let mut key = [0u8; 32];
    #[cfg(unix)]
    {
        use std::io::Read as _;
        let mut f = std::fs::File::open("/dev/urandom").map_err(|_| LearnError::EntropyUnavailable)?;
        f.read_exact(&mut key)
            .map_err(|_| LearnError::EntropyUnavailable)?;
        Ok(key)
    }
    #[cfg(not(unix))]
    {
        Err(LearnError::EntropyUnavailable)
    }
}

fn count_learn_bpe_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    get_cl100k_bpe()
        .map(|bpe| bpe.encode_ordinary(text).len())
        .unwrap_or_else(|| text.len())
}

struct TokenPayloadRef<'a> {
    kid: &'a str,
    actor_kind: ActorKind,
    actor_id: &'a str,
    tenant_id: &'a str,
    project_id: &'a str,
    card_id: &'a str,
    content_sha256: &'a str,
    action: CardDecisionAction,
    expected_revision: u64,
    reason: &'a str,
    issued_at_s: i64,
    expires_at_s: i64,
    nonce: &'a str,
}

fn format_token_signing_payload(p: &TokenPayloadRef<'_>) -> Vec<u8> {
    let kind_str = match p.actor_kind {
        ActorKind::HumanWorkbenchSession => "HUMAN_WORKBENCH_SESSION",
        ActorKind::AgentHmac => "AGENT_HMAC",
        ActorKind::McpClient => "MCP_CLIENT",
        ActorKind::SuperadminCli => "SUPERADMIN_CLI",
    };
    format!(
        "{APPROVER_HMAC_DOMAIN}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        p.kid.len(),
        p.kid,
        kind_str,
        p.actor_id.len(),
        p.actor_id,
        p.tenant_id.len(),
        p.tenant_id,
        p.project_id.len(),
        p.project_id,
        p.card_id.len(),
        p.card_id,
        p.content_sha256.len(),
        p.content_sha256,
        p.action.as_wire_str(),
        p.expected_revision,
        p.reason.len(),
        p.reason,
        p.issued_at_s,
        p.expires_at_s,
        p.nonce.len(),
        p.nonce,
        "end"
    )
    .into_bytes()
}

fn is_valid_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn is_disallowed_control_or_bidi(c: char) -> bool {
    (c.is_control() && c != '\n' && c != '\r' && c != '\t')
        || matches!(
            c as u32,
            0x200B..=0x200F | 0x2028..=0x202E | 0x2066..=0x2069 | 0xFEFF | 0xE0001..=0xE007F
        )
}

fn validate_scope_segment(label: &str, value: &str) -> Result<String, LearnError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_SCOPE_SEGMENT_BYTES {
        return Err(LearnError::InvalidInput(format!(
            "{label} must be 1..={MAX_SCOPE_SEGMENT_BYTES} UTF-8 bytes"
        )));
    }
    if trimmed
        .chars()
        .any(|c| c.is_control() || is_disallowed_control_or_bidi(c))
    {
        return Err(LearnError::InvalidInput(format!(
            "{label} contains disallowed control or bidi characters"
        )));
    }
    Ok(trimmed.to_string())
}

fn validate_card_field(label: &str, value: &str, max_bytes: usize) -> Result<String, LearnError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(LearnError::InvalidInput(format!("{label} must not be empty")));
    }
    if trimmed.len() > max_bytes {
        return Err(LearnError::InvalidInput(format!(
            "{label} exceeds {max_bytes} byte ceiling"
        )));
    }
    if trimmed.chars().any(is_disallowed_control_or_bidi) {
        return Err(LearnError::InvalidInput(format!(
            "{label} contains disallowed control characters"
        )));
    }
    validate_skill_metadata_text(label, trimmed)
        .map_err(|e| LearnError::PromptInjectionBlocked(e.to_string()))?;
    scan_skill_markdown_safety(label, trimmed)
        .map_err(|e| LearnError::PromptInjectionBlocked(e.to_string()))?;
    Ok(trimmed.to_string())
}

fn compute_content_sha256(clean_title: &str, clean_content: &str) -> String {
    let hash_input = format!(
        "{}:{}|{}:{}",
        clean_title.len(),
        clean_title,
        clean_content.len(),
        clean_content
    );
    sha256_hex(hash_input.as_bytes())
}

fn escape_xml_text(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn render_card_xml_block(card: &KnowledgeCard) -> String {
    format!(
        "  <card id=\"{}\" category=\"{}\" tenant=\"{}\" project=\"{}\">\n    <title>{}</title>\n    <evidence>{}</evidence>\n    <content>{}</content>\n  </card>\n",
        escape_xml_text(&card.id),
        card.category.as_wire_str(),
        escape_xml_text(&card.tenant_id),
        escape_xml_text(&card.project_id),
        escape_xml_text(&card.title),
        escape_xml_text(&card.source_evidence),
        escape_xml_text(&card.content),
    )
}

/// Authority bound strictly to Story 08.4's verified HumanWorkbenchSession daemon middleware.
#[derive(Clone)]
pub struct HumanSessionAuthority {
    kid: String,
    signing_key: [u8; 32],
}

impl fmt::Debug for HumanSessionAuthority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HumanSessionAuthority")
            .field("kid", &self.kid)
            .field("signing_key", &"[REDACTED_KEY]")
            .finish()
    }
}

impl Drop for HumanSessionAuthority {
    fn drop(&mut self) {
        zeroize_key_bytes(&mut self.signing_key);
    }
}

impl HumanSessionAuthority {
    pub fn from_key_bytes(signing_key: [u8; 32]) -> Self {
        Self {
            kid: "default".to_string(),
            signing_key,
        }
    }

    pub fn from_key_with_kid(kid: &str, signing_key: [u8; 32]) -> Result<Self, LearnError> {
        let clean_kid = validate_scope_segment("kid", kid)?;
        Ok(Self {
            kid: clean_kid,
            signing_key,
        })
    }

    pub fn try_random_ephemeral() -> Result<Self, LearnError> {
        Ok(Self::from_key_bytes(read_os_csprng_32()?))
    }

    /// Panics only if OS entropy fails; callers requiring fallible handling should use `try_random_ephemeral`.
    pub fn random_ephemeral() -> Self {
        Self::try_random_ephemeral().expect("OS CSPRNG required for HumanSessionAuthority")
    }

    pub fn kid(&self) -> &str {
        &self.kid
    }

    pub fn mint_token(&self, req: &MintTokenInput<'_>) -> Result<HumanApproverToken, LearnError> {
        if req.actor_kind != ActorKind::HumanWorkbenchSession {
            return Err(LearnError::UnauthorizedApprover(
                "Only HUMAN_WORKBENCH_SESSION may mint approval tokens".to_string(),
            ));
        }
        let aid = req.actor_id.trim();
        let tid = req.tenant_id.trim();
        let pid = req.project_id.trim();
        let cid = req.card_id.trim();
        let sha = req.content_sha256.trim();
        let rs = req.reason.trim();
        let n = req.nonce.trim();
        if aid.is_empty()
            || tid.is_empty()
            || pid.is_empty()
            || cid.is_empty()
            || !is_valid_sha256_hex(sha)
            || req.expected_revision < 1
            || rs.len() < 4
            || n.is_empty()
            || req.now_s <= 0
            || req.ttl_s <= 0
            || req.ttl_s > MAX_APPROVAL_TTL_SECONDS
        {
            return Err(LearnError::UnauthorizedApprover(
                "Malformed actor, scope, card binding, reason (<4B), nonce, timestamp, or TTL (1..=900s)".to_string(),
            ));
        }
        let expires_at_s = req
            .now_s
            .checked_add(req.ttl_s)
            .ok_or_else(|| LearnError::UnauthorizedApprover("Timestamp overflow".to_string()))?;
        let payload = format_token_signing_payload(&TokenPayloadRef {
            kid: &self.kid,
            actor_kind: req.actor_kind,
            actor_id: aid,
            tenant_id: tid,
            project_id: pid,
            card_id: cid,
            content_sha256: sha,
            action: req.action,
            expected_revision: req.expected_revision,
            reason: rs,
            issued_at_s: req.now_s,
            expires_at_s,
            nonce: n,
        });
        let hmac_sha256 = hmac_sha256_hex(&self.signing_key, &payload);
        Ok(HumanApproverToken {
            kid: self.kid.clone(),
            actor_kind: req.actor_kind,
            actor_id: aid.to_string(),
            tenant_id: tid.to_string(),
            project_id: pid.to_string(),
            card_id: cid.to_string(),
            content_sha256: sha.to_string(),
            action: req.action,
            expected_revision: req.expected_revision,
            reason: rs.to_string(),
            issued_at_s: req.now_s,
            expires_at_s,
            nonce: n.to_string(),
            hmac_sha256,
        })
    }
}

pub struct LearnPipeline {
    cards: Vec<KnowledgeCard>,
    keys_by_kid: BTreeMap<String, [u8; 32]>,
    consumed_nonces: BTreeMap<(String, String, String), i64>,
}

impl fmt::Debug for LearnPipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LearnPipeline")
            .field("card_count", &self.cards.len())
            .field("kid_count", &self.keys_by_kid.len())
            .field("signing_keys", &"[REDACTED_KEYS]")
            .field("consumed_nonces_count", &self.consumed_nonces.len())
            .finish()
    }
}

impl Drop for LearnPipeline {
    fn drop(&mut self) {
        for key in self.keys_by_kid.values_mut() {
            zeroize_key_bytes(key);
        }
    }
}

impl Default for LearnPipeline {
    fn default() -> Self {
        Self::with_authority(&HumanSessionAuthority::random_ephemeral())
    }
}

impl LearnPipeline {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_authority(authority: &HumanSessionAuthority) -> Self {
        let mut keys_by_kid = BTreeMap::new();
        keys_by_kid.insert(authority.kid.clone(), authority.signing_key);
        Self {
            cards: Vec::new(),
            keys_by_kid,
            consumed_nonces: BTreeMap::new(),
        }
    }

    /// Registers an additional key version (`kid`) to support graceful rotation across daemon restarts.
    pub fn register_authority(&mut self, authority: &HumanSessionAuthority) {
        self.keys_by_kid
            .insert(authority.kid.clone(), authority.signing_key);
    }

    /// Rehydrates persisted SQLite `knowledge_cards` + `learn_consumed_nonces` with full invariant validation.
    pub fn from_persisted_state(
        authority: &HumanSessionAuthority,
        cards: Vec<KnowledgeCard>,
        consumed_nonces: Vec<ConsumedNonceRecord>,
        now_s: i64,
    ) -> Result<Self, LearnError> {
        let mut pipeline = Self::with_authority(authority);
        for card in cards {
            let tid = validate_scope_segment("tenant_id", &card.tenant_id)?;
            let pid = validate_scope_segment("project_id", &card.project_id)?;
            let cid = validate_scope_segment("id", &card.id)?;
            let title = validate_card_field("title", &card.title, MAX_CARD_TITLE_BYTES)?;
            let content = validate_card_field("content", &card.content, MAX_CARD_CONTENT_BYTES)?;
            let source_evidence = validate_card_field(
                "source_evidence",
                &card.source_evidence,
                MAX_CARD_EVIDENCE_BYTES,
            )?;
            let expected_sha = compute_content_sha256(&title, &content);
            if card.revision < 1 || card.content_sha256 != expected_sha {
                return Err(LearnError::InvalidInput(format!(
                    "Persisted card '{cid}' failed revision >= 1 or content_sha256 invariant"
                )));
            }
            if pipeline
                .cards
                .iter()
                .any(|c| c.tenant_id == tid && c.project_id == pid && c.id == cid)
            {
                return Err(LearnError::DuplicateCardId(cid));
            }
            pipeline.cards.push(KnowledgeCard {
                id: cid,
                tenant_id: tid,
                project_id: pid,
                idempotency_key: card
                    .idempotency_key
                    .as_deref()
                    .map(|k| validate_scope_segment("idempotency_key", k))
                    .transpose()?,
                category: card.category,
                title,
                content,
                content_sha256: expected_sha,
                status: card.status,
                source_evidence,
                revision: card.revision,
                reviewed_by: card.reviewed_by,
                decision_reason: card.decision_reason,
                created_at: card.created_at,
                accepted_at: card.accepted_at,
            });
        }
        for rec in consumed_nonces {
            if rec.expires_at_s >= now_s {
                let tid = validate_scope_segment("nonce.tenant_id", &rec.tenant_id)?;
                let pid = validate_scope_segment("nonce.project_id", &rec.project_id)?;
                let nonce = validate_scope_segment("nonce", &rec.nonce)?;
                pipeline
                    .consumed_nonces
                    .insert((tid, pid, nonce), rec.expires_at_s);
            }
        }
        Ok(pipeline)
    }

    pub fn export_consumed_nonces(&self) -> Vec<ConsumedNonceRecord> {
        self.consumed_nonces
            .iter()
            .map(|((tid, pid, nonce), &expires_at_s)| ConsumedNonceRecord {
                tenant_id: tid.clone(),
                project_id: pid.clone(),
                nonce: nonce.clone(),
                expires_at_s,
            })
            .collect()
    }

    pub fn prune_expired_nonces(&mut self, now_s: i64) -> usize {
        let before = self.consumed_nonces.len();
        self.consumed_nonces
            .retain(|_, &mut expires_at_s| expires_at_s >= now_s);
        before - self.consumed_nonces.len()
    }

    pub fn propose_card_scoped(
        &mut self,
        input: &ProposeCardInput<'_>,
    ) -> Result<&KnowledgeCard, LearnError> {
        let tid = validate_scope_segment("tenant_id", input.tenant_id)?;
        let pid = validate_scope_segment("project_id", input.project_id)?;
        let cid = validate_scope_segment("id", input.id)?;
        let idem_key = input
            .idempotency_key
            .map(|k| validate_scope_segment("idempotency_key", k))
            .transpose()?;

        let clean_title = validate_card_field("title", input.title, MAX_CARD_TITLE_BYTES)?;
        let clean_content = validate_card_field("content", input.content, MAX_CARD_CONTENT_BYTES)?;
        let clean_evidence = validate_card_field(
            "source_evidence",
            input.source_evidence,
            MAX_CARD_EVIDENCE_BYTES,
        )?;
        let content_sha256 = compute_content_sha256(&clean_title, &clean_content);

        if let Some(ref key) = idem_key {
            if let Some(existing) = self.cards.iter().find(|c| {
                c.tenant_id == tid
                    && c.project_id == pid
                    && c.idempotency_key.as_deref() == Some(key.as_str())
            }) {
                if existing.id == cid
                    && existing.category == input.category
                    && existing.content_sha256 == content_sha256
                {
                    return Err(LearnError::IdempotentReplay(existing.id.clone()));
                }
                return Err(LearnError::IdempotencyPayloadMismatch);
            }
        }

        let scope_count = self
            .cards
            .iter()
            .filter(|c| c.tenant_id == tid && c.project_id == pid)
            .count();
        if scope_count >= MAX_CARDS_PER_SCOPE {
            return Err(LearnError::CapacityExceeded(format!(
                "Scope ({tid}, {pid}) reached {MAX_CARDS_PER_SCOPE} card limit"
            )));
        }

        if self
            .cards
            .iter()
            .any(|c| c.tenant_id == tid && c.project_id == pid && c.id == cid)
        {
            return Err(LearnError::DuplicateCardId(cid));
        }
        if self.cards.iter().any(|c| {
            c.tenant_id == tid
                && c.project_id == pid
                && c.content_sha256 == content_sha256
                && c.status != CardStatus::Rejected
        }) {
            return Err(LearnError::DuplicateActiveContent);
        }

        self.cards.push(KnowledgeCard {
            id: cid,
            tenant_id: tid,
            project_id: pid,
            idempotency_key: idem_key,
            category: input.category,
            title: clean_title,
            content: clean_content,
            content_sha256,
            status: CardStatus::Proposed,
            source_evidence: clean_evidence,
            revision: 1,
            reviewed_by: None,
            decision_reason: None,
            created_at: input.created_at,
            accepted_at: None,
        });
        Ok(self.cards.last().expect("pushed card"))
    }

    /// Synthesizes a failed `GateReport` + `LintIssue` findings into a `PROPOSED` knowledge card (AC-04).
    pub fn propose_card_from_gate_report(
        &mut self,
        tenant_id: &str,
        project_id: &str,
        id: &str,
        idempotency_key: Option<&str>,
        gate_report: &GateReport,
        issues: &[LintIssue],
    ) -> Result<&KnowledgeCard, LearnError> {
        if gate_report.passed {
            return Err(LearnError::InvalidInput(
                "Cannot synthesize failure card from a passed GateReport".to_string(),
            ));
        }
        let title = format!("Gate failure remediation: {}", gate_report.gate_name.trim());
        let truncated_title: String = title.chars().take(120).collect();
        let issue_summary = if issues.is_empty() {
            format!(
                "Gate '{}' reported failure by checker '{}' (evidence {}).",
                gate_report.gate_name.trim(),
                gate_report.checker_id.trim(),
                gate_report.evidence_hash.trim()
            )
        } else {
            issues
                .iter()
                .take(6)
                .map(|iss| format!("[{}] {}", iss.rule_id.trim(), iss.message.trim()))
                .collect::<Vec<_>>()
                .join("; ")
        };
        let evidence = format!(
            "gate:{}:{}",
            gate_report.gate_name.trim(),
            gate_report.evidence_hash.trim()
        );
        self.propose_card_scoped(&ProposeCardInput {
            tenant_id,
            project_id,
            id,
            idempotency_key,
            category: LearnCardCategory::AntiPattern,
            title: &truncated_title,
            content: &issue_summary,
            source_evidence: &evidence,
            created_at: *gate_report.timestamp,
        })
    }

    fn verify_token_signature_and_ttl(
        &self,
        tenant_id: &str,
        project_id: &str,
        token: &HumanApproverToken,
        now_s: i64,
    ) -> Result<(), LearnError> {
        if token.actor_kind != ActorKind::HumanWorkbenchSession {
            return Err(LearnError::UnauthorizedApprover(
                "HUMAN_WORKBENCH_SESSION required".to_string(),
            ));
        }
        if [
            token.kid.as_str(),
            token.actor_id.as_str(),
            token.tenant_id.as_str(),
            token.project_id.as_str(),
            token.card_id.as_str(),
            token.nonce.as_str(),
        ]
        .iter()
        .any(|f| f.is_empty() || *f != f.trim())
            || token.reason != token.reason.trim()
            || token.reason.len() < 4
            || !is_valid_sha256_hex(&token.content_sha256)
            || token.expected_revision < 1
        {
            return Err(LearnError::UnauthorizedApprover(
                "Malformed or non-canonical token metadata/whitespace".to_string(),
            ));
        }
        if token.tenant_id != tenant_id || token.project_id != project_id {
            return Err(LearnError::UnauthorizedApprover(
                "HumanApproverToken tenant/project scope mismatch".to_string(),
            ));
        }
        let ttl_span = token.expires_at_s.checked_sub(token.issued_at_s);
        if now_s <= 0
            || token.issued_at_s <= 0
            || ttl_span.is_none_or(|d| !(1..=MAX_APPROVAL_TTL_SECONDS).contains(&d))
            || now_s < token.issued_at_s
            || now_s > token.expires_at_s
        {
            return Err(LearnError::UnauthorizedApprover(
                "Approval token expired, not yet valid, or bounded TTL violated".to_string(),
            ));
        }
        let signing_key = self.keys_by_kid.get(&token.kid).ok_or_else(|| {
            LearnError::UnauthorizedApprover(format!("Unknown signing kid '{}'", token.kid))
        })?;
        let payload = format_token_signing_payload(&TokenPayloadRef {
            kid: &token.kid,
            actor_kind: token.actor_kind,
            actor_id: &token.actor_id,
            tenant_id: &token.tenant_id,
            project_id: &token.project_id,
            card_id: &token.card_id,
            content_sha256: &token.content_sha256,
            action: token.action,
            expected_revision: token.expected_revision,
            reason: &token.reason,
            issued_at_s: token.issued_at_s,
            expires_at_s: token.expires_at_s,
            nonce: &token.nonce,
        });
        let expected = hmac_sha256_hex(signing_key, &payload);
        if !ct_eq_hex(&expected, &token.hmac_sha256) {
            return Err(LearnError::UnauthorizedApprover(
                "Forged or mismatched HumanApproverToken HMAC signature".to_string(),
            ));
        }
        Ok(())
    }

    fn decide_card(
        &mut self,
        scope: (&str, &str),
        id: &str,
        action: CardDecisionAction,
        token: &HumanApproverToken,
        expected_revision: u64,
        now_s: i64,
    ) -> Result<(), LearnError> {
        let tid = scope.0.trim();
        let pid = scope.1.trim();
        let cid = id.trim();
        // Step 1: Verify cryptographic token & time bounds first (prevents unauthenticated oracles).
        self.verify_token_signature_and_ttl(tid, pid, token, now_s)?;

        // Step 2: Check replay before card lookup without mutating consumed_nonces yet.
        let nonce_key = (tid.to_string(), pid.to_string(), token.nonce.clone());
        if self.consumed_nonces.contains_key(&nonce_key) {
            return Err(LearnError::ReplayedApprovalNonce(token.nonce.clone()));
        }

        // Step 3: Validate card state, CAS revision, and token-to-card/action bindings.
        let target = action.target_status();
        let card_idx = self
            .cards
            .iter()
            .position(|c| c.id == cid && c.tenant_id == tid && c.project_id == pid)
            .ok_or_else(|| LearnError::CardNotFound(cid.to_string()))?;

        {
            let card = &self.cards[card_idx];
            if card.status != CardStatus::Proposed {
                return Err(LearnError::InvalidCardTransition {
                    from: card.status,
                    to: target,
                });
            }
            if card.revision != expected_revision {
                return Err(LearnError::RevisionConflict {
                    expected: expected_revision,
                    actual: card.revision,
                });
            }
            if now_s < card.created_at {
                return Err(LearnError::UnauthorizedApprover(
                    "Decision timestamp precedes card creation".to_string(),
                ));
            }
            if token.card_id != card.id
                || token.content_sha256 != card.content_sha256
                || token.action != action
                || token.expected_revision != expected_revision
            {
                return Err(LearnError::UnauthorizedApprover(
                    "HumanApproverToken binding mismatch (card_id, content_sha256, action, or revision)".to_string(),
                ));
            }
        }

        // Step 4: Prune expired nonces and check scope cap; consume nonce only after all checks pass.
        self.prune_expired_nonces(now_s);
        let scope_nonces = self
            .consumed_nonces
            .keys()
            .filter(|(t, p, _)| t == tid && p == pid)
            .count();
        if scope_nonces >= MAX_NONCES_PER_SCOPE {
            return Err(LearnError::CapacityExceeded(format!(
                "Scope ({tid}, {pid}) reached {MAX_NONCES_PER_SCOPE} active nonce limit"
            )));
        }
        self.consumed_nonces.insert(nonce_key, token.expires_at_s);

        let card = &mut self.cards[card_idx];
        card.status = target;
        card.reviewed_by = Some(token.actor_id.clone());
        card.decision_reason = Some(token.reason.clone());
        card.revision += 1;
        if target == CardStatus::Accepted {
            card.accepted_at = Some(now_s);
        }
        Ok(())
    }

    pub fn accept_card_verified(
        &mut self,
        tenant_id: &str,
        project_id: &str,
        id: &str,
        token: &HumanApproverToken,
        expected_revision: u64,
        now_s: i64,
    ) -> Result<(), LearnError> {
        self.decide_card(
            (tenant_id, project_id),
            id,
            CardDecisionAction::Accept,
            token,
            expected_revision,
            now_s,
        )
    }

    pub fn reject_card_verified(
        &mut self,
        tenant_id: &str,
        project_id: &str,
        id: &str,
        token: &HumanApproverToken,
        expected_revision: u64,
        now_s: i64,
    ) -> Result<(), LearnError> {
        self.decide_card(
            (tenant_id, project_id),
            id,
            CardDecisionAction::Reject,
            token,
            expected_revision,
            now_s,
        )
    }

    pub fn list_cards_for_scope(&self, tenant_id: &str, project_id: &str) -> Vec<&KnowledgeCard> {
        let tid = tenant_id.trim();
        let pid = project_id.trim();
        self.cards
            .iter()
            .filter(|c| c.tenant_id == tid && c.project_id == pid)
            .collect()
    }

    /// Packs ONLY ACCEPTED cards for exact (tenant_id, project_id) within <= 1,500 exact cl100k_base BPE tokens.
    pub fn render_prompt_fenced_cards(
        &self,
        tenant_id: &str,
        project_id: &str,
        max_tokens: usize,
    ) -> PromptInjectionResult {
        let tid = tenant_id.trim();
        let pid = project_id.trim();
        let effective_limit = max_tokens.min(MAX_LEARN_PROMPT_TOKENS);
        let mut eligible: Vec<&KnowledgeCard> = self
            .cards
            .iter()
            .filter(|c| {
                c.status == CardStatus::Accepted && c.tenant_id == tid && c.project_id == pid
            })
            .collect();
        eligible.sort_by(|a, b| a.created_at.cmp(&b.created_at).then_with(|| a.id.cmp(&b.id)));

        let mut selected_ids = Vec::new();
        let mut body = String::new();
        for card in eligible {
            let block = render_card_xml_block(card);
            let candidate_xml = format!("<LEARNED_CARDS_DATA>\n{body}{block}</LEARNED_CARDS_DATA>");
            if count_learn_bpe_tokens(&candidate_xml) <= effective_limit {
                body.push_str(&block);
                selected_ids.push(card.id.clone());
            }
        }
        let fenced_xml = if selected_ids.is_empty() {
            String::new()
        } else {
            format!("<LEARNED_CARDS_DATA>\n{body}</LEARNED_CARDS_DATA>")
        };
        let total_tokens = count_learn_bpe_tokens(&fenced_xml);
        PromptInjectionResult {
            card_ids: selected_ids,
            total_tokens,
            fenced_xml,
        }
    }

    /// Returns scoped ACCEPTED metadata cards matching the fenced prompt pack; prompt assembly must use `render_prompt_fenced_cards`.
    pub fn get_active_prompt_injections(
        &self,
        tenant_id: &str,
        project_id: &str,
        max_tokens: usize,
    ) -> Vec<&KnowledgeCard> {
        let tid = tenant_id.trim();
        let pid = project_id.trim();
        let rendered = self.render_prompt_fenced_cards(tid, pid, max_tokens);
        rendered
            .card_ids
            .iter()
            .filter_map(|id| {
                self.cards
                    .iter()
                    .find(|c| c.tenant_id == tid && c.project_id == pid && c.id == *id)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_unapproved_poisoning_guard() {
        let auth = HumanSessionAuthority::from_key_bytes([7u8; 32]);
        let mut pipeline = LearnPipeline::with_authority(&auth);

        let proposed = pipeline
            .propose_card_scoped(&ProposeCardInput {
                tenant_id: "tenant-a",
                project_id: "proj-1",
                id: "card-01",
                idempotency_key: None,
                category: LearnCardCategory::AntiPattern,
                title: "No Purple Gradients",
                content: "Do not use #6366f1 as default gradient.",
                source_evidence: "gate-history.jsonl:line 42",
                created_at: 1_700_000_000,
            })
            .unwrap()
            .clone();

        assert!(pipeline
            .get_active_prompt_injections("tenant-a", "proj-1", MAX_LEARN_PROMPT_TOKENS)
            .is_empty());

        let token = auth
            .mint_token(&MintTokenInput {
                actor_kind: ActorKind::HumanWorkbenchSession,
                actor_id: "user-1",
                tenant_id: "tenant-a",
                project_id: "proj-1",
                card_id: "card-01",
                content_sha256: &proposed.content_sha256,
                action: CardDecisionAction::Accept,
                expected_revision: 1,
                reason: "Verified contrast issue",
                now_s: 1_700_000_010,
                ttl_s: 300,
                nonce: "nonce-1",
            })
            .unwrap();
        pipeline
            .accept_card_verified("tenant-a", "proj-1", "card-01", &token, 1, 1_700_000_020)
            .unwrap();

        let active = pipeline.get_active_prompt_injections("tenant-a", "proj-1", 1_500);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "card-01");
    }

    #[test]
    fn test_rejected_cards_never_injected() {
        let auth = HumanSessionAuthority::from_key_bytes([9u8; 32]);
        let mut pipeline = LearnPipeline::with_authority(&auth);
        let proposed = pipeline
            .propose_card_scoped(&ProposeCardInput {
                tenant_id: "tenant-a",
                project_id: "proj-1",
                id: "card-02",
                idempotency_key: None,
                category: LearnCardCategory::Rule,
                title: "Dubious Rule",
                content: "Use red background everywhere",
                source_evidence: "unverified suggestion",
                created_at: 1_700_000_000,
            })
            .unwrap()
            .clone();

        let token = auth
            .mint_token(&MintTokenInput {
                actor_kind: ActorKind::HumanWorkbenchSession,
                actor_id: "user-1",
                tenant_id: "tenant-a",
                project_id: "proj-1",
                card_id: "card-02",
                content_sha256: &proposed.content_sha256,
                action: CardDecisionAction::Reject,
                expected_revision: 1,
                reason: "Rejected hallucinated rule",
                now_s: 1_700_000_010,
                ttl_s: 300,
                nonce: "nonce-2",
            })
            .unwrap();
        pipeline
            .reject_card_verified("tenant-a", "proj-1", "card-02", &token, 1, 1_700_000_020)
            .unwrap();
        assert!(pipeline
            .get_active_prompt_injections("tenant-a", "proj-1", 1_500)
            .is_empty());
    }
}
