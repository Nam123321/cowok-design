use serde::de::{self, Deserializer};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "ts")]
use ts_rs::TS;

fn serialize_unsafe_i64_as_string<S>(val: &i64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(val) {
        return Err(serde::ser::Error::custom(format!(
            "UNSAFE_INTEGER value {} is within safe JS integer bounds",
            val
        )));
    }
    serializer.serialize_str(&val.to_string())
}

fn deserialize_unsafe_i64_from_string<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let digits = s.strip_prefix('-').unwrap_or(&s);
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(de::Error::custom(
            "UNSAFE_INTEGER value must be a canonical base-10 integer string matching /^-?\\d+$/",
        ));
    }
    let val = s.parse::<i64>().map_err(de::Error::custom)?;
    if (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&val) {
        return Err(de::Error::custom(format!(
            "UNSAFE_INTEGER value {} is within safe JS integer bounds",
            val
        )));
    }
    Ok(val)
}

/// Canonical Harness Errors for cowok-design kernel
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub enum HarnessError {
    #[error("Invalid canonical spec key: {key}")]
    #[serde(rename = "INVALID_SPEC_KEY")]
    InvalidSpecKey { key: String },

    #[error("Stale file at {path}: expected hash {expected_hash}, got {actual_hash}")]
    #[serde(rename = "STALE_FILE")]
    StaleFile {
        path: String,
        expected_hash: String,
        actual_hash: String,
    },

    #[error("Project not found: {project_id}")]
    #[serde(rename = "PROJECT_NOT_FOUND")]
    ProjectNotFound { project_id: String },

    #[error("Path traversal attempt denied: {path}")]
    #[serde(rename = "PATH_TRAVERSAL_DENIED")]
    PathTraversalDenied { path: String },

    #[error("SQLite database busy or locked: {details}")]
    #[serde(rename = "SQLITE_BUSY")]
    SqliteBusy { details: String },

    #[error("Integer {value} exceeds JavaScript safe integer bounds [-9007199254740991, 9007199254740991]")]
    #[serde(rename = "UNSAFE_INTEGER")]
    UnsafeInteger {
        #[serde(
            serialize_with = "serialize_unsafe_i64_as_string",
            deserialize_with = "deserialize_unsafe_i64_from_string"
        )]
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        value: i64,
    },

    #[error("Validation failed: {reason}")]
    #[serde(rename = "VALIDATION_FAILED")]
    ValidationFailed { reason: String },

    #[error("Security boundary violation: {reason}")]
    #[serde(rename = "BOUNDARY_VIOLATION")]
    BoundaryViolation { reason: String },

    #[error("Prompt token budget exceeded: {tokens} exceeds ceiling {ceiling}")]
    #[serde(rename = "PROMPT_BUDGET_EXCEEDED")]
    PromptBudgetExceeded { tokens: usize, ceiling: usize },

    #[error("Invalid prompt payload: {details}")]
    #[serde(rename = "INVALID_PROMPT_PAYLOAD")]
    InvalidPromptPayload { details: String },

    #[error("Model capability adaptation failed: {reason}")]
    #[serde(rename = "MODEL_ADAPTATION_FAILED")]
    ModelAdaptationFailed { reason: String },
}

impl HarnessError {
    pub fn kind_str(&self) -> &'static str {
        match self {
            Self::InvalidSpecKey { .. } => "INVALID_SPEC_KEY",
            Self::StaleFile { .. } => "STALE_FILE",
            Self::ProjectNotFound { .. } => "PROJECT_NOT_FOUND",
            Self::PathTraversalDenied { .. } => "PATH_TRAVERSAL_DENIED",
            Self::SqliteBusy { .. } => "SQLITE_BUSY",
            Self::UnsafeInteger { .. } => "UNSAFE_INTEGER",
            Self::ValidationFailed { .. } => "VALIDATION_FAILED",
            Self::BoundaryViolation { .. } => "BOUNDARY_VIOLATION",
            Self::PromptBudgetExceeded { .. } => "PROMPT_BUDGET_EXCEEDED",
            Self::InvalidPromptPayload { .. } => "INVALID_PROMPT_PAYLOAD",
            Self::ModelAdaptationFailed { .. } => "MODEL_ADAPTATION_FAILED",
        }
    }
}
