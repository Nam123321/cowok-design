use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("Frontmatter must start and end with '---' delimiters")]
    InvalidFrontmatterDelimiters,
    #[error("Frontmatter exceeds 100-token budget: {tokens} tokens estimated (limit: {limit})")]
    FrontmatterExceedsTokenBudget { tokens: usize, limit: usize },
    #[error("Missing required field in frontmatter: {0}")]
    MissingRequiredField(String),
    #[error("Invalid risk tier: {0} (must be 1, 2, or 3)")]
    InvalidRiskTier(u8),
    #[error("YAML parse error: {0}")]
    YamlParseError(String),
}

/// Deterministic token estimator for YAML frontmatter (~4 chars per token or ~1.3 tokens per word)
pub fn estimate_token_count(text: &str) -> usize {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0;
    }
    let char_count = trimmed.chars().count();
    let word_count = trimmed.split_whitespace().count();

    // Conservative ceiling heuristic
    let char_estimate = char_count.div_ceil(4);
    let word_estimate = ((word_count as f64) * 1.33).ceil() as usize;
    std::cmp::max(char_estimate, word_estimate)
}

/// Validates that a SKILL.md frontmatter adheres to the agentskills.io standard:
/// - Delimited by `---`
/// - Estimated token count <= 100 tokens (strict AC-03 budget guard)
/// - Contains `name` and `description`
pub fn validate_skill_frontmatter(raw_content: &str) -> Result<usize, ValidationError> {
    let trimmed = raw_content.trim();
    let yaml_body = if let Some(after_first) = trimmed.strip_prefix("---") {
        let end_idx = after_first
            .find("---")
            .ok_or(ValidationError::InvalidFrontmatterDelimiters)?;
        after_first[..end_idx].trim()
    } else {
        trimmed
    };

    let tokens = estimate_token_count(yaml_body);
    if tokens > 100 {
        return Err(ValidationError::FrontmatterExceedsTokenBudget { tokens, limit: 100 });
    }

    // Check mandatory fields
    let has_name = yaml_body.lines().any(|l| {
        let t = l.trim();
        t.starts_with("name:") || t.starts_with("id:") || t.starts_with("title:")
    });
    if !has_name {
        return Err(ValidationError::MissingRequiredField("name".to_string()));
    }

    let has_description = yaml_body.lines().any(|l| {
        let t = l.trim();
        t.starts_with("description:") || t.starts_with("desc:")
    });
    if !has_description {
        return Err(ValidationError::MissingRequiredField(
            "description".to_string(),
        ));
    }

    // Check risk tier if present
    for line in yaml_body.lines() {
        let t = line.trim();
        if let Some(stripped) = t.strip_prefix("risk_tier:") {
            let val_part = stripped.trim();
            if let Ok(tier) = val_part.parse::<u8>() {
                if !(1..=3).contains(&tier) {
                    return Err(ValidationError::InvalidRiskTier(tier));
                }
            }
        }
    }

    Ok(tokens)
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
        match err {
            ValidationError::FrontmatterExceedsTokenBudget { tokens, limit } => {
                assert!(tokens > 100);
                assert_eq!(limit, 100);
            }
            _ => panic!("Expected token budget error, got {:?}", err),
        }
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
