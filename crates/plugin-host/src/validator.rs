// @story 08.1
// @implements AC-03, AC-06
use domain::token_budget::count_exact_bpe_tokens;
use std::collections::BTreeSet;
use thiserror::Error;

pub const MAX_FRONTMATTER_TOKENS: usize = 100;
pub const MAX_FRONTMATTER_BYTES: usize = 1_024;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("Frontmatter must start and end with '---' line delimiters")]
    InvalidFrontmatterDelimiters,
    #[error("Frontmatter exceeds 100-token budget: {tokens} tokens estimated (limit: {limit})")]
    FrontmatterExceedsTokenBudget { tokens: usize, limit: usize },
    #[error("Frontmatter exceeds {limit}-byte hard ceiling: {bytes} bytes")]
    FrontmatterExceedsByteBudget { bytes: usize, limit: usize },
    #[error("Missing required field in frontmatter: {0}")]
    MissingRequiredField(String),
    #[error("Invalid risk tier: {0} (must be 1, 2, or 3)")]
    InvalidRiskTier(u8),
    #[error("Unsafe YAML construct disallowed in frontmatter: {0}")]
    UnsafeYamlConstruct(String),
    #[error("YAML parse error: {0}")]
    YamlParseError(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontmatterMetadata {
    pub name: String,
    pub version: Option<String>,
    pub description: String,
    pub entry_point: Option<String>,
    pub capabilities_required: Vec<String>,
    pub risk_tier: Option<u8>,
    pub tags: Vec<String>,
    pub tokens: usize,
    pub bytes: usize,
}

/// Conservative character/word token estimator kept as a safety floor alongside cl100k_base BPE.
pub fn estimate_token_count(text: &str) -> usize {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0;
    }
    let char_estimate = trimmed.chars().count().div_ceil(4);
    let word_estimate = ((trimmed.split_whitespace().count() as f64) * 1.33).ceil() as usize;
    std::cmp::max(char_estimate, word_estimate)
}

fn extract_bounded_frontmatter(raw_content: &str) -> Result<&str, ValidationError> {
    let content = raw_content.strip_prefix('\u{FEFF}').unwrap_or(raw_content);
    let after_open = if let Some(rest) = content.strip_prefix("---\r\n") {
        rest
    } else if let Some(rest) = content.strip_prefix("---\n") {
        rest
    } else {
        return Err(ValidationError::InvalidFrontmatterDelimiters);
    };

    let mut offset = 0usize;
    for segment in after_open.split_inclusive('\n') {
        let line_no_nl = segment.strip_suffix('\n').unwrap_or(segment);
        let line = line_no_nl.strip_suffix('\r').unwrap_or(line_no_nl);
        if line == "---" {
            return Ok(after_open[..offset].trim());
        }
        offset += segment.len();
    }

    if after_open[offset..].trim_end_matches('\r') == "---" {
        return Ok(after_open[..offset].trim());
    }

    Err(ValidationError::InvalidFrontmatterDelimiters)
}

fn reject_unsafe_yaml_tokens(yaml_body: &str) -> Result<(), ValidationError> {
    let mut in_single = false;
    let mut in_double = false;
    let mut in_block_scalar_indent: Option<usize> = None;
    for raw_line in yaml_body.lines() {
        let line_indent = raw_line.len() - raw_line.trim_start_matches(' ').len();
        if let Some(block_indent) = in_block_scalar_indent {
            if raw_line.trim().is_empty() || line_indent > block_indent {
                continue;
            }
            in_block_scalar_indent = None;
        }
        let mut escaped = false;
        let bytes = raw_line.as_bytes();
        let mut idx = 0usize;

        while idx < bytes.len() {
            let b = bytes[idx];
            if in_double {
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'"' {
                    in_double = false;
                }
                idx += 1;
                continue;
            }
            if in_single {
                if b == b'\'' {
                    if idx + 1 < bytes.len() && bytes[idx + 1] == b'\'' {
                        idx += 2;
                        continue;
                    }
                    in_single = false;
                }
                idx += 1;
                continue;
            }
            match b {
                b'"' => in_double = true,
                b'\'' => in_single = true,
                b'#' if idx == 0 || bytes[idx - 1].is_ascii_whitespace() => break,
                b'|' | b'>'
                    if idx > 0
                        && bytes[..idx]
                            .iter()
                            .rev()
                            .find(|&&c| c != b' ' && c != b'\t')
                            == Some(&b':')
                        && raw_line[idx + 1..]
                            .trim_matches(|c| matches!(c, '+' | '-' | ' ' | '\t'))
                            .chars()
                            .all(|c| c.is_ascii_digit()) =>
                {
                    in_block_scalar_indent = Some(line_indent);
                    break;
                }
                b'&' | b'*' | b'!' => {
                    let prev_ok = idx == 0
                        || bytes[idx - 1].is_ascii_whitespace()
                        || matches!(bytes[idx - 1], b':' | b'[' | b'{' | b',');
                    let next_non_ws =
                        idx + 1 < bytes.len() && !bytes[idx + 1].is_ascii_whitespace();
                    if prev_ok && next_non_ws {
                        let kind = match b {
                            b'&' => "YAML anchors (&)",
                            b'*' => "YAML aliases (*)",
                            _ => "YAML custom tags (!)",
                        };
                        return Err(ValidationError::UnsafeYamlConstruct(kind.to_string()));
                    }
                }
                _ => {}
            }
            idx += 1;
        }
    }
    Ok(())
}

fn check_duplicate_top_level_keys(yaml_body: &str) -> Result<(), ValidationError> {
    let mut seen = BTreeSet::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut block_indent: Option<usize> = None;
    for line in yaml_body.lines() {
        let indent = line.len() - line.trim_start_matches(' ').len();
        if let Some(bi) = block_indent {
            if line.trim().is_empty() || indent > bi {
                continue;
            }
            block_indent = None;
        }
        let was_in_quoted = in_single || in_double;
        let mut esc = false;
        for b in line.bytes() {
            if in_double {
                if esc {
                    esc = false;
                } else if b == b'\\' {
                    esc = true;
                } else if b == b'"' {
                    in_double = false;
                }
            } else if in_single {
                if b == b'\'' {
                    in_single = false;
                }
            } else if b == b'"' {
                in_double = true;
            } else if b == b'\'' {
                in_single = true;
            } else if b == b'#' {
                break;
            }
        }
        if was_in_quoted {
            continue;
        }
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            let inner = &trimmed[1..trimmed.len() - 1];
            let mut flow_seen = BTreeSet::new();
            for pair in inner.split(',') {
                if let Some((raw_k, _)) = pair.split_once(':') {
                    let fk = raw_k.trim().trim_matches('"').trim_matches('\'');
                    if !fk.is_empty() && !flow_seen.insert(fk.to_string()) {
                        return Err(ValidationError::YamlParseError(format!(
                            "Duplicate YAML flow key '{fk}'"
                        )));
                    }
                }
            }
            continue;
        }
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        if let Some((key, rest)) = line.split_once(':') {
            let k = key.trim().trim_matches('"').trim_matches('\'');
            if !k.is_empty() && !seen.insert(k.to_string()) {
                return Err(ValidationError::YamlParseError(format!(
                    "Duplicate YAML key '{k}'"
                )));
            }
            let rtrim = rest.trim();
            if rtrim.starts_with('|') || rtrim.starts_with('>') {
                block_indent = Some(indent);
            }
        }
    }
    Ok(())
}

fn mapping_str<'a>(
    map: &'a serde_yaml::Mapping,
    keys: &[&str],
) -> Result<Option<&'a str>, ValidationError> {
    for &key in keys {
        let yaml_key = serde_yaml::Value::String(key.to_string());
        if let Some(val) = map.get(&yaml_key) {
            let s = val.as_str().ok_or_else(|| {
                ValidationError::YamlParseError(format!("Field '{key}' must be a string"))
            })?;
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Ok(Some(trimmed));
            }
        }
    }
    Ok(None)
}

fn mapping_string_list(
    map: &serde_yaml::Mapping,
    key: &str,
) -> Result<Vec<String>, ValidationError> {
    let yaml_key = serde_yaml::Value::String(key.to_string());
    let Some(val) = map.get(&yaml_key) else {
        return Ok(Vec::new());
    };
    let seq = val
        .as_sequence()
        .ok_or_else(|| ValidationError::YamlParseError(format!("Field '{key}' must be a list")))?;
    let mut out = Vec::with_capacity(seq.len());
    for item in seq {
        let s = item.as_str().ok_or_else(|| {
            ValidationError::YamlParseError(format!("List element in '{key}' must be a string"))
        })?;
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(ValidationError::YamlParseError(format!(
                "Field '{key}' items must be non-empty strings"
            )));
        }
        out.push(trimmed.to_string());
    }
    Ok(out)
}

/// Parses and validates bounded agentskills.io YAML frontmatter via serde_yaml + cl100k_base BPE.
pub fn parse_and_validate_skill_frontmatter(
    raw_content: &str,
) -> Result<FrontmatterMetadata, ValidationError> {
    let yaml_body = extract_bounded_frontmatter(raw_content)?;
    let bytes = yaml_body.len();
    if bytes > MAX_FRONTMATTER_BYTES {
        return Err(ValidationError::FrontmatterExceedsByteBudget {
            bytes,
            limit: MAX_FRONTMATTER_BYTES,
        });
    }

    reject_unsafe_yaml_tokens(yaml_body)?;
    check_duplicate_top_level_keys(yaml_body)?;

    let bpe_tokens = count_exact_bpe_tokens(yaml_body);
    let tokens = std::cmp::max(bpe_tokens, estimate_token_count(yaml_body));
    if tokens > MAX_FRONTMATTER_TOKENS {
        return Err(ValidationError::FrontmatterExceedsTokenBudget {
            tokens,
            limit: MAX_FRONTMATTER_TOKENS,
        });
    }

    let parsed: serde_yaml::Value = serde_yaml::from_str(yaml_body)
        .map_err(|e| ValidationError::YamlParseError(e.to_string()))?;
    let map = parsed.as_mapping().ok_or_else(|| {
        ValidationError::YamlParseError("Frontmatter must be a YAML mapping".to_string())
    })?;

    let name = mapping_str(map, &["name", "id", "title"])?
        .ok_or_else(|| ValidationError::MissingRequiredField("name".to_string()))?
        .to_string();
    let description = mapping_str(map, &["description", "desc"])?
        .ok_or_else(|| ValidationError::MissingRequiredField("description".to_string()))?
        .to_string();
    let version = mapping_str(map, &["version"])?.map(|s| s.to_string());
    let entry_key = serde_yaml::Value::String("entry_point".to_string());
    let entry_point = if let Some(val) = map.get(&entry_key) {
        let s = val.as_str().ok_or_else(|| {
            ValidationError::YamlParseError("Field 'entry_point' must be a string".to_string())
        })?;
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(ValidationError::YamlParseError(
                "Field 'entry_point' must not be empty".to_string(),
            ));
        }
        Some(trimmed.to_string())
    } else {
        None
    };

    let tier_key = serde_yaml::Value::String("risk_tier".to_string());
    let risk_tier = if let Some(val) = map.get(&tier_key) {
        let raw_num = val.as_u64().ok_or_else(|| {
            ValidationError::YamlParseError("risk_tier must be an integer".to_string())
        })?;
        let tier = u8::try_from(raw_num).unwrap_or(u8::MAX);
        if !(1..=3).contains(&tier) {
            return Err(ValidationError::InvalidRiskTier(tier));
        }
        Some(tier)
    } else {
        None
    };

    let capabilities_required = mapping_string_list(map, "capabilities_required")?;
    let tags = mapping_string_list(map, "tags")?;

    Ok(FrontmatterMetadata {
        name,
        version,
        description,
        entry_point,
        capabilities_required,
        risk_tier,
        tags,
        tokens,
        bytes,
    })
}

/// Validates SKILL.md frontmatter and returns the calibrated token count.
pub fn validate_skill_frontmatter(raw_content: &str) -> Result<usize, ValidationError> {
    parse_and_validate_skill_frontmatter(raw_content).map(|meta| meta.tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_compact_frontmatter() {
        let fm = r#"---
name: intent-clarifier-sdi
version: 1.0.0
description: Discovers user intent and presents visual clarification variants
risk_tier: 1
---
# Content here"#;
        let tokens = validate_skill_frontmatter(fm).expect("Should pass validation");
        assert!(tokens <= 100);
        assert!(tokens > 0);
    }

    #[test]
    fn test_rejects_bloated_frontmatter_exceeding_100_tokens() {
        let bloated_desc = "very ".repeat(120);
        let fm = format!(
            r#"---
name: bloated-skill
description: {}
---"#,
            bloated_desc
        );
        let err = validate_skill_frontmatter(&fm).unwrap_err();
        assert!(matches!(
            err,
            ValidationError::FrontmatterExceedsByteBudget { .. }
                | ValidationError::FrontmatterExceedsTokenBudget { .. }
        ));
    }

    #[test]
    fn test_rejects_missing_required_fields() {
        let fm = r#"---
version: 1.0.0
tags: [test]
---"#;
        let err = validate_skill_frontmatter(fm).unwrap_err();
        assert_eq!(
            err,
            ValidationError::MissingRequiredField("name".to_string())
        );
    }
}
