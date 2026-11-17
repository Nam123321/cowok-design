// @story 08.3
// @implements AC-03, AC-05
use crate::models::SkillManifest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct SemVer {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub major: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub minor: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub patch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct BreakingDiffReport {
    pub is_breaking: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SemVerParseError {
    #[error("Invalid SemVer '{0}': expected strict 'MAJOR.MINOR.PATCH' numeric digits")]
    InvalidSemVer(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SchemaTypeKind {
    NormalizedSet(BTreeSet<String>),
    Raw(Option<serde_json::Value>),
}

fn extract_schema_type_kind(schema: &serde_json::Value) -> SchemaTypeKind {
    match schema.get("type") {
        None => SchemaTypeKind::Raw(None),
        Some(serde_json::Value::String(s)) => {
            SchemaTypeKind::NormalizedSet([s.clone()].into_iter().collect())
        }
        Some(serde_json::Value::Array(arr)) if arr.iter().all(|v| v.is_string()) => {
            let set: BTreeSet<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
            SchemaTypeKind::NormalizedSet(set)
        }
        Some(other) => SchemaTypeKind::Raw(Some(other.clone())),
    }
}

pub fn parse_semver_strict(raw: &str) -> Result<SemVer, SemVerParseError> {
    let parts: Vec<&str> = raw.split('.').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(SemVerParseError::InvalidSemVer(raw.to_string()));
    }
    if parts.iter().any(|p| p.len() > 1 && p.starts_with('0')) {
        return Err(SemVerParseError::InvalidSemVer(raw.to_string()));
    }
    let major = parts[0]
        .parse::<u64>()
        .map_err(|_| SemVerParseError::InvalidSemVer(raw.to_string()))?;
    let minor = parts[1]
        .parse::<u64>()
        .map_err(|_| SemVerParseError::InvalidSemVer(raw.to_string()))?;
    let patch = parts[2]
        .parse::<u64>()
        .map_err(|_| SemVerParseError::InvalidSemVer(raw.to_string()))?;
    Ok(SemVer {
        major,
        minor,
        patch,
    })
}

fn diff_parameter_schemas(
    current: &Option<serde_json::Value>,
    candidate: &Option<serde_json::Value>,
    reasons: &mut Vec<String>,
) {
    let cur_eff = current.as_ref().filter(|v| !v.is_null());
    let cand_eff = candidate.as_ref().filter(|v| !v.is_null());
    let (Some(cur_val), Some(cand_val)) = (cur_eff, cand_eff) else {
        if cur_eff.is_some() && cand_eff.is_none() {
            reasons.push("Parameter schema removed completely".to_string());
        } else if let Some(c_val) = cand_eff {
            match c_val.as_object() {
                Some(cand_obj)
                    if cand_obj
                        .get("required")
                        .and_then(|r| r.as_array())
                        .is_some_and(|a| !a.is_empty()) =>
                {
                    reasons
                        .push("Added new required parameters without existing schema".to_string());
                }
                Some(_) => {}
                None => {
                    reasons.push(
                        "Parameter schema structural type changed (expected JSON object)"
                            .to_string(),
                    );
                }
            }
        }
        return;
    };

    let (Some(cur_obj), Some(cand_obj)) = (cur_val.as_object(), cand_val.as_object()) else {
        if cur_val != cand_val {
            reasons.push("Parameter schema structural type changed".to_string());
        }
        return;
    };

    let cur_req: BTreeSet<&str> = cur_obj
        .get("required")
        .and_then(|r| r.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .collect();
    let cand_req: BTreeSet<&str> = cand_obj
        .get("required")
        .and_then(|r| r.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .collect();

    for added in cand_req.difference(&cur_req) {
        reasons.push(format!("Added new required parameter '{added}'"));
    }
    for removed in cur_req.difference(&cand_req) {
        reasons.push(format!("Removed previously required parameter '{removed}'"));
    }

    let cur_props = cur_obj.get("properties").and_then(|p| p.as_object());
    let cand_props = cand_obj.get("properties").and_then(|p| p.as_object());
    if let (Some(cp), Some(np)) = (cur_props, cand_props) {
        for (prop_name, old_schema) in cp {
            match np.get(prop_name) {
                None => reasons.push(format!("Removed parameter property '{prop_name}'")),
                Some(new_schema) => {
                    if extract_schema_type_kind(old_schema) != extract_schema_type_kind(new_schema)
                    {
                        reasons.push(format!("Parameter '{prop_name}' type changed"));
                    }
                }
            }
        }
    } else if cur_props.is_some() && cand_props.is_none() {
        reasons.push("Removed parameter 'properties' definition".to_string());
    }
}

/// Authoritative structural manifest & SemVer breaking-change detector (EC-E08-03).
pub fn detect_breaking_change(
    current: &SkillManifest,
    candidate: &SkillManifest,
    force_breaking_hint: bool,
) -> Result<BreakingDiffReport, SemVerParseError> {
    let cur_ver = parse_semver_strict(&current.version)?;
    let cand_ver = parse_semver_strict(&candidate.version)?;
    let mut reasons = Vec::new();

    if cand_ver < cur_ver {
        reasons.push(format!(
            "Version downgrade from {} to {}",
            current.version, candidate.version
        ));
    } else if cand_ver.major > cur_ver.major {
        reasons.push(format!(
            "Major SemVer bump from {} to {}",
            current.version, candidate.version
        ));
    } else if cur_ver.major == 0 && cand_ver.minor > cur_ver.minor {
        reasons.push(format!(
            "Pre-1.0 minor bump from {} to {}",
            current.version, candidate.version
        ));
    }

    if candidate.entry_point != current.entry_point {
        reasons.push(format!(
            "Entry point changed from '{}' to '{}'",
            current.entry_point, candidate.entry_point
        ));
    }

    let cur_caps: BTreeSet<&str> = current
        .capabilities_required
        .iter()
        .map(String::as_str)
        .collect();
    let cand_caps: BTreeSet<&str> = candidate
        .capabilities_required
        .iter()
        .map(String::as_str)
        .collect();
    for added in cand_caps.difference(&cur_caps) {
        reasons.push(format!("Privilege escalation: added capability '{added}'"));
    }
    for removed in cur_caps.difference(&cand_caps) {
        reasons.push(format!("Contract change: removed capability '{removed}'"));
    }
    if candidate.risk_tier > current.risk_tier {
        reasons.push(format!(
            "Privilege escalation: risk_tier increased from {} to {}",
            current.risk_tier, candidate.risk_tier
        ));
    }

    diff_parameter_schemas(&current.parameters, &candidate.parameters, &mut reasons);

    if force_breaking_hint {
        reasons.push("Caller flagged conservative breaking-change hint".to_string());
    }

    Ok(BreakingDiffReport {
        is_breaking: !reasons.is_empty(),
        reasons,
    })
}
