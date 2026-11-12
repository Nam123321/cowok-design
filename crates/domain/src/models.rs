use serde::de::{self, Deserializer};
use serde::ser::{self, Serializer};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "ts")]
use ts_rs::TS;

pub use crate::error::HarnessError;
pub use crate::safe_integer::SafeJsInteger;
pub use crate::spec_key::CanonicalSpecKey;

fn serialize_positive_revision<S>(val: &u32, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if *val == 0 || *val > i32::MAX as u32 {
        return Err(ser::Error::custom(
            "SpecRevision.revision_number must be in 1..=2147483647",
        ));
    }
    serializer.serialize_u32(*val)
}

fn deserialize_positive_revision<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let val = u32::deserialize(deserializer)?;
    if val == 0 || val > i32::MAX as u32 {
        return Err(de::Error::custom(
            "SpecRevision.revision_number must be in 1..=2147483647",
        ));
    }
    Ok(val)
}

#[allow(clippy::ptr_arg)]
fn serialize_sha256_hash<S>(val: &String, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let hex_part = val
        .strip_prefix("sha256-")
        .or_else(|| val.strip_prefix("sha256:"))
        .unwrap_or(val.as_str());
    if hex_part.len() != 64 || !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ser::Error::custom(
            "SpecRevision.content_hash must be a valid 64-character hex SHA-256 digest",
        ));
    }
    serializer.serialize_str(val)
}

fn deserialize_sha256_hash<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let val = String::deserialize(deserializer)?;
    let hex_part = val
        .strip_prefix("sha256-")
        .or_else(|| val.strip_prefix("sha256:"))
        .unwrap_or(&val);
    if hex_part.len() != 64 || !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(de::Error::custom(
            "SpecRevision.content_hash must be a valid 64-character hex SHA-256 digest",
        ));
    }
    Ok(val)
}

#[allow(clippy::ptr_arg)]
fn serialize_non_empty_string<S>(val: &String, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if val.is_empty() {
        return Err(ser::Error::custom("String field must not be empty"));
    }
    serializer.serialize_str(val)
}

fn deserialize_non_empty_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let val = String::deserialize(deserializer)?;
    if val.is_empty() {
        return Err(de::Error::custom("String field must not be empty"));
    }
    Ok(val)
}

/// Status of context extraction and provenance traceability.
/// Wire values are strictly SCREAMING_SNAKE_CASE: CONFIRMED, INFERRED, MISSING.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub enum ProvenanceStatus {
    Confirmed,
    Inferred,
    Missing,
}

/// Project root aggregate (P1 Persistence Entity)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct Project {
    pub id: Uuid,
    #[serde(
        serialize_with = "serialize_non_empty_string",
        deserialize_with = "deserialize_non_empty_string"
    )]
    pub name: String,
    #[serde(
        serialize_with = "serialize_non_empty_string",
        deserialize_with = "deserialize_non_empty_string"
    )]
    pub slug: String,
    pub created_at: SafeJsInteger,
    pub updated_at: SafeJsInteger,
}

/// Canonical spec bound to a project (P1 Persistence Entity)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct ProjectSpec {
    pub id: Uuid,
    pub project_id: Uuid,
    pub spec_key: CanonicalSpecKey,
    pub status: ProvenanceStatus,
    pub updated_at: SafeJsInteger,
}

/// Immutable historical revision of a project spec (P1 Persistence Entity)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct SpecRevision {
    pub id: Uuid,
    pub spec_id: Uuid,
    #[serde(
        serialize_with = "serialize_positive_revision",
        deserialize_with = "deserialize_positive_revision"
    )]
    pub revision_number: u32,
    #[serde(
        serialize_with = "serialize_sha256_hash",
        deserialize_with = "deserialize_sha256_hash"
    )]
    pub content_hash: String,
    pub created_at: SafeJsInteger,
}

impl SpecRevision {
    /// Creates a new validated `SpecRevision` enforcing `revision_number >= 1`
    /// and a 64-character hexadecimal SHA-256 `content_hash`.
    pub fn new(
        id: Uuid,
        spec_id: Uuid,
        revision_number: u32,
        content_hash: impl Into<String>,
        created_at: SafeJsInteger,
    ) -> Result<Self, HarnessError> {
        let rev = Self {
            id,
            spec_id,
            revision_number,
            content_hash: content_hash.into(),
            created_at,
        };
        rev.validate()?;
        Ok(rev)
    }

    /// Validates domain invariants for `SpecRevision`:
    /// 1. `revision_number` must be in `1..=i32::MAX` (2,147,483,647).
    /// 2. `content_hash` must be a valid 64-character ASCII hex SHA-256 digest.
    pub fn validate(&self) -> Result<(), HarnessError> {
        if self.revision_number == 0 || self.revision_number > i32::MAX as u32 {
            return Err(HarnessError::ValidationFailed {
                reason: "SpecRevision.revision_number must be in 1..=2147483647".to_string(),
            });
        }
        let hex_part = self
            .content_hash
            .strip_prefix("sha256-")
            .or_else(|| self.content_hash.strip_prefix("sha256:"))
            .unwrap_or(&self.content_hash);
        if hex_part.len() != 64 || !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(HarnessError::ValidationFailed {
                reason: "SpecRevision.content_hash must be a valid 64-character hex SHA-256 digest"
                    .to_string(),
            });
        }
        Ok(())
    }
}

/// Spec provenance summary representing completeness across the 9 canonical keys
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct SpecProvenanceSummary {
    pub project_id: Uuid,
    pub confirmed_count: u32,
    pub inferred_count: u32,
    pub missing_count: u32,
    pub overall_status: ProvenanceStatus,
}

/// Captured Design Intent V1 from brief or prototype
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct DesignIntentV1 {
    pub id: Uuid,
    pub project_id: Uuid,
    pub visual_style: String,
    pub color_palette: Vec<String>,
    pub typography: Vec<String>,
    pub key_principles: Vec<String>,
}

/// Sandboxed ChangeProposal contract for code modifications and validation gates
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct ChangeProposalV1 {
    pub id: Uuid,
    pub tenant_id: String,
    pub project_id: String,
    pub intent_id: Option<Uuid>,
    pub title: String,
    pub description: String,
    pub validation_status: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Project-wide constraint profile for SDLC rule enforcement
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct ConstraintProfile {
    pub id: Uuid,
    pub project_id: Uuid,
    pub max_complexity_score: u32,
    pub enforce_clean_code: bool,
    pub forbidden_patterns: Vec<String>,
    pub target_framework: String,
}

/// Deprecated wire alias for ConstraintProfile
pub type ProjectConstraintProfile = ConstraintProfile;

/// Screen state node in interactive user journey
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct ScreenState {
    pub id: String,
    pub name: String,
    pub route_path: String,
    pub components: Vec<String>,
}

/// State transition edge in interactive flow
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct Transition {
    pub id: String,
    pub from_state: String,
    pub to_state: String,
    pub trigger_action: String,
}

/// Directed user flow graph
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct FlowGraph {
    pub id: Uuid,
    pub project_id: Uuid,
    pub states: Vec<ScreenState>,
    pub transitions: Vec<Transition>,
}

/// Multi-slide presentation deck generated from design specs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct PresentationDeck {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub slides_count: u32,
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub last_exported_at: Option<SafeJsInteger>,
}

/// Domain contract for external skill plugin integration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct SkillPlugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub entry_point: String,
    pub permissions: Vec<String>,
}

/// Asynchronous background worker task in the harness bot fleet
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct BotJob {
    pub id: Uuid,
    pub task_type: String,
    pub payload_hash: String,
    pub status: String,
    pub enqueued_at: SafeJsInteger,
}

/// Deprecated wire alias for BotJob
pub type FleetTask = BotJob;

/// Zero-Trust gate verification report
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct GateReport {
    pub gate_name: String,
    pub passed: bool,
    pub checker_id: String,
    pub timestamp: SafeJsInteger,
    pub evidence_hash: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ut_rev_01_accepts_valid_spec_revision() {
        let rev = SpecRevision::new(
            Uuid::nil(),
            Uuid::nil(),
            1,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            SafeJsInteger::new(1_730_500_000).unwrap(),
        );
        assert!(rev.is_ok());
    }

    #[test]
    fn ut_rev_02_rejects_zero_revision_number() {
        let rev = SpecRevision::new(
            Uuid::nil(),
            Uuid::nil(),
            0,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            SafeJsInteger::new(1_730_500_000).unwrap(),
        );
        assert!(matches!(rev, Err(HarnessError::ValidationFailed { .. })));
    }

    #[test]
    fn ut_rev_03_rejects_non_sha256_content_hash() {
        let rev = SpecRevision::new(
            Uuid::nil(),
            Uuid::nil(),
            1,
            "invalid-short-hash",
            SafeJsInteger::new(1_730_500_000).unwrap(),
        );
        assert!(matches!(rev, Err(HarnessError::ValidationFailed { .. })));
    }
}
