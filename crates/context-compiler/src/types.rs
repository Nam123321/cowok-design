// @story 02.1
// @implements AC-01, AC-02, AC-03, AC-04
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const HTTP_BAD_REQUEST: u16 = 400;
pub const HTTP_FORBIDDEN: u16 = 403;
pub const HTTP_CONFLICT: u16 = 409;
pub const HTTP_UNPROCESSABLE_ENTITY: u16 = 422;
pub const HTTP_SERVICE_UNAVAILABLE: u16 = 503;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PortalId {
    SuperAdmin,
    TenantAdmin,
    SalesApp,
    MarketingWeb,
    DeveloperPortal,
    Presentation,
}

impl PortalId {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SuperAdmin => "SuperAdmin",
            Self::TenantAdmin => "TenantAdmin",
            Self::SalesApp => "SalesApp",
            Self::MarketingWeb => "MarketingWeb",
            Self::DeveloperPortal => "DeveloperPortal",
            Self::Presentation => "Presentation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Horizon {
    Short,
    Mid,
    Long,
}

impl Horizon {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Short => "short",
            Self::Mid => "mid",
            Self::Long => "long",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelProfile {
    #[serde(rename = "A", alias = "ProfileA_Frontier")]
    A,
    #[serde(rename = "B", alias = "ProfileB_LocalSmall")]
    B,
    #[serde(rename = "C", alias = "ProfileC_OfflineFTS5")]
    C,
}

impl ModelProfile {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
        }
    }

    pub fn adapter_mode(&self) -> AdapterMode {
        match self {
            Self::A => AdapterMode::FrontierFull,
            Self::B => AdapterMode::LocalReduced,
            Self::C => AdapterMode::OfflineFts5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterMode {
    FrontierFull,
    LocalReduced,
    OfflineFts5,
}

impl AdapterMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FrontierFull => "frontier_full",
            Self::LocalReduced => "local_reduced",
            Self::OfflineFts5 => "offline_fts5",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextReasonCode {
    AtomicComplete,
    PrunedByBudget,
}

impl ContextReasonCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AtomicComplete => "atomic_complete",
            Self::PrunedByBudget => "pruned_by_budget",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockFormat {
    Html,
    Json,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DomOutlineNode {
    #[serde(alias = "ff_id")]
    pub ff_id: String,
    pub tag: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionalContextBlock {
    pub id: String,
    pub format: BlockFormat,
    pub content: String,
    #[serde(default)]
    pub priority: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContextAssembleHttpRequest {
    pub project_id: Uuid,
    pub portal_id: PortalId,
    pub horizon: Horizon,
    pub model_profile: ModelProfile,
    pub intent: String,
    pub requested_image_tokens: usize,
    #[serde(default)]
    pub source_ids: Option<Vec<Uuid>>,
    #[serde(default)]
    pub dom_outline_nodes: Option<Vec<DomOutlineNode>>,
    #[serde(default)]
    pub inspect_only: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ContextAssemblyCommand {
    pub project_id: Uuid,
    pub tenant_id: String,
    pub idempotency_key: Option<String>,
    pub portal_id: PortalId,
    pub horizon: Horizon,
    pub model_profile: ModelProfile,
    pub intent: String,
    pub requested_image_tokens: usize,
    pub max_image_tokens_per_turn: usize,
    pub source_ids: Option<Vec<Uuid>>,
    pub inspect_only: bool,
    pub brand_tokens_prefix: String,
    pub active_skill_prefix: String,
    pub extra_constraints: Vec<String>,
    pub dom_outline_nodes: Vec<DomOutlineNode>,
    pub optional_blocks: Vec<OptionalContextBlock>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TokenUsageDto {
    pub static_prefix: usize,
    pub dynamic_tail: usize,
    pub outline: usize,
    pub text_total: usize,
    pub image: usize,
    pub ssot_dom: usize,
    pub non_ssot_text: usize,
    pub ssot_dom_ratio: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SourcesSummaryDto {
    pub requested: usize,
    pub included: usize,
    pub pruned: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContextAssembleDataDto {
    pub telemetry_id: String,
    pub project_id: String,
    pub portal_id: PortalId,
    pub horizon: Horizon,
    pub model_profile: ModelProfile,
    pub adapter_mode: AdapterMode,
    pub reason_code: ContextReasonCode,
    pub static_prefix_sha256: String,
    pub token_usage: TokenUsageDto,
    pub sources: SourcesSummaryDto,
    pub latency_ms: u64,
    pub assembled_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContextAssembleSuccessDto {
    pub ok: bool,
    pub data: ContextAssembleDataDto,
    pub request_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContextTelemetryTokenUsageDto {
    pub outline: usize,
    pub text_total: usize,
    pub image: usize,
    pub ssot_dom_ratio: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContextTelemetryItemDto {
    pub telemetry_id: String,
    pub project_id: String,
    pub portal_id: PortalId,
    pub horizon: Horizon,
    pub model_profile: ModelProfile,
    pub adapter_mode: AdapterMode,
    pub reason_code: ContextReasonCode,
    pub static_prefix_sha256: String,
    pub token_usage: ContextTelemetryTokenUsageDto,
    pub latency_ms: u64,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContextTelemetryPageMetaDto {
    pub limit: usize,
    pub next_cursor: Option<String>,
    pub order: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContextTelemetryPageDataDto {
    pub items: Vec<ContextTelemetryItemDto>,
    pub page: ContextTelemetryPageMetaDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContextTelemetryPageDto {
    pub ok: bool,
    pub data: ContextTelemetryPageDataDto,
    pub request_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContextTelemetryQueryDto {
    pub project_id: Uuid,
    #[serde(default)]
    pub portal_id: Option<PortalId>,
    #[serde(default)]
    pub model_profile: Option<ModelProfile>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextApiErrorDto {
    pub ok: bool,
    pub error: ContextErrorDetail,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledContextBundle {
    pub data: ContextAssembleDataDto,
    pub static_prefix: String,
    pub dynamic_tail: String,
    pub outline: String,
    pub downgraded_to_micro_tools: bool,
    pub adapted_tool_count: usize,
    pub offline_fallback_active: bool,
    pub adapted_tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectPromptPayload {
    pub prefix_sha256: String,
    pub static_prefix: String,
    pub dynamic_tail: String,
    pub outline: String,
    pub outline_tokens: usize,
    pub total_text_tokens: usize,
    pub image_tokens: usize,
    pub ssot_dom_ratio: f64,
    pub reason_code: ContextReasonCode,
    pub atomic_complete: bool,
    pub profile: ModelProfile,
    pub adapter_mode: AdapterMode,
    pub downgraded_to_micro_tools: bool,
    pub adapted_tool_count: usize,
    pub offline_fallback_active: bool,
}

#[derive(Debug, Error)]
pub enum ContextCompilerError {
    #[error("INVALID_REQUEST: {message}")]
    InvalidRequest {
        message: String,
        field: Option<String>,
    },
    #[error("INVALID_CURSOR")]
    InvalidCursor,
    #[error("PROJECT_TENANT_MISMATCH")]
    ProjectTenantMismatch,
    #[error("CROSS_PORTAL_LEAK_BLOCKED: {reason}")]
    CrossPortalLeakBlocked { reason: String },
    #[error("IMAGE_TOKEN_BUDGET_EXCEEDED: requested={requested} cap={cap}")]
    ImageTokenBudgetExceeded { requested: usize, cap: usize },
    #[error("ATOMIC_BLOCK_OVER_BUDGET: {reason}")]
    AtomicBlockOverBudget { reason: String },
    #[error("SSOT_DOM_RATIO_BELOW_MINIMUM: ratio={ratio:.2}")]
    SsotDomRatioBelowMinimum { ratio: f64 },
    #[error("IDEMPOTENCY_KEY_REUSED")]
    IdempotencyKeyReused,
    #[error("TELEMETRY_PROJECT_REFERENCE_INVALID")]
    TelemetryProjectReferenceInvalid,
    #[error("TELEMETRY_CONFLICT")]
    TelemetryConflict,
    #[error("TELEMETRY_WRITE_BUSY")]
    TelemetryWriteBusy,
    #[error("TELEMETRY_STORAGE_UNAVAILABLE: {reason}")]
    TelemetryStorageUnavailable { reason: String },
}

impl ContextCompilerError {
    pub fn descriptor(&self) -> (u16, &'static str, &'static str, Option<&str>, bool) {
        match self {
            Self::InvalidRequest { field, .. } => (
                HTTP_UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                "contextMeter.errors.invalidRequest",
                field.as_deref(),
                false,
            ),
            Self::InvalidCursor => (
                HTTP_UNPROCESSABLE_ENTITY,
                "INVALID_CURSOR",
                "contextMeter.errors.invalidCursor",
                Some("cursor"),
                false,
            ),
            Self::ProjectTenantMismatch => (
                HTTP_FORBIDDEN,
                "PROJECT_TENANT_MISMATCH",
                "contextMeter.errors.projectTenantMismatch",
                None,
                false,
            ),
            Self::CrossPortalLeakBlocked { .. } => (
                HTTP_FORBIDDEN,
                "CROSS_PORTAL_LEAK_BLOCKED",
                "contextMeter.errors.crossPortalLeak",
                None,
                false,
            ),
            Self::ImageTokenBudgetExceeded { .. } => (
                HTTP_UNPROCESSABLE_ENTITY,
                "IMAGE_TOKEN_BUDGET_EXCEEDED",
                "contextMeter.errors.imageBudgetExceeded",
                Some("requestedImageTokens"),
                false,
            ),
            Self::AtomicBlockOverBudget { .. } => (
                HTTP_UNPROCESSABLE_ENTITY,
                "ATOMIC_BLOCK_OVER_BUDGET",
                "contextMeter.errors.atomicBlockOverBudget",
                None,
                false,
            ),
            Self::SsotDomRatioBelowMinimum { .. } => (
                HTTP_UNPROCESSABLE_ENTITY,
                "SSOT_DOM_RATIO_BELOW_MINIMUM",
                "contextMeter.errors.ssotRatioBelowMinimum",
                None,
                false,
            ),
            Self::IdempotencyKeyReused => (
                HTTP_CONFLICT,
                "IDEMPOTENCY_KEY_REUSED",
                "contextMeter.errors.idempotencyKeyReused",
                None,
                false,
            ),
            Self::TelemetryProjectReferenceInvalid => (
                HTTP_CONFLICT,
                "TELEMETRY_PROJECT_REFERENCE_INVALID",
                "contextMeter.errors.projectReferenceInvalid",
                None,
                false,
            ),
            Self::TelemetryConflict => (
                HTTP_CONFLICT,
                "TELEMETRY_CONFLICT",
                "contextMeter.errors.telemetryConflict",
                None,
                false,
            ),
            Self::TelemetryWriteBusy => (
                HTTP_SERVICE_UNAVAILABLE,
                "TELEMETRY_WRITE_BUSY",
                "contextMeter.errors.telemetryWriteBusy",
                None,
                true,
            ),
            Self::TelemetryStorageUnavailable { .. } => (
                HTTP_SERVICE_UNAVAILABLE,
                "TELEMETRY_STORAGE_UNAVAILABLE",
                "contextMeter.errors.telemetryStorageUnavailable",
                None,
                true,
            ),
        }
    }
}
